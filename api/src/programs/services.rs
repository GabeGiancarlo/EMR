//! Program-service routing for sessions, groups, enrollment, and claim codes.
//! Callers hold the organization mutation lock, exclude client `service_snapshot`
//! values, and invoke these helpers before insert or patch. Snapshot writes happen
//! only on creation or a genuine service change.

use crate::{access::Actor, records, scheduling, ApiError, Result};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

const SNAPSHOT_INVALID: &str =
    "The stored service snapshot is missing or does not match the assigned program service.";
const CLAIM_SNAPSHOT: &str = "The session service snapshot is missing or malformed, so the claim cannot use a catalog service code.";
const CLAIM_MISMATCH: &str = "Claim service code must match the historical program service code.";

enum ServiceChoice {
    Unspecified,
    Clear,
    Set(Uuid),
}

struct Catalog {
    service: Value,
    program: Value,
    facility_active: bool,
}

pub(crate) fn claim_code_error(session: &Value, claim: &Value) -> Option<&'static str> {
    if session.get("program_service_id").is_none_or(Value::is_null) {
        return None;
    }
    let Some(service_id) = session.get("program_service_id").and_then(parse_uuid_value) else {
        return Some(CLAIM_SNAPSHOT);
    };
    let Some(snapshot) = session
        .get("service_snapshot")
        .filter(|value| value.is_object())
    else {
        return Some(CLAIM_SNAPSHOT);
    };
    if parse_uuid_value(&snapshot["id"]) != Some(service_id)
        || normalized_code(&snapshot["service_code"]).is_none()
        || parse_uuid_value(&snapshot["program_id"]).is_none()
        || parse_uuid_value(&snapshot["organization_id"])
            != parse_uuid_value(&session["organization_id"])
        || parse_uuid_value(&snapshot["organization_id"]).is_none()
        || !matches!(
            snapshot["code_system"].as_str(),
            Some("internal" | "cpt" | "hcpcs")
        )
        || snapshot["session_kind"] != session["kind"]
        || bounds(snapshot).is_err()
    {
        return Some(CLAIM_SNAPSHOT);
    }
    if normalized_code(&claim["service_code"]) != normalized_code(&snapshot["service_code"]) {
        return Some(CLAIM_MISMATCH);
    }
    None
}

pub(crate) async fn validate_claim(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    claim: &Value,
) -> Result<()> {
    let session = records::get(
        tx,
        "sessions",
        records::uuid(claim, "session_id")?,
        actor.org,
        false,
    )
    .await?;
    if let Some(error) = claim_code_error(&session, claim) {
        return Err(ApiError::invalid(error));
    }
    Ok(())
}

pub(crate) async fn validate_session(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &mut Map<String, Value>,
    before: Option<&Value>,
) -> Result<()> {
    apply(tx, actor, data, before, false).await
}

pub(crate) async fn validate_group(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &mut Map<String, Value>,
    before: Option<&Value>,
) -> Result<()> {
    apply(tx, actor, data, before, true).await
}

pub(crate) async fn enrollment_context(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    group: &Value,
    episode: &Value,
) -> Result<Map<String, Value>> {
    same_actor_org(actor, group)?;
    same_actor_org(actor, episode)?;
    if optional_uuid(group, "program_service_id")?.is_none() {
        if optional_uuid(episode, "program_id")?.is_some() {
            return Err(ApiError::invalid(
                "A program-linked episode cannot join a group that has no program service.",
            ));
        }
        return Ok(Map::new());
    }
    let service_id = optional_uuid(group, "program_service_id")?
        .ok_or_else(|| ApiError::conflict(SNAPSHOT_INVALID))?;
    let snapshot = group
        .get("service_snapshot")
        .cloned()
        .unwrap_or(Value::Null);
    let catalog = load_catalog(tx, actor, service_id).await?;
    if !snapshot_matches(&snapshot, service_id, &catalog.service) || bounds(&snapshot).is_err() {
        return Err(ApiError::conflict(SNAPSHOT_INVALID));
    }
    if snapshot["session_kind"] != "group" {
        return Err(ApiError::invalid(
            "A group requires a program service with session kind group.",
        ));
    }
    require_active(&catalog)?;
    require_same_facility(group, &catalog.program)?;
    require_episode_program(episode, &catalog.program)?;
    let mut context = Map::new();
    context.insert(
        "program_service_id".into(),
        group["program_service_id"].clone(),
    );
    context.insert("service_snapshot".into(), snapshot);
    Ok(context)
}

