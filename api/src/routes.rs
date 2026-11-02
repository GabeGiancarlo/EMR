use crate::{
    access::{self, Actor},
    records::{self, RESOURCES},
    ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
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
    let mut tx = state.pool.begin().await?;
    crate::clinical::prepare_create(&mut tx, &actor, &resource, &mut data).await?;
    if resource == "form_responses" {
        crate::forms::prepare_create(&mut tx, &actor, &mut data).await?;
    }
    records::scope(&mut tx, &actor, &resource, &Value::Object(data.clone())).await?;
    records::validate_links(&mut tx, &actor, &resource, &Value::Object(data.clone())).await?;
    if resource == "payments" {
        validate_payment(&mut tx, &actor, &Value::Object(data.clone())).await?;
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
pub async fn update(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, Uuid)>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let (resource, id) = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, &resource, true).await?;
    let mut data = records::clean(&resource, &body)?;
    if matches!(resource.as_str(), "payments" | "payment_reversals") {
        return Err(ApiError::conflict("Payments are append only"));
    }
    let mut tx = state.pool.begin().await?;
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
    crate::clinical::before_update(&mut tx, &actor, &resource, &before, &data).await?;
    if resource == "form_responses" {
        crate::forms::before_update(&actor, &before, &mut data)?;
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
    let result = records::patch(&mut tx, &resource, id, actor.org, data).await?;
    crate::clinical::after_update(&mut tx, &actor, &resource, &result).await?;
    access::audit(&mut tx, &actor, "update", &resource, Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(result))
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
