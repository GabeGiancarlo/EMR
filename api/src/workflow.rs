use crate::{access, records, ApiError, AppState, Result};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{NaiveDate, Utc};
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub async fn action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, Uuid, String)>,
    body: web::Json<Value>,
) -> Result<HttpResponse> {
    let (resource, id, action) = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, &resource, true).await?;
    let mut tx = state.pool.begin().await?;
    if resource == "staff" {
        sqlx::query("SELECT pg_advisory_xact_lock(9182402)")
            .execute(&mut *tx)
            .await?;
    }
    let old = records::get(&mut tx, &resource, id, actor.org, true).await?;
    records::scope(&mut tx, &actor, &resource, &old).await?;
    let result = match (resource.as_str(), action.as_str()) {
        ("notes", "sign") => {
            if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
                return Err(ApiError::denied());
            }
            if !old["session_id"].is_null() {
                let session = records::get(
                    &mut tx,
                    "sessions",
                    records::uuid(&old, "session_id")?,
                    actor.org,
                    true,
                )
                .await?;
                if session["status"] != "completed" {
                    return Err(ApiError::conflict(
                        "Complete attendance or the session before signing its note",
                    ));
                }
                records::ensure_session_ended(&session)?;
            }
            if old["status"] != "draft" {
                return Err(ApiError::conflict("Only draft notes can be signed"));
            }
            if old["author_id"] != actor.id.to_string()
                && !matches!(actor.role.as_str(), "admin" | "clinical_lead")
            {
                return Err(ApiError::denied());
            }
            records::text(&old, "body")?;
            records::patch(
                &mut tx,
                "notes",
                id,
                actor.org,
                json!({"status":"signed","signed_at":Utc::now(),"signed_by":actor.id})
                    .as_object()
                    .unwrap()
                    .clone(),
            )
            .await?
        }
        ("notes", "amend") => {
            if old["status"] != "signed" {
                return Err(ApiError::conflict("Only signed notes can be amended"));
            }
            if old["author_id"] != actor.id.to_string()
                && !matches!(actor.role.as_str(), "admin" | "clinical_lead")
            {
                return Err(ApiError::denied());
            }
            let reason = records::text(&body, "reason")?;
            let note = records::text(&body, "body")?;
            if reason.len() < 10 {
                return Err(ApiError::invalid(
                    "Explain the amendment reason in at least 10 characters",
                ));
            }
            let data = json!({"client_id":old["client_id"],"session_id":old["session_id"],"template_id":old["template_id"],"author_id":actor.id,"title":format!("Amendment: {}",old["title"].as_str().unwrap_or_default()),"body":note,"amends_note_id":id,"amendment_reason":reason});
            records::insert(&mut tx, "notes", data.as_object().unwrap().clone(), &actor).await?
        }
        ("payments", "reverse") => {
            let claim_id = records::uuid(&old, "claim_id")?;
            let claim = records::get(&mut tx, "claims", claim_id, actor.org, true).await?;
            let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM payment_reversals WHERE organization_id=$1 AND payment_id=$2)").bind(actor.org).bind(id).fetch_one(&mut *tx).await?;
            if duplicate {
                return Err(ApiError::conflict("This payment has already been reversed"));
            }
            let reason = records::text(&body, "reason")?;
            if reason.len() < 10 {
                return Err(ApiError::invalid(
                    "Explain the reversal in at least 10 characters",
                ));
            }
            let date = NaiveDate::parse_from_str(records::text(&body, "reversed_on")?, "%Y-%m-%d")
                .map_err(|_| ApiError::invalid("Enter a valid reversal date"))?;
            let received = NaiveDate::parse_from_str(
                old["received_on"].as_str().unwrap_or_default(),
                "%Y-%m-%d",
            )
            .map_err(|_| ApiError::invalid("Invalid original receipt date"))?;
            if date < received || date > Utc::now().date_naive() {
                return Err(ApiError::invalid(
                    "Reversal date must be between the receipt date and today",
                ));
            }
            let result=records::insert(&mut tx,"payment_reversals",json!({"payment_id":id,"claim_id":claim_id,"amount_cents":old["amount_cents"],"reversed_on":date,"reason":reason,"actor_id":actor.id}).as_object().unwrap().clone(),&actor).await?;
            if claim["status"] == "paid" {
                let status = if claim["denial_reason"]
                    .as_str()
                    .unwrap_or_default()
                    .is_empty()
                {
                    "submitted"
                } else {
                    "denied"
                };
                records::patch(
                    &mut tx,
                    "claims",
                    claim_id,
                    actor.org,
                    json!({"status":status}).as_object().unwrap().clone(),
                )
                .await?;
            }
            result
        }
        ("claims", "return-to-draft") => {
            if old["status"] != "ready" {
                return Err(ApiError::conflict(
                    "Only ready unsubmitted claims can return to draft",
                ));
            }
            records::patch(
                &mut tx,
                "claims",
                id,
                actor.org,
                json!({"status":"draft"}).as_object().unwrap().clone(),
            )
            .await?
        }
        ("claims", "void") => {
            if !matches!(old["status"].as_str(), Some("draft" | "ready")) {
                return Err(ApiError::conflict(
                    "Only draft or ready unsubmitted claims can be voided locally",
                ));
            }
            let reason = records::text(&body, "reason")?;
            if reason.len() < 10 {
                return Err(ApiError::invalid(
                    "Explain the void in at least 10 characters",
                ));
            }
            records::patch(
                &mut tx,
                "claims",
                id,
                actor.org,
                json!({"status":"voided","void_reason":reason})
                    .as_object()
                    .unwrap()
                    .clone(),
            )
            .await?
        }
        ("claims", "ready") => {
            if old["status"] != "draft" {
                return Err(ApiError::conflict("Only draft claims can become ready"));
            }
            validate_claim(&mut tx, &actor, &old).await?;
            records::patch(
                &mut tx,
                "claims",
                id,
                actor.org,
                json!({"status":"ready"}).as_object().unwrap().clone(),
            )
            .await?
        }
        ("claims", "record-submission") => {
            if old["status"] != "ready" {
                return Err(ApiError::conflict(
                    "Only a ready claim can record an external submission",
                ));
            }
            validate_claim(&mut tx, &actor, &old).await?;
            let reference = records::text(&body, "external_reference")?;
            records::patch(
                &mut tx,
                "claims",
                id,
                actor.org,
                json!({"status":"submitted","external_reference":reference})
                    .as_object()
                    .unwrap()
                    .clone(),
            )
            .await?
        }
        ("claims", "deny") => {
            if old["status"] != "submitted" {
                return Err(ApiError::conflict("Only submitted claims can be denied"));
            }
            let reason = records::text(&body, "reason")?;
            records::patch(
                &mut tx,
                "claims",
                id,
                actor.org,
                json!({"status":"denied","denial_reason":reason})
                    .as_object()
                    .unwrap()
                    .clone(),
            )
            .await?
        }
        ("episodes", "discharge") => {
            if old["status"] == "discharged" {
                return Err(ApiError::conflict("Episode is already discharged"));
            }
            let reason = records::text(&body, "discharge_reason")?;
            let date = records::text(&body, "end_date")?;
            let end = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| ApiError::invalid("Enter a valid discharge date"))?;
            if end > Utc::now().date_naive() {
                return Err(ApiError::invalid(
                    "Discharge cannot be recorded in the future",
                ));
            }
            let updated = records::patch(
                &mut tx,
                "episodes",
                id,
                actor.org,
                json!({"status":"discharged","end_date":date,"discharge_reason":reason})
                    .as_object()
                    .unwrap()
                    .clone(),
            )
            .await?;
            sqlx::query("UPDATE clients SET status='discharged',updated_at=now() WHERE id=$1 AND organization_id=$2 AND NOT EXISTS(SELECT 1 FROM episodes WHERE client_id=$1 AND organization_id=$2 AND status='active')").bind(records::uuid(&old,"client_id")?).bind(actor.org).execute(&mut *tx).await?;
            sqlx::query("UPDATE sessions s SET status='cancelled',updated_at=now() FROM episodes e JOIN facilities f ON f.id=e.facility_id AND f.organization_id=e.organization_id WHERE s.episode_id=e.id AND s.organization_id=e.organization_id AND s.episode_id=$1 AND s.organization_id=$2 AND s.status='scheduled' AND (s.starts_at AT TIME ZONE f.timezone)::date>$3").bind(id).bind(actor.org).bind(end).execute(&mut *tx).await?;
            sqlx::query("UPDATE group_attendance a SET status='excused',updated_at=now() FROM sessions s WHERE a.session_id=s.id AND a.organization_id=s.organization_id AND a.organization_id=$1 AND s.episode_id=$2 AND a.status='scheduled' AND s.status='cancelled'").bind(actor.org).bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE treatment_plans SET status='completed',completion_reason=$1,updated_at=now() WHERE organization_id=$2 AND episode_id=$3 AND status='active'").bind(reason).bind(actor.org).bind(id).execute(&mut *tx).await?;
            updated
        }
        ("staff", "offboard") => {
            if id == actor.id {
                return Err(ApiError::conflict(
                    "Select a different administrator to offboard this account",
                ));
            }
            if old["active"] != true {
                return Err(ApiError::conflict("Staff member is already inactive"));
            }
            let replacement_id = records::uuid(&body, "transfer_to")?;
            if id == replacement_id {
                return Err(ApiError::invalid(
                    "Transfer to a different active staff member",
                ));
            }
            let replacement =
                records::get(&mut tx, "staff", replacement_id, actor.org, true).await?;
            if replacement["active"] != true {
                return Err(ApiError::invalid("Replacement must be active"));
            }
            if matches!(old["role"].as_str(), Some("clinician" | "clinical_lead"))
                && !matches!(
                    replacement["role"].as_str(),
                    Some("admin" | "clinical_lead" | "clinician")
                )
            {
                return Err(ApiError::invalid(
                    "Clinical assignments must transfer to clinical staff",
                ));
            }
            if old["role"] == "admin" {
                let count:i64=sqlx::query_scalar("SELECT count(*) FROM staff WHERE organization_id=$1 AND role='admin' AND active").bind(actor.org).fetch_one(&mut *tx).await?;
                if count <= 1 {
                    return Err(ApiError::conflict(
                        "At least one active administrator must remain",
                    ));
                }
            }
            sqlx::query("UPDATE tasks SET assignee_id=$1,updated_at=now() WHERE organization_id=$2 AND assignee_id=$3 AND status IN ('open','in_progress')").bind(replacement_id).bind(actor.org).bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE groups SET clinician_id=$1,updated_at=now() WHERE organization_id=$2 AND clinician_id=$3 AND status='scheduled' AND starts_at>now()").bind(replacement_id).bind(actor.org).bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE sessions SET clinician_id=$1,updated_at=now() WHERE organization_id=$2 AND clinician_id=$3 AND status='scheduled' AND starts_at>now()").bind(replacement_id).bind(actor.org).bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE clients SET primary_clinician_id=$1,updated_at=now() WHERE organization_id=$2 AND primary_clinician_id=$3").bind(replacement_id).bind(actor.org).bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE exceptions SET revoked=true,updated_at=now() WHERE organization_id=$1 AND staff_id=$2 AND NOT revoked").bind(actor.org).bind(id).execute(&mut *tx).await?;
            records::patch(
                &mut tx,
                "staff",
                id,
                actor.org,
                json!({"active":false}).as_object().unwrap().clone(),
            )
            .await?
        }
        ("treatment_plans" | "groups", _) => {
            crate::clinical::perform(&mut tx, &actor, &resource, &action, &old, &body).await?
        }
        ("form_responses", _) => {
            crate::forms::perform(&mut tx, &actor, &action, &old, &body).await?
        }
        _ => return Err(ApiError::missing()),
    };
    access::audit(&mut tx, &actor, &action, &resource, Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(result))
}

