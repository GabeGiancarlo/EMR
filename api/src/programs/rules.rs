//! Shared program policies and source-aware required forms.
use crate::{access::Actor, records, ApiError, Result};
use chrono::{Datelike, NaiveDate};
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn program(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    id: Uuid,
    lock: bool,
) -> Result<Value> {
    let sql = format!(
        "SELECT to_jsonb(p) FROM care_programs p WHERE id=$1 AND organization_id=$2 {}",
        if lock { "FOR UPDATE" } else { "" }
    );
    let mut row: Value = sqlx::query_scalar(&sql)
        .bind(id)
        .bind(actor.org)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::missing)?;
    for (stage, key) in [
        ("intake", "required_intake_template_ids"),
        ("discharge", "required_discharge_template_ids"),
    ] {
        row[key] = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(template_id ORDER BY template_id),'[]'::jsonb) FROM program_required_forms WHERE organization_id=$1 AND program_id=$2 AND stage=$3").bind(actor.org).bind(id).bind(stage).fetch_one(&mut **tx).await?;
    }
    Ok(row)
}

pub(crate) fn completed_age(birth: NaiveDate, today: NaiveDate) -> i32 {
    today.year()
        - birth.year()
        - i32::from((today.month(), today.day()) < (birth.month(), birth.day()))
}
pub(crate) fn age_matches(program: &Value, birth: NaiveDate, today: NaiveDate) -> bool {
    let age = completed_age(birth, today) as i64;
    program["min_age"].as_i64().is_some_and(|min| age >= min)
        && (program["max_age"].is_null()
            || program["max_age"].as_i64().is_some_and(|max| age <= max))
}
pub(crate) async fn context(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    program: &Value,
) -> Result<(Value, NaiveDate)> {
    let facility = records::get(
        tx,
        "facilities",
        records::uuid(program, "facility_id")?,
        actor.org,
        false,
    )
    .await?;
    let today = sqlx::query_scalar("SELECT (CURRENT_TIMESTAMP AT TIME ZONE $1)::date")
        .bind(facility["timezone"].as_str().unwrap_or("UTC"))
        .fetch_one(&mut **tx)
        .await?;
    Ok((facility, today))
}
pub(crate) async fn require_available(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    program: &Value,
    client: &Value,
) -> Result<()> {
    let (facility, today) = context(tx, actor, program).await?;
    if program["active"] != true || facility["active"] != true {
        return Err(ApiError::conflict(
            "Choose an active program at an active facility.",
        ));
    }
    if !age_matches(
        program,
        records::parse_record_date(client, "birth_date")?,
        today,
    ) {
        return Err(ApiError::invalid(
            "The recorded age does not meet this program's configured age range.",
        ));
    }
    Ok(())
}
pub(crate) async fn active_census(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    program_id: Uuid,
    exclude_episode: Option<Uuid>,
) -> Result<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM episodes WHERE organization_id=$1 AND program_id=$2 AND status='active' AND ($3::uuid IS NULL OR id<>$3)").bind(actor.org).bind(program_id).bind(exclude_episode).fetch_one(&mut **tx).await?)
}
fn item(code: &str, title: &str, required: bool, passed: bool, detail: impl Into<String>) -> Value {
    json!({"code":code,"title":title,"required":required,"passed":passed,"detail":detail.into()})
}
pub(crate) async fn checklist(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: &Value,
    episode: &Value,
) -> Result<Vec<Value>> {
    let Some(id) = episode["program_id"]
        .as_str()
        .and_then(|v| Uuid::parse_str(v).ok())
    else {
        return Ok(vec![item(
            "program_assignment",
            "Care program",
            true,
            false,
            "Assign this older episode to a configured care program before continuing intake.",
        )]);
    };
    let program = program(tx, actor, id, false).await?;
    let (facility, today) = context(tx, actor, &program).await?;
    let age_ok = age_matches(
        &program,
        records::parse_record_date(client, "birth_date")?,
        today,
    );
    let used = active_census(tx, actor, id, Some(records::uuid(episode, "id")?)).await?;
    let capacity = program["capacity"].as_i64().unwrap_or(0);
    let matching = episode["facility_id"] == program["facility_id"]
        && episode["level_of_care"] == program["level_of_care"];
    Ok(vec![
        item("program_assignment","Care program",true,matching,if matching { format!("{} · version {}",program["name"].as_str().unwrap_or("Program"),program["version"]) } else { "Program and episode context do not match.".into() }),
        item("program_active","Program accepting admissions",true,program["active"]==true && facility["active"]==true,"The program and its facility must be active for admission."),
        item("program_age_range","Program age range",true,age_ok,if age_ok { "The recorded age is within this program's configured range." } else { "Review the patient's date of birth and this program's configured age range." }),
        item("admission_program_capacity","Program capacity",false,used<capacity,format!("{used} other active episodes of {capacity} places. Planned intakes do not reserve a place.")),
    ])
}
pub(crate) async fn required_forms(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    episode: &Value,
    stage: &str,
) -> Result<Vec<Value>> {
    if !matches!(stage, "intake" | "discharge") {
        return Err(ApiError::invalid("Unknown program workflow stage"));
    }
    let program_id = episode["program_id"]
        .as_str()
        .and_then(|v| Uuid::parse_str(v).ok());
    let global_stage = if stage == "discharge" {
        "discharged"
    } else {
        "intake"
    };
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('id',t.id,'name',t.name,'active',t.active,'program_required',EXISTS(SELECT 1 FROM program_required_forms r WHERE r.organization_id=t.organization_id AND r.program_id=$2 AND r.template_id=t.id AND r.stage=$3)) FROM templates t WHERE t.organization_id=$1 AND ((t.active AND t.required_for_stage AND t.applies_to_stage IN ('any',$4) AND t.applies_to_care_level IN ('any',$5)) OR EXISTS(SELECT 1 FROM program_required_forms r WHERE r.organization_id=t.organization_id AND r.program_id=$2 AND r.template_id=t.id AND r.stage=$3)) ORDER BY t.name,t.id").bind(actor.org).bind(program_id).bind(stage).bind(global_stage).bind(episode["level_of_care"].as_str().unwrap_or_default()).fetch_all(&mut **tx).await?)
}
pub(crate) async fn form_checks(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    episode: &Value,
    stage: &str,
) -> Result<Vec<Value>> {
    let mut result = Vec::new();
    for template in required_forms(tx, actor, episode, stage).await? {
        let template_id = records::uuid(&template, "id")?;
        let rows:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(f) FROM form_responses f WHERE organization_id=$1 AND client_id=$2 AND episode_id=$3 AND template_id=$4 AND status='signed'").bind(actor.org).bind(records::uuid(episode,"client_id")?).bind(records::uuid(episode,"id")?).bind(template_id).fetch_all(&mut **tx).await?;
        let mut passed = false;
        let mut invalid = false;
        if template["active"] == true {
            for row in rows {
                match crate::telehealth::recording::transcripts::excerpts::validate_form(
                    tx, actor, &row,
                )
                .await
                {
                    Ok(()) => {
                        passed = true;
                        break;
                    }
                    Err(error)
                        if matches!(
                            error.status,
                            actix_web::http::StatusCode::FORBIDDEN
                                | actix_web::http::StatusCode::NOT_FOUND
                                | actix_web::http::StatusCode::CONFLICT
                        ) =>
                    {
                        invalid = true
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        result.push(item(&format!("{stage}_form:{template_id}"),template["name"].as_str().unwrap_or("Required form"),true,passed,
            if template["active"]!=true {"This program still requires an inactive template. Ask an administrator to update the program rule."}
            else if passed {"Signed for this care episode."}
            else if invalid {"The signed form needs a source review. Open its history before continuing."}
            else {"Complete and sign this required form for this care episode."}));
    }
    Ok(result)
}
pub(crate) async fn require_discharge_forms(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    episode: &Value,
) -> Result<()> {
    // Old unassigned episodes retain their pre-migration discharge route.
    if episode["program_id"].is_null() {
        return Ok(());
    }
    let checks = form_checks(tx, actor, episode, "discharge").await?;
    let missing = checks.iter().filter(|c| c["passed"] != true).count();
    if missing > 0 {
        return Err(ApiError::conflict(format!("Review the {missing} incomplete discharge form requirements for this care episode before discharge.")));
    }
    Ok(())
}
pub(crate) async fn record_decision(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    case_id: Uuid,
    event_id: Uuid,
    episode: &Value,
    checklist: &[Value],
) -> Result<()> {
    let program_id = records::uuid(episode, "program_id")?;
    let snapshot = program(tx, actor, program_id, false).await?;
    sqlx::query("INSERT INTO intake_decision_evidence(id,organization_id,intake_case_id,event_id,client_id,episode_id,program_id,program_version,program_snapshot,checklist) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").bind(Uuid::new_v4()).bind(actor.org).bind(case_id).bind(event_id).bind(records::uuid(episode,"client_id")?).bind(records::uuid(episode,"id")?).bind(program_id).bind(snapshot["version"].as_i64().unwrap_or(0) as i32).bind(snapshot).bind(json!(checklist)).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_age_ranges_use_calendar_anniversaries() {
        let birth = NaiveDate::from_ymd_opt(2000, 2, 29).unwrap();
        let p = json!({"min_age":21,"max_age":25});
        assert!(!age_matches(
            &p,
            birth,
            NaiveDate::from_ymd_opt(2021, 2, 28).unwrap()
        ));
        assert!(age_matches(
            &p,
            birth,
            NaiveDate::from_ymd_opt(2021, 3, 1).unwrap()
        ));
        assert!(age_matches(
            &p,
            birth,
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        ));
        assert!(!age_matches(
            &p,
            birth,
            NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()
        ));
    }
}