async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &mut Map<String, Value>,
    before: Option<&Value>,
    group_resource: bool,
) -> Result<()> {
    let supplied = data.remove("service_snapshot");
    let choice = service_choice(data)?;
    let creating = before.and_then(|value| value.get("id")).is_none();
    let previous = if creating {
        None
    } else {
        optional_uuid(before.unwrap_or(&Value::Null), "program_service_id")?
    };
    if let Some(supplied) = supplied.as_ref() {
        if creating {
            // Replaced below with the catalog or the matching group snapshot.
        } else if matches!(choice, ServiceChoice::Set(id) if Some(id) != previous)
            || supplied == &before.unwrap_or(&Value::Null)["service_snapshot"]
        {
            // A genuine service change assigns a server snapshot; an identical copy is ignored.
        } else {
            return Err(ApiError::invalid(
                "service_snapshot is server-managed and cannot be supplied",
            ));
        }
    }

    let selected = match choice {
        ServiceChoice::Clear if previous.is_some() => {
            return Err(ApiError::conflict(
                "An assigned program service cannot be removed.",
            ));
        }
        ServiceChoice::Clear => None,
        ServiceChoice::Set(id) => Some(id),
        ServiceChoice::Unspecified => previous,
    };
    let Some(service_id) = selected else {
        if creating {
            return Err(ApiError::invalid(format!(
                "program_service_id is required for a new {}.",
                if group_resource { "group" } else { "session" }
            )));
        }
        let existing = before.unwrap_or(&Value::Null);
        if field_changed(existing, data, "starts_at")
            || field_changed(existing, data, "ends_at")
            || reopens_schedule(existing, data)
        {
            return Err(ApiError::invalid(
                "Rescheduling a historical booking requires selecting a program service first.",
            ));
        }
        return Ok(());
    };

    let service_changed = previous != Some(service_id);
    if !creating
        && !service_changed
        && !structural_edit(before.unwrap_or(&Value::Null), data, group_resource)
    {
        return Ok(());
    }

    let catalog = load_catalog(tx, actor, service_id).await?;
    require_active(&catalog)?;
    let record = merged(before, data);
    if group_resource {
        require_same_facility(&record, &catalog.program)?;
    } else {
        let episode = records::get(
            tx,
            "episodes",
            records::uuid(&record, "episode_id")?,
            actor.org,
            false,
        )
        .await?;
        require_episode_program(&episode, &catalog.program)?;
        require_same_facility(&episode, &catalog.program)?;
    }

    let semantics = if !creating && !service_changed {
        let snapshot = before
            .and_then(|value| value.get("service_snapshot"))
            .cloned()
            .unwrap_or(Value::Null);
        if !snapshot_matches(&snapshot, service_id, &catalog.service) {
            return Err(ApiError::conflict(SNAPSHOT_INVALID));
        }
        snapshot
    } else {
        catalog.service.clone()
    };

    if group_resource {
        if semantics["session_kind"] != "group" {
            return Err(ApiError::invalid(
                "A group requires a program service with session kind group.",
            ));
        }
    } else {
        let kind = record["kind"]
            .as_str()
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| ApiError::invalid("kind is required"))?;
        let expected = semantics["session_kind"].as_str().unwrap_or_default();
        if kind != expected {
            return Err(ApiError::invalid(format!(
                "The session kind {kind} does not match the program service session kind {expected}."
            )));
        }
    }
    let (min, max) = bounds(&semantics)?;
    ensure_bounds(elapsed_minutes(&record)?, min, max)?;
    if creating || service_changed {
        data.insert("program_service_id".into(), json!(service_id));
        data.insert("service_snapshot".into(), semantics);
    }
    Ok(())
}

async fn load_catalog(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    service_id: Uuid,
) -> Result<Catalog> {
    let service: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM program_services s WHERE id=$1 AND organization_id=$2",
    )
    .bind(service_id)
    .bind(actor.org)
    .fetch_optional(&mut **tx)
    .await?;
    let service = service.ok_or_else(ApiError::missing)?;
    let program_id = optional_uuid(&service, "program_id")?
        .ok_or_else(|| ApiError::conflict(SNAPSHOT_INVALID))?;
    let program: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(p) FROM care_programs p WHERE id=$1 AND organization_id=$2",
    )
    .bind(program_id)
    .bind(actor.org)
    .fetch_optional(&mut **tx)
    .await?;
    let program = program.ok_or_else(ApiError::missing)?;
    let facility = records::get(
        tx,
        "facilities",
        records::uuid(&program, "facility_id")?,
        actor.org,
        false,
    )
    .await?;
    Ok(Catalog {
        service,
        program,
        facility_active: facility["active"] == true,
    })
}

