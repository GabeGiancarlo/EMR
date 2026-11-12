use crate::{
    auth::{Auth, Identity},
    ApiError, AppState, Result,
};
use actix_web::{web, HttpMessage, HttpRequest};
use serde_json::Value;
use sqlx::{PgExecutor, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Actor {
    pub id: Uuid,
    pub org: Uuid,
    pub role: String,
    pub value: Value,
}
/// The staff member this request acts as. Local mode trusts the development header.
/// Hosted mode uses only the identity the OIDC gate verified and never reads the
/// header, so a caller cannot name someone else.
pub(crate) fn staff_id(req: &HttpRequest) -> Result<Uuid> {
    let state = req
        .app_data::<web::Data<AppState>>()
        .ok_or_else(ApiError::unauthenticated)?;
    match &state.auth {
        Auth::Local => req
            .headers()
            .get("x-nexus-actor")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(ApiError::denied),
        Auth::Oidc(_) => req
            .extensions()
            .get::<Identity>()
            .map(|identity| identity.id)
            .ok_or_else(ApiError::unauthenticated),
    }
}
/// `db` is the pool, or the caller's open transaction. A handler that already holds a
/// transaction must pass that transaction: asking the pool for a second connection
/// while holding the first is how a busy pool deadlocks.
pub async fn actor<'e>(req: &HttpRequest, db: impl PgExecutor<'e>) -> Result<Actor> {
    let id = staff_id(req)?;
    // Hosted mode: the gate read this active staff row moments ago, in this same
    // request. Mutations still re-read it inside their transaction (`lock_mutation`).
    let verified = req
        .extensions()
        .get::<Identity>()
        .map(|identity| identity.staff.clone());
    let value: Value = match verified {
        Some(staff) => staff,
        None => sqlx::query_scalar("SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND active")
            .bind(id)
            .fetch_optional(db)
            .await?
            .ok_or_else(ApiError::denied)?,
    };
    Ok(Actor {
        id,
        org: Uuid::parse_str(value["organization_id"].as_str().unwrap_or_default())
            .map_err(|_| ApiError::denied())?,
        role: value["role"].as_str().unwrap_or_default().into(),
        value,
    })
}
pub fn role_allows(role: &str, resource: &str, write: bool) -> bool {
    if resource == "payment_reversals" && write {
        return false;
    }
    if role == "admin" {
        return true;
    }
    match resource {
        "organizations" | "facilities" | "staff" => !write,
        "audit" | "exceptions" => !write && matches!(role, "clinical_lead" | "executive"),
        "notes" | "treatment_plans" | "goals" | "plan_reviews" | "form_responses" => {
            matches!(role, "clinical_lead" | "clinician")
        }
        "groups" | "group_attendance" => {
            if write {
                matches!(role, "clinical_lead" | "clinician" | "support")
            } else {
                true
            }
        }
        "templates" => {
            if write {
                role == "clinical_lead"
            } else {
                matches!(role, "clinical_lead" | "clinician" | "support")
            }
        }
        "policies" | "authorizations" | "claims" | "payments" | "payment_reversals" => {
            if write {
                role == "billing"
            } else {
                matches!(role, "billing" | "executive" | "clinical_lead")
            }
        }
        "clients" | "episodes" | "sessions" | "consents" => {
            if write {
                matches!(role, "clinical_lead" | "clinician" | "support")
            } else {
                true
            }
        }
        "tasks" => true,
        _ => false,
    }
}
/// See [`actor`] for which executor to pass.
pub async fn allowed<'e>(
    db: impl PgExecutor<'e>,
    actor: &Actor,
    resource: &str,
    write: bool,
) -> Result<bool> {
    if role_allows(&actor.role, resource, write) {
        return Ok(true);
    }
    if matches!(
        resource,
        "staff" | "exceptions" | "facilities" | "organizations" | "audit"
    ) {
        return Ok(false);
    }
    if resource == "payment_reversals" && write {
        return Ok(false);
    }
    let grant_resource = if resource == "payment_reversals" {
        "payments"
    } else {
        resource
    };
    Ok(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM exceptions WHERE organization_id=$1 AND staff_id=$2 AND resource=$3 AND NOT revoked AND expires_at>now() AND (NOT $4 OR access_mode='write'))").bind(actor.org).bind(actor.id).bind(grant_resource).bind(write).fetch_one(db).await?)
}
pub async fn require<'e>(
    db: impl PgExecutor<'e>,
    actor: &Actor,
    resource: &str,
    write: bool,
) -> Result<()> {
    if allowed(db, actor, resource, write).await? {
        Ok(())
    } else {
        Err(ApiError::denied())
    }
}
/// Serialize an organization's mutations before taking record locks, then recheck
/// the operator and permission so a request waiting behind offboarding cannot act
/// with the role it held before waiting.
pub async fn lock_mutation(
    req: &HttpRequest,
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    resource: &str,
) -> Result<Actor> {
    crate::scheduling::lock_org(tx, org).await?;
    let id = staff_id(req)?;
    let value: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND organization_id=$2 AND active",
    )
    .bind(id)
    .bind(org)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::denied)?;
    let current = Actor {
        id,
        org,
        role: value["role"].as_str().unwrap_or_default().into(),
        value,
    };
    if !role_allows(&current.role, resource, true) {
        if matches!(
            resource,
            "staff" | "exceptions" | "facilities" | "organizations" | "audit" | "payment_reversals"
        ) {
            return Err(ApiError::denied());
        }
        let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM exceptions WHERE organization_id=$1 AND staff_id=$2 AND resource=$3 AND NOT revoked AND expires_at>clock_timestamp() AND access_mode='write')")
            .bind(org).bind(id).bind(resource).fetch_one(&mut **tx).await?;
        if !allowed {
            return Err(ApiError::denied());
        }
    }
    Ok(current)
}
pub async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    action: &str,
    resource: &str,
    id: Option<Uuid>,
) -> Result<()> {
    sqlx::query("INSERT INTO audit_events(id,organization_id,actor_id,action,resource,record_id) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::new_v4()).bind(actor.org).bind(actor.id).bind(action).bind(resource).bind(id).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clinical_narrative_is_not_available_to_finance_or_leadership() {
        assert!(!role_allows("billing", "notes", false));
        assert!(!role_allows("executive", "notes", false));
        assert!(role_allows("clinician", "notes", true));
        assert!(!role_allows("support", "notes", true));
    }
    #[test]
    fn administrative_mutations_need_admin() {
        for role in [
            "clinical_lead",
            "clinician",
            "billing",
            "executive",
            "support",
        ] {
            assert!(!role_allows(role, "staff", true));
            assert!(!role_allows(role, "exceptions", true));
        }
    }
}
