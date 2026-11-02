use crate::adult::{self, has_completed_adult_years};
use crate::{access::Actor, ApiError, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub const RESOURCES: &[&str] = &[
    "facilities",
    "staff",
    "clients",
    "episodes",
    "sessions",
    "notes",
    "tasks",
    "policies",
    "authorizations",
    "claims",
    "payments",
    "payment_reversals",
    "templates",
    "consents",
    "exceptions",
    "treatment_plans",
    "goals",
    "plan_reviews",
    "groups",
    "group_attendance",
    "form_responses",
];
pub fn fields(resource: &str) -> Result<Vec<&'static str>> {
    let value = match resource {
        "facilities"=>"name timezone active",
        "staff"=>"name email role facility_id active",
        "clients"=>"first_name last_name preferred_name birth_date email phone status facility_id primary_clinician_id emergency_contact pronouns preferred_language address_line1 address_line2 city region postal_code country",
        "episodes"=>"client_id facility_id level_of_care start_date end_date status discharge_reason",
        "sessions"=>"client_id episode_id clinician_id starts_at ends_at kind status location",
        "notes"=>"client_id session_id template_id title body",
        "tasks"=>"client_id assignee_id title due_date priority status description",
        "policies"=>"client_id payer_name member_id group_number effective_date termination_date status",
        "authorizations"=>"client_id policy_id authorization_number service_code units_approved starts_on ends_on status",
        "claims"=>"client_id policy_id session_id authorization_id service_code diagnosis_code amount_cents units",
        "payments"=>"claim_id amount_cents received_on reference source",
        "payment_reversals"=>"",
        "templates"=>"name kind description fields active applies_to_stage applies_to_care_level required_for_stage",
        "consents"=>"client_id kind granted_on expires_on revoked_on scope",
        "exceptions"=>"staff_id resource reason expires_at revoked access_mode",
        "treatment_plans"=>"client_id episode_id title presenting_needs strengths next_review_date",
        "goals"=>"plan_id description measure baseline target target_date",
        "plan_reviews"=>"plan_id summary goal_progress next_review_date",
        "groups"=>"facility_id clinician_id title kind starts_at ends_at capacity location",
        "group_attendance"=>"group_id client_id episode_id status",
        "form_responses"=>"client_id episode_id session_id template_id answers source_type source_excerpt field_provenance transcript_excerpt_id",
        _=>return Err(ApiError::missing())
    };
    Ok(value.split_whitespace().collect())
}
pub fn uuid(value: &Value, key: &str) -> Result<Uuid> {
    Uuid::parse_str(
        value[key]
            .as_str()
            .ok_or_else(|| ApiError::invalid(format!("{key} is required")))?,
    )
    .map_err(|_| ApiError::invalid(format!("{key} must be a UUID")))
}
pub fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ApiError::invalid(format!("{key} is required")))
}
pub fn clean(resource: &str, input: &Value) -> Result<Map<String, Value>> {
    let allowed = fields(resource)?;
    if resource == "clients" {
        crate::patient_identity::validate_demographics(input)?;
    }
    let object = input
        .as_object()
        .ok_or_else(|| ApiError::invalid("Expected a JSON object"))?;
    let mut result = Map::new();
    for (key, value) in object {
        if !allowed.contains(&key.as_str()) {
            return Err(ApiError::invalid(format!(
                "Field {key} cannot be changed on {resource}"
            )));
        }
        let value = match value {
            Value::String(s) => Value::String(s.trim().into()),
            other => other.clone(),
        };
        let nullable = match resource {
            "staff" => "facility_id",
            "clients" => "primary_clinician_id",
            "episodes" => "end_date",
            "notes" => "session_id template_id",
            "tasks" => "client_id assignee_id due_date",
            "policies" => "termination_date",
            "claims" => "authorization_id",
            "consents" => "expires_on revoked_on",
            "treatment_plans" | "plan_reviews" => "next_review_date",
            "goals" => "target_date",
            "form_responses" => "episode_id session_id transcript_excerpt_id",
            _ => "",
        };
        if value.is_null() && !nullable.split_whitespace().any(|k| k == key) {
            return Err(ApiError::invalid(format!(
                "{key} cannot be null; omit an unused optional field or provide its value"
            )));
        }
        if matches!(key.as_str(), "active" | "revoked" | "required_for_stage")
            && !value.is_boolean()
        {
            return Err(ApiError::invalid(format!("{key} must be true or false")));
        }
        if value.as_str().map(|s| s.len() > 100_000).unwrap_or(false) {
            return Err(ApiError::invalid("Text exceeds the record limit"));
        }
        if key.ends_with("_id") && key != "member_id" && !value.is_null() {
            uuid(&json!({key:value}), key)?;
        }
        if matches!(
            key.as_str(),
            "birth_date"
                | "start_date"
                | "end_date"
                | "due_date"
                | "effective_date"
                | "termination_date"
                | "starts_on"
                | "ends_on"
                | "received_on"
                | "granted_on"
                | "expires_on"
                | "revoked_on"
                | "next_review_date"
                | "target_date"
        ) && !value.is_null()
        {
            let date = NaiveDate::parse_from_str(value.as_str().unwrap_or_default(), "%Y-%m-%d")
                .map_err(|_| ApiError::invalid(format!("{key} must be a valid YYYY-MM-DD date")))?;
            if matches!(
                key.as_str(),
                "birth_date" | "received_on" | "granted_on" | "revoked_on"
            ) && date > Utc::now().date_naive()
            {
                return Err(ApiError::invalid(format!("{key} cannot be in the future")));
            }
        }
        if matches!(key.as_str(), "starts_at" | "ends_at" | "expires_at") && !value.is_null() {
            let date =
                DateTime::parse_from_rfc3339(value.as_str().unwrap_or_default()).map_err(|_| {
                    ApiError::invalid(format!("{key} needs an RFC3339 timestamp with timezone"))
                })?;
            if key == "expires_at" && date <= Utc::now() {
                return Err(ApiError::invalid(
                    "Permission exception expiry must be in the future",
                ));
            }
        }
        if matches!(
            key.as_str(),
            "amount_cents" | "units_approved" | "units" | "capacity"
        ) && value
            .as_i64()
            .filter(|n| *n > 0 && *n <= 1_000_000_000)
            .is_none()
        {
            return Err(ApiError::invalid(format!(
                "{key} must be a positive whole number within the supported range"
            )));
        }
        if key == "email"
            && value
                .as_str()
                .map(|s| !s.is_empty() && (!s.contains('@') || s.contains(' ')))
                .unwrap_or(false)
        {
            return Err(ApiError::invalid("Enter a valid email address"));
        }
        result.insert(key.clone(), value);
    }
    if resource == "templates" {
        if let Some(value) = result.get("applies_to_stage") {
            if !matches!(
                value.as_str(),
                Some("any" | "intake" | "active" | "discharged")
            ) {
                return Err(ApiError::invalid(
                    "applies_to_stage must be any, intake, active, or discharged",
                ));
            }
        }
        if let Some(value) = result.get("applies_to_care_level") {
            if !matches!(value.as_str(), Some("any" | "outpatient" | "iop" | "php")) {
                return Err(ApiError::invalid(
                    "applies_to_care_level must be any, outpatient, iop, or php",
                ));
            }
        }
        if let Some(value) = result.get("fields") {
            let arr = value
                .as_array()
                .ok_or_else(|| ApiError::invalid("Template fields must be an array"))?;
            if arr.len() > 100 {
                return Err(ApiError::invalid(
                    "A template may contain at most 100 fields",
                ));
            }
            let mut seen = std::collections::HashSet::new();
            for f in arr {
                let key = text(f, "key")?;
                text(f, "label")?;
                if key.len() > 64 || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    return Err(ApiError::invalid(
                        "Field keys must use at most 64 letters, digits or underscores",
                    ));
                }
                if f.get("required").map(|v| !v.is_boolean()).unwrap_or(false) {
                    return Err(ApiError::invalid("Field required flags must be boolean"));
                }
                if f["type"] == "select" {
                    let options = f["options"]
                        .as_array()
                        .ok_or_else(|| ApiError::invalid("Select fields need an options array"))?;
                    if options.is_empty()
                        || options.len() > 100
                        || options
                            .iter()
                            .any(|v| v.as_str().map(|s| s.trim().is_empty()).unwrap_or(true))
                    {
                        return Err(ApiError::invalid(
                            "Select fields require 1 to 100 nonempty text options",
                        ));
                    }
                }
                if !seen.insert(key)
                    || !matches!(
                        text(f, "type")?,
                        "text" | "textarea" | "date" | "number" | "checkbox" | "select"
                    )
                {
                    return Err(ApiError::invalid(
                        "Template keys must be unique and field types supported",
                    ));
                }
            }
        }
    }
    Ok(result)
}
pub async fn get(
    tx: &mut Transaction<'_, Postgres>,
    resource: &str,
    id: Uuid,
    org: Uuid,
    lock: bool,
) -> Result<Value> {
    fields(resource)?;
    let sql = format!(
        "SELECT to_jsonb(t) FROM {resource} t WHERE id=$1 AND organization_id=$2 {}",
        if lock { "FOR UPDATE" } else { "" }
    );
    sqlx::query_scalar(&sql)
        .bind(id)
        .bind(org)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::missing)
}
pub async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    resource: &str,
    mut data: Map<String, Value>,
    actor: &Actor,
) -> Result<Value> {
    fields(resource)?;
    data.insert("id".into(), json!(Uuid::new_v4()));
    data.insert("organization_id".into(), json!(actor.org));
    let keys: Vec<String> = data.keys().map(|x| format!("\"{x}\"")).collect();
    let columns = keys.join(",");
    let sql=format!("WITH inserted AS (INSERT INTO {resource} ({columns}) SELECT {columns} FROM jsonb_populate_record(NULL::{resource},$1) RETURNING *) SELECT to_jsonb(inserted) FROM inserted");
    Ok(sqlx::query_scalar(&sql)
        .bind(Value::Object(data))
        .fetch_one(&mut **tx)
        .await?)
}
pub async fn patch(
    tx: &mut Transaction<'_, Postgres>,
    resource: &str,
    id: Uuid,
    org: Uuid,
    mut data: Map<String, Value>,
) -> Result<Value> {
    fields(resource)?;
    data.insert("updated_at".into(), json!(Utc::now()));
    let sets = data
        .keys()
        .map(|k| format!("\"{k}\"=p.\"{k}\""))
        .collect::<Vec<_>>()
        .join(",");
    let sql=format!("WITH changed AS (UPDATE {resource} t SET {sets} FROM jsonb_populate_record(NULL::{resource},$1) p WHERE t.id=$2 AND t.organization_id=$3 RETURNING t.*) SELECT to_jsonb(changed) FROM changed");
    sqlx::query_scalar(&sql)
        .bind(Value::Object(data))
        .bind(id)
        .bind(org)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::missing)
}
pub async fn scope(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    resource: &str,
    data: &Value,
) -> Result<()> {
    if actor.role != "clinician" {
        return Ok(());
    }
    let client = if resource == "clients" {
        data.get("id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
    } else if matches!(resource, "payments" | "payment_reversals") {
        let claim = get(tx, "claims", uuid(data, "claim_id")?, actor.org, false).await?;
        Some(uuid(&claim, "client_id")?)
    } else {
        data.get("client_id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
    };
    if let Some(id) = client {
        let owns:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM clients WHERE id=$1 AND organization_id=$2 AND primary_clinician_id=$3)").bind(id).bind(actor.org).bind(actor.id).fetch_one(&mut **tx).await?;
        if !owns {
            return Err(ApiError::denied());
        }
    }
    Ok(())
}
pub async fn validate_links(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    resource: &str,
    data: &Value,
) -> Result<()> {
    if resource == "facilities" {
        if let Some(zone) = data.get("timezone") {
            let zone = zone.as_str().ok_or_else(|| {
                ApiError::invalid("Facility timezone must be an IANA timezone name")
            })?;
            let valid: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_timezone_names WHERE name=$1)")
                    .bind(zone)
                    .fetch_one(&mut **tx)
                    .await?;
            if !valid {
                return Err(ApiError::invalid(
                    "Choose a recognized facility timezone, such as America/New_York",
                ));
            }
        }
    }
    for key in ["primary_clinician_id", "clinician_id", "assignee_id"] {
        if let Some(v) = data.get(key).filter(|v| !v.is_null()) {
            let id = Uuid::parse_str(v.as_str().unwrap_or_default())
                .map_err(|_| ApiError::invalid("Invalid staff reference"))?;
            // Serialize assignment with offboarding so a newly inserted record
            // cannot retain an operator who was deactivated after this check.
            let staff = get(tx, "staff", id, actor.org, true).await?;
            if staff["active"] != true {
                return Err(ApiError::invalid("Assigned staff must be active"));
            }
            if key != "assignee_id"
                && !matches!(
                    staff["role"].as_str(),
                    Some("admin" | "clinical_lead" | "clinician")
                )
            {
                return Err(ApiError::invalid(
                    "A clinical assignment requires a clinician or clinical lead",
                ));
            }
        }
    }
    if resource == "sessions" {
        let ep = get(tx, "episodes", uuid(data, "episode_id")?, actor.org, false).await?;
        if ep["status"] != "active" {
            return Err(ApiError::invalid("Sessions require an active episode"));
        }
        if data["status"] == "completed" {
            ensure_session_ended(data)?;
        }
    }
    if resource == "form_responses"
        && !data["session_id"].is_null()
        && !data["episode_id"].is_null()
    {
        let session = get(tx, "sessions", uuid(data, "session_id")?, actor.org, false).await?;
        if session["episode_id"] != data["episode_id"] {
            return Err(ApiError::invalid(
                "The selected session does not belong to this episode",
            ));
        }
    }
    Ok(())
}

