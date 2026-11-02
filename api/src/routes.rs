use crate::{
    access::{self, Actor},
    records::{self, RESOURCES},
    ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub async fn health(state: web::Data<AppState>) -> Result<HttpResponse> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(HttpResponse::Ok().json(
        json!({"status":"healthy","database":"connected","authentication":"deferred-local-only"}),
    ))
}
#[derive(Deserialize)]
pub struct Setup {
    organization_name: String,
    facility_name: String,
    admin_name: String,
    admin_email: String,
}
pub async fn setup(state: web::Data<AppState>, body: web::Json<Setup>) -> Result<HttpResponse> {
    for value in [
        &body.organization_name,
        &body.facility_name,
        &body.admin_name,
    ] {
        if value.trim().is_empty() || value.len() > 200 {
            return Err(ApiError::invalid("Organization, facility and administrator names are required (up to 200 characters)"));
        }
    }
    if !body.admin_email.contains('@') || body.admin_email.contains(' ') {
        return Err(ApiError::invalid("A valid administrator email is required"));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(9182401)")
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations")
        .fetch_one(&mut *tx)
        .await?;
    if count > 0 {
        return Err(ApiError::conflict("Workspace setup is already complete"));
    }
    let org_id = Uuid::new_v4();
    let facility_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let organization: Value = sqlx::query_scalar(
        "INSERT INTO organizations(id,name) VALUES($1,$2) RETURNING to_jsonb(organizations)",
    )
    .bind(org_id)
    .bind(body.organization_name.trim())
    .fetch_one(&mut *tx)
    .await?;
    let facility:Value=sqlx::query_scalar("INSERT INTO facilities(id,organization_id,name) VALUES($1,$2,$3) RETURNING to_jsonb(facilities)").bind(facility_id).bind(org_id).bind(body.facility_name.trim()).fetch_one(&mut *tx).await?;
    let value:Value=sqlx::query_scalar("INSERT INTO staff(id,organization_id,facility_id,name,email,role) VALUES($1,$2,$3,$4,$5,'admin') RETURNING to_jsonb(staff)").bind(actor_id).bind(org_id).bind(facility_id).bind(body.admin_name.trim()).bind(body.admin_email.trim().to_lowercase()).fetch_one(&mut *tx).await?;
    let actor = Actor {
        id: actor_id,
        org: org_id,
        role: "admin".into(),
        value: value.clone(),
    };
    access::audit(&mut tx, &actor, "setup", "organizations", Some(org_id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Created()
        .json(json!({"organization":organization,"facility":facility,"actor":value})))
}

pub async fn rows(
    tx: &mut Transaction<'_, Postgres>,
    resource: &str,
    actor: &Actor,
) -> Result<Vec<Value>> {
    if resource == "audit" {
        return Ok(sqlx::query_scalar("SELECT to_jsonb(a) FROM audit_events a WHERE organization_id=$1 ORDER BY occurred_at DESC LIMIT 500").bind(actor.org).fetch_all(&mut **tx).await?);
    }
    records::fields(resource)?;
    let scope = if actor.role == "clinician" {
        match resource {
        "clients"=>" AND t.primary_clinician_id=$2",
        "episodes"|"sessions"|"notes"|"consents"|"policies"|"authorizations"|"claims"|"treatment_plans"|"goals"|"plan_reviews"|"group_attendance"|"form_responses"=>" AND t.client_id IN (SELECT id FROM clients WHERE organization_id=$1 AND primary_clinician_id=$2)",
        "groups"=>" AND (t.clinician_id=$2 OR t.id IN (SELECT a.group_id FROM group_attendance a JOIN clients c ON c.id=a.client_id AND c.organization_id=a.organization_id WHERE a.organization_id=$1 AND c.primary_clinician_id=$2))",
        "payments"|"payment_reversals"=>" AND t.claim_id IN (SELECT c.id FROM claims c JOIN clients p ON p.id=c.client_id AND p.organization_id=c.organization_id WHERE c.organization_id=$1 AND p.primary_clinician_id=$2)",
        "tasks"=>" AND (t.assignee_id=$2 OR t.client_id IN (SELECT id FROM clients WHERE organization_id=$1 AND primary_clinician_id=$2))",
        _=>""
    }
    } else {
        ""
    };
    let query=format!("SELECT to_jsonb(t) FROM {resource} t WHERE t.organization_id=$1{scope} ORDER BY t.created_at DESC");
    let q = sqlx::query_scalar(&query).bind(actor.org);
    Ok(if scope.is_empty() {
        q.fetch_all(&mut **tx).await?
    } else {
        q.bind(actor.id).fetch_all(&mut **tx).await?
    })
}
pub async fn bootstrap(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse> {
    let mut data = json!({"setup_required":false,"authentication":"deferred-local-only","actor":null,"organizations":[],"audit":[],"metrics":{}});
    for resource in RESOURCES {
        data[*resource] = json!([]);
    }
    let organizations: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(o) FROM organizations o ORDER BY created_at")
            .fetch_all(&state.pool)
            .await?;
    data["setup_required"] = json!(organizations.is_empty());
    data["organizations"] = json!(organizations);
    if req.headers().get("x-nexus-actor").is_none() {
        // Minimal local actor picker. No clinical or financial records without an actor.
        data["staff"]=json!(sqlx::query_scalar::<_,Value>("SELECT jsonb_build_object('id',id,'name',name,'role',role,'organization_id',organization_id,'facility_id',facility_id,'active',active) FROM staff WHERE active ORDER BY name").fetch_all(&state.pool).await?);
        data["facilities"] = json!(
            sqlx::query_scalar::<_, Value>("SELECT to_jsonb(f) FROM facilities f WHERE active")
                .fetch_all(&state.pool)
                .await?
        );
        return Ok(HttpResponse::Ok().json(data));
    }
    let actor = access::actor(&req, &state.pool).await?;
    data["actor"] = actor.value.clone();
    data["permissions"] = json!({});
    data["organizations"] = json!(organizations
        .into_iter()
        .filter(|o| o["id"] == actor.org.to_string())
        .collect::<Vec<_>>());
    let mut tx = state.pool.begin().await?;
    for resource in RESOURCES.iter().copied().chain(std::iter::once("audit")) {
        let read = access::allowed(&state.pool, &actor, resource, false).await?;
        let write = access::allowed(&state.pool, &actor, resource, true).await?;
        data["permissions"][resource] = json!({"read":read,"write":write});
        if read {
            data[resource] = json!(rows(&mut tx, resource, &actor).await?);
        }
    }
    let count = |r: &str, field: &str, value: &str| {
        data[r]
            .as_array()
            .map(|a| a.iter().filter(|r| r[field] == value).count())
            .unwrap_or_default()
    };
    let claim_total: i64 = data["claims"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|r| matches!(r["status"].as_str(), Some("submitted" | "denied" | "paid")))
                .map(|r| r["amount_cents"].as_i64().unwrap_or_default())
                .sum()
        })
        .unwrap_or_default();
    let paid: i64 = data["payments"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| r["amount_cents"].as_i64().unwrap_or_default())
                .sum()
        })
        .unwrap_or_default();
    let unbilled: i64 = data["claims"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|r| matches!(r["status"].as_str(), Some("draft" | "ready")))
                .map(|r| r["amount_cents"].as_i64().unwrap_or_default())
                .sum()
        })
        .unwrap_or_default();
    let cash: i64 = data["payments"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|r| r["source"] != "adjustment")
                .map(|r| r["amount_cents"].as_i64().unwrap_or_default())
                .sum()
        })
        .unwrap_or_default();
    let reversed: i64 = data["payment_reversals"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| r["amount_cents"].as_i64().unwrap_or_default())
                .sum()
        })
        .unwrap_or_default();
    let reversed_cash: i64 = data["payment_reversals"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|r| {
                    data["payments"]
                        .as_array()
                        .map(|ps| {
                            ps.iter()
                                .any(|p| p["id"] == r["payment_id"] && p["source"] != "adjustment")
                        })
                        .unwrap_or(false)
                })
                .map(|r| r["amount_cents"].as_i64().unwrap_or_default())
                .sum()
        })
        .unwrap_or_default();
    data["metrics"] = json!({"active_clients":count("clients","status","active"),"open_tasks":count("tasks","status","open")+count("tasks","status","in_progress"),"scheduled_sessions":count("sessions","status","scheduled"),"draft_notes":count("notes","status","draft"),"outstanding_cents":claim_total-paid+reversed,"unbilled_cents":unbilled,"cash_received_cents":cash-reversed_cash,"adjustments_cents":paid-cash-reversed+reversed_cash});
    if matches!(actor.role.as_str(), "executive" | "billing") {
        let draft_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notes WHERE organization_id=$1 AND status='draft'",
        )
        .bind(actor.org)
        .fetch_one(&mut *tx)
        .await?;
        data["metrics"]["draft_notes"] = json!(draft_count);
    }
    access::audit(&mut tx, &actor, "read", "workspace", None).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(data))
}
pub async fn list(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let resource = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, &resource, false).await?;
    let mut tx = state.pool.begin().await?;
    let result = rows(&mut tx, &resource, &actor).await?;
    access::audit(&mut tx, &actor, "read", &resource, None).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(result))
}
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let resource = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, &resource, true).await?;
    if resource == "payment_reversals" {
        return Err(ApiError::conflict(
            "Use the original payment's reversal action",
        ));
    }
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, actor.org, &resource).await?;
    let mut data = records::clean(&resource, &body)?;
    if resource == "sessions" && data.get("kind") == Some(&json!("group")) {
        return Err(ApiError::invalid(
            "Create a shared group and enroll attendance to generate group sessions",
        ));
    }
    if resource == "notes" {
        data.insert("author_id".into(), json!(actor.id));
    }
    if resource == "clients" && actor.role == "clinician" {
        data.insert("primary_clinician_id".into(), json!(actor.id));
    }
    if resource == "episodes" && data.get("status") == Some(&json!("discharged")) {
        return Err(ApiError::invalid(
            "Create an active or planned episode, then use discharge",
        ));
    }
    crate::clinical::prepare_create(&mut tx, &actor, &resource, &mut data).await?;
    if resource == "form_responses" {
        crate::forms::prepare_create(&mut tx, &actor, &mut data).await?;
    }
    records::scope(&mut tx, &actor, &resource, &Value::Object(data.clone())).await?;
    records::validate_links(&mut tx, &actor, &resource, &Value::Object(data.clone())).await?;
    if resource == "clients" {
        ensure_new_client_is_adult(&mut tx, &actor, &Value::Object(data.clone())).await?;
    }
    if resource == "episodes" {
        ensure_episode_client_is_adult(&mut tx, &actor, &Value::Object(data.clone())).await?;
    }
    if resource == "payments" {
        validate_payment(&mut tx, &actor, &Value::Object(data.clone())).await?;
    }
    if resource == "sessions" {
        crate::scheduling::check_session(&mut tx, &actor, &Value::Object(data.clone())).await?;
    } else if resource == "groups" {
        crate::scheduling::check_group(&mut tx, &actor, &Value::Object(data.clone())).await?;
    }
    let result = records::insert(&mut tx, &resource, data, &actor).await?;
    if resource == "episodes" && result["status"] == "active" {
        sqlx::query("UPDATE clients SET status='active',updated_at=now() WHERE id=$1 AND organization_id=$2").bind(records::uuid(&result,"client_id")?).bind(actor.org).execute(&mut *tx).await?;
    }
    if resource == "payments" {
        sqlx::query("UPDATE claims SET status='paid',updated_at=now() WHERE id=$1 AND organization_id=$2 AND amount_cents=(SELECT COALESCE(sum(amount_cents),0) FROM payments WHERE claim_id=$1 AND organization_id=$2)-(SELECT COALESCE(sum(amount_cents),0) FROM payment_reversals WHERE claim_id=$1 AND organization_id=$2)").bind(records::uuid(&result,"claim_id")?).bind(actor.org).execute(&mut *tx).await?;
    }
    access::audit(
        &mut tx,
        &actor,
        "create",
        &resource,
        Some(records::uuid(&result, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(HttpResponse::Created().json(result))
}
const STALE_RECORD: &str = "This record was updated after you opened it. Reload the latest version and preserve your unsaved edits; they were not saved.";

fn take_expected_updated_at(input: &mut Value) -> Result<DateTime<Utc>> {
    let object = input
        .as_object_mut()
        .ok_or_else(|| ApiError::invalid("Expected a JSON object"))?;
    let text = match object.remove("_expected_updated_at") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(_) => {
            return Err(ApiError::invalid(
                "_expected_updated_at must be an RFC3339 timestamp",
            ))
        }
    };
    let Some(text) = text else {
        return Err(ApiError::precondition(
            "Reload the record before saving. The originally loaded updated_at is required so a newer edit is not overwritten.",
        ));
    };
    DateTime::parse_from_rfc3339(&text)
        .map(|stamp| stamp.with_timezone(&Utc))
        .map_err(|_| ApiError::invalid("_expected_updated_at must be an RFC3339 timestamp"))
}

fn require_current_version(before: &Value, expected: DateTime<Utc>) -> Result<()> {
    let current = before["updated_at"].as_str().unwrap_or_default();
    let current = DateTime::parse_from_rfc3339(current)
        .map(|stamp| stamp.with_timezone(&Utc))
        .map_err(|_| ApiError::conflict(STALE_RECORD))?;
    if current != expected {
        return Err(ApiError::conflict(STALE_RECORD));
    }
    Ok(())
}

pub async fn update(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, Uuid)>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let (resource, id) = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, &resource, true).await?;
    let mut input = body.into_inner();
    let expected = take_expected_updated_at(&mut input)?;
    let mut data = records::clean(&resource, &input)?;
    if matches!(resource.as_str(), "payments" | "payment_reversals") {
        return Err(ApiError::conflict("Payments are append only"));
    }
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, actor.org, &resource).await?;
    if resource == "staff" {
        sqlx::query("SELECT pg_advisory_xact_lock(9182402)")
            .execute(&mut *tx)
            .await?;
    }
    if resource == "group_attendance" {
        let current = records::get(&mut tx, &resource, id, actor.org, false).await?;
        records::get(
            &mut tx,
            "groups",
            records::uuid(&current, "group_id")?,
            actor.org,
            true,
        )
        .await?;
    }
    let before = records::get(&mut tx, &resource, id, actor.org, true).await?;
    records::scope(&mut tx, &actor, &resource, &before).await?;
    require_current_version(&before, expected)?;
    crate::clinical::before_update(&mut tx, &actor, &resource, &before, &data).await?;
    if resource == "form_responses" {
        crate::forms::before_update(&mut tx, &actor, &before, &mut data).await?;
    }
    for key in [
        "client_id",
        "episode_id",
        "policy_id",
        "session_id",
        "authorization_id",
        "plan_id",
        "group_id",
    ] {
        if data.get(key).map(|v| v != &before[key]).unwrap_or(false) {
            return Err(ApiError::conflict(format!(
                "{key} cannot be reassigned after creation"
            )));
        }
    }
    if resource == "sessions" {
        let locked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notes WHERE organization_id=$1 AND session_id=$2 AND status='signed') OR EXISTS(SELECT 1 FROM claims WHERE organization_id=$1 AND session_id=$2 AND status<>'draft')").bind(actor.org).bind(id).fetch_one(&mut *tx).await?;
        if locked {
            return Err(ApiError::conflict(
                "A session with signed documentation or an advanced claim cannot be rewritten",
            ));
        }
    }
    if matches!(resource.as_str(), "policies" | "authorizations") {
        let column = if resource == "policies" {
            "policy_id"
        } else {
            "authorization_id"
        };
        let query=format!("SELECT EXISTS(SELECT 1 FROM claims WHERE organization_id=$1 AND {column}=$2 AND status<>'draft')");
        let locked: bool = sqlx::query_scalar(&query)
            .bind(actor.org)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if locked {
            return Err(ApiError::conflict("Coverage used by a ready or submitted claim is preserved; create a new coverage record"));
        }
    }
    if resource == "notes" {
        if before["status"] == "signed" {
            return Err(ApiError::conflict(
                "Signed notes are immutable; create an amendment",
            ));
        }
        if before["author_id"] != actor.id.to_string()
            && !matches!(actor.role.as_str(), "admin" | "clinical_lead")
        {
            return Err(ApiError::denied());
        }
    }
    if resource == "claims" && before["status"] != "draft" {
        return Err(ApiError::conflict("Only draft claims can be edited"));
    }
    if resource == "staff" {
        if data.get("active") == Some(&json!(false)) {
            return Err(ApiError::conflict(
                "Use offboarding to deactivate staff and transfer assignments",
            ));
        }
        if let Some(role) = data.get("role").and_then(Value::as_str) {
            if before["role"] != role && !matches!(role, "admin" | "clinical_lead" | "clinician") {
                // The staff row is already locked. Assignment creation takes this
                // same lock before validating role, preventing a concurrent bypass.
                let assigned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM clients WHERE organization_id=$1 AND primary_clinician_id=$2) OR EXISTS(SELECT 1 FROM sessions WHERE organization_id=$1 AND clinician_id=$2 AND status='scheduled' AND starts_at>now()) OR EXISTS(SELECT 1 FROM groups WHERE organization_id=$1 AND clinician_id=$2 AND status='scheduled' AND starts_at>now())")
                    .bind(actor.org).bind(id).fetch_one(&mut *tx).await?;
                if assigned {
                    return Err(ApiError::conflict("Reassign this person's clients and future clinical sessions before choosing a nonclinical role"));
                }
            }
        }
        if before["role"] == "admin" && data.get("role").map(|r| r != "admin").unwrap_or(false) {
            let admins: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM staff WHERE organization_id=$1 AND role='admin' AND active",
            )
            .bind(actor.org)
            .fetch_one(&mut *tx)
            .await?;
            if admins <= 1 {
                return Err(ApiError::conflict(
                    "At least one active administrator is required",
                ));
            }
        }
    }
    if resource == "episodes" && data.get("status") == Some(&json!("discharged")) {
        return Err(ApiError::conflict("Use the discharge workflow"));
    }
    let mut merged = before.clone();
    for (k, v) in &data {
        merged[k] = v.clone();
    }
    records::scope(&mut tx, &actor, &resource, &merged).await?;
    records::validate_links(&mut tx, &actor, &resource, &merged).await?;
    if schedule_changed(&before, &merged) {
        if resource == "sessions" {
            crate::scheduling::check_session(&mut tx, &actor, &merged).await?;
        } else if resource == "groups" {
            crate::scheduling::check_group(&mut tx, &actor, &merged).await?;
        }
    }
    if resource == "clients" && data.contains_key("birth_date") {
        ensure_birth_date_change_is_adult(&mut tx, &actor, &before, &merged).await?;
    }
    if resource == "clients"
        && before["status"] != merged["status"]
        && matches!(merged["status"].as_str(), Some("intake" | "active"))
    {
        ensure_new_client_is_adult(&mut tx, &actor, &merged).await?;
    }
    if resource == "episodes" && before["status"] != "active" && merged["status"] == "active" {
        ensure_episode_client_is_adult(&mut tx, &actor, &merged).await?;
    }
    let result = records::patch(&mut tx, &resource, id, actor.org, data).await?;
    crate::clinical::after_update(&mut tx, &actor, &resource, &result).await?;
    access::audit(&mut tx, &actor, "update", &resource, Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(result))
}
fn schedule_changed(before: &Value, after: &Value) -> bool {
    ["starts_at", "ends_at", "clinician_id"]
        .iter()
        .any(|key| before[*key] != after[*key])
        || (!matches!(before["status"].as_str(), Some("scheduled" | "completed"))
            && matches!(after["status"].as_str(), Some("scheduled" | "completed")))
}
async fn facility_local_today(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    facility_id: Uuid,
) -> Result<NaiveDate> {
    let facility = records::get(tx, "facilities", facility_id, actor.org, false).await?;
    let zone = facility["timezone"].as_str().unwrap_or("UTC");
    let today: NaiveDate = sqlx::query_scalar(
        "SELECT (CURRENT_TIMESTAMP AT TIME ZONE $1)::date",
    )
    .bind(zone)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| {
        ApiError::invalid(
            "facility_id timezone could not supply a local calendar date. Choose a recognized facility timezone, such as America/New_York.",
        )
    })?;
    Ok(today)
}

