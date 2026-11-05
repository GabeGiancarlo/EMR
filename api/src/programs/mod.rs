//! Organization-authored care program and service catalog.
//! Admission routing, encounter snapshots, and legacy assignment are owned elsewhere.
//! Authentication remains deferred; callers are local active staff selected by header.

use crate::access::{self, Actor};
use crate::{ApiError, AppState, Result};
use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use chrono::DateTime;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

const STALE: &str = "This record was updated after you opened it. Reload the latest version and preserve your unsaved edits; they were not saved.";
const NEED_STAMP: &str = "Reload the record before saving. The originally loaded updated_at is required so a newer edit is not overwritten.";
const LEVELS: &[&str] = &["outpatient", "iop", "php"];
const SYSTEMS: &[&str] = &["internal", "cpt", "hcpcs"];
const KINDS: &[&str] = &[
    "individual",
    "group",
    "assessment",
    "family",
    "case_management",
];

const PROGRAM_VIEW: &str = "SELECT jsonb_build_object(
 'id', p.id,
 'organization_id', p.organization_id,
 'facility_id', p.facility_id,
 'code', p.code,
 'name', p.name,
 'level_of_care', p.level_of_care,
 'min_age', p.min_age,
 'max_age', p.max_age,
 'capacity', p.capacity,
 'active', p.active,
 'version', p.version,
 'created_at', p.created_at,
 'updated_at', p.updated_at,
 'required_intake_template_ids', COALESCE((SELECT jsonb_agg(f.template_id ORDER BY f.template_id) FROM program_required_forms f WHERE f.program_id=p.id AND f.organization_id=p.organization_id AND f.stage='intake'), '[]'::jsonb),
 'required_discharge_template_ids', COALESCE((SELECT jsonb_agg(f.template_id ORDER BY f.template_id) FROM program_required_forms f WHERE f.program_id=p.id AND f.organization_id=p.organization_id AND f.stage='discharge'), '[]'::jsonb),
 'active_census', (SELECT count(*) FROM episodes e WHERE e.program_id=p.id AND e.organization_id=p.organization_id AND e.status='active'),
 'available_places', GREATEST(p.capacity::bigint - (SELECT count(*) FROM episodes e WHERE e.program_id=p.id AND e.organization_id=p.organization_id AND e.status='active'), 0::bigint),
 'configuration_blockers', COALESCE((SELECT jsonb_agg(jsonb_build_object('template_id', f.template_id, 'stage', f.stage, 'code', 'inactive_template') ORDER BY f.stage, f.template_id) FROM program_required_forms f JOIN templates t ON t.id=f.template_id AND t.organization_id=f.organization_id WHERE f.program_id=p.id AND f.organization_id=p.organization_id AND NOT t.active), '[]'::jsonb)
) FROM care_programs p";

const SERVICE_VIEW: &str = "SELECT jsonb_build_object(
 'id', s.id,
 'organization_id', s.organization_id,
 'program_id', s.program_id,
 'code_system', s.code_system,
 'service_code', s.service_code,
 'name', s.name,
 'session_kind', s.session_kind,
 'min_minutes', s.min_minutes,
 'default_minutes', s.default_minutes,
 'max_minutes', s.max_minutes,
 'default_charge_cents', s.default_charge_cents,
 'note_template_id', s.note_template_id,
 'active', s.active,
 'version', s.version,
 'created_at', s.created_at,
 'updated_at', s.updated_at,
 'configuration_blockers', COALESCE((SELECT jsonb_agg(jsonb_build_object('template_id', t.id, 'code', 'inactive_template')) FROM templates t WHERE s.note_template_id IS NOT NULL AND t.id=s.note_template_id AND t.organization_id=s.organization_id AND NOT t.active), '[]'::jsonb)
) FROM program_services s";

pub(crate) mod adoptions;
pub(crate) mod rules;
pub(crate) mod services;

pub fn configure(cfg: &mut web::ServiceConfig) {
    adoptions::configure(cfg);
    cfg.route("/program-catalog", web::get().to(catalog))
        .route("/care-programs", web::post().to(create_program))
        .route(
            "/care-programs/{id}/history",
            web::get().to(program_history),
        )
        .route("/care-programs/{id}", web::patch().to(patch_program))
        .route("/program-services", web::post().to(create_service))
        .route(
            "/program-services/{id}/history",
            web::get().to(service_history),
        )
        .route("/program-services/{id}", web::patch().to(patch_service));
}

