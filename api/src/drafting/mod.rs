//! Local model proposals remain separate from authored, reviewed clinical notes.
pub(crate) mod worker;
use crate::{
    access::{self, Actor},
    clinical_model::{self, DraftInput, DraftOutput, DraftSource},
    knowledge::{self, KnowledgeChunk},
    records, ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/clients/{id}/clinical-drafts", web::get().to(list))
        .route("/clients/{id}/clinical-drafts", web::post().to(enqueue))
        .route("/clinical-drafts/{id}", web::get().to(read))
        .route("/clinical-drafts/{id}/cancel", web::post().to(cancel))
        .route("/clinical-drafts/{id}/review", web::post().to(review));
}
fn uid(v: &Value, key: &str) -> Result<Uuid> {
    records::uuid(v, key)
}
fn bounded_text(text: &str, min: usize, max: usize) -> bool {
    !text.contains('\0') && (min..=max).contains(&text.trim().chars().count())
}
fn response(v: Value) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(v)
}
async fn job(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Value> {
    sqlx::query_scalar("SELECT to_jsonb(j) FROM clinical_draft_jobs j WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::missing)
}
async fn scoped(
    req: &HttpRequest,
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(Actor, Value)> {
    let initial = access::actor(req, &state.pool).await?;
    let client: Uuid = sqlx::query_scalar(
        "SELECT client_id FROM clinical_draft_jobs WHERE id=$1 AND organization_id=$2",
    )
    .bind(id)
    .bind(initial.org)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::missing)?;
    let actor = knowledge::authorize(req, tx, initial.org, client).await?;
    Ok((actor, job(tx, id).await?))
}
async fn context(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: Uuid,
    episode: Option<Uuid>,
    session: Option<Uuid>,
) -> Result<()> {
    if let Some(ep) = episode {
        let row = records::get(tx, "episodes", ep, actor.org, false).await?;
        if row["client_id"] != client.to_string() {
            return Err(ApiError::denied());
        }
    }
    if let Some(s) = session {
        let row = records::get(tx, "sessions", s, actor.org, false).await?;
        if row["client_id"] != client.to_string()
            || episode.is_some_and(|ep| row["episode_id"] != ep.to_string())
        {
            return Err(ApiError::invalid(
                "The session must belong to this patient and selected episode",
            ));
        }
    }
    Ok(())
}
fn optional_id(v: &Value, key: &str) -> Result<Option<Uuid>> {
    if v[key].is_null() {
        Ok(None)
    } else {
        Ok(Some(uid(v, key)?))
    }
}
pub(super) async fn current_sources(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    row: &Value,
) -> Result<Vec<KnowledgeChunk>> {
    let saved: Vec<KnowledgeChunk> = serde_json::from_value(row["sources"].clone())
        .map_err(|_| ApiError::conflict("The saved generation sources are invalid"))?;
    let current=knowledge::resolve(tx,actor,uid(row,"client_id")?,&saved.iter().map(|s|s.id).collect::<Vec<_>>()).await.map_err(|e|if e.status==actix_web::http::StatusCode::NOT_FOUND{ApiError::conflict("A selected source is no longer current. Select current evidence and request a new draft.")}else{e})?;
    if saved.len() != current.len()
        || saved.iter().any(|s| {
            !current.iter().any(|c| {
                c.id == s.id
                    && c.source_hash == s.source_hash
                    && c.source_version == s.source_version
                    && c.text == s.text
            })
        })
    {
        return Err(ApiError::conflict("A selected source changed or is no longer available. Select current evidence and request a new draft."));
    }
    context(
        tx,
        actor,
        uid(row, "client_id")?,
        optional_id(row, "episode_id")?,
        optional_id(row, "session_id")?,
    )
    .await?;
    Ok(current)
}
pub(crate) async fn validate_note(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    lineage: &crate::note_lineage::NoteLineage,
) -> Result<()> {
    let note = &lineage.row;
    if note["clinical_draft_job_id"].is_null() {
        return Ok(());
    }
    let row = job(tx, uid(note, "clinical_draft_job_id")?).await?;
    if row["organization_id"] != actor.org.to_string() || row["client_id"] != note["client_id"] {
        return Err(ApiError::denied());
    }
    current_sources(tx, actor, &row).await?;
    let reviewed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM clinical_draft_reviews WHERE job_id=$1 AND decision='accept' AND note_id=ANY($2) AND organization_id=$3 AND client_id=$4)",
    )
    .bind(uid(&row, "id")?)
    .bind(&lineage.ids)
    .bind(actor.org)
    .bind(uid(note, "client_id")?)
    .fetch_one(&mut **tx)
    .await?;
    if !reviewed {
        return Err(ApiError::conflict(
            "Review the model proposal before signing its note",
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enqueue {
    template_id: Uuid,
    source_ids: Vec<Uuid>,
    episode_id: Option<Uuid>,
    session_id: Option<Uuid>,
    title: String,
    purpose: String,
}
async fn enqueue(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Json<Enqueue>,
) -> Result<HttpResponse> {
    let client = id.into_inner();
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = knowledge::authorize(&req, &mut tx, initial.org, client).await?;
    if !bounded_text(&body.title, 1, 200)
        || !bounded_text(&body.purpose, 10, 2000)
        || body.source_ids.is_empty()
        || body.source_ids.len() > 12
        || body.source_ids.iter().collect::<HashSet<_>>().len() != body.source_ids.len()
    {
        return Err(ApiError::invalid(
            "Provide a title, a 10–2000 character purpose and 1–12 evidence excerpts",
        ));
    }
    context(&mut tx, &actor, client, body.episode_id, body.session_id).await?;
    let template = records::get(&mut tx, "templates", body.template_id, actor.org, false).await?;
    if template["active"] != true || template["fields"].as_array().is_none_or(|f| f.is_empty()) {
        return Err(ApiError::invalid(
            "Choose an active template with defined fields",
        ));
    }
    let sources = knowledge::resolve(&mut tx, &actor, client, &body.source_ids).await?;
    if serde_json::to_vec(&sources)
        .map_err(|_| ApiError::invalid("Invalid evidence"))?
        .len()
        > 60000
    {
        return Err(ApiError::invalid(
            "Select fewer evidence excerpts for one draft",
        ));
    }
    clinical_model::validate_input(&DraftInput {
        purpose: body.purpose.clone(),
        template: template.clone(),
        sources: sources
            .iter()
            .map(|s| DraftSource {
                id: s.id,
                title: s.title.clone(),
                text: s.text.clone(),
                source_date: s.source_date.clone(),
            })
            .collect(),
    })?;
    if !clinical_model::configured() {
        return Err(ApiError::conflict(
            "Start the configured local drafting model before requesting a draft",
        ));
    }
    let row:Value=sqlx::query_scalar("INSERT INTO clinical_draft_jobs(id,organization_id,client_id,episode_id,session_id,requested_by,template_id,title,purpose,template_snapshot,sources) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING to_jsonb(clinical_draft_jobs)").bind(Uuid::new_v4()).bind(actor.org).bind(client).bind(body.episode_id).bind(body.session_id).bind(actor.id).bind(body.template_id).bind(body.title.trim()).bind(body.purpose.trim()).bind(template).bind(json!(sources)).fetch_one(&mut *tx).await?;
    access::audit(
        &mut tx,
        &actor,
        "request_generation",
        "clinical_draft_jobs",
        Some(uid(&row, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(HttpResponse::Accepted()
        .insert_header(("Cache-Control", "no-store"))
        .json(row))
}
async fn list(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let client = id.into_inner();
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = knowledge::authorize(&req, &mut tx, initial.org, client).await?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT (to_jsonb(j)-ARRAY['sources','template_snapshot','output']) || jsonb_build_object('review',(SELECT to_jsonb(r) FROM clinical_draft_reviews r WHERE r.job_id=j.id)) FROM clinical_draft_jobs j WHERE j.organization_id=$1 AND j.client_id=$2 ORDER BY j.created_at DESC,j.id LIMIT 50").bind(actor.org).bind(client).fetch_all(&mut *tx).await?;
    access::audit(&mut tx, &actor, "read", "clinical_draft_jobs", Some(client)).await?;
    tx.commit().await?;
    Ok(response(
        json!({"configured":clinical_model::configured(),"jobs":rows}),
    ))
}
async fn read(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, mut row) = scoped(&req, &state, &mut tx, id.into_inner()).await?;
    let review: Option<Value> =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM clinical_draft_reviews r WHERE job_id=$1")
            .bind(uid(&row, "id")?)
            .fetch_optional(&mut *tx)
            .await?;
    row["review"] = json!(review);
    access::audit(
        &mut tx,
        &actor,
        "read",
        "clinical_draft_jobs",
        Some(uid(&row, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(response(row))
}
async fn cancel(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, row) = scoped(&req, &state, &mut tx, id.into_inner()).await?;
    let out:Value=sqlx::query_scalar("UPDATE clinical_draft_jobs SET state='cancelled',error_code='cancelled_by_operator',completed_at=clock_timestamp() WHERE id=$1 AND state IN ('queued','running') RETURNING to_jsonb(clinical_draft_jobs)").bind(uid(&row,"id")?).fetch_optional(&mut *tx).await?.ok_or_else(||ApiError::conflict("Only queued or running drafts can be cancelled"))?;
    access::audit(
        &mut tx,
        &actor,
        "cancel_generation",
        "clinical_draft_jobs",
        Some(uid(&row, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(response(out))
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    section_key: String,
    index: usize,
    decision: String,
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    decision: String,
    reason: String,
    attested: bool,
    decisions: Vec<Decision>,
}
fn reviewed_body(output: &DraftOutput, decisions: &[Decision]) -> Result<String> {
    let expected: usize = output.sections.iter().map(|s| s.statements.len()).sum();
    let mut keys = HashSet::new();
    if decisions.len() != expected {
        return Err(ApiError::invalid(
            "Record a decision for every proposed statement",
        ));
    }
    for d in decisions {
        if !keys.insert((&d.section_key, d.index))
            || !matches!(d.decision.as_str(), "accept" | "exclude")
            || !output
                .sections
                .iter()
                .any(|s| s.key == d.section_key && d.index < s.statements.len())
            || !bounded_text(
                &d.reason,
                if d.decision == "exclude" { 10 } else { 0 },
                2000,
            )
        {
            return Err(ApiError::invalid(
                "Each statement needs one valid decision; explain exclusions in 10–2000 characters",
            ));
        }
    }
    let mut sections = Vec::new();
    let mut count = 0;
    for s in &output.sections {
        let selected: Vec<&str> = s
            .statements
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                decisions
                    .iter()
                    .any(|d| d.section_key == s.key && d.index == *i && d.decision == "accept")
            })
            .map(|(_, s)| s.text.as_str())
            .collect();
        count += selected.len();
        if !selected.is_empty() {
            sections.push(format!("{}\n{}", s.label, selected.join("\n")));
        }
    }
    if count == 0 {
        return Err(ApiError::invalid(
            "Accept at least one supported statement or reject the proposal",
        ));
    }
    let body = sections.join("\n\n");
    if body.len() > 100000 {
        return Err(ApiError::invalid(
            "The reviewed note exceeds the text limit",
        ));
    }
    Ok(body)
}
async fn review(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Json<Review>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, row) = scoped(&req, &state, &mut tx, id.into_inner()).await?;
    if row["state"] != "ready"
        || !body.attested
        || !bounded_text(&body.reason, 10, 2000)
        || !matches!(body.decision.as_str(), "accept" | "reject")
    {
        return Err(ApiError::invalid(
            "Explicitly review a ready proposal and explain your decision",
        ));
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM clinical_draft_reviews WHERE job_id=$1)")
            .bind(uid(&row, "id")?)
            .fetch_one(&mut *tx)
            .await?;
    if exists {
        return Err(ApiError::conflict(
            "This proposal already has a recorded review",
        ));
    }
    let note = if body.decision == "accept" {
        current_sources(&mut tx, &actor, &row).await?;
        let output: DraftOutput = serde_json::from_value(row["output"].clone())
            .map_err(|_| ApiError::conflict("Invalid saved model result"))?;
        let narrative = reviewed_body(&output, &body.decisions)?;
        let note=records::insert(&mut tx,"notes",json!({"client_id":row["client_id"],"session_id":row["session_id"],"template_id":row["template_id"],"author_id":actor.id,"title":row["title"],"body":narrative,"clinical_draft_job_id":row["id"]}).as_object().unwrap().clone(),&actor).await?;
        access::audit(
            &mut tx,
            &actor,
            "create_from_review",
            "notes",
            Some(uid(&note, "id")?),
        )
        .await?;
        Some(note)
    } else {
        if !body.decisions.is_empty() {
            return Err(ApiError::invalid(
                "A rejected proposal records its reason without accepting statements",
            ));
        }
        None
    };
    let review:Value=sqlx::query_scalar("INSERT INTO clinical_draft_reviews(id,organization_id,client_id,job_id,reviewer_id,decision,reason,decisions,note_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING to_jsonb(clinical_draft_reviews)").bind(Uuid::new_v4()).bind(actor.org).bind(uid(&row,"client_id")?).bind(uid(&row,"id")?).bind(actor.id).bind(&body.decision).bind(body.reason.trim()).bind(json!(body.decisions)).bind(note.as_ref().map(|n|uid(n,"id")).transpose()?).fetch_one(&mut *tx).await?;
    access::audit(
        &mut tx,
        &actor,
        "review_generation",
        "clinical_draft_jobs",
        Some(uid(&row, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(response(json!({"review":review,"note":note})))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn proposal() -> DraftOutput {
        serde_json::from_value(json!({
            "model":"synthetic-fixture", "model_sha256":"a".repeat(64),"prompt_version":"fixture",
            "sections":[{"key":"history","label":"History","statements":[
                {"text":"A historical observation.","citations":[{"source_id":Uuid::new_v4(),"quote":"historical"}]},
                {"text":"Another proposed observation.","citations":[{"source_id":Uuid::new_v4(),"quote":"observation"}]}
            ]}],"unresolved":["Current status is not established."]
        })).unwrap()
    }
    fn decision(index: usize, value: &str, reason: &str) -> Decision {
        Decision {
            section_key: "history".into(),
            index,
            decision: value.into(),
            reason: reason.into(),
        }
    }
    #[test]
    fn review_requires_exactly_one_decision_per_statement_and_explanation_for_exclusions() {
        let output = proposal();
        assert!(reviewed_body(&output, &[decision(0, "accept", "")]).is_err());
        assert!(reviewed_body(
            &output,
            &[decision(0, "accept", ""), decision(0, "accept", "")]
        )
        .is_err());
        assert!(reviewed_body(
            &output,
            &[decision(0, "accept", ""), decision(2, "accept", "")]
        )
        .is_err());
        assert!(reviewed_body(
            &output,
            &[decision(0, "accept", ""), decision(1, "exclude", "no")]
        )
        .is_err());
        assert!(reviewed_body(
            &output,
            &[
                decision(0, "exclude", "Not supported now"),
                decision(1, "exclude", "Not supported now")
            ]
        )
        .is_err());
        let body = reviewed_body(
            &output,
            &[
                decision(0, "accept", ""),
                decision(1, "exclude", "Unsupported statement excluded"),
            ],
        )
        .unwrap();
        assert_eq!(body, "History\nA historical observation.");
        assert!(!body.contains("Current status"));
    }
    #[test]
    fn review_text_bounds_count_characters_and_reject_null_bytes() {
        assert!(bounded_text("A valid explanation", 10, 2000));
        assert!(!bounded_text("\0A valid explanation", 10, 2000));
        assert!(!bounded_text("    ", 1, 200));
        assert!(bounded_text("é", 1, 1));
        assert!(!bounded_text(&"x".repeat(2001), 10, 2000));
    }
}
