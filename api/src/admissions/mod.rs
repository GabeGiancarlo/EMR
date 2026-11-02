//! Intake cases and local staff identity attestations.
//! Program-capacity configuration stays separate. Legacy episode workflows remain
//! available until a configurable admission policy exists; linked cases do not.
mod checklist;

use crate::patient_identity::fingerprint;
use crate::{
    access::{self, Actor},
    records, ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

const STALE: &str = "This record was updated after you opened it. Reload the latest version and preserve your unsaved edits; they were not saved.";
const LINKED_EPISODE: &str = "This episode belongs to an intake case. Use its admission or discharge action; its facility, care level, and dates are preserved.";

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/clients/{id}/intake", web::get().to(list_intake))
        .route("/clients/{id}/intake", web::post().to(create_case))
        .route(
            "/clients/{id}/identity-checks",
            web::post().to(create_identity),
        )
        .route("/intake-cases/{id}", web::patch().to(patch_case))
        .route("/intake-cases/{id}/transition", web::post().to(transition));
}

fn intake_role(role: &str) -> bool {
    matches!(role, "admin" | "clinical_lead" | "clinician" | "support")
}
fn clinical_role(role: &str) -> bool {
    matches!(role, "admin" | "clinical_lead" | "clinician")
}
fn no_store(status: actix_web::http::StatusCode, body: Value) -> HttpResponse {
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
        return Err(ApiError::invalid(format!(
            "Field {key} cannot be changed on an intake record"
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
            "{field} must contain {min}–{max} characters"
        )));
    }
    Ok(trimmed.to_string())
}
fn one_of<'a>(value: &str, allowed: &'a [&str], field: &str) -> Result<&'a str> {
    allowed
        .iter()
        .copied()
        .find(|item| *item == value.trim())
        .ok_or_else(|| ApiError::invalid(format!("{field} is not an accepted value")))
}
fn allows(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("open", "screening" | "declined" | "withdrawn")
            | ("screening", "ready" | "declined" | "withdrawn")
            | ("ready", "screening" | "admitted" | "declined" | "withdrawn")
    )
}
fn expected_stamp(value: Option<&Value>) -> Result<DateTime<Utc>> {
    let Some(Value::String(text)) = value else {
        return Err(ApiError::precondition("Reload the record before saving. The originally loaded updated_at is required so a newer edit is not overwritten."));
    };
    let text = text.trim();
    if text.is_empty() {
        return Err(ApiError::precondition("Reload the record before saving. The originally loaded updated_at is required so a newer edit is not overwritten."));
    }
    DateTime::parse_from_rfc3339(text)
        .map(|stamp| stamp.with_timezone(&Utc))
        .map_err(|_| ApiError::invalid("_expected_updated_at must be an RFC3339 timestamp"))
}
fn require_version(row: &Value, expected: DateTime<Utc>) -> Result<()> {
    let current = DateTime::parse_from_rfc3339(row["updated_at"].as_str().unwrap_or_default())
        .map(|stamp| stamp.with_timezone(&Utc))
        .map_err(|_| ApiError::conflict(STALE))?;
    if current != expected {
        Err(ApiError::conflict(STALE))
    } else {
        Ok(())
    }
}
fn changed(before: &Value, data: &Map<String, Value>, key: &str) -> bool {
    data.get(key).is_some_and(|value| value != &before[key])
}

