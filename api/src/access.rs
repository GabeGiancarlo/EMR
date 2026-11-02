use crate::{ApiError, Result};
use actix_web::HttpRequest;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Actor {
    pub id: Uuid,
    pub org: Uuid,
    pub role: String,
    pub value: Value,
}
pub async fn actor(req: &HttpRequest, pool: &PgPool) -> Result<Actor> {
    let id = req
        .headers()
        .get("x-nexus-actor")
        .and_then(|x| x.to_str().ok())
        .and_then(|x| Uuid::parse_str(x).ok())
        .ok_or_else(ApiError::denied)?;
    let value: Value = sqlx::query_scalar("SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND active")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(ApiError::denied)?;
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
pub async fn allowed(pool: &PgPool, actor: &Actor, resource: &str, write: bool) -> Result<bool> {
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
    Ok(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM exceptions WHERE organization_id=$1 AND staff_id=$2 AND resource=$3 AND NOT revoked AND expires_at>now() AND (NOT $4 OR access_mode='write'))").bind(actor.org).bind(actor.id).bind(grant_resource).bind(write).fetch_one(pool).await?)
}
pub async fn require(pool: &PgPool, actor: &Actor, resource: &str, write: bool) -> Result<()> {
    if allowed(pool, actor, resource, write).await? {
        Ok(())
    } else {
        Err(ApiError::denied())
    }
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
