//! Explicit assignment of pre-existing care without fabricating a new admission.
use super::rules;
use crate::{access, records, ApiError, AppState, Result};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/episodes/{id}/assign-program", web::post().to(assign));
}

pub(crate) async fn attach(
    tx: &mut Transaction<'_, Postgres>,
    actor: &access::Actor,
    episode: &Value,
    program: &Value,
    reason: &str,
) -> Result<Value> {
    if !episode["program_id"].is_null() {
        return Err(ApiError::conflict(
            "This care episode already has a program. Use a new episode for a different program.",
        ));
    }
    if episode["facility_id"] != program["facility_id"]
        || episode["level_of_care"] != program["level_of_care"]
    {
        return Err(ApiError::invalid(
            "Choose a program matching the episode's existing facility and care level.",
        ));
    }
    let client = records::get(
        tx,
        "clients",
        records::uuid(episode, "client_id")?,
        actor.org,
        false,
    )
    .await?;
    rules::require_available(tx, actor, program, &client).await?;
    let program_id = records::uuid(program, "id")?;
    let episode_id = records::uuid(episode, "id")?;
    if episode["status"] == "active"
        && rules::active_census(tx, actor, program_id, Some(episode_id)).await?
            >= program["capacity"].as_i64().unwrap_or(0)
    {
        return Err(ApiError::conflict(
            "This program has no available places for the existing active episode.",
        ));
    }
    let result = records::patch(
        tx,
        "episodes",
        episode_id,
        actor.org,
        json!({"program_id":program_id})
            .as_object()
            .unwrap()
            .clone(),
    )
    .await?;
    sqlx::query("INSERT INTO episode_program_assignments(id,organization_id,episode_id,client_id,program_id,actor_id,reason,snapshot) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(Uuid::new_v4()).bind(actor.org).bind(episode_id).bind(records::uuid(episode,"client_id")?).bind(program_id).bind(actor.id).bind(reason).bind(program).execute(&mut **tx).await?;
    access::audit(tx, actor, "assign_program", "episodes", Some(episode_id)).await?;
    Ok(result)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Assignment {
    program_id: Uuid,
    #[serde(rename = "_expected_updated_at")]
    expected: Option<DateTime<Utc>>,
    reason: String,
    attested: bool,
}
async fn assign(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<Assignment>,
) -> Result<HttpResponse> {
    let expected = body.expected.ok_or_else(|| {
        ApiError::precondition("Reload the episode before assigning its program.")
    })?;
    let reason = body.reason.trim();
    if !body.attested || !(10..=2000).contains(&reason.chars().count()) || reason.contains('\0') {
        return Err(ApiError::invalid(
            "Confirm the assignment and explain the reason in 10–2000 characters.",
        ));
    }
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, initial.org, "episodes").await?;
    let episode = records::get(&mut tx, "episodes", path.into_inner(), actor.org, true).await?;
    records::scope(&mut tx, &actor, "episodes", &episode).await?;
    if !matches!(episode["status"].as_str(), Some("planned" | "active")) {
        return Err(ApiError::conflict("Closed historical care retains its original program context. Assign a program to planned or active care."));
    }
    let planned = episode["status"] == "planned";
    if !(matches!(actor.role.as_str(), "admin" | "clinical_lead")
        || planned && matches!(actor.role.as_str(), "clinician" | "support"))
    {
        return Err(ApiError::denied());
    }
    let current = DateTime::parse_from_rfc3339(episode["updated_at"].as_str().unwrap_or_default())
        .map_err(|_| ApiError::conflict("Reload the latest episode."))?
        .with_timezone(&Utc);
    if current != expected {
        return Err(ApiError::conflict(
            "This episode changed after you opened it. Reload before assigning a program.",
        ));
    }
    let closed_case:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intake_cases WHERE organization_id=$1 AND episode_id=$2 AND status IN ('declined','withdrawn'))").bind(actor.org).bind(records::uuid(&episode,"id")?).fetch_one(&mut *tx).await?;
    if closed_case {
        return Err(ApiError::conflict("This intake was declined or withdrawn. Start a new intake under the existing patient identity."));
    }
    let program = rules::program(&mut tx, &actor, body.program_id, true).await?;
    let result = attach(&mut tx, &actor, &episode, &program, reason).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(result))
}