fn no_store(status: StatusCode, body: Value) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header(("Cache-Control", "no-store"))
        .json(body)
}
fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| ApiError::invalid("Expected a JSON object"))
}
fn reject_unknown(body: &Map<String, Value>, allowed: &[&str]) -> Result<()> {
    if let Some(key) = body.keys().find(|key| !allowed.contains(&key.as_str())) {
        if key.chars().count() > 80 {
            return Err(ApiError::invalid("A supplied field is not recognized."));
        }
        return Err(ApiError::invalid(format!(
            "Field {key} cannot be changed on a program catalog record"
        )));
    }
    Ok(())
}
fn bounded(value: &str, min: usize, max: usize, field: &str) -> Result<String> {
    if value.contains('\0') {
        return Err(ApiError::invalid(format!(
            "{field} contains an invalid character"
        )));
    }
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    if count < min || count > max {
        return Err(ApiError::invalid(format!(
            "{field} must contain {min}-{max} characters"
        )));
    }
    Ok(trimmed.to_string())
}
fn text_field(value: &Value, field: &str, min: usize, max: usize) -> Result<String> {
    bounded(
        value
            .as_str()
            .ok_or_else(|| ApiError::invalid(format!("{field} must be text")))?,
        min,
        max,
        field,
    )
}
fn normalize_code(value: &Value, field: &str) -> Result<String> {
    let code = text_field(value, field, 1, 40)?.to_uppercase();
    let count = code.chars().count();
    if !(1..=40).contains(&count) {
        return Err(ApiError::invalid(format!(
            "{field} must contain 1-40 characters"
        )));
    }
    Ok(code)
}
fn one_of<'a>(value: &str, allowed: &'a [&str], field: &str) -> Result<&'a str> {
    allowed
        .iter()
        .copied()
        .find(|item| *item == value.trim())
        .ok_or_else(|| ApiError::invalid(format!("{field} is not an accepted value")))
}
fn enum_field<'a>(value: &Value, allowed: &'a [&str], field: &str) -> Result<&'a str> {
    one_of(
        value
            .as_str()
            .ok_or_else(|| ApiError::invalid(format!("{field} must be text")))?,
        allowed,
        field,
    )
}
fn parse_i64(value: &Value, field: &str) -> Result<i64> {
    value
        .as_i64()
        .ok_or_else(|| ApiError::invalid(format!("{field} must be a whole number")))
}
fn parse_i32(value: &Value, field: &str) -> Result<i32> {
    i32::try_from(parse_i64(value, field)?)
        .map_err(|_| ApiError::invalid(format!("{field} must be a whole number")))
}
fn json_bool(value: &Value, field: &str) -> Result<bool> {
    value
        .as_bool()
        .ok_or_else(|| ApiError::invalid(format!("{field} must be true or false")))
}
fn json_uuid(value: &Value, field: &str) -> Result<Uuid> {
    Uuid::parse_str(
        value
            .as_str()
            .ok_or_else(|| ApiError::invalid(format!("{field} must be a UUID")))?
            .trim(),
    )
    .map_err(|_| ApiError::invalid(format!("{field} must be a UUID")))
}
fn uuid_list(value: &Value, field: &str) -> Result<Vec<Uuid>> {
    let items = value
        .as_array()
        .ok_or_else(|| ApiError::invalid(format!("{field} must be an array of template ids")))?;
    let mut ids = Vec::with_capacity(items.len());
    let mut seen = HashSet::new();
    for item in items {
        let id = json_uuid(item, field)?;
        if !seen.insert(id) {
            return Err(ApiError::invalid("Duplicate template ids are not allowed."));
        }
        ids.push(id);
    }
    ids.sort();
    Ok(ids)
}
fn expected_stamp(value: Option<&Value>) -> Result<String> {
    let Some(Value::String(text)) = value else {
        return Err(ApiError::precondition(NEED_STAMP));
    };
    let text = text.trim();
    if text.is_empty() {
        return Err(ApiError::precondition(NEED_STAMP));
    }
    DateTime::parse_from_rfc3339(text)
        .map_err(|_| ApiError::invalid("_expected_updated_at must be an RFC3339 timestamp"))?;
    Ok(text.to_string())
}
fn validate_ages(min_age: i32, max_age: Option<i32>) -> Result<()> {
    if !(18..=120).contains(&min_age) {
        return Err(ApiError::invalid(
            "min_age must be a whole number from 18 through 120.",
        ));
    }
    if let Some(max_age) = max_age {
        if max_age < min_age || max_age > 130 {
            return Err(ApiError::invalid(
                "max_age must be empty or a whole number from min_age through 130.",
            ));
        }
    }
    Ok(())
}
fn validate_capacity(capacity: i32) -> Result<()> {
    if (1..=10_000).contains(&capacity) {
        Ok(())
    } else {
        Err(ApiError::invalid(
            "capacity must be a whole number from 1 through 10000.",
        ))
    }
}
fn validate_minutes(min_minutes: i32, default_minutes: i32, max_minutes: i32) -> Result<()> {
    if (1..=480).contains(&min_minutes)
        && (1..=480).contains(&default_minutes)
        && (1..=480).contains(&max_minutes)
        && min_minutes <= default_minutes
        && default_minutes <= max_minutes
    {
        Ok(())
    } else {
        Err(ApiError::invalid(
            "min_minutes, default_minutes, and max_minutes must be whole numbers from 1 through 480, with min_minutes <= default_minutes <= max_minutes.",
        ))
    }
}
fn validate_charge(charge: Option<i64>) -> Result<()> {
    if let Some(charge) = charge {
        if !(0..=100_000_000).contains(&charge) {
            return Err(ApiError::invalid(
                "default_charge_cents must be a whole number from 0 through 100000000, or empty.",
            ));
        }
    }
    Ok(())
}
fn available_places(capacity: i64, census: i64) -> i64 {
    capacity.saturating_sub(census.max(0)).max(0)
}
fn with_places(mut view: Value) -> Value {
    let capacity = view.get("capacity").and_then(Value::as_i64).unwrap_or(0);
    let census = view
        .get("active_census")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    if let Some(obj) = view.as_object_mut() {
        obj.insert("active_census".into(), json!(census));
        obj.insert(
            "available_places".into(),
            json!(available_places(capacity, census)),
        );
    }
    view
}
fn configuration_snapshot(view: &Value) -> Value {
    let mut clone = view.clone();
    if let Some(obj) = clone.as_object_mut() {
        obj.remove("active_census");
        obj.remove("available_places");
        obj.remove("configuration_blockers");
    }
    clone
}
fn service_snapshot(view: &Value) -> Value {
    let mut clone = view.clone();
    if let Some(obj) = clone.as_object_mut() {
        obj.remove("configuration_blockers");
    }
    clone
}
fn required_text(view: &Value, key: &str) -> Result<String> {
    view.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ApiError::conflict("Program configuration could not be read back."))
}
fn required_i32(view: &Value, key: &str) -> Result<i32> {
    view.get(key)
        .and_then(Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| ApiError::conflict("Program configuration could not be read back."))
}
fn required_bool(view: &Value, key: &str) -> Result<bool> {
    view.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| ApiError::conflict("Program configuration could not be read back."))
}
fn current_optional_i32(view: &Value, key: &str) -> Result<Option<i32>> {
    match view.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(parse_i32(value, key)?)),
    }
}
fn current_optional_i64(view: &Value, key: &str) -> Result<Option<i64>> {
    match view.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(parse_i64(value, key)?)),
    }
}
fn ids_of(view: &Value, key: &str) -> Result<Vec<Uuid>> {
    let Some(items) = view.get(key).and_then(Value::as_array) else {
        return Err(ApiError::conflict(
            "Program configuration could not be read back.",
        ));
    };
    items.iter().map(|item| json_uuid(item, key)).collect()
}
fn same_ids(left: &[Uuid], right: &[Uuid]) -> bool {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort();
    right.sort();
    left == right
}
fn version_of(view: &Value) -> Result<i32> {
    required_i32(view, "version")
}
async fn recheck(
    req: &HttpRequest,
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    admin: bool,
) -> Result<Actor> {
    crate::scheduling::lock_org(tx, org).await?;
    let id = req
        .headers()
        .get("x-nexus-actor")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(ApiError::denied)?;
    let value: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND organization_id=$2 AND active",
    )
    .bind(id)
    .bind(org)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::denied)?;
    let actor = Actor {
        id,
        org,
        role: value["role"].as_str().unwrap_or_default().into(),
        value,
    };
    // Resource exceptions do not expand catalog administration or history.
    if admin && actor.role != "admin" {
        return Err(ApiError::denied());
    }
    Ok(actor)
}
async fn open(
    req: &HttpRequest,
    state: &AppState,
    admin: bool,
) -> Result<(Transaction<'static, Postgres>, Actor)> {
    let initial = access::actor(req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = recheck(req, &mut tx, initial.org, admin).await?;
    Ok((tx, actor))
}

async fn program_rows(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    id: Option<Uuid>,
) -> Result<Vec<Value>> {
    let sql = if id.is_some() {
        format!("{PROGRAM_VIEW} WHERE p.organization_id=$1 AND p.id=$2 ORDER BY p.code, p.id")
    } else {
        format!("{PROGRAM_VIEW} WHERE p.organization_id=$1 ORDER BY p.code, p.id")
    };
    let mut query = sqlx::query_scalar(&sql).bind(org);
    if let Some(id) = id {
        query = query.bind(id);
    }
    let rows: Vec<Value> = query.fetch_all(&mut **tx).await?;
    Ok(rows.into_iter().map(with_places).collect())
}
async fn service_rows(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    id: Option<Uuid>,
) -> Result<Vec<Value>> {
    let sql = if id.is_some() {
        format!(
            "{SERVICE_VIEW} WHERE s.organization_id=$1 AND s.id=$2 ORDER BY s.service_code, s.id"
        )
    } else {
        format!("{SERVICE_VIEW} WHERE s.organization_id=$1 ORDER BY s.service_code, s.id")
    };
    let mut query = sqlx::query_scalar(&sql).bind(org);
    if let Some(id) = id {
        query = query.bind(id);
    }
    Ok(query.fetch_all(&mut **tx).await?)
}
async fn one_program(tx: &mut Transaction<'_, Postgres>, org: Uuid, id: Uuid) -> Result<Value> {
    program_rows(tx, org, Some(id))
        .await?
        .into_iter()
        .next()
        .ok_or_else(ApiError::missing)
}
async fn one_service(tx: &mut Transaction<'_, Postgres>, org: Uuid, id: Uuid) -> Result<Value> {
    service_rows(tx, org, Some(id))
        .await?
        .into_iter()
        .next()
        .ok_or_else(ApiError::missing)
}
async fn require_active_facility(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    id: Uuid,
) -> Result<()> {
    let active: Option<bool> = sqlx::query_scalar(
        "SELECT active FROM facilities WHERE id=$1 AND organization_id=$2 FOR UPDATE",
    )
    .bind(id)
    .bind(org)
    .fetch_optional(&mut **tx)
    .await?;
    if active == Some(true) {
        Ok(())
    } else {
        Err(ApiError::invalid(
            "The facility must be active in this workspace.",
        ))
    }
}
async fn require_active_templates(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    ids: &[Uuid],
) -> Result<()> {
    for id in ids {
        let active: Option<bool> = sqlx::query_scalar(
            "SELECT active FROM templates WHERE id=$1 AND organization_id=$2 FOR UPDATE",
        )
        .bind(id)
        .bind(org)
        .fetch_optional(&mut **tx)
        .await?;
        match active {
            Some(true) => {}
            Some(false) => {
                return Err(ApiError::invalid(
                    "Required form templates must be active in this workspace.",
                ))
            }
            None => {
                return Err(ApiError::invalid(
                    "Required form templates must belong to this workspace.",
                ))
            }
        }
    }
    Ok(())
}
async fn require_active_note_template(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    id: Uuid,
) -> Result<()> {
    let active: Option<bool> = sqlx::query_scalar(
        "SELECT active FROM templates WHERE id=$1 AND organization_id=$2 FOR UPDATE",
    )
    .bind(id)
    .bind(org)
    .fetch_optional(&mut **tx)
    .await?;
    match active {
        Some(true) => Ok(()),
        Some(false) => Err(ApiError::invalid(
            "The note template must be active in this workspace.",
        )),
        None => Err(ApiError::invalid(
            "The note template must belong to this workspace.",
        )),
    }
}
async fn replace_forms(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    program: Uuid,
    stage: &str,
    ids: &[Uuid],
) -> Result<()> {
    sqlx::query(
        "DELETE FROM program_required_forms WHERE program_id=$1 AND organization_id=$2 AND stage=$3",
    )
    .bind(program)
    .bind(org)
    .bind(stage)
    .execute(&mut **tx)
    .await?;
    for id in ids {
        sqlx::query(
            "INSERT INTO program_required_forms(program_id, organization_id, template_id, stage) VALUES($1,$2,$3,$4)",
        )
        .bind(program)
        .bind(org)
        .bind(id)
        .bind(stage)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
async fn insert_program_revision(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    program_id: Uuid,
    version: i32,
    snapshot: Value,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO program_revisions(id, organization_id, program_id, version, snapshot, reason, actor_id) VALUES($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::new_v4())
    .bind(actor.org)
    .bind(program_id)
    .bind(version)
    .bind(snapshot)
    .bind(reason)
    .bind(actor.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
async fn insert_service_revision(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    service_id: Uuid,
    version: i32,
    snapshot: Value,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO program_service_revisions(id, organization_id, service_id, version, snapshot, reason, actor_id) VALUES($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::new_v4())
    .bind(actor.org)
    .bind(service_id)
    .bind(version)
    .bind(snapshot)
    .bind(reason)
    .bind(actor.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
async fn lock_expected(
    tx: &mut Transaction<'_, Postgres>,
    table: &str,
    id: Uuid,
    org: Uuid,
    expected: &str,
) -> Result<()> {
    let sql = format!(
        "SELECT updated_at = $3::text::timestamptz FROM {table} WHERE id=$1 AND organization_id=$2 FOR UPDATE"
    );
    let fresh: Option<bool> = sqlx::query_scalar(&sql)
        .bind(id)
        .bind(org)
        .bind(expected)
        .fetch_optional(&mut **tx)
        .await?;
    match fresh {
        None => Err(ApiError::missing()),
        Some(false) => Err(ApiError::conflict(STALE)),
        Some(true) => Ok(()),
    }
}
async fn active_census(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    program: Uuid,
) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM episodes WHERE organization_id=$1 AND program_id=$2 AND status='active'",
    )
    .bind(org)
    .bind(program)
    .fetch_one(&mut **tx)
    .await?)
}

async fn catalog(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse> {
    let (mut tx, actor) = open(&req, &state, false).await?;
    let programs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM care_programs WHERE organization_id=$1")
            .bind(actor.org)
            .fetch_one(&mut *tx)
            .await?;
    if programs > 1000 {
        return Err(ApiError::conflict(
            "Program catalog exceeds 1000 programs. No partial catalog was returned.",
        ));
    }
    let services: i64 =
        sqlx::query_scalar("SELECT count(*) FROM program_services WHERE organization_id=$1")
            .bind(actor.org)
            .fetch_one(&mut *tx)
            .await?;
    if services > 5000 {
        return Err(ApiError::conflict(
            "Program catalog exceeds 5000 services. No partial catalog was returned.",
        ));
    }
    let programs = program_rows(&mut tx, actor.org, None).await?;
    let services = service_rows(&mut tx, actor.org, None).await?;
    access::audit(&mut tx, &actor, "read", "program_catalog", None).await?;
    tx.commit().await?;
    Ok(no_store(
        StatusCode::OK,
        json!({
            "programs": programs,
            "services": services,
            "can_manage": actor.role == "admin"
        }),
    ))
}

fn require<'a>(body: &'a Map<String, Value>, key: &str) -> Result<&'a Value> {
    body.get(key)
        .ok_or_else(|| ApiError::invalid(format!("{key} is required")))
}
fn optional_charge(body: &Map<String, Value>) -> Result<Option<i64>> {
    match body.get("default_charge_cents") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(parse_i64(value, "default_charge_cents")?)),
    }
}
fn optional_template(body: &Map<String, Value>, key: &str) -> Result<Option<Uuid>> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(json_uuid(value, key)?)),
    }
}