async fn ensure_new_client_is_adult(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &Value,
) -> Result<()> {
    let birth = records::parse_record_date(data, "birth_date")?;
    let facility_id = records::uuid(data, "facility_id").map_err(|_| {
        ApiError::invalid(
            "facility_id is required to check adult program eligibility against the facility calendar",
        )
    })?;
    let today = facility_local_today(tx, actor, facility_id).await?;
    records::require_adult_program(birth, today, "birth_date")
}

async fn ensure_birth_date_change_is_adult(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    before: &Value,
    merged: &Value,
) -> Result<()> {
    if before.get("birth_date") == merged.get("birth_date") {
        return Ok(());
    }
    let birth = records::parse_record_date(merged, "birth_date")?;
    let facility_id =
        records::uuid(merged, "facility_id").or_else(|_| records::uuid(before, "facility_id"))?;
    let today = facility_local_today(tx, actor, facility_id).await?;
    records::require_adult_program(birth, today, "birth_date")
}

async fn ensure_episode_client_is_adult(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &Value,
) -> Result<()> {
    let client = records::get(
        tx,
        "clients",
        records::uuid(data, "client_id")?,
        actor.org,
        true,
    )
    .await?;
    let birth = records::parse_record_date(&client, "birth_date").map_err(|_| {
        ApiError::invalid(
            "client_id birth_date must be a valid YYYY-MM-DD date before an episode can be created or activated",
        )
    })?;
    let facility_id =
        records::uuid(data, "facility_id").or_else(|_| records::uuid(&client, "facility_id"))?;
    let today = facility_local_today(tx, actor, facility_id).await?;
    if records::require_adult_program(birth, today, "client_id").is_err() {
        return Err(ApiError::invalid(
            "client_id: a new or newly active episode requires a client aged 18 or older as of the facility's local date. Check the recorded date of birth; existing records remain available for correction.",
        ));
    }
    Ok(())
}