fn require_active(catalog: &Catalog) -> Result<()> {
    if catalog.service["active"] != true {
        return Err(ApiError::conflict(
            "The program service is inactive and cannot accept a new booking, enrollment, or reschedule.",
        ));
    }
    if catalog.program["active"] != true {
        return Err(ApiError::conflict(
            "The care program is inactive and cannot accept a new booking, enrollment, or reschedule.",
        ));
    }
    if !catalog.facility_active {
        return Err(ApiError::conflict(
            "The facility is inactive and cannot accept a new booking, enrollment, or reschedule.",
        ));
    }
    Ok(())
}

fn require_episode_program(episode: &Value, program: &Value) -> Result<()> {
    let episode_program = optional_uuid(episode, "program_id")?
        .ok_or_else(|| ApiError::invalid("The episode must belong to the group's care program."))?;
    let program_id =
        optional_uuid(program, "id")?.ok_or_else(|| ApiError::conflict(SNAPSHOT_INVALID))?;
    if episode_program != program_id {
        return Err(ApiError::invalid(
            "The episode program does not match the selected program service.",
        ));
    }
    Ok(())
}

fn require_same_facility(record: &Value, program: &Value) -> Result<()> {
    if records::uuid(record, "facility_id")? != records::uuid(program, "facility_id")? {
        return Err(ApiError::invalid(
            "The group facility does not match the program facility.",
        ));
    }
    Ok(())
}

fn snapshot_matches(snapshot: &Value, service_id: Uuid, service: &Value) -> bool {
    snapshot.is_object()
        && parse_uuid_value(&snapshot["id"]) == Some(service_id)
        && parse_uuid_value(&snapshot["program_id"]) == parse_uuid_value(&service["program_id"])
        && same_token(&snapshot["code_system"], &service["code_system"])
        && same_token(&snapshot["session_kind"], &service["session_kind"])
        && normalized_code(&snapshot["service_code"]).is_some()
        && normalized_code(&snapshot["service_code"]) == normalized_code(&service["service_code"])
}

fn bounds(source: &Value) -> Result<(i64, i64)> {
    let min = source.get("min_minutes").and_then(json_i64);
    let max = source.get("max_minutes").and_then(json_i64);
    match (min, max) {
        (Some(min), Some(max))
            if (1..=480).contains(&min) && (1..=480).contains(&max) && min <= max =>
        {
            Ok((min, max))
        }
        _ => Err(ApiError::conflict(SNAPSHOT_INVALID)),
    }
}

fn ensure_bounds(minutes: f64, min: i64, max: i64) -> Result<()> {
    if ((min as f64)..=(max as f64)).contains(&minutes) {
        Ok(())
    } else {
        Err(ApiError::invalid(format!(
            "Duration of {minutes} minutes is outside the service bounds of {min} to {max} minutes"
        )))
    }
}

fn elapsed_minutes(record: &Value) -> Result<f64> {
    let (start, end) = scheduling::interval(record)?;
    let nanos = end
        .signed_duration_since(start)
        .num_nanoseconds()
        .ok_or_else(|| {
            ApiError::invalid("The appointment duration is outside supported bounds.")
        })?;
    Ok(nanos as f64 / 60_000_000_000.0)
}

fn structural_edit(before: &Value, data: &Map<String, Value>, group_resource: bool) -> bool {
    let keys: &[&str] = if group_resource {
        &[
            "starts_at",
            "ends_at",
            "location",
            "clinician_id",
            "facility_id",
        ]
    } else {
        &[
            "starts_at",
            "ends_at",
            "location",
            "clinician_id",
            "client_id",
            "episode_id",
            "kind",
        ]
    };
    keys.iter().any(|key| field_changed(before, data, key)) || reopens_schedule(before, data)
}

fn reopens_schedule(before: &Value, data: &Map<String, Value>) -> bool {
    matches!(before["status"].as_str(), Some("cancelled" | "no_show"))
        && data.get("status").and_then(Value::as_str) == Some("scheduled")
}

fn field_changed(before: &Value, data: &Map<String, Value>, key: &str) -> bool {
    data.get(key)
        .is_some_and(|value| before.get(key) != Some(value))
}