async fn validate_claim(
    tx: &mut Transaction<'_, Postgres>,
    actor: &access::Actor,
    claim: &Value,
) -> Result<()> {
    let session = records::get(
        tx,
        "sessions",
        records::uuid(claim, "session_id")?,
        actor.org,
        true,
    )
    .await?;
    if session["status"] != "completed" {
        return Err(ApiError::conflict(
            "Complete the session before marking the claim ready",
        ));
    }
    records::ensure_session_ended(&session)?;
    let signed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notes WHERE organization_id=$1 AND session_id=$2 AND status='signed')").bind(actor.org).bind(records::uuid(claim,"session_id")?).fetch_one(&mut **tx).await?;
    if !signed {
        return Err(ApiError::conflict(
            "A signed session note is required for billing readiness",
        ));
    }
    let policy = records::get(
        tx,
        "policies",
        records::uuid(claim, "policy_id")?,
        actor.org,
        true,
    )
    .await?;
    let service_date: NaiveDate = sqlx::query_scalar("SELECT (s.starts_at AT TIME ZONE f.timezone)::date FROM sessions s JOIN episodes e ON e.id=s.episode_id AND e.organization_id=s.organization_id JOIN facilities f ON f.id=e.facility_id AND f.organization_id=e.organization_id WHERE s.id=$1 AND s.organization_id=$2")
        .bind(records::uuid(claim, "session_id")?).bind(actor.org).fetch_one(&mut **tx).await?;
    let date_text = service_date.to_string();
    let date = date_text.as_str();
    if policy["status"] != "active"
        || date < policy["effective_date"].as_str().unwrap_or_default()
        || policy["termination_date"]
            .as_str()
            .map(|d| date > d)
            .unwrap_or(false)
    {
        return Err(ApiError::conflict("The policy must cover the session date"));
    }
    if !claim["authorization_id"].is_null() {
        let authorization = records::get(
            tx,
            "authorizations",
            records::uuid(claim, "authorization_id")?,
            actor.org,
            true,
        )
        .await?;
        if authorization["policy_id"] != claim["policy_id"]
            || authorization["service_code"] != claim["service_code"]
            || authorization["status"] != "active"
            || date < authorization["starts_on"].as_str().unwrap_or_default()
            || date > authorization["ends_on"].as_str().unwrap_or_default()
        {
            return Err(ApiError::conflict(
                "Authorization must cover this policy, service and service date",
            ));
        }
        let used:i64=sqlx::query_scalar("SELECT COALESCE(sum(units),0)::bigint FROM claims WHERE organization_id=$1 AND authorization_id=$2 AND id<>$3 AND status IN ('ready','submitted','denied','paid')").bind(actor.org).bind(records::uuid(claim,"authorization_id")?).bind(records::uuid(claim,"id")?).fetch_one(&mut **tx).await?;
        if used + claim["units"].as_i64().unwrap_or(1)
            > authorization["units_approved"].as_i64().unwrap_or_default()
        {
            return Err(ApiError::conflict(
                "This claim exceeds the remaining authorized units",
            ));
        }
    }
    Ok(())
}
