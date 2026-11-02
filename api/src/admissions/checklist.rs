//! Live intake checklist. Items describe missing staff actions only.
//! Coverage and task counts are informational and are not electronic verification.
use crate::{access::Actor, records, ApiError, Result};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

fn item(code: &str, title: &str, required: bool, passed: bool, detail: &str) -> Value {
    json!({"code":code,"title":title,"required":required,"passed":passed,"detail":detail})
}
async fn transcript_ok(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    form: &Value,
) -> Result<bool> {
    match crate::telehealth::recording::transcripts::excerpts::validate_form(tx, actor, form).await
    {
        Ok(()) => Ok(true),
        Err(error)
            if matches!(
                error.status,
                actix_web::http::StatusCode::FORBIDDEN
                    | actix_web::http::StatusCode::CONFLICT
                    | actix_web::http::StatusCode::NOT_FOUND
            ) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}
pub(super) async fn evaluate(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: &Value,
    episode: &Value,
) -> Result<Vec<Value>> {
    let facility_id = records::uuid(episode, "facility_id")?;
    let facility = records::get(tx, "facilities", facility_id, actor.org, false).await?;
    let zone = facility["timezone"].as_str().unwrap_or("UTC");
    let today: NaiveDate = sqlx::query_scalar("SELECT (CURRENT_TIMESTAMP AT TIME ZONE $1)::date")
        .bind(zone)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| {
            ApiError::invalid("The intended facility timezone could not supply a local date.")
        })?;
    let adult = records::parse_record_date(client, "birth_date")
        .ok()
        .is_some_and(|birth| records::require_adult_program(birth, today, "birth_date").is_ok());
    let facility_active = facility["active"] == true;
    let primary_id = client["primary_clinician_id"]
        .as_str()
        .and_then(|value| Uuid::parse_str(value).ok());
    let primary = if let Some(id) = primary_id {
        Some(records::get(tx, "staff", id, actor.org, false).await?)
    } else {
        None
    };
    let primary_ok = primary.as_ref().is_some_and(|staff| {
        staff["active"] == true
            && matches!(
                staff["role"].as_str(),
                Some("admin" | "clinical_lead" | "clinician")
            )
    });
    let client_id = records::uuid(client, "id")?;
    let fingerprint = crate::patient_identity::fingerprint(client);
    let latest: Option<Value> = sqlx::query_scalar("SELECT jsonb_build_object('outcome',outcome,'identity_fingerprint',identity_fingerprint) FROM patient_identity_checks WHERE organization_id=$1 AND client_id=$2 ORDER BY created_at DESC, id DESC LIMIT 1").bind(actor.org).bind(client_id).fetch_optional(&mut **tx).await?;
    let identity_ok = latest.as_ref().is_some_and(|row| {
        row["outcome"] == "confirmed" && row["identity_fingerprint"] == fingerprint
    });
    let consent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM consents WHERE organization_id=$1 AND client_id=$2 AND lower(trim(kind))='treatment' AND granted_on<=$3 AND (expires_on IS NULL OR expires_on>=$3) AND (revoked_on IS NULL OR revoked_on>$3))").bind(actor.org).bind(client_id).bind(today).fetch_one(&mut **tx).await?;
    let mut items = vec![
        item(
            "adult_eligibility",
            "Adult program age",
            true,
            adult,
            if adult {
                "Age 18 or older on the facility's local date."
            } else {
                "Review the date of birth. This program accepts adults 18 and older."
            },
        ),
        item(
            "active_facility",
            "Intended facility",
            true,
            facility_active,
            if facility_active {
                "The intended facility is active."
            } else {
                "The intended facility is not active. Choose an active facility before continuing."
            },
        ),
        item(
            "primary_clinician",
            "Assigned primary clinician",
            true,
            primary_ok,
            if primary_ok {
                "An active primary clinician is assigned."
            } else {
                "Assign an active primary clinician before continuing."
            },
        ),
        item(
            "identity_confirmation",
            "Identity attestation",
            true,
            identity_ok,
            if identity_ok {
                "Staff confirmed the current legal name, date of birth, and MRN."
            } else {
                "Review the current legal name, date of birth, and MRN, then record an identity check."
            },
        ),
        item(
            "treatment_consent",
            "Treatment consent",
            true,
            consent,
            if consent {
                "Treatment consent is active today."
            } else {
                "Record treatment consent with dates that cover today."
            },
        ),
    ];
    let level = episode["level_of_care"].as_str().unwrap_or_default();
    let episode_id = records::uuid(episode, "id")?;
    let templates: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('id',id,'name',name) FROM templates WHERE organization_id=$1 AND active AND required_for_stage AND applies_to_stage IN ('any','intake') AND applies_to_care_level IN ('any',$2) ORDER BY name, id").bind(actor.org).bind(level).fetch_all(&mut **tx).await?;
    for template in templates {
        let template_id = records::uuid(&template, "id")?;
        let forms: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(f) FROM form_responses f WHERE organization_id=$1 AND client_id=$2 AND episode_id=$3 AND template_id=$4 AND status='signed'").bind(actor.org).bind(client_id).bind(episode_id).bind(template_id).fetch_all(&mut **tx).await?;
        let mut passed = false;
        let mut invalid = false;
        for form in &forms {
            if transcript_ok(tx, actor, form).await? {
                passed = true;
                break;
            }
            invalid = true;
        }
        let name = template["name"].as_str().unwrap_or("Intake form");
        let detail = if passed {
            "Signed for this planned episode."
        } else if invalid {
            "The signed form needs a source review. Open its history before continuing."
        } else {
            "Complete and sign this required organization form for this planned episode."
        };
        items.push(item(
            &format!("intake_form:{template_id}"),
            name,
            true,
            passed,
            detail,
        ));
    }
    let start = records::parse_record_date(episode, "start_date").ok();
    let start_ok = start.is_some_and(|date| date <= today);
    let planned = episode["status"] == "planned";
    let overlap: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM episodes WHERE organization_id=$1 AND client_id=$2 AND facility_id=$3 AND level_of_care=$4 AND status='active' AND id<>$5)").bind(actor.org).bind(client_id).bind(facility_id).bind(level).bind(episode_id).fetch_one(&mut **tx).await?;
    let role_ok = matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician");
    items.push(item(
        "admission_clinical_role",
        "Admitting role",
        false,
        role_ok,
        "An administrator, clinical lead, or clinician must record admission.",
    ));
    items.push(item(
        "admission_start_date",
        "Intended start date",
        false,
        start_ok,
        "Admission requires the intended start date to be on or before the facility's local date.",
    ));
    items.push(item(
        "admission_no_overlapping_active_episode",
        "Facility and care level",
        false,
        !overlap,
        "Admission requires no other active episode at the same facility and care level.",
    ));
    items.push(item(
        "admission_episode_planned",
        "Planned episode",
        false,
        planned,
        "Admission requires the linked episode to still be planned.",
    ));
    let tasks: i64 = sqlx::query_scalar("SELECT count(*) FROM tasks WHERE organization_id=$1 AND client_id=$2 AND status IN ('open','in_progress')").bind(actor.org).bind(client_id).fetch_one(&mut **tx).await?;
    let coverage: i64 = sqlx::query_scalar("SELECT count(*) FROM policies WHERE organization_id=$1 AND client_id=$2 AND status='active'").bind(actor.org).bind(client_id).fetch_one(&mut **tx).await?;
    items.push(item(
        "open_tasks",
        "Open tasks",
        false,
        false,
        &format!("Open tasks linked to this patient: {tasks}."),
    ));
    items.push(item(
        "coverage_information",
        "Coverage on file",
        false,
        false,
        &format!(
            "Informational only; not electronically verified. Active coverage records: {coverage}."
        ),
    ));
    Ok(items)
}