async fn recheck(
    req: &HttpRequest,
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
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
    if intake_role(&actor.role) {
        Ok(actor)
    } else {
        Err(ApiError::denied())
    }
}
async fn open(
    req: &HttpRequest,
    state: &AppState,
) -> Result<(Transaction<'static, Postgres>, Actor)> {
    let initial = access::actor(req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = recheck(req, &mut tx, initial.org).await?;
    Ok((tx, actor))
}
async fn load_client(tx: &mut Transaction<'_, Postgres>, actor: &Actor, id: Uuid) -> Result<Value> {
    let client = records::get(tx, "clients", id, actor.org, true).await?;
    records::scope(tx, actor, "clients", &client).await?;
    Ok(client)
}
async fn facility_context(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    facility_id: Uuid,
) -> Result<(Value, NaiveDate)> {
    let facility = records::get(tx, "facilities", facility_id, actor.org, false)
        .await
        .map_err(|_| {
            ApiError::invalid("The intended facility must be active in this workspace.")
        })?;
    let zone = facility["timezone"].as_str().unwrap_or("UTC");
    let today: NaiveDate = sqlx::query_scalar("SELECT (CURRENT_TIMESTAMP AT TIME ZONE $1)::date")
        .bind(zone)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| {
            ApiError::invalid("The intended facility timezone could not supply a local date.")
        })?;
    Ok((facility, today))
}
async fn require_owner(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    owner_id: Uuid,
    client: &Value,
) -> Result<()> {
    let staff = records::get(tx, "staff", owner_id, actor.org, true).await.map_err(|_| ApiError::invalid("The intake owner must be an active admin, clinical lead, clinician, or support staff member in this workspace."))?;
    if staff["role"] == "clinician" && client["primary_clinician_id"] != json!(owner_id) {
        return Err(ApiError::invalid(
            "A clinician intake owner must be this patient's assigned primary clinician.",
        ));
    }
    if staff["active"] == true && intake_role(staff["role"].as_str().unwrap_or_default()) {
        Ok(())
    } else {
        Err(ApiError::invalid("The intake owner must be an active admin, clinical lead, clinician, or support staff member in this workspace."))
    }
}

async fn case_row(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    id: Uuid,
    lock: bool,
) -> Result<Value> {
    let sql = format!(
        "SELECT to_jsonb(c) FROM intake_cases c WHERE id=$1 AND organization_id=$2 {}",
        if lock { "FOR UPDATE" } else { "" }
    );
    sqlx::query_scalar(&sql)
        .bind(id)
        .bind(org)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::missing)
}
async fn case_view(tx: &mut Transaction<'_, Postgres>, actor: &Actor, id: Uuid) -> Result<Value> {
    let mut row: Value = sqlx::query_scalar("SELECT jsonb_build_object('id',c.id,'client_id',c.client_id,'episode_id',c.episode_id,'facility_id',e.facility_id,'level_of_care',e.level_of_care,'starts_on',e.start_date,'owner_id',c.owner_id,'referral_type',c.referral_type,'referrer_name',c.referrer_name,'referrer_contact',c.referrer_contact,'referral_reason',c.referral_reason,'received_on',c.received_on,'status',c.status,'disposition_reason',c.disposition_reason,'created_at',c.created_at,'updated_at',c.updated_at) FROM intake_cases c JOIN episodes e ON e.id=c.episode_id AND e.client_id=c.client_id AND e.organization_id=c.organization_id WHERE c.id=$1 AND c.organization_id=$2").bind(id).bind(actor.org).fetch_optional(&mut **tx).await?.ok_or_else(ApiError::missing)?;
    let client = records::get(
        tx,
        "clients",
        records::uuid(&row, "client_id")?,
        actor.org,
        false,
    )
    .await?;
    let episode = records::get(
        tx,
        "episodes",
        records::uuid(&row, "episode_id")?,
        actor.org,
        false,
    )
    .await?;
    row["events"] = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(jsonb_build_object('id',id,'from_status',from_status,'to_status',to_status,'reason',reason,'actor_id',actor_id,'created_at',created_at) ORDER BY created_at ASC, id ASC), '[]'::jsonb) FROM intake_events WHERE organization_id=$1 AND intake_case_id=$2").bind(actor.org).bind(id).fetch_one(&mut **tx).await?;
    row["checklist"] = json!(checklist::evaluate(tx, actor, &client, &episode).await?);
    Ok(row)
}
fn with_current(checks: Vec<Value>, client: &Value) -> Vec<Value> {
    let current_fp = fingerprint(client);
    let latest = checks
        .first()
        .and_then(|row| row["id"].as_str())
        .map(str::to_owned);
    checks
        .into_iter()
        .map(|mut row| {
            let current = latest.as_deref() == row["id"].as_str()
                && row["outcome"] == "confirmed"
                && row["identity_fingerprint"] == current_fp;
            row["current"] = json!(current);
            row
        })
        .collect()
}