fn merged(before: Option<&Value>, data: &Map<String, Value>) -> Value {
    let mut object = before
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in data {
        object.insert(key.clone(), value.clone());
    }
    Value::Object(object)
}

fn service_choice(data: &Map<String, Value>) -> Result<ServiceChoice> {
    match data.get("program_service_id") {
        None => Ok(ServiceChoice::Unspecified),
        Some(Value::Null) => Ok(ServiceChoice::Clear),
        Some(value) => parse_uuid_value(value)
            .map(ServiceChoice::Set)
            .ok_or_else(|| ApiError::invalid("program_service_id must be a UUID")),
    }
}

fn same_actor_org(actor: &Actor, value: &Value) -> Result<()> {
    match value.get("organization_id").and_then(Value::as_str) {
        Some(org) if org != actor.org.to_string() => Err(ApiError::missing()),
        _ => Ok(()),
    }
}

fn optional_uuid(value: &Value, key: &str) -> Result<Option<Uuid>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(other) => parse_uuid_value(other)
            .map(Some)
            .ok_or_else(|| ApiError::invalid(format!("{key} must be a UUID"))),
    }
}

fn parse_uuid_value(value: &Value) -> Option<Uuid> {
    value
        .as_str()
        .and_then(|text| Uuid::parse_str(text.trim()).ok())
}

fn normalized_code(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|text| text.trim().to_uppercase())
        .filter(|text| !text.is_empty())
}

fn same_token(left: &Value, right: &Value) -> bool {
    left.as_str()
        .is_some_and(|text| right.as_str() == Some(text) && !text.is_empty())
}

fn json_i64(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    if let Some(number) = value.as_u64() {
        return i64::try_from(number).ok();
    }
    value.as_f64().and_then(|number| {
        if number.is_finite() && number.fract() == 0.0 {
            Some(number as i64)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claim_codes_use_historical_snapshot_only_when_linked() {
        let linked = json!({
            "program_service_id": "11111111-1111-1111-1111-111111111111",
            "organization_id":"22222222-2222-2222-2222-222222222222","kind":"individual",
            "service_snapshot": {
                "id": "11111111-1111-1111-1111-111111111111",
                "organization_id":"22222222-2222-2222-2222-222222222222","program_id":"33333333-3333-3333-3333-333333333333",
                "code_system":"internal","session_kind":"individual","min_minutes":30,"max_minutes":60,
                "service_code": "H2019"
            }
        });
        assert_eq!(
            claim_code_error(&linked, &json!({"service_code": "  h2019  "})),
            None
        );
        assert_eq!(
            claim_code_error(&linked, &json!({"service_code": "H2020"})),
            Some(CLAIM_MISMATCH)
        );
        assert_eq!(
            claim_code_error(&linked, &json!({"service_code": ""})),
            Some(CLAIM_MISMATCH)
        );
        let malformed = json!({
            "program_service_id": "11111111-1111-1111-1111-111111111111",
            "service_snapshot": {}
        });
        assert_eq!(
            claim_code_error(&malformed, &json!({"service_code": "H2019"})),
            Some(CLAIM_SNAPSHOT)
        );
        assert_eq!(
            claim_code_error(
                &json!({"program_service_id": null, "service_snapshot": {}}),
                &json!({"service_code": "FREE"})
            ),
            None
        );
    }

    #[test]
    fn duration_keeps_exact_elapsed_bounds_without_rounding() {
        let exact = json!({"starts_at": "2030-01-01T09:00:00Z", "ends_at": "2030-01-01T09:30:00Z"});
        assert_eq!(elapsed_minutes(&exact).unwrap(), 30.0);
        assert!(ensure_bounds(30.0, 30, 60).is_ok());
        assert!(ensure_bounds(60.0, 30, 60).is_ok());
        assert!(ensure_bounds(29.0, 30, 60).is_err());
        assert!(ensure_bounds(61.0, 30, 60).is_err());
        let partial =
            json!({"starts_at": "2030-01-01T09:00:00Z", "ends_at": "2030-01-01T09:30:01Z"});
        assert!(ensure_bounds(elapsed_minutes(&partial).unwrap(), 30, 60).is_ok());
        let fractional =
            json!({"starts_at": "2030-01-01T09:00:00Z", "ends_at": "2030-01-01T10:00:00.001Z"});
        assert!(ensure_bounds(elapsed_minutes(&fractional).unwrap(), 30, 60).is_err());
    }
}