pub fn parse_record_date(value: &Value, key: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value[key].as_str().unwrap_or_default(), "%Y-%m-%d")
        .map_err(|_| ApiError::invalid(format!("{key} must be a valid YYYY-MM-DD date")))
}

/// Adult program gate. Callers supply the facility-local calendar date.
/// Does not read or rewrite stored client rows.
pub fn require_adult_program(
    birth: NaiveDate,
    facility_today: NaiveDate,
    field: &str,
) -> Result<()> {
    if has_completed_adult_years(birth, facility_today) {
        return Ok(());
    }
    Err(ApiError::invalid(adult::ineligible_message(field)))
}

pub fn ensure_session_ended(session: &Value) -> Result<()> {
    let end = DateTime::parse_from_rfc3339(text(session, "ends_at")?).map_err(|_| {
        ApiError::invalid("Session end must include a valid timestamp and timezone")
    })?;
    if end > Utc::now() {
        return Err(ApiError::conflict(
            "The session must end before completion, signing or billing readiness",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_injection_and_server_managed_fields() {
        assert!(fields("clients;DROP TABLE staff").is_err());
        assert!(clean("clients", &json!({"organization_id":Uuid::new_v4()})).is_err());
        assert!(clean("notes", &json!({"status":"signed"})).is_err());
    }
    #[test]
    fn rejects_invalid_calendar_dates_and_fractional_money() {
        assert!(clean("clients", &json!({"birth_date":"2025-02-30"})).is_err());
        assert!(clean("claims", &json!({"amount_cents":10.5})).is_err());
    }
    #[test]
    fn validates_dynamic_forms() {
        assert!(clean("templates",&json!({"fields":[{"key":"a","label":"A","type":"text"},{"key":"a","label":"B","type":"text"}]})).is_err());
    }
    #[test]
    fn rejects_null_optional_text_with_named_field() {
        let err = clean("clients", &json!({"phone":null})).unwrap_err();
        assert!(err.message.contains("phone"));
        assert!(clean("clients", &json!({"primary_clinician_id":null})).is_ok());
    }
    #[test]
    fn adult_program_uses_calendar_anniversary_not_day_count() {
        use chrono::NaiveDate;
        let birth = NaiveDate::from_ymd_opt(2008, 4, 9).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 4, 9).unwrap();
        assert!(require_adult_program(birth, today, "birth_date").is_ok());
        let err =
            require_adult_program(birth, today.pred_opt().unwrap(), "birth_date").unwrap_err();
        assert!(err.message.contains("birth_date"));
        assert!(err.message.contains("18"));
        let leap = NaiveDate::from_ymd_opt(2004, 2, 29).unwrap();
        assert!(require_adult_program(
            leap,
            NaiveDate::from_ymd_opt(2022, 2, 28).unwrap(),
            "birth_date"
        )
        .is_err());
        assert!(require_adult_program(
            leap,
            NaiveDate::from_ymd_opt(2022, 3, 1).unwrap(),
            "birth_date"
        )
        .is_ok());
    }
    #[test]
    fn validates_template_applicability() {
        let ok = clean(
            "templates",
            &json!({
                "name": "Packet",
                "applies_to_stage": " intake ",
                "applies_to_care_level": "iop",
                "required_for_stage": false,
                "fields": [{"key": "a", "label": "A", "type": "text"}]
            }),
        )
        .unwrap();
        assert_eq!(ok["applies_to_stage"], "intake");
        assert_eq!(ok["required_for_stage"], false);
        assert!(clean("templates", &json!({"applies_to_stage": "inquiry"})).is_err());
        assert!(clean(
            "templates",
            &json!({"applies_to_care_level": "residential"})
        )
        .is_err());
        assert!(clean("templates", &json!({"applies_to_care_level": "IOP"})).is_err());
        let err = clean("templates", &json!({"required_for_stage": "true"})).unwrap_err();
        assert!(err.message.contains("required_for_stage"));
    }
}