async fn list_intake(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let client_id = path.into_inner();
    let (mut tx, actor) = open(&req, &state).await?;
    let client = load_client(&mut tx, &actor, client_id).await?;
    let case_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM intake_cases WHERE organization_id=$1 AND client_id=$2",
    )
    .bind(actor.org)
    .bind(client_id)
    .fetch_one(&mut *tx)
    .await?;
    if case_count > 100 {
        return Err(ApiError::conflict(
            "Intake history exceeds 100 cases. No partial history was returned.",
        ));
    }
    let check_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM patient_identity_checks WHERE organization_id=$1 AND client_id=$2",
    )
    .bind(actor.org)
    .bind(client_id)
    .fetch_one(&mut *tx)
    .await?;
    if check_count > 100 {
        return Err(ApiError::conflict(
            "Identity attestation history exceeds 100 checks. No partial history was returned.",
        ));
    }
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM intake_cases WHERE organization_id=$1 AND client_id=$2 ORDER BY created_at DESC, id DESC").bind(actor.org).bind(client_id).fetch_all(&mut *tx).await?;
    let mut cases = Vec::with_capacity(ids.len());
    for id in ids {
        cases.push(case_view(&mut tx, &actor, id).await?);
    }
    let checks: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('id',id,'client_id',client_id,'actor_id',actor_id,'method',method,'outcome',outcome,'reason',reason,'attested',attested,'identity_fingerprint',identity_fingerprint,'created_at',created_at) FROM patient_identity_checks WHERE organization_id=$1 AND client_id=$2 ORDER BY created_at DESC, id DESC").bind(actor.org).bind(client_id).fetch_all(&mut *tx).await?;
    access::audit(&mut tx, &actor, "read", "intake_cases", Some(client_id)).await?;
    tx.commit().await?;
    Ok(no_store(
        actix_web::http::StatusCode::OK,
        json!({"client_id":client_id,"cases":cases,"identity_checks":with_current(checks, &client)}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityInput {
    method: String,
    outcome: String,
    reason: String,
    attested: bool,
}
async fn create_identity(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<IdentityInput>,
) -> Result<HttpResponse> {
    let client_id = path.into_inner();
    if !body.attested {
        return Err(ApiError::invalid(
            "Identity attestation must be explicitly confirmed.",
        ));
    }
    let method = one_of(
        &body.method,
        &["patient_report", "document_review", "prior_record"],
        "method",
    )?;
    let outcome = one_of(&body.outcome, &["confirmed", "unable"], "outcome")?;
    let reason = bounded(&body.reason, 10, 2000, "reason")?;
    let (mut tx, actor) = open(&req, &state).await?;
    let client = load_client(&mut tx, &actor, client_id).await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO patient_identity_checks(id,organization_id,client_id,actor_id,method,outcome,reason,attested,identity_fingerprint) VALUES($1,$2,$3,$4,$5,$6,$7,true,$8)").bind(id).bind(actor.org).bind(client_id).bind(actor.id).bind(method).bind(outcome).bind(&reason).bind(fingerprint(&client)).execute(&mut *tx).await?;
    access::audit(
        &mut tx,
        &actor,
        "attest",
        "patient_identity_checks",
        Some(id),
    )
    .await?;
    let checks: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('id',id,'client_id',client_id,'actor_id',actor_id,'method',method,'outcome',outcome,'reason',reason,'attested',attested,'identity_fingerprint',identity_fingerprint,'created_at',created_at) FROM patient_identity_checks WHERE id=$1").bind(id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(no_store(
        actix_web::http::StatusCode::CREATED,
        with_current(checks, &client)
            .into_iter()
            .next()
            .unwrap_or(Value::Null),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCase {
    facility_id: Uuid,
    level_of_care: String,
    starts_on: NaiveDate,
    owner_id: Uuid,
    referral_type: String,
    referrer_name: String,
    referrer_contact: String,
    referral_reason: String,
    received_on: NaiveDate,
}
async fn create_case(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<CreateCase>,
) -> Result<HttpResponse> {
    let client_id = path.into_inner();
    let level = one_of(
        &body.level_of_care,
        &["outpatient", "iop", "php"],
        "level_of_care",
    )?;
    let referral = one_of(
        &body.referral_type,
        &["self", "provider", "facility", "family", "justice", "other"],
        "referral_type",
    )?;
    let referrer_name = bounded(&body.referrer_name, 0, 200, "referrer_name")?;
    let referrer_contact = bounded(&body.referrer_contact, 0, 200, "referrer_contact")?;
    let referral_reason = bounded(&body.referral_reason, 1, 2000, "referral_reason")?;
    let (mut tx, actor) = open(&req, &state).await?;
    let client = load_client(&mut tx, &actor, client_id).await?;
    let (facility, today) = facility_context(&mut tx, &actor, body.facility_id).await?;
    if facility["active"] != true {
        return Err(ApiError::invalid(
            "The intended facility must be active in this workspace.",
        ));
    }
    if body.received_on > today {
        return Err(ApiError::invalid(
            "received_on cannot be after the facility's local date",
        ));
    }
    let birth = records::parse_record_date(&client, "birth_date")?;
    records::require_adult_program(birth, today, "birth_date")?;
    require_owner(&mut tx, &actor, body.owner_id, &client).await?;
    let open_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intake_cases WHERE organization_id=$1 AND client_id=$2 AND status IN ('open','screening','ready'))").bind(actor.org).bind(client_id).fetch_one(&mut *tx).await?;
    if open_exists {
        return Err(ApiError::conflict(
            "This patient already has a nonterminal intake case.",
        ));
    }
    let mut episode_data = Map::new();
    episode_data.insert("client_id".into(), json!(client_id));
    episode_data.insert("facility_id".into(), json!(body.facility_id));
    episode_data.insert("level_of_care".into(), json!(level));
    episode_data.insert("start_date".into(), json!(body.starts_on));
    episode_data.insert("status".into(), json!("planned"));
    records::validate_links(
        &mut tx,
        &actor,
        "episodes",
        &Value::Object(episode_data.clone()),
    )
    .await?;
    let episode = records::insert(&mut tx, "episodes", episode_data, &actor).await?;
    let case_id = Uuid::new_v4();
    sqlx::query("INSERT INTO intake_cases(id,organization_id,client_id,episode_id,owner_id,referral_type,referrer_name,referrer_contact,referral_reason,received_on,status,disposition_reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'open','')").bind(case_id).bind(actor.org).bind(client_id).bind(records::uuid(&episode, "id")?).bind(body.owner_id).bind(referral).bind(&referrer_name).bind(&referrer_contact).bind(&referral_reason).bind(body.received_on).execute(&mut *tx).await?;
    sqlx::query("UPDATE clients SET status='intake', updated_at=clock_timestamp() WHERE id=$1 AND organization_id=$2 AND status IS DISTINCT FROM 'intake' AND NOT EXISTS (SELECT 1 FROM episodes e WHERE e.organization_id=$2 AND e.client_id=$1 AND e.status='active')").bind(client_id).bind(actor.org).execute(&mut *tx).await?;
    access::audit(&mut tx, &actor, "create", "intake_cases", Some(case_id)).await?;
    let view = case_view(&mut tx, &actor, case_id).await?;
    tx.commit().await?;
    Ok(no_store(actix_web::http::StatusCode::CREATED, view))
}

async fn patch_case(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let input = object(&body)?;
    reject_unknown(
        input,
        &[
            "_expected_updated_at",
            "owner_id",
            "referrer_name",
            "referrer_contact",
            "referral_reason",
        ],
    )?;
    let expected = expected_stamp(input.get("_expected_updated_at"))?;
    let owner_id = match input.get("owner_id") {
        None => None,
        Some(Value::String(value)) => Some(
            Uuid::parse_str(value.trim())
                .map_err(|_| ApiError::invalid("owner_id must be a UUID"))?,
        ),
        Some(_) => return Err(ApiError::invalid("owner_id must be a UUID")),
    };
    let referrer_name = input
        .get("referrer_name")
        .map(|value| {
            bounded(
                value
                    .as_str()
                    .ok_or_else(|| ApiError::invalid("referrer_name must be text"))?,
                0,
                200,
                "referrer_name",
            )
        })
        .transpose()?;
    let referrer_contact = input
        .get("referrer_contact")
        .map(|value| {
            bounded(
                value
                    .as_str()
                    .ok_or_else(|| ApiError::invalid("referrer_contact must be text"))?,
                0,
                200,
                "referrer_contact",
            )
        })
        .transpose()?;
    let referral_reason = input
        .get("referral_reason")
        .map(|value| {
            bounded(
                value
                    .as_str()
                    .ok_or_else(|| ApiError::invalid("referral_reason must be text"))?,
                1,
                2000,
                "referral_reason",
            )
        })
        .transpose()?;
    if owner_id.is_none()
        && referrer_name.is_none()
        && referrer_contact.is_none()
        && referral_reason.is_none()
    {
        return Err(ApiError::invalid(
            "No editable intake fields were provided.",
        ));
    }
    let (mut tx, actor) = open(&req, &state).await?;
    let before = case_row(&mut tx, actor.org, id, true).await?;
    let client = load_client(&mut tx, &actor, records::uuid(&before, "client_id")?).await?;
    require_version(&before, expected)?;
    if !matches!(
        before["status"].as_str(),
        Some("open" | "screening" | "ready")
    ) {
        return Err(ApiError::conflict("Only an open, screening, or ready intake case can be edited. Terminal dispositions are preserved."));
    }
    if let Some(owner_id) = owner_id {
        require_owner(&mut tx, &actor, owner_id, &client).await?;
    }
    let changed = sqlx::query("UPDATE intake_cases SET owner_id=COALESCE($1,owner_id), referrer_name=COALESCE($2,referrer_name), referrer_contact=COALESCE($3,referrer_contact), referral_reason=COALESCE($4,referral_reason), updated_at=clock_timestamp() WHERE id=$5 AND organization_id=$6 AND status IN ('open','screening','ready')").bind(owner_id).bind(referrer_name).bind(referrer_contact).bind(referral_reason).bind(id).bind(actor.org).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(ApiError::conflict("Only an open, screening, or ready intake case can be edited. Terminal dispositions are preserved."));
    }
    access::audit(&mut tx, &actor, "update", "intake_cases", Some(id)).await?;
    let view = case_view(&mut tx, &actor, id).await?;
    tx.commit().await?;
    Ok(no_store(actix_web::http::StatusCode::OK, view))
}

fn unmet(items: &[Value], required_only: bool) -> Vec<String> {
    items
        .iter()
        .filter(|item| {
            let gate = if required_only {
                item["required"] == true
            } else {
                item["code"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("admission_")
            };
            gate && item["passed"] != true
        })
        .filter_map(|item| item["code"].as_str().map(str::to_owned))
        .collect()
}
async fn transition(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let input = object(&body)?;
    reject_unknown(
        input,
        &["_expected_updated_at", "to_status", "reason", "attested"],
    )?;
    let expected = expected_stamp(input.get("_expected_updated_at"))?;
    let to_status = one_of(
        input
            .get("to_status")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::invalid("to_status is not an accepted value"))?,
        &["screening", "ready", "admitted", "declined", "withdrawn"],
        "to_status",
    )?;
    let reason = bounded(
        input
            .get("reason")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::invalid("reason must be text"))?,
        10,
        2000,
        "reason",
    )?;
    if input.get("attested") != Some(&json!(true)) {
        return Err(ApiError::invalid(
            "The disposition must be explicitly attested.",
        ));
    }
    let (mut tx, actor) = open(&req, &state).await?;
    let before = case_row(&mut tx, actor.org, id, true).await?;
    let client_id = records::uuid(&before, "client_id")?;
    let client = load_client(&mut tx, &actor, client_id).await?;
    require_version(&before, expected)?;
    let from = before["status"].as_str().unwrap_or_default();
    if matches!(from, "admitted" | "declined" | "withdrawn") {
        return Err(ApiError::conflict(
            "This intake case is already closed. Its disposition was preserved.",
        ));
    }
    if !allows(from, to_status) {
        return Err(ApiError::conflict(
            "That intake status change is not allowed from the current state.",
        ));
    }
    let episode_id = records::uuid(&before, "episode_id")?;
    let episode = records::get(&mut tx, "episodes", episode_id, actor.org, true).await?;
    let items = checklist::evaluate(&mut tx, &actor, &client, &episode).await?;
    if matches!(to_status, "ready" | "admitted") {
        require_owner(
            &mut tx,
            &actor,
            records::uuid(&before, "owner_id")?,
            &client,
        )
        .await?;
        let missing = unmet(&items, true);
        if !missing.is_empty() {
            return Err(ApiError::conflict(format!(
                "Intake is not ready. Unmet checklist: {}.",
                missing.join(", ")
            )));
        }
    }
    if to_status == "admitted" {
        if !clinical_role(&actor.role) {
            return Err(ApiError::denied());
        }
        let missing = unmet(&items, false);
        if !missing.is_empty() {
            return Err(ApiError::conflict(format!(
                "Admission was not recorded. Unmet admission checks: {}.",
                missing.join(", ")
            )));
        }
        let activated = sqlx::query("UPDATE episodes SET status='active', updated_at=clock_timestamp() WHERE id=$1 AND organization_id=$2 AND client_id=$3 AND status='planned'").bind(episode_id).bind(actor.org).bind(client_id).execute(&mut *tx).await?;
        if activated.rows_affected() != 1 {
            return Err(ApiError::conflict(
                "The linked episode is no longer planned. Admission was not recorded.",
            ));
        }
        sqlx::query("UPDATE clients SET status='active', updated_at=clock_timestamp() WHERE id=$1 AND organization_id=$2").bind(client_id).bind(actor.org).execute(&mut *tx).await?;
    }
    let updated = sqlx::query("UPDATE intake_cases SET status=$1, disposition_reason=$2, updated_at=clock_timestamp() WHERE id=$3 AND organization_id=$4 AND status=$5").bind(to_status).bind(&reason).bind(id).bind(actor.org).bind(from).execute(&mut *tx).await?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::conflict(
            "The intake case changed before the disposition was recorded. Reload and try again.",
        ));
    }
    if matches!(to_status, "declined" | "withdrawn") {
        sqlx::query("UPDATE clients SET status=CASE WHEN EXISTS(SELECT 1 FROM episodes WHERE organization_id=$1 AND client_id=$2 AND status='active') THEN 'active' WHEN EXISTS(SELECT 1 FROM episodes WHERE organization_id=$1 AND client_id=$2 AND status='discharged') THEN 'discharged' ELSE 'inquiry' END,updated_at=clock_timestamp() WHERE organization_id=$1 AND id=$2").bind(actor.org).bind(client_id).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO intake_events(id,organization_id,client_id,intake_case_id,from_status,to_status,reason,actor_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(Uuid::new_v4()).bind(actor.org).bind(client_id).bind(id).bind(from).bind(to_status).bind(&reason).bind(actor.id).execute(&mut *tx).await?;
    access::audit(&mut tx, &actor, "transition", "intake_cases", Some(id)).await?;
    let view = case_view(&mut tx, &actor, id).await?;
    tx.commit().await?;
    Ok(no_store(actix_web::http::StatusCode::OK, view))
}

pub(crate) async fn before_episode_update(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    before: &Value,
    data: &Map<String, Value>,
) -> Result<()> {
    let activating = changed(before, data, "status");
    let context = [
        "facility_id",
        "level_of_care",
        "start_date",
        "client_id",
        "end_date",
        "discharge_reason",
    ]
    .iter()
    .any(|key| changed(before, data, key));
    if !activating && !context {
        return Ok(());
    }
    let episode_id = records::uuid(before, "id")?;
    let linked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM intake_cases WHERE organization_id=$1 AND episode_id=$2)",
    )
    .bind(actor.org)
    .bind(episode_id)
    .fetch_one(&mut **tx)
    .await?;
    if linked {
        Err(ApiError::conflict(LINKED_EPISODE))
    } else {
        Ok(())
    }
}

