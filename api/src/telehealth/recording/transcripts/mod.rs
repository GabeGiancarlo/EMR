use super::*;
pub(crate) mod excerpts;
mod review;
pub(crate) mod worker;
use crate::AppState;
use serde::Deserialize;
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/telehealth/rooms/{id}/transcripts", web::get().to(index))
        .route(
            "/telehealth/rooms/{id}/transcription-jobs",
            web::post().to(enqueue),
        )
        .route("/transcription-jobs/{id}/cancel", web::post().to(cancel))
        .route("/transcripts/{id}", web::get().to(review::read))
        .route("/transcripts/{id}/versions", web::post().to(review::save))
        .route("/transcripts/{id}/approve", web::post().to(review::approve))
        .route(
            "/transcripts/{id}/excerpts",
            web::post().to(review::excerpt),
        )
        .route(
            "/clients/{id}/transcript-excerpts",
            web::get().to(excerpts::list),
        );
}
fn processing_denied() -> ApiError {
    ApiError{status:actix_web::http::StatusCode::FORBIDDEN,code:"transcription_consent",message:"Every client in this recording needs a current transcription consent. Capture agreement alone does not authorize processing; changed consent requires a new processing request.".into()}
}
async fn processing_consents(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    rid: Uuid,
    expected: Option<&Value>,
) -> Result<Value> {
    let clients:Vec<Uuid>=sqlx::query_scalar("SELECT DISTINCT p.client_id FROM recording_participants m JOIN telehealth_participants p ON p.id=m.participant_id WHERE m.recording_id=$1 AND p.client_id IS NOT NULL ORDER BY p.client_id").bind(rid).fetch_all(&mut **tx).await?;
    if clients.is_empty() {
        return Err(processing_denied());
    }
    let mut snapshots = Vec::new();
    for client in clients {
        let selected = expected
            .and_then(Value::as_array)
            .and_then(|rows| rows.iter().find(|c| c["client_id"] == client.to_string()));
        if expected.is_some() && selected.is_none() {
            return Err(processing_denied());
        }
        let id = selected.map(|c| uid(c, "id")).transpose()?;
        let consent:Option<Value>=sqlx::query_scalar("SELECT to_jsonb(c) FROM consents c JOIN clients p ON p.id=c.client_id JOIN facilities f ON f.id=p.facility_id WHERE c.organization_id=$1 AND c.client_id=$2 AND c.kind='transcription' AND c.revoked_on IS NULL AND c.granted_on<=(clock_timestamp() AT TIME ZONE f.timezone)::date AND (c.expires_on IS NULL OR c.expires_on>=(clock_timestamp() AT TIME ZONE f.timezone)::date) AND ($3::uuid IS NULL OR c.id=$3) ORDER BY c.granted_on DESC,c.id LIMIT 1 FOR UPDATE OF c").bind(uid(room,"organization_id")?).bind(client).bind(id).fetch_optional(&mut **tx).await?;
        let consent = consent.ok_or_else(processing_denied)?;
        if selected.is_some_and(|old| old != &consent) {
            return Err(processing_denied());
        }
        snapshots.push(consent);
    }
    if expected
        .and_then(Value::as_array)
        .is_some_and(|old| old.len() != snapshots.len())
    {
        return Err(processing_denied());
    }
    Ok(json!(snapshots))
}
async fn track(tx: &mut Transaction<'_, Postgres>, room: &Value, id: Uuid) -> Result<Value> {
    let row:Value=sqlx::query_scalar("SELECT to_jsonb(t)-'token_hash' FROM recording_tracks t JOIN recordings r ON r.id=t.recording_id WHERE t.id=$1 AND r.room_id=$2 FOR UPDATE OF t").bind(id).bind(uid(room,"id")?).fetch_optional(&mut **tx).await?.ok_or_else(ApiError::missing)?;
    if row["state"] != "complete" {
        return Err(ApiError::conflict(
            "Finish saving the media segment before transcription",
        ));
    }
    Ok(row)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    track_id: Uuid,
}
pub async fn index(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    q: web::Query<Query>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room) =
        super::super::manager::managed(&req, &state, id.into_inner(), &mut tx).await?;
    track(&mut tx, &room, q.track_id).await?;
    let jobs:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(j)-'consent_snapshot' FROM transcription_jobs j WHERE room_id=$1 AND track_id=$2 ORDER BY created_at DESC,id LIMIT 30").bind(uid(&room,"id")?).bind(q.track_id).fetch_all(&mut *tx).await?;
    let docs:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(t)-'recognized_segments' FROM transcripts t WHERE room_id=$1 AND track_id=$2 ORDER BY created_at DESC,id LIMIT 30").bind(uid(&room,"id")?).bind(q.track_id).fetch_all(&mut *tx).await?;
    access::audit(&mut tx, &actor, "read", "transcripts", Some(q.track_id)).await?;
    tx.commit().await?;
    Ok(no_store(
        json!({"configured":crate::media_transcription::configured(),"jobs":jobs,"transcripts":docs}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enqueue {
    track_id: Uuid,
    purpose: String,
}
pub async fn enqueue(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Json<Enqueue>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room) =
        super::super::manager::managed(&req, &state, id.into_inner(), &mut tx).await?;
    if !(10..=2000).contains(&body.purpose.trim().chars().count()) {
        return Err(ApiError::invalid(
            "Explain the processing purpose in 10–2000 characters",
        ));
    }
    let track = track(&mut tx, &room, body.track_id).await?;
    let consents = processing_consents(&mut tx, &room, uid(&track, "recording_id")?, None).await?;
    if !crate::media_transcription::configured() {
        return Err(ApiError::conflict(
            "Configure the local media decoder and speech recognizer before processing recordings",
        ));
    }
    let row:Value=sqlx::query_scalar("INSERT INTO transcription_jobs(id,organization_id,room_id,recording_id,track_id,requested_by,purpose,consent_snapshot) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING to_jsonb(transcription_jobs)-'consent_snapshot'").bind(Uuid::new_v4()).bind(actor.org).bind(uid(&room,"id")?).bind(uid(&track,"recording_id")?).bind(body.track_id).bind(actor.id).bind(body.purpose.trim()).bind(consents).fetch_one(&mut *tx).await?;
    access::audit(
        &mut tx,
        &actor,
        "request_transcription",
        "recording_tracks",
        Some(body.track_id),
    )
    .await?;
    tx.commit().await?;
    Ok(HttpResponse::Accepted()
        .insert_header(("Cache-Control", "no-store"))
        .json(row))
}
async fn job(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Value> {
    sqlx::query_scalar("SELECT to_jsonb(j) FROM transcription_jobs j WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::missing)
}
pub async fn cancel(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = id.into_inner();
    let mut tx = state.pool.begin().await?;
    let room_id: Uuid = sqlx::query_scalar("SELECT room_id FROM transcription_jobs WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(ApiError::missing)?;
    let (actor, _) = super::super::manager::managed(&req, &state, room_id, &mut tx).await?;
    let row:Value=sqlx::query_scalar("UPDATE transcription_jobs SET state='cancelled',error_code='cancelled_by_operator',completed_at=clock_timestamp() WHERE id=$1 AND state IN ('queued','running') RETURNING to_jsonb(transcription_jobs)-'consent_snapshot'").bind(id).fetch_optional(&mut *tx).await?.ok_or_else(||ApiError::conflict("Only queued or running processing can be cancelled"))?;
    access::audit(&mut tx, &actor, "cancel", "transcription_jobs", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(row))
}
async fn scoped_doc(
    req: &HttpRequest,
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(Actor, Value, Value)> {
    let room_id: Uuid = sqlx::query_scalar("SELECT room_id FROM transcripts WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::missing)?;
    let (actor, room) = super::super::manager::managed(req, state, room_id, tx).await?;
    let doc: Value =
        sqlx::query_scalar("SELECT to_jsonb(t) FROM transcripts t WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
    Ok((actor, room, doc))
}
async fn authorize_processing(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    doc: &Value,
) -> Result<()> {
    let job = job(tx, uid(doc, "job_id")?).await?;
    processing_consents(
        tx,
        room,
        uid(doc, "recording_id")?,
        Some(&job["consent_snapshot"]),
    )
    .await?;
    Ok(())
}
async fn participants_for(tx: &mut Transaction<'_, Postgres>, doc: &Value) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('id',p.id,'label',p.label,'role',CASE WHEN p.staff_id IS NULL THEN 'client' ELSE 'host' END,'client_id',p.client_id) FROM recording_participants m JOIN telehealth_participants p ON p.id=m.participant_id WHERE m.recording_id=$1 ORDER BY p.created_at,p.id").bind(uid(doc,"recording_id")?).fetch_all(&mut **tx).await?)
}
async fn detail(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    doc: &Value,
    selected: Option<Uuid>,
) -> Result<Value> {
    let id = uid(doc, "id")?;
    let selected = selected.unwrap_or(uid(doc, "current_version_id")?);
    let current: Value = sqlx::query_scalar(
        "SELECT to_jsonb(v) FROM transcript_versions v WHERE transcript_id=$1 AND id=$2",
    )
    .bind(id)
    .bind(selected)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::missing)?;
    let versions:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(v)-'segments' FROM transcript_versions v WHERE transcript_id=$1 ORDER BY number DESC").bind(id).fetch_all(&mut **tx).await?;
    let participants = participants_for(tx, doc).await?;
    let clients: Vec<Value> = participants
        .iter()
        .filter(|p| p["client_id"].is_string())
        .map(|p| json!({"id":p["client_id"],"label":p["label"]}))
        .collect();
    let excerpts:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(e) FROM transcript_excerpts e JOIN clients c ON c.id=e.client_id WHERE e.transcript_id=$1 AND e.version_id=$4 AND ($2::boolean OR c.primary_clinician_id=$3) ORDER BY e.created_at DESC LIMIT 100").bind(id).bind(matches!(actor.role.as_str(),"admin"|"clinical_lead")).bind(actor.id).bind(selected).fetch_all(&mut **tx).await?;
    let mut summary = doc.clone();
    summary
        .as_object_mut()
        .unwrap()
        .remove("recognized_segments");
    Ok(
        json!({"transcript":summary,"current_version":current,"versions":versions,"participants":participants,"clients":clients,"excerpts":excerpts}),
    )
}