async fn create_program(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let (mut tx, actor) = open(&req, &state, true).await?;
    let input = object(&body)?;
    reject_unknown(
        input,
        &[
            "facility_id",
            "code",
            "name",
            "level_of_care",
            "min_age",
            "max_age",
            "capacity",
            "active",
            "required_intake_template_ids",
            "required_discharge_template_ids",
            "change_reason",
        ],
    )?;
    let facility_id = json_uuid(require(input, "facility_id")?, "facility_id")?;
    let code = normalize_code(require(input, "code")?, "code")?;
    let name = text_field(require(input, "name")?, "name", 1, 160)?;
    let level = enum_field(require(input, "level_of_care")?, LEVELS, "level_of_care")?.to_string();
    let min_age = parse_i32(require(input, "min_age")?, "min_age")?;
    let max_age = match input.get("max_age") {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_i32(value, "max_age")?),
    };
    let capacity = parse_i32(require(input, "capacity")?, "capacity")?;
    let active = json_bool(require(input, "active")?, "active")?;
    let intake = uuid_list(
        require(input, "required_intake_template_ids")?,
        "required_intake_template_ids",
    )?;
    let discharge = uuid_list(
        require(input, "required_discharge_template_ids")?,
        "required_discharge_template_ids",
    )?;
    let reason = text_field(require(input, "change_reason")?, "change_reason", 10, 2000)?;
    validate_ages(min_age, max_age)?;
    validate_capacity(capacity)?;
    require_active_facility(&mut tx, actor.org, facility_id).await?;
    require_active_templates(&mut tx, actor.org, &intake).await?;
    require_active_templates(&mut tx, actor.org, &discharge).await?;
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM care_programs WHERE organization_id=$1 AND upper(trim(code))=$2)",
    )
    .bind(actor.org)
    .bind(&code)
    .fetch_one(&mut *tx)
    .await?;
    if taken {
        return Err(ApiError::conflict(
            "A program code must be unique in this workspace regardless of letter case.",
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO care_programs(id, organization_id, facility_id, code, name, level_of_care, min_age, max_age, capacity, active, version) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,1)",
    )
    .bind(id)
    .bind(actor.org)
    .bind(facility_id)
    .bind(&code)
    .bind(&name)
    .bind(&level)
    .bind(min_age)
    .bind(max_age)
    .bind(capacity)
    .bind(active)
    .execute(&mut *tx)
    .await?;
    replace_forms(&mut tx, actor.org, id, "intake", &intake).await?;
    replace_forms(&mut tx, actor.org, id, "discharge", &discharge).await?;
    let view = one_program(&mut tx, actor.org, id).await?;
    insert_program_revision(
        &mut tx,
        &actor,
        id,
        version_of(&view)?,
        configuration_snapshot(&view),
        &reason,
    )
    .await?;
    access::audit(&mut tx, &actor, "create", "care_programs", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(StatusCode::CREATED, view))
}

