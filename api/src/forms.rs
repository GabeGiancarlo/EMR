//! Typed user-authored forms with immutable template snapshots and answer provenance.
use crate::telehealth::recording::transcripts::excerpts;
use crate::{access::Actor, records, ApiError, Result};
use chrono::{NaiveDate, Utc};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};

fn object(value: Value) -> Map<String, Value> {
    value.as_object().expect("internal object").clone()
}
pub fn validate(value: &Value, required: bool) -> Result<()> {
    let fields = value["template_snapshot"]["fields"]
        .as_array()
        .ok_or_else(|| ApiError::invalid("The form template has no field definition"))?;
    let answers = value["answers"]
        .as_object()
        .ok_or_else(|| ApiError::invalid("Answers must be an object keyed by field name"))?;
    for key in answers.keys() {
        if !fields.iter().any(|f| f["key"] == *key) {
            return Err(ApiError::invalid(format!("Unknown form field: {key}")));
        }
    }
    for field in fields {
        let key = records::text(field, "key")?;
        let label = records::text(field, "label")?;
        let answer = answers.get(key).unwrap_or(&Value::Null);
        let empty = answer.is_null()
            || answer
                .as_str()
                .map(|s| s.trim().is_empty())
                .unwrap_or(false);
        if empty {
            if required && field["required"] == true {
                return Err(ApiError::invalid(format!(
                    "Complete the required field: {label}"
                )));
            }
            continue;
        }
        let valid = match field["type"].as_str().unwrap_or_default() {
            "text" | "textarea" => answer.as_str().map(|s| s.len() <= 100_000).unwrap_or(false),
            "number" => answer.is_number(),
            "checkbox" => answer.is_boolean(),
            "date" => answer
                .as_str()
                .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok())
                .unwrap_or(false),
            "select" => {
                answer.is_string()
                    && field["options"]
                        .as_array()
                        .map(|options| options.contains(answer))
                        .unwrap_or(false)
            }
            _ => false,
        };
        if !valid {
            return Err(ApiError::invalid(format!(
                "Invalid answer type or option for {label}"
            )));
        }
    }
    let provenance = value["field_provenance"]
        .as_object()
        .ok_or_else(|| ApiError::invalid("Field provenance must be an object"))?;
    let source = value["source_excerpt"]
        .as_str()
        .ok_or_else(|| ApiError::invalid("Source excerpt must be text"))?;
    for (key, entry) in provenance {
        if !answers.contains_key(key) {
            return Err(ApiError::invalid(
                "A provenance entry must refer to an answered field",
            ));
        }
        let excerpt = records::text(entry, "excerpt")?;
        if !source.contains(excerpt) {
            return Err(ApiError::invalid(format!(
                "The evidence excerpt for {key} does not occur in the supplied source"
            )));
        }
    }
    Ok(())
}
pub async fn prepare_create(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &mut Map<String, Value>,
) -> Result<()> {
    excerpts::bind_form(tx, actor, data, None).await?;
    let value = Value::Object(data.clone());
    let template = records::get(
        tx,
        "templates",
        records::uuid(&value, "template_id")?,
        actor.org,
        true,
    )
    .await?;
    if template["active"] != true {
        return Err(ApiError::conflict("Choose an active form template"));
    }
    if template["fields"]
        .as_array()
        .map(|a| a.is_empty())
        .unwrap_or(true)
    {
        return Err(ApiError::invalid(
            "Add fields to the template before completing a response",
        ));
    }
    data.insert("template_snapshot".into(), template);
    data.insert("author_id".into(), json!(actor.id));
    for (key, default) in [
        ("answers", json!({})),
        ("field_provenance", json!({})),
        ("source_type", json!("manual")),
        ("source_excerpt", json!("")),
    ] {
        data.entry(key.to_owned()).or_insert(default);
    }
    validate(&Value::Object(data.clone()), false)
}
pub async fn before_update(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    before: &Value,
    data: &mut Map<String, Value>,
) -> Result<()> {
    if before["status"] == "signed" {
        return Err(ApiError::conflict(
            "Signed responses are immutable; create an amendment",
        ));
    }
    if before["author_id"] != actor.id.to_string()
        && !matches!(actor.role.as_str(), "admin" | "clinical_lead")
    {
        return Err(ApiError::denied());
    }
    for key in ["template_id", "client_id", "episode_id", "session_id"] {
        if data.get(key).map(|v| v != &before[key]).unwrap_or(false) {
            return Err(ApiError::conflict(
                "Form references are fixed to preserve clinical context",
            ));
        }
    }
    excerpts::bind_form(tx, actor, data, Some(before)).await?;
    let mut combined = before.clone();
    for (key, value) in data.iter() {
        combined[key] = value.clone();
    }
    validate(&combined, false)?;
    data.insert("status".into(), json!("draft"));
    data.insert("reviewer_id".into(), Value::Null);
    data.insert("reviewed_at".into(), Value::Null);
    Ok(())
}
pub async fn perform(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    action: &str,
    old: &Value,
    body: &Value,
) -> Result<Value> {
    let id = records::uuid(old, "id")?;
    match action {
        "review" => {
            if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
                return Err(ApiError::denied());
            }
            if old["status"] != "draft" {
                return Err(ApiError::conflict("Only drafts can enter review"));
            }
            excerpts::validate_form(tx, actor, old).await?;
            validate(old, true)?;
            records::patch(
                tx,
                "form_responses",
                id,
                actor.org,
                object(
                    json!({"status":"reviewed","reviewer_id":actor.id,"reviewed_at":Utc::now()}),
                ),
            )
            .await
        }
        "sign" => {
            if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician")
                || (old["author_id"] != actor.id.to_string()
                    && !matches!(actor.role.as_str(), "admin" | "clinical_lead"))
            {
                return Err(ApiError::denied());
            }
            if old["status"] != "reviewed" {
                return Err(ApiError::conflict("Review the form before signing"));
            }
            excerpts::validate_form(tx, actor, old).await?;
            validate(old, true)?;
            records::patch(
                tx,
                "form_responses",
                id,
                actor.org,
                object(json!({"status":"signed","signed_by":actor.id,"signed_at":Utc::now()})),
            )
            .await
        }
        "amend" => {
            if old["status"] != "signed" {
                return Err(ApiError::conflict("Only signed forms require an amendment"));
            }
            let reason = records::text(body, "reason")?;
            if reason.len() < 10 {
                return Err(ApiError::invalid(
                    "Explain the amendment in at least 10 characters",
                ));
            }
            let mut data = Map::new();
            for key in [
                "client_id",
                "episode_id",
                "session_id",
                "template_id",
                "template_snapshot",
                "answers",
                "source_type",
                "source_excerpt",
                "field_provenance",
                "transcript_excerpt_id",
            ] {
                data.insert(key.into(), old[key].clone());
            }
            data.insert("author_id".into(), json!(actor.id));
            data.insert("amends_response_id".into(), json!(id));
            data.insert("amendment_reason".into(), json!(reason));
            records::insert(tx, "form_responses", data, actor).await
        }
        "create-note" => {
            if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
                return Err(ApiError::denied());
            }
            if old["status"] != "signed" {
                return Err(ApiError::conflict(
                    "Sign the reviewed form before creating its clinical note",
                ));
            }
            excerpts::validate_form(tx, actor, old).await?;
            let fields = old["template_snapshot"]["fields"]
                .as_array()
                .ok_or_else(|| ApiError::invalid("Invalid form snapshot"))?;
            let mut lines = Vec::new();
            for field in fields {
                let key = records::text(field, "key")?;
                if let Some(answer) = old["answers"].get(key).filter(|a| !a.is_null()) {
                    let rendered = answer
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| answer.to_string());
                    lines.push(format!("{}: {}", records::text(field, "label")?, rendered));
                }
            }
            records::insert(tx,"notes",object(json!({"client_id":old["client_id"],"session_id":old["session_id"],"template_id":old["template_id"],"author_id":actor.id,"title":records::text(body,"title")?,"body":lines.join("\n\n"),"form_response_id":id})),actor).await
        }
        _ => Err(ApiError::missing()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn response() -> Value {
        json!({"template_snapshot":{"fields":[{"key":"mood","label":"Mood","type":"select","options":["stable","variable"],"required":true},{"key":"count","label":"Count","type":"number"}]},"answers":{},"field_provenance":{},"source_excerpt":"Mood: stable"})
    }
    #[test]
    fn drafts_allow_incomplete_but_reviews_do_not() {
        let value = response();
        assert!(validate(&value, false).is_ok());
        assert!(validate(&value, true).is_err());
    }
    #[test]
    fn rejects_unknown_fields_wrong_types_and_unlisted_options() {
        let mut value = response();
        value["answers"] = json!({"mood":"invented"});
        assert!(validate(&value, true).is_err());
        value["answers"] = json!({"mood":"stable","count":"three"});
        assert!(validate(&value, true).is_err());
        value["answers"] = json!({"mood":"stable","unknown":"x"});
        assert!(validate(&value, true).is_err());
    }
    #[test]
    fn verifies_provenance_against_actual_excerpt() {
        let mut value = response();
        value["answers"] = json!({"mood":"stable"});
        value["field_provenance"] = json!({"mood":{"excerpt":"Mood: unstable"}});
        assert!(validate(&value, true).is_err());
        value["field_provenance"] = json!({"mood":{"excerpt":"Mood: stable"}});
        assert!(validate(&value, true).is_ok());
    }
}
