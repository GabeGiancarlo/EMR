//! Treatment-plan attestation, review history, and shared group attendance workflows.
use crate::{access::Actor, records, ApiError, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};

fn object(v: Value) -> Map<String, Value> {
    v.as_object().expect("internal JSON object").clone()
}
fn attendance_status(status: &str) -> Result<&'static str> {
    match status {
        "scheduled" => Ok("scheduled"),
        "attended" => Ok("completed"),
        "no_show" => Ok("no_show"),
        "excused" => Ok("cancelled"),
        _ => Err(ApiError::invalid("Invalid attendance status")),
    }
}
fn check_attendance_time(group: &Value, status: &str) -> Result<()> {
    if matches!(status, "attended" | "no_show") {
        let start = DateTime::parse_from_rfc3339(group["starts_at"].as_str().unwrap_or_default())
            .map_err(|_| ApiError::invalid("Invalid group start time"))?;
        if start > Utc::now() {
            return Err(ApiError::conflict(
                "Attendance cannot be recorded before the group starts",
            ));
        }
    }
    Ok(())
}
fn can_attest(actor: &Actor, plan: &Value) -> bool {
    matches!(actor.role.as_str(), "admin" | "clinical_lead")
        || (actor.role == "clinician" && plan["author_id"] == actor.id.to_string())
}

pub async fn prepare_create(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    resource: &str,
    data: &mut Map<String, Value>,
) -> Result<()> {
    match resource {
        "treatment_plans" => {
            let plan = Value::Object(data.clone());
            let episode = records::get(
                tx,
                "episodes",
                records::uuid(&plan, "episode_id")?,
                actor.org,
                true,
            )
            .await?;
            if episode["status"] != "active" {
                return Err(ApiError::conflict(
                    "Treatment plans require an active episode",
                ));
            }
            data.insert("author_id".into(), json!(actor.id));
        }
        "goals" | "plan_reviews" => {
            let value = Value::Object(data.clone());
            let plan = records::get(
                tx,
                "treatment_plans",
                records::uuid(&value, "plan_id")?,
                actor.org,
                true,
            )
            .await?;
            data.insert("client_id".into(), plan["client_id"].clone());
            if resource == "goals" {
                if plan["status"] != "draft" {
                    return Err(ApiError::conflict(
                        "Revise the signed plan before changing goal definitions",
                    ));
                }
            } else {
                if plan["status"] != "active" {
                    return Err(ApiError::conflict(
                        "Progress reviews require an active treatment plan",
                    ));
                }
                let goals:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(g) FROM goals g WHERE plan_id=$1 AND organization_id=$2 ORDER BY created_at").bind(records::uuid(&plan,"id")?).bind(actor.org).fetch_all(&mut **tx).await?;
                let entries = data
                    .get("goal_progress")
                    .and_then(Value::as_array)
                    .ok_or_else(|| ApiError::invalid("goal_progress must be an array"))?;
                if entries.is_empty() || entries.len() != goals.len() {
                    return Err(ApiError::invalid(
                        "Record one progress observation for every goal in this plan",
                    ));
                }
                let mut seen = std::collections::HashSet::new();
                for entry in entries {
                    let id = records::uuid(entry, "goal_id")?;
                    if !seen.insert(id) || !goals.iter().any(|g| g["id"] == id.to_string()) {
                        return Err(ApiError::invalid(
                            "Each progress entry must identify a unique goal in this plan",
                        ));
                    }
                    if !matches!(
                        records::text(entry, "status")?,
                        "on_track" | "at_risk" | "achieved" | "discontinued"
                    ) {
                        return Err(ApiError::invalid("Invalid goal progress status"));
                    }
                    records::text(entry, "observation")?;
                }
                data.insert("reviewer_id".into(), json!(actor.id));
                data.insert("goal_snapshot".into(), json!(goals));
            }
        }
        "groups"
            if actor.role == "clinician" && data.get("clinician_id") != Some(&json!(actor.id)) =>
        {
            return Err(ApiError::denied());
        }
        "group_attendance" => {
            let value = Value::Object(data.clone());
            let group = records::get(
                tx,
                "groups",
                records::uuid(&value, "group_id")?,
                actor.org,
                true,
            )
            .await?;
            if actor.role == "clinician" && group["clinician_id"] != actor.id.to_string() {
                return Err(ApiError::denied());
            }
            if group["status"] != "scheduled" {
                return Err(ApiError::conflict(
                    "Only scheduled groups accept participants",
                ));
            }
            let episode = records::get(
                tx,
                "episodes",
                records::uuid(&value, "episode_id")?,
                actor.org,
                false,
            )
            .await?;
            if episode["status"] != "active"
                || episode["client_id"] != value["client_id"]
                || episode["facility_id"] != group["facility_id"]
            {
                return Err(ApiError::invalid(
                    "Participant needs an active episode in the group's facility",
                ));
            }
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM group_attendance WHERE organization_id=$1 AND group_id=$2 AND status<>'excused'").bind(actor.org).bind(records::uuid(&group,"id")?).fetch_one(&mut **tx).await?;
            if count >= group["capacity"].as_i64().unwrap_or_default() {
                return Err(ApiError::conflict("The group is at capacity"));
            }
            let status = value["status"].as_str().unwrap_or("scheduled");
            check_attendance_time(&group, status)?;
            let candidate = json!({"client_id":value["client_id"],"episode_id":value["episode_id"],"clinician_id":group["clinician_id"],"starts_at":group["starts_at"],"ends_at":group["ends_at"],"kind":"group","status":attendance_status(status)?,"group_id":group["id"]});
            crate::scheduling::check_session(tx, actor, &candidate).await?;
            let mut session_data = object(
                json!({"client_id":value["client_id"],"episode_id":value["episode_id"],"clinician_id":group["clinician_id"],"starts_at":group["starts_at"],"ends_at":group["ends_at"],"kind":"group","status":attendance_status(status)?,"location":group["location"],"group_id":group["id"]}),
            );
            session_data.extend(
                crate::programs::services::enrollment_context(tx, actor, &group, &episode).await?,
            );
            let session = records::insert(tx, "sessions", session_data, actor).await?;
            data.insert("session_id".into(), session["id"].clone());
        }
        _ => {}
    }
    Ok(())
}