async fn validate_payment(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &Value,
) -> Result<()> {
    let claim = records::get(
        tx,
        "claims",
        records::uuid(data, "claim_id")?,
        actor.org,
        true,
    )
    .await?;
    if !matches!(claim["status"].as_str(), Some("submitted" | "denied")) {
        return Err(ApiError::conflict(
            "Record the external submission before posting payment",
        ));
    }
    let paid:i64=sqlx::query_scalar("SELECT ((SELECT COALESCE(sum(amount_cents),0) FROM payments WHERE claim_id=$1 AND organization_id=$2)-(SELECT COALESCE(sum(amount_cents),0) FROM payment_reversals WHERE claim_id=$1 AND organization_id=$2))::bigint").bind(records::uuid(data,"claim_id")?).bind(actor.org).fetch_one(&mut **tx).await?;
    if paid + data["amount_cents"].as_i64().unwrap_or_default()
        > claim["amount_cents"].as_i64().unwrap_or_default()
    {
        return Err(ApiError::conflict(
            "Payment exceeds the outstanding claim balance",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod concurrency_tests {
    use super::{require_current_version, take_expected_updated_at};
    use actix_web::http::StatusCode;
    use chrono::{DateTime, Utc};
    use serde_json::json;

    #[test]
    fn strips_expected_updated_at_before_field_cleaning() {
        let mut body = json!({"name":"QA facility","_expected_updated_at":"2026-02-01T14:00:00Z"});
        let expected = take_expected_updated_at(&mut body).unwrap();
        assert_eq!(
            expected,
            DateTime::parse_from_rfc3339("2026-02-01T14:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
        assert!(body.get("_expected_updated_at").is_none());
        assert!(crate::records::clean("facilities", &body).is_ok());
        assert!(crate::records::clean(
            "facilities",
            &json!({"_expected_updated_at":"2026-02-01T14:00:00Z"})
        )
        .is_err());
    }

    #[test]
    fn missing_token_is_precondition_required() {
        let mut body = json!({"title":"draft"});
        let err = take_expected_updated_at(&mut body).unwrap_err();
        assert_eq!(err.status, StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(err.code, "precondition_required");
        let mut body = json!({"title":"draft","_expected_updated_at":null});
        let err = take_expected_updated_at(&mut body).unwrap_err();
        assert_eq!(err.status, StatusCode::PRECONDITION_REQUIRED);
        let mut body = json!({"_expected_updated_at":"yesterday"});
        let err = take_expected_updated_at(&mut body).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn stale_loaded_timestamp_conflicts_without_treating_format_as_different() {
        let before = json!({"updated_at":"2026-02-01T14:00:00.123456+00:00"});
        let same = DateTime::parse_from_rfc3339("2026-02-01T14:00:00.123456Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(require_current_version(&before, same).is_ok());
        let stale = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let err = require_current_version(&before, stale).unwrap_err();
        assert_eq!(err.status, StatusCode::CONFLICT);
        assert_eq!(err.code, "conflict");
        let message = err.message.to_lowercase();
        assert!(message.contains("reload"), "{message}");
        assert!(
            message.contains("preserve") && message.contains("unsaved"),
            "{message}"
        );
    }
}