pub(crate) async fn before_client_update(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    before: &Value,
    data: &Map<String, Value>,
) -> Result<()> {
    if changed(before, data, "primary_clinician_id") {
        let owner: Option<Uuid> = sqlx::query_scalar("SELECT c.owner_id FROM intake_cases c JOIN staff s ON s.id=c.owner_id AND s.organization_id=c.organization_id WHERE c.organization_id=$1 AND c.client_id=$2 AND c.status IN ('open','screening','ready') AND s.role='clinician'").bind(actor.org).bind(records::uuid(before,"id")?).fetch_optional(&mut **tx).await?;
        if owner.is_some_and(|id| data.get("primary_clinician_id") != Some(&json!(id))) {
            return Err(ApiError::conflict(
                "Reassign the open intake owner before changing this patient's primary clinician.",
            ));
        }
    }
    if !changed(before, data, "status") {
        return Ok(());
    }
    let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intake_cases WHERE organization_id=$1 AND client_id=$2 AND status IN ('open','screening','ready'))").bind(actor.org).bind(records::uuid(before,"id")?).fetch_one(&mut **tx).await?;
    if pending {
        return Err(ApiError::conflict(
            "Use the open intake case to record this patient's admission or disposition.",
        ));
    }
    Ok(())
}

pub(crate) async fn before_staff_update(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    before: &Value,
    data: &Map<String, Value>,
) -> Result<()> {
    if let Some(role) = data.get("role").and_then(Value::as_str) {
        if changed(before, data, "role") && role == "clinician" {
            let outside_panel: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intake_cases c JOIN clients p ON p.id=c.client_id AND p.organization_id=c.organization_id WHERE c.organization_id=$1 AND c.owner_id=$2 AND c.status IN ('open','screening','ready') AND p.primary_clinician_id IS DISTINCT FROM $2)").bind(actor.org).bind(records::uuid(before,"id")?).fetch_one(&mut **tx).await?;
            if outside_panel {
                return Err(ApiError::conflict("Reassign open intakes outside this person's patient panel before changing to clinician."));
            }
        }
        if changed(before, data, "role") && !intake_role(role) {
            let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intake_cases WHERE organization_id=$1 AND owner_id=$2 AND status IN ('open','screening','ready'))").bind(actor.org).bind(records::uuid(before,"id")?).fetch_one(&mut **tx).await?;
            if pending {
                return Err(ApiError::conflict("Reassign this person's open intake cases before changing to a billing or executive role."));
            }
        }
    }
    Ok(())
}

