//! Transactional reservations for individual care, shared groups and staff time away.
//! Callers acquire the organization lock before any row lock or schedule mutation.
use crate::{access::Actor, ApiError, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub async fn lock_org(tx: &mut Transaction<'_, Postgres>, org: Uuid) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,9182410))")
        .bind(org.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub fn interval(value: &Value) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let parse = |key: &str| {
        DateTime::parse_from_rfc3339(value[key].as_str().unwrap_or_default())
            .map(|time| time.with_timezone(&Utc))
            .map_err(|_| ApiError::invalid(format!("{key} needs a timestamp with timezone")))
    };
    let (start, end) = (parse("starts_at")?, parse("ends_at")?);
    if end <= start {
        return Err(ApiError::invalid("End time must be after start time"));
    }
    Ok((start, end))
}
fn optional_id(value: &Value, key: &str) -> Result<Option<Uuid>> {
    if value.get(key).is_none_or(Value::is_null) {
        return Ok(None);
    }
    Uuid::parse_str(value[key].as_str().unwrap_or_default())
        .map(Some)
        .map_err(|_| ApiError::invalid(format!("{key} must identify a valid record")))
}
fn id(value: &Value, key: &str) -> Result<Uuid> {
    optional_id(value, key)?.ok_or_else(|| ApiError::invalid(format!("{key} is required")))
}
fn reserves(status: Option<&str>) -> bool {
    matches!(status, None | Some("scheduled" | "completed"))
}

async fn check_clinician(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    clinician: Uuid,
    interval: (DateTime<Utc>, DateTime<Utc>),
    session: Option<Uuid>,
    group: Option<Uuid>,
) -> Result<()> {
    let (start, end) = interval;
    let busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions s WHERE s.organization_id=$1 AND s.clinician_id=$2 AND s.group_id IS NULL AND s.status IN ('scheduled','completed') AND s.starts_at<$4 AND s.ends_at>$3 AND ($5::uuid IS NULL OR s.id<>$5)) OR EXISTS(SELECT 1 FROM groups g WHERE g.organization_id=$1 AND g.clinician_id=$2 AND g.status IN ('scheduled','completed') AND g.starts_at<$4 AND g.ends_at>$3 AND ($6::uuid IS NULL OR g.id<>$6))")
        .bind(actor.org).bind(clinician).bind(start).bind(end).bind(session).bind(group).fetch_one(&mut **tx).await?;
    if busy {
        return Err(ApiError::conflict("The clinician already has care scheduled during this time. Choose another time or clinician."));
    }
    let unavailable: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff_unavailability WHERE organization_id=$1 AND staff_id=$2 AND active AND starts_at<$4 AND ends_at>$3)")
        .bind(actor.org).bind(clinician).bind(start).bind(end).fetch_one(&mut **tx).await?;
    if unavailable {
        return Err(ApiError::conflict("The clinician is unavailable during this time. Review staff availability or choose another time."));
    }
    Ok(())
}
async fn check_client(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: Uuid,
    interval: (DateTime<Utc>, DateTime<Utc>),
    session: Option<Uuid>,
    group: Option<Uuid>,
) -> Result<()> {
    let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions s WHERE s.organization_id=$1 AND s.client_id=$2 AND s.status IN ('scheduled','completed') AND s.starts_at<$4 AND s.ends_at>$3 AND ($5::uuid IS NULL OR s.id<>$5) AND ($6::uuid IS NULL OR s.group_id IS DISTINCT FROM $6))")
        .bind(actor.org).bind(client).bind(interval.0).bind(interval.1).bind(session).bind(group).fetch_one(&mut **tx).await?;
    if busy {
        return Err(ApiError::conflict(
            "The client already has care scheduled during this time. Choose another time.",
        ));
    }
    Ok(())
}
pub async fn check_session(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    candidate: &Value,
) -> Result<()> {
    let times = interval(candidate)?;
    if !reserves(candidate["status"].as_str()) {
        return Ok(());
    }
    let session = optional_id(candidate, "id")?;
    let group = optional_id(candidate, "group_id")?;
    check_clinician(
        tx,
        actor,
        id(candidate, "clinician_id")?,
        times,
        session,
        group,
    )
    .await?;
    check_client(
        tx,
        actor,
        id(candidate, "client_id")?,
        times,
        session,
        group,
    )
    .await
}
pub async fn check_group(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    candidate: &Value,
) -> Result<()> {
    let times = interval(candidate)?;
    if !reserves(candidate["status"].as_str()) {
        return Ok(());
    }
    let group = optional_id(candidate, "id")?;
    check_clinician(
        tx,
        actor,
        id(candidate, "clinician_id")?,
        times,
        None,
        group,
    )
    .await?;
    if let Some(group) = group {
        let clients:Vec<Uuid>=sqlx::query_scalar("SELECT client_id FROM group_attendance WHERE organization_id=$1 AND group_id=$2 AND status IN ('scheduled','attended')")
            .bind(actor.org).bind(group).fetch_all(&mut **tx).await?;
        for client in clients {
            check_client(tx, actor, client, times, None, Some(group)).await?;
        }
    }
    Ok(())
}
pub async fn check_staff_schedule(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    staff_id: Uuid,
) -> Result<()> {
    let groups:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(g) FROM groups g WHERE organization_id=$1 AND clinician_id=$2 AND status='scheduled' AND starts_at>now()")
        .bind(actor.org).bind(staff_id).fetch_all(&mut **tx).await?;
    for group in groups {
        check_group(tx, actor, &group).await?;
    }
    let sessions:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(s) FROM sessions s WHERE organization_id=$1 AND clinician_id=$2 AND group_id IS NULL AND status='scheduled' AND starts_at>now()")
        .bind(actor.org).bind(staff_id).fetch_all(&mut **tx).await?;
    for session in sessions {
        check_session(tx, actor, &session).await?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn reservation_status_and_interval_validation() {
        assert!(reserves(None));
        assert!(reserves(Some("completed")));
        assert!(!reserves(Some("cancelled")));
        assert!(!reserves(Some("no_show")));
        assert!(interval(
            &json!({"starts_at":"2030-01-01T09:00:00Z","ends_at":"2030-01-01T09:00:00Z"})
        )
        .is_err());
        assert!(interval(
            &json!({"starts_at":"2030-01-01T09:00:00Z","ends_at":"2030-01-01T10:00:00Z"})
        )
        .is_ok());
    }
}