struct ProgramDraft {
    name: String,
    min_age: i32,
    max_age: Option<i32>,
    capacity: i32,
    active: bool,
    intake: Option<Vec<Uuid>>,
    discharge: Option<Vec<Uuid>>,
}
fn merge_program(current: &Value, body: &Map<String, Value>) -> Result<ProgramDraft> {
    let name = match body.get("name") {
        None => required_text(current, "name")?,
        Some(value) => text_field(value, "name", 1, 160)?,
    };
    let min_age = match body.get("min_age") {
        None => required_i32(current, "min_age")?,
        Some(value) => parse_i32(value, "min_age")?,
    };
    let max_age = match body.get("max_age") {
        None => current_optional_i32(current, "max_age")?,
        Some(Value::Null) => None,
        Some(value) => Some(parse_i32(value, "max_age")?),
    };
    let capacity = match body.get("capacity") {
        None => required_i32(current, "capacity")?,
        Some(value) => parse_i32(value, "capacity")?,
    };
    let active = match body.get("active") {
        None => required_bool(current, "active")?,
        Some(value) => json_bool(value, "active")?,
    };
    let intake = match body.get("required_intake_template_ids") {
        None => None,
        Some(value) => Some(uuid_list(value, "required_intake_template_ids")?),
    };
    let discharge = match body.get("required_discharge_template_ids") {
        None => None,
        Some(value) => Some(uuid_list(value, "required_discharge_template_ids")?),
    };
    validate_ages(min_age, max_age)?;
    validate_capacity(capacity)?;
    Ok(ProgramDraft {
        name,
        min_age,
        max_age,
        capacity,
        active,
        intake,
        discharge,
    })
}