pub async fn before_update(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    resource: &str,
    before: &Value,
    data: &Map<String, Value>,
) -> Result<()> {
    match resource {
        "treatment_plans" if before["status"] != "draft" => {
            return Err(ApiError::conflict(
                "Signed plans are immutable; create a revision",
            ));
        }
        "goals" => {
            let plan = records::get(
                tx,
                "treatment_plans",
                records::uuid(before, "plan_id")?,
                actor.org,
                true,
            )
            .await?;
            if plan["status"] != "draft" {
                return Err(ApiError::conflict(
                    "Signed goal definitions are preserved; record progress in a review",
                ));
            }
        }
        "plan_reviews" => {
            return Err(ApiError::conflict(
                "Progress reviews are append only; create a new review",
            ))
        }
        "sessions" if !before["group_id"].is_null() => {
            return Err(ApiError::conflict(
                "Update shared group details or attendance instead of the linked session",
            ));
        }
        "groups" => {
            if actor.role == "clinician" && before["clinician_id"] != actor.id.to_string() {
                return Err(ApiError::denied());
            }
            if before["status"] != "scheduled" {
                return Err(ApiError::conflict(
                    "Completed or cancelled groups cannot be rewritten",
                ));
            }
            let group_id = records::uuid(before, "id")?;
            if data
                .get("program_service_id")
                .is_some_and(|id| id != &before["program_service_id"])
            {
                let enrolled:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM group_attendance WHERE organization_id=$1 AND group_id=$2)").bind(actor.org).bind(group_id).fetch_one(&mut **tx).await?;
                if enrolled {
                    return Err(ApiError::conflict("A group with enrolled participants keeps its original service. Schedule a new configured group to change that care context."));
                }
            }
            sqlx::query("SELECT id FROM sessions WHERE group_id=$1 AND organization_id=$2 ORDER BY id FOR UPDATE").bind(group_id).bind(actor.org).fetch_all(&mut **tx).await?;
            let recorded:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM group_attendance WHERE group_id=$1 AND organization_id=$2 AND status IN ('attended','no_show')) OR EXISTS(SELECT 1 FROM notes n JOIN sessions s ON s.id=n.session_id AND s.organization_id=n.organization_id WHERE s.group_id=$1 AND n.organization_id=$2 AND n.status='signed')").bind(group_id).bind(actor.org).fetch_one(&mut **tx).await?;
            if recorded {
                return Err(ApiError::conflict(
                    "A group with recorded attendance or signed documentation cannot be rewritten",
                ));
            }
            if let Some(capacity) = data.get("capacity") {
                let count:i64=sqlx::query_scalar("SELECT count(*) FROM group_attendance WHERE group_id=$1 AND organization_id=$2 AND status<>'excused'").bind(group_id).bind(actor.org).fetch_one(&mut **tx).await?;
                if capacity.as_i64().unwrap_or_default() < count {
                    return Err(ApiError::conflict(
                        "Capacity cannot be lower than the current roster",
                    ));
                }
            }
        }
        "group_attendance" => {
            records::get(
                tx,
                "sessions",
                records::uuid(before, "session_id")?,
                actor.org,
                true,
            )
            .await?;
            let group = records::get(
                tx,
                "groups",
                records::uuid(before, "group_id")?,
                actor.org,
                true,
            )
            .await?;
            if group["status"] != "scheduled" {
                return Err(ApiError::conflict(
                    "Attendance is finalized when a group completes or is cancelled",
                ));
            }
            if actor.role == "clinician" && group["clinician_id"] != actor.id.to_string() {
                return Err(ApiError::denied());
            }
            let locked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notes WHERE session_id=$1 AND organization_id=$2 AND status='signed') OR EXISTS(SELECT 1 FROM claims WHERE session_id=$1 AND organization_id=$2 AND status<>'draft')").bind(records::uuid(before,"session_id")?).bind(actor.org).fetch_one(&mut **tx).await?;
            if locked {
                return Err(ApiError::conflict(
                    "Signed or billed attendance cannot be rewritten",
                ));
            }
            if let Some(status) = data.get("status").and_then(Value::as_str) {
                attendance_status(status)?;
                check_attendance_time(&group, status)?;
                if matches!(status, "scheduled" | "attended")
                    && !matches!(before["status"].as_str(), Some("scheduled" | "attended"))
                {
                    let mut candidate = records::get(
                        tx,
                        "sessions",
                        records::uuid(before, "session_id")?,
                        actor.org,
                        false,
                    )
                    .await?;
                    candidate["status"] = json!(attendance_status(status)?);
                    crate::scheduling::check_session(tx, actor, &candidate).await?;
                }
                if before["status"] == "excused" && status != "excused" {
                    let count:i64=sqlx::query_scalar("SELECT count(*) FROM group_attendance WHERE group_id=$1 AND organization_id=$2 AND status<>'excused'").bind(records::uuid(&group,"id")?).bind(actor.org).fetch_one(&mut **tx).await?;
                    if count >= group["capacity"].as_i64().unwrap_or_default() {
                        return Err(ApiError::conflict("The group is at capacity"));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
pub async fn after_update(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    resource: &str,
    value: &Value,
) -> Result<()> {
    match resource {
        "group_attendance" => {
            records::patch(tx,"sessions",records::uuid(value,"session_id")?,actor.org,object(json!({"status":attendance_status(value["status"].as_str().unwrap_or_default())?}))).await?;
        }
        "groups" => {
            sqlx::query("UPDATE sessions SET starts_at=$1::text::timestamptz,ends_at=$2::text::timestamptz,clinician_id=$3,location=$4,updated_at=now() WHERE group_id=$5 AND organization_id=$6 AND status IN ('scheduled','cancelled')").bind(value["starts_at"].as_str().unwrap_or_default()).bind(value["ends_at"].as_str().unwrap_or_default()).bind(records::uuid(value,"clinician_id")?).bind(value["location"].as_str().unwrap_or_default()).bind(records::uuid(value,"id")?).bind(actor.org).execute(&mut **tx).await?;
        }
        _ => {}
    }
    Ok(())
}

pub async fn perform(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    resource: &str,
    action: &str,
    old: &Value,
    body: &Value,
) -> Result<Value> {
    let id = records::uuid(old, "id")?;
    match (resource, action) {
        ("treatment_plans", "activate") => {
            let episode = records::get(
                tx,
                "episodes",
                records::uuid(old, "episode_id")?,
                actor.org,
                true,
            )
            .await?;
            if episode["status"] != "active" {
                return Err(ApiError::conflict(
                    "Only an active episode can activate a treatment plan",
                ));
            }
            if !can_attest(actor, old) {
                return Err(ApiError::denied());
            }
            if old["status"] != "draft" {
                return Err(ApiError::conflict("Only draft plans can be activated"));
            }
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM goals WHERE plan_id=$1 AND organization_id=$2",
            )
            .bind(id)
            .bind(actor.org)
            .fetch_one(&mut **tx)
            .await?;
            if count == 0 {
                return Err(ApiError::conflict(
                    "Add at least one measurable goal before activating the plan",
                ));
            }
            if !old["supersedes_plan_id"].is_null() {
                let original = records::get(
                    tx,
                    "treatment_plans",
                    records::uuid(old, "supersedes_plan_id")?,
                    actor.org,
                    true,
                )
                .await?;
                if original["status"] != "active" {
                    return Err(ApiError::conflict(
                        "The original plan is no longer active; review the latest revision",
                    ));
                }
                records::patch(
                    tx,
                    "treatment_plans",
                    records::uuid(&original, "id")?,
                    actor.org,
                    object(json!({"status":"superseded"})),
                )
                .await?;
            }
            records::patch(
                tx,
                resource,
                id,
                actor.org,
                object(json!({"status":"active","signed_at":Utc::now(),"signed_by":actor.id})),
            )
            .await
        }
        ("treatment_plans", "complete") => {
            if !can_attest(actor, old) {
                return Err(ApiError::denied());
            }
            if old["status"] != "active" {
                return Err(ApiError::conflict("Only active plans can be completed"));
            }
            records::patch(
                tx,
                resource,
                id,
                actor.org,
                object(
                    json!({"status":"completed","completion_reason":records::text(body,"reason")?}),
                ),
            )
            .await
        }
        ("treatment_plans", "revise") => {
            if !can_attest(actor, old) {
                return Err(ApiError::denied());
            }
            if old["status"] != "active" {
                return Err(ApiError::conflict("Only active plans can be revised"));
            }
            let reason = records::text(body, "reason")?;
            if reason.len() < 10 {
                return Err(ApiError::invalid(
                    "Give a revision reason of at least 10 characters",
                ));
            }
            let mut data = Map::new();
            for key in [
                "client_id",
                "episode_id",
                "title",
                "presenting_needs",
                "strengths",
                "next_review_date",
            ] {
                data.insert(key.into(), old[key].clone());
            }
            data.insert("author_id".into(), json!(actor.id));
            data.insert("supersedes_plan_id".into(), json!(id));
            data.insert("revision_reason".into(), json!(reason));
            let revision = records::insert(tx, resource, data, actor).await?;
            let goals: Vec<Value> = sqlx::query_scalar(
                "SELECT to_jsonb(g) FROM goals g WHERE plan_id=$1 AND organization_id=$2",
            )
            .bind(id)
            .bind(actor.org)
            .fetch_all(&mut **tx)
            .await?;
            for goal in goals {
                let mut data = Map::new();
                for key in [
                    "client_id",
                    "description",
                    "measure",
                    "baseline",
                    "target",
                    "target_date",
                ] {
                    data.insert(key.into(), goal[key].clone());
                }
                data.insert("plan_id".into(), revision["id"].clone());
                records::insert(tx, "goals", data, actor).await?;
            }
            Ok(revision)
        }
        ("groups", "complete") => {
            if !matches!(actor.role.as_str(), "admin" | "clinical_lead")
                && old["clinician_id"] != actor.id.to_string()
            {
                return Err(ApiError::denied());
            }
            if old["status"] != "scheduled" {
                return Err(ApiError::conflict("Only a scheduled group can complete"));
            }
            let end = DateTime::parse_from_rfc3339(old["ends_at"].as_str().unwrap_or_default())
                .map_err(|_| ApiError::invalid("Invalid group end time"))?;
            if end > Utc::now() {
                return Err(ApiError::conflict("A group cannot complete before it ends"));
            }
            let pending:i64=sqlx::query_scalar("SELECT count(*) FROM group_attendance WHERE organization_id=$1 AND group_id=$2 AND status='scheduled'").bind(actor.org).bind(id).fetch_one(&mut **tx).await?;
            let total: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM group_attendance WHERE organization_id=$1 AND group_id=$2",
            )
            .bind(actor.org)
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
            if pending > 0 || total == 0 {
                return Err(ApiError::conflict(
                    "Resolve attendance for all participants before completing the group",
                ));
            }
            records::patch(
                tx,
                resource,
                id,
                actor.org,
                object(json!({"status":"completed"})),
            )
            .await
        }
        ("groups", "cancel") => {
            if actor.role == "clinician" && old["clinician_id"] != actor.id.to_string() {
                return Err(ApiError::denied());
            }
            if old["status"] != "scheduled" {
                return Err(ApiError::conflict("Only scheduled groups can be cancelled"));
            }
            sqlx::query("SELECT id FROM sessions WHERE group_id=$1 AND organization_id=$2 ORDER BY id FOR UPDATE").bind(id).bind(actor.org).fetch_all(&mut **tx).await?;
            let documented:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notes n JOIN sessions s ON s.id=n.session_id AND s.organization_id=n.organization_id WHERE s.group_id=$1 AND s.organization_id=$2 AND n.status='signed') OR EXISTS(SELECT 1 FROM claims c JOIN sessions s ON s.id=c.session_id AND s.organization_id=c.organization_id WHERE s.group_id=$1 AND s.organization_id=$2 AND c.status<>'draft')").bind(id).bind(actor.org).fetch_one(&mut **tx).await?;
            if documented {
                return Err(ApiError::conflict(
                    "A documented or billed group cannot be cancelled",
                ));
            }
            let recorded:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM group_attendance WHERE organization_id=$1 AND group_id=$2 AND status IN ('attended','no_show'))").bind(actor.org).bind(id).fetch_one(&mut **tx).await?;
            if recorded {
                return Err(ApiError::conflict(
                    "Recorded attendance cannot be erased by cancelling a group",
                ));
            }
            sqlx::query("UPDATE group_attendance SET status='excused',updated_at=now() WHERE organization_id=$1 AND group_id=$2 AND status='scheduled'").bind(actor.org).bind(id).execute(&mut **tx).await?;
            sqlx::query("UPDATE sessions SET status='cancelled',updated_at=now() WHERE organization_id=$1 AND group_id=$2 AND status='scheduled'").bind(actor.org).bind(id).execute(&mut **tx).await?;
            records::patch(tx,resource,id,actor.org,object(json!({"status":"cancelled","cancellation_reason":records::text(body,"reason")?}))).await
        }
        _ => Err(ApiError::missing()),
    }
}