pub(crate) async fn before_discharge(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    episode: &Value,
) -> Result<()> {
    let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intake_cases WHERE organization_id=$1 AND episode_id=$2 AND status<>'admitted')").bind(actor.org).bind(records::uuid(episode,"id")?).fetch_one(&mut **tx).await?;
    if pending {
        return Err(ApiError::conflict("This intake has not been admitted. Record a declined or withdrawn disposition in the intake case."));
    }
    Ok(())
}

pub(crate) async fn transfer_owner(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    old_owner: Uuid,
    replacement: &Value,
) -> Result<()> {
    let cases: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(c) FROM intake_cases c WHERE organization_id=$1 AND owner_id=$2 AND status IN ('open','screening','ready') FOR UPDATE").bind(actor.org).bind(old_owner).fetch_all(&mut **tx).await?;
    if !cases.is_empty() && !intake_role(replacement["role"].as_str().unwrap_or_default()) {
        return Err(ApiError::invalid(
            "Open intakes must transfer to an active admissions or clinical staff member.",
        ));
    }
    for case in cases {
        if replacement["role"] == "clinician" {
            let patient = records::get(
                tx,
                "clients",
                records::uuid(&case, "client_id")?,
                actor.org,
                false,
            )
            .await?;
            if patient["primary_clinician_id"] != json!(old_owner)
                && patient["primary_clinician_id"] != replacement["id"]
            {
                return Err(ApiError::invalid("The replacement clinician must be assigned to each transferred intake patient."));
            }
        }
        let id = records::uuid(&case, "id")?;
        sqlx::query("UPDATE intake_cases SET owner_id=$1,updated_at=clock_timestamp() WHERE id=$2")
            .bind(records::uuid(replacement, "id")?)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("INSERT INTO intake_events(id,organization_id,client_id,intake_case_id,from_status,to_status,reason,actor_id) VALUES($1,$2,$3,$4,$5,$5,'Intake ownership transferred during staff offboarding.',$6)").bind(Uuid::new_v4()).bind(actor.org).bind(records::uuid(&case,"client_id")?).bind(id).bind(case["status"].as_str().unwrap_or_default()).bind(actor.id).execute(&mut **tx).await?;
        access::audit(tx, actor, "transfer_owner", "intake_cases", Some(id)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::allows;
    #[test]
    fn transition_matrix_matches_contract() {
        for to in ["screening", "declined", "withdrawn"] {
            assert!(allows("open", to));
        }
        assert!(!allows("open", "ready") && !allows("open", "admitted"));
        for to in ["ready", "declined", "withdrawn"] {
            assert!(allows("screening", to));
        }
        assert!(!allows("screening", "open") && !allows("screening", "admitted"));
        for to in ["screening", "admitted", "declined", "withdrawn"] {
            assert!(allows("ready", to));
        }
        assert!(!allows("ready", "open"));
        for from in ["admitted", "declined", "withdrawn"] {
            for to in [
                "open",
                "screening",
                "ready",
                "admitted",
                "declined",
                "withdrawn",
            ] {
                assert!(!allows(from, to));
            }
        }
    }
    #[test]
    fn validation_errors_do_not_echo_submitted_text() {
        let err = super::bounded("secret clinical narrative", 10, 12, "reason").unwrap_err();
        assert!(!err.message.contains("secret"));
        assert!(super::one_of("not-a-method", &["patient_report"], "method").is_err());
    }
}