async fn patch_program(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let (mut tx, actor) = open(&req, &state, true).await?;
    let input = object(&body)?;
    reject_unknown(
        input,
        &[
            "_expected_updated_at",
            "change_reason",
            "name",
            "min_age",
            "max_age",
            "capacity",
            "active",
            "required_intake_template_ids",
            "required_discharge_template_ids",
        ],
    )?;
    let expected = expected_stamp(input.get("_expected_updated_at"))?;
    let reason = text_field(require(input, "change_reason")?, "change_reason", 10, 2000)?;
    lock_expected(&mut tx, "care_programs", id, actor.org, &expected).await?;
    let current = one_program(&mut tx, actor.org, id).await?;
    let draft = merge_program(&current, input)?;
    let current_intake = ids_of(&current, "required_intake_template_ids")?;
    let current_discharge = ids_of(&current, "required_discharge_template_ids")?;
    let intake_same = draft
        .intake
        .as_ref()
        .map(|ids| same_ids(ids, &current_intake))
        .unwrap_or(true);
    let discharge_same = draft
        .discharge
        .as_ref()
        .map(|ids| same_ids(ids, &current_discharge))
        .unwrap_or(true);
    let unchanged = draft.name == required_text(&current, "name")?
        && draft.min_age == required_i32(&current, "min_age")?
        && draft.max_age == current_optional_i32(&current, "max_age")?
        && draft.capacity == required_i32(&current, "capacity")?
        && draft.active == required_bool(&current, "active")?
        && intake_same
        && discharge_same;
    if unchanged {
        return Err(ApiError::invalid(
            "No configuration changes were submitted.",
        ));
    }
    if draft.active && current["active"] != true {
        let facility_id = json_uuid(&current["facility_id"], "facility_id")?;
        let active: Option<bool> = sqlx::query_scalar(
            "SELECT active FROM facilities WHERE id=$1 AND organization_id=$2 FOR UPDATE",
        )
        .bind(facility_id)
        .bind(actor.org)
        .fetch_optional(&mut *tx)
        .await?;
        if active != Some(true) {
            return Err(ApiError::conflict(
                "A program can be activated only while its facility is active.",
            ));
        }
    }
    let census = active_census(&mut tx, actor.org, id).await?;
    if i64::from(draft.capacity) < census {
        return Err(ApiError::conflict(
            "Capacity cannot be reduced below the active census. Planned care does not consume a place, and existing active care was left unchanged.",
        ));
    }
    if let Some(ids) = &draft.intake {
        require_active_templates(&mut tx, actor.org, ids).await?;
    }
    if let Some(ids) = &draft.discharge {
        require_active_templates(&mut tx, actor.org, ids).await?;
    }
    let updated = sqlx::query(
        "UPDATE care_programs SET name=$1, min_age=$2, max_age=$3, capacity=$4, active=$5, version=version+1, updated_at=clock_timestamp() WHERE id=$6 AND organization_id=$7 AND updated_at=$8::text::timestamptz",
    )
    .bind(&draft.name)
    .bind(draft.min_age)
    .bind(draft.max_age)
    .bind(draft.capacity)
    .bind(draft.active)
    .bind(id)
    .bind(actor.org)
    .bind(&expected)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::conflict(STALE));
    }
    if let Some(ids) = &draft.intake {
        replace_forms(&mut tx, actor.org, id, "intake", ids).await?;
    }
    if let Some(ids) = &draft.discharge {
        replace_forms(&mut tx, actor.org, id, "discharge", ids).await?;
    }
    let view = one_program(&mut tx, actor.org, id).await?;
    insert_program_revision(
        &mut tx,
        &actor,
        id,
        version_of(&view)?,
        configuration_snapshot(&view),
        &reason,
    )
    .await?;
    access::audit(&mut tx, &actor, "update", "care_programs", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(StatusCode::OK, view))
}

async fn program_history(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let (mut tx, actor) = open(&req, &state, true).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM care_programs WHERE id=$1 AND organization_id=$2)",
    )
    .bind(id)
    .bind(actor.org)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(ApiError::missing());
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM program_revisions WHERE organization_id=$1 AND program_id=$2",
    )
    .bind(actor.org)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if count > 200 {
        return Err(ApiError::conflict(
            "Revision history exceeds 200 entries. No partial history was returned.",
        ));
    }
    let revisions: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('id', r.id, 'organization_id', r.organization_id, 'program_id', r.program_id, 'version', r.version, 'snapshot', r.snapshot, 'reason', r.reason, 'actor_id', r.actor_id, 'actor_name', s.name, 'created_at', r.created_at) FROM program_revisions r JOIN staff s ON s.id=r.actor_id AND s.organization_id=r.organization_id WHERE r.organization_id=$1 AND r.program_id=$2 ORDER BY r.version DESC, r.created_at DESC, r.id DESC",
    )
    .bind(actor.org)
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    access::audit(&mut tx, &actor, "read", "program_revisions", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(StatusCode::OK, json!({"revisions": revisions})))
}

async fn create_service(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let (mut tx, actor) = open(&req, &state, true).await?;
    let input = object(&body)?;
    reject_unknown(
        input,
        &[
            "program_id",
            "code_system",
            "service_code",
            "name",
            "session_kind",
            "min_minutes",
            "default_minutes",
            "max_minutes",
            "default_charge_cents",
            "note_template_id",
            "active",
            "change_reason",
        ],
    )?;
    let program_id = json_uuid(require(input, "program_id")?, "program_id")?;
    let code_system =
        enum_field(require(input, "code_system")?, SYSTEMS, "code_system")?.to_string();
    let raw_code = text_field(require(input, "service_code")?, "service_code", 1, 40)?;
    let service_code = crate::service_codes::canonical(&mut tx, &raw_code)
        .await?
        .filter(|code| (1..=40).contains(&code.chars().count()))
        .ok_or_else(|| {
            ApiError::invalid("service_code must contain 1-40 characters after canonicalization")
        })?;
    let name = text_field(require(input, "name")?, "name", 1, 160)?;
    let session_kind =
        enum_field(require(input, "session_kind")?, KINDS, "session_kind")?.to_string();
    let min_minutes = parse_i32(require(input, "min_minutes")?, "min_minutes")?;
    let default_minutes = parse_i32(require(input, "default_minutes")?, "default_minutes")?;
    let max_minutes = parse_i32(require(input, "max_minutes")?, "max_minutes")?;
    let charge = optional_charge(input)?;
    let note_template_id = optional_template(input, "note_template_id")?;
    let active = json_bool(require(input, "active")?, "active")?;
    let reason = text_field(require(input, "change_reason")?, "change_reason", 10, 2000)?;
    validate_minutes(min_minutes, default_minutes, max_minutes)?;
    validate_charge(charge)?;
    let program: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(p) FROM care_programs p WHERE id=$1 AND organization_id=$2 FOR UPDATE",
    )
    .bind(program_id)
    .bind(actor.org)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(program) = program else {
        return Err(ApiError::invalid(
            "The parent program must be active in this workspace.",
        ));
    };
    if program["active"] != true {
        return Err(ApiError::invalid(
            "The parent program must be active in this workspace.",
        ));
    }
    let facility_id = json_uuid(&program["facility_id"], "facility_id")?;
    require_active_facility(&mut tx, actor.org, facility_id).await?;
    if let Some(template) = note_template_id {
        require_active_note_template(&mut tx, actor.org, template).await?;
    }
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM program_services WHERE program_id=$1 AND organization_id=$2 AND code_system=$3 AND (nexus_service_code_v1(service_code) COLLATE \"C\")=($4 COLLATE \"C\"))",
    )
    .bind(program_id)
    .bind(actor.org)
    .bind(&code_system)
    .bind(&service_code)
    .fetch_one(&mut *tx)
    .await?;
    if taken {
        return Err(ApiError::conflict(
            "A service code must be unique for this program and code system regardless of letter case.",
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO program_services(id, organization_id, program_id, code_system, service_code, name, session_kind, min_minutes, default_minutes, max_minutes, default_charge_cents, note_template_id, active, version) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,1)",
    )
    .bind(id)
    .bind(actor.org)
    .bind(program_id)
    .bind(&code_system)
    .bind(&service_code)
    .bind(&name)
    .bind(&session_kind)
    .bind(min_minutes)
    .bind(default_minutes)
    .bind(max_minutes)
    .bind(charge)
    .bind(note_template_id)
    .bind(active)
    .execute(&mut *tx)
    .await?;
    let view = one_service(&mut tx, actor.org, id).await?;
    insert_service_revision(
        &mut tx,
        &actor,
        id,
        version_of(&view)?,
        service_snapshot(&view),
        &reason,
    )
    .await?;
    access::audit(&mut tx, &actor, "create", "program_services", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(StatusCode::CREATED, view))
}

struct ServiceDraft {
    name: String,
    min_minutes: i32,
    default_minutes: i32,
    max_minutes: i32,
    charge: Option<i64>,
    template: Option<Uuid>,
    active: bool,
}
fn merge_service(current: &Value, body: &Map<String, Value>) -> Result<ServiceDraft> {
    let name = match body.get("name") {
        None => required_text(current, "name")?,
        Some(value) => text_field(value, "name", 1, 160)?,
    };
    let min_minutes = match body.get("min_minutes") {
        None => required_i32(current, "min_minutes")?,
        Some(value) => parse_i32(value, "min_minutes")?,
    };
    let default_minutes = match body.get("default_minutes") {
        None => required_i32(current, "default_minutes")?,
        Some(value) => parse_i32(value, "default_minutes")?,
    };
    let max_minutes = match body.get("max_minutes") {
        None => required_i32(current, "max_minutes")?,
        Some(value) => parse_i32(value, "max_minutes")?,
    };
    let charge = match body.get("default_charge_cents") {
        None => current_optional_i64(current, "default_charge_cents")?,
        Some(Value::Null) => None,
        Some(value) => Some(parse_i64(value, "default_charge_cents")?),
    };
    let template = match body.get("note_template_id") {
        None => match current.get("note_template_id") {
            None | Some(Value::Null) => None,
            Some(value) => Some(json_uuid(value, "note_template_id")?),
        },
        Some(Value::Null) => None,
        Some(value) => Some(json_uuid(value, "note_template_id")?),
    };
    let active = match body.get("active") {
        None => required_bool(current, "active")?,
        Some(value) => json_bool(value, "active")?,
    };
    validate_minutes(min_minutes, default_minutes, max_minutes)?;
    validate_charge(charge)?;
    Ok(ServiceDraft {
        name,
        min_minutes,
        default_minutes,
        max_minutes,
        charge,
        template,
        active,
    })
}

async fn patch_service(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let (mut tx, actor) = open(&req, &state, true).await?;
    let input = object(&body)?;
    reject_unknown(
        input,
        &[
            "_expected_updated_at",
            "change_reason",
            "name",
            "min_minutes",
            "default_minutes",
            "max_minutes",
            "default_charge_cents",
            "note_template_id",
            "active",
        ],
    )?;
    let expected = expected_stamp(input.get("_expected_updated_at"))?;
    let reason = text_field(require(input, "change_reason")?, "change_reason", 10, 2000)?;
    lock_expected(&mut tx, "program_services", id, actor.org, &expected).await?;
    let current = one_service(&mut tx, actor.org, id).await?;
    let draft = merge_service(&current, input)?;
    let current_template = match current.get("note_template_id") {
        None | Some(Value::Null) => None,
        Some(value) => Some(json_uuid(value, "note_template_id")?),
    };
    let unchanged = draft.name == required_text(&current, "name")?
        && draft.min_minutes == required_i32(&current, "min_minutes")?
        && draft.default_minutes == required_i32(&current, "default_minutes")?
        && draft.max_minutes == required_i32(&current, "max_minutes")?
        && draft.charge == current_optional_i64(&current, "default_charge_cents")?
        && draft.template == current_template
        && draft.active == required_bool(&current, "active")?;
    if unchanged {
        return Err(ApiError::invalid(
            "No configuration changes were submitted.",
        ));
    }
    if input.contains_key("note_template_id") {
        if let Some(template) = draft.template {
            require_active_note_template(&mut tx, actor.org, template).await?;
        }
    }
    if draft.active && current["active"] != true {
        let program_id = json_uuid(&current["program_id"], "program_id")?;
        let program: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(p) FROM care_programs p WHERE id=$1 AND organization_id=$2 FOR UPDATE",
        )
        .bind(program_id)
        .bind(actor.org)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(program) = program else {
            return Err(ApiError::conflict(
                "A service can be activated only while its program and facility are active.",
            ));
        };
        let facility_active: Option<bool> = sqlx::query_scalar(
            "SELECT active FROM facilities WHERE id=$1 AND organization_id=$2 FOR UPDATE",
        )
        .bind(json_uuid(&program["facility_id"], "facility_id")?)
        .bind(actor.org)
        .fetch_optional(&mut *tx)
        .await?;
        if program["active"] != true || facility_active != Some(true) {
            return Err(ApiError::conflict(
                "A service can be activated only while its program and facility are active.",
            ));
        }
    }
    let updated = sqlx::query(
        "UPDATE program_services SET name=$1, min_minutes=$2, default_minutes=$3, max_minutes=$4, default_charge_cents=$5, note_template_id=$6, active=$7, version=version+1, updated_at=clock_timestamp() WHERE id=$8 AND organization_id=$9 AND updated_at=$10::text::timestamptz",
    )
    .bind(&draft.name)
    .bind(draft.min_minutes)
    .bind(draft.default_minutes)
    .bind(draft.max_minutes)
    .bind(draft.charge)
    .bind(draft.template)
    .bind(draft.active)
    .bind(id)
    .bind(actor.org)
    .bind(&expected)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::conflict(STALE));
    }
    let view = one_service(&mut tx, actor.org, id).await?;
    insert_service_revision(
        &mut tx,
        &actor,
        id,
        version_of(&view)?,
        service_snapshot(&view),
        &reason,
    )
    .await?;
    access::audit(&mut tx, &actor, "update", "program_services", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(StatusCode::OK, view))
}

async fn service_history(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let (mut tx, actor) = open(&req, &state, true).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM program_services WHERE id=$1 AND organization_id=$2)",
    )
    .bind(id)
    .bind(actor.org)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(ApiError::missing());
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM program_service_revisions WHERE organization_id=$1 AND service_id=$2",
    )
    .bind(actor.org)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if count > 200 {
        return Err(ApiError::conflict(
            "Revision history exceeds 200 entries. No partial history was returned.",
        ));
    }
    let revisions: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('id', r.id, 'organization_id', r.organization_id, 'service_id', r.service_id, 'version', r.version, 'snapshot', r.snapshot, 'reason', r.reason, 'actor_id', r.actor_id, 'actor_name', s.name, 'created_at', r.created_at) FROM program_service_revisions r JOIN staff s ON s.id=r.actor_id AND s.organization_id=r.organization_id WHERE r.organization_id=$1 AND r.service_id=$2 ORDER BY r.version DESC, r.created_at DESC, r.id DESC",
    )
    .bind(actor.org)
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    access::audit(
        &mut tx,
        &actor,
        "read",
        "program_service_revisions",
        Some(id),
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(StatusCode::OK, json!({"revisions": revisions})))
}
