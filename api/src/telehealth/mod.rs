//! Encounter-bound local telehealth. Admission is authorization, not identity verification.
mod manager;
pub(crate) mod recording;
mod visitor;

use crate::{
    access::{self, Actor},
    records, ApiError, Result,
};
use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Events one participant may cause in ten minutes. Joining, leaving, clearing the
/// board and each recording step write an append-only event, so without a ceiling an
/// invitation holder could grow that history for as long as the room is open. An
/// ordinary visit causes a handful.
const PARTICIPANT_EVENT_LIMIT: i64 = 60;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.configure(recording::configure);
    cfg.route("/telehealth/rooms", web::post().to(manager::create))
        .route("/telehealth/rooms", web::get().to(manager::list))
        .route("/telehealth/rooms/{id}", web::get().to(manager::read))
        .route(
            "/telehealth/rooms/{id}/invite",
            web::post().to(manager::invite),
        )
        .route("/telehealth/rooms/{id}/host", web::post().to(manager::host))
        .route("/telehealth/rooms/{id}/end", web::post().to(manager::end))
        .route(
            "/telehealth/rooms/{id}/participants/{participant}/{action}",
            web::post().to(manager::admission),
        )
        .route("/telehealth/visit", web::get().to(visitor::inspect))
        .route(
            "/telehealth/visit/{action}",
            web::post().to(visitor::action),
        );
}
pub(super) fn no_store(value: Value) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .insert_header(("Referrer-Policy", "no-referrer"))
        .json(value)
}
pub(super) fn denied() -> ApiError {
    ApiError{status:StatusCode::FORBIDDEN,code:"visit_access",message:"This visit is no longer available with this invitation. Ask the care team for a new invitation.".into()}
}
pub(super) fn fresh_token() -> (String, String) {
    let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let hash = hash_token(&raw);
    (raw, hash)
}
pub(super) fn hash_token(raw: &str) -> String {
    crate::sha256_hex(raw.as_bytes())
}
pub(super) fn uid(row: &Value, key: &str) -> Result<Uuid> {
    records::uuid(row, key)
}
pub(super) fn time(row: &Value, key: &str) -> Option<DateTime<Utc>> {
    row[key]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc))
}
pub(super) fn online(row: &Value) -> bool {
    time(row, "last_seen_at").is_some_and(|t| t > Utc::now() - chrono::Duration::seconds(20))
        && row["connection_id"].is_string()
}
pub(super) fn public_participant(row: &Value) -> Value {
    json!({"id":row["id"],"label":row["label"],"role":if row["staff_id"].is_null(){"client"}else{"host"},"state":row["state"],"connection_id":row["connection_id"],"online":online(row)})
}
pub(super) async fn room(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    org: Option<Uuid>,
) -> Result<Value> {
    let found:Option<Value>=sqlx::query_scalar("SELECT to_jsonb(r) FROM telehealth_rooms r WHERE id=$1 AND ($2::uuid IS NULL OR organization_id=$2) FOR UPDATE").bind(id).bind(org).fetch_optional(&mut **tx).await?;
    found.ok_or_else(ApiError::missing)
}
pub(super) fn live(room: &Value) -> Result<()> {
    if !room["ended_at"].is_null() || time(room, "expires_at").is_none_or(|t| t <= Utc::now()) {
        return Err(denied());
    }
    Ok(())
}
pub(super) async fn encounter(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    session: Option<Uuid>,
    group: Option<Uuid>,
) -> Result<Value> {
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
        return Err(ApiError::denied());
    }
    let (resource, id) = match (session, group) {
        (Some(id), None) => ("sessions", id),
        (None, Some(id)) => ("groups", id),
        _ => {
            return Err(ApiError::invalid(
                "Choose exactly one individual session or shared group",
            ))
        }
    };
    let row = records::get(tx, resource, id, actor.org, false).await?;
    records::scope(tx, actor, resource, &row).await?;
    if actor.role == "clinician" && row["clinician_id"] != actor.id.to_string() {
        return Err(ApiError::denied());
    }
    if resource == "sessions" && !row["group_id"].is_null() {
        return Err(ApiError::invalid(
            "Open telehealth from the shared group encounter",
        ));
    }
    Ok(row)
}
pub(super) async fn manager_scope(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    room: &Value,
) -> Result<Value> {
    encounter(
        tx,
        actor,
        room["session_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok()),
        room["group_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok()),
    )
    .await
}
pub(super) async fn eligible(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    p: &Value,
) -> Result<bool> {
    if time(p, "token_expires_at").is_none_or(|t| t <= Utc::now()) {
        return Ok(false);
    }
    let result:bool=sqlx::query_scalar("SELECT CASE WHEN p.staff_id IS NOT NULL THEN EXISTS(SELECT 1 FROM staff st WHERE st.id=p.staff_id AND st.organization_id=p.organization_id AND st.active AND (st.role IN ('admin','clinical_lead') OR (st.role='clinician' AND (EXISTS(SELECT 1 FROM sessions s JOIN clients c ON c.id=s.client_id WHERE s.id=r.session_id AND s.clinician_id=st.id AND c.primary_clinician_id=st.id) OR EXISTS(SELECT 1 FROM groups g WHERE g.id=r.group_id AND g.clinician_id=st.id))))) ELSE EXISTS(SELECT 1 FROM sessions s JOIN episodes e ON e.id=s.episode_id WHERE s.id=r.session_id AND s.client_id=p.client_id AND e.status='active') OR EXISTS(SELECT 1 FROM group_attendance a JOIN episodes e ON e.id=a.episode_id WHERE a.group_id=r.group_id AND a.client_id=p.client_id AND a.status IN ('scheduled','attended') AND e.status='active') END AND (EXISTS(SELECT 1 FROM sessions s WHERE s.id=r.session_id AND s.status='scheduled') OR EXISTS(SELECT 1 FROM groups g WHERE g.id=r.group_id AND g.status='scheduled')) FROM telehealth_participants p JOIN telehealth_rooms r ON r.id=p.room_id WHERE p.id=$1 AND r.id=$2")
        .bind(uid(p,"id")?).bind(uid(room,"id")?).fetch_one(&mut **tx).await?;
    Ok(result)
}
pub(super) async fn event(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    participant: Option<Uuid>,
    actor: Option<Uuid>,
    action: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO telehealth_events(id,organization_id,room_id,participant_id,actor_id,action) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::new_v4()).bind(uid(room,"organization_id")?).bind(uid(room,"id")?).bind(participant).bind(actor).bind(action).execute(&mut **tx).await?;
    Ok(())
}
/// The bearer credential, if it has the shape of a token this module issues.
pub(super) fn capability(req: &HttpRequest) -> Option<&str> {
    req.headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
/// Run before a visit route reads its request body: the caller must present an
/// invitation that exists. This is one indexed lookup on a pooled connection that is
/// returned before the body is read. It grants nothing; [`authorize`] still makes
/// every check, under the organization lock, once the body has arrived.
pub(super) async fn known_invitation(req: &HttpRequest, pool: &PgPool) -> Result<()> {
    let raw = capability(req).ok_or_else(denied)?;
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM telehealth_participants WHERE token_hash=$1)",
    )
    .bind(hash_token(raw))
    .fetch_one(pool)
    .await?;
    if known {
        Ok(())
    } else {
        Err(denied())
    }
}
/// Refuse a participant who has already caused [`PARTICIPANT_EVENT_LIMIT`] events in
/// the last ten minutes. Call it before writing an event on a participant's behalf.
pub(super) async fn event_budget(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    participant: Uuid,
) -> Result<()> {
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM telehealth_events WHERE participant_id=$1 AND room_id=$2 AND occurred_at>clock_timestamp()-interval '10 minutes'")
        .bind(participant)
        .bind(uid(room, "id")?)
        .fetch_one(&mut **tx)
        .await?;
    if recent >= PARTICIPANT_EVENT_LIMIT {
        return Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "visit_rate",
            message: "Too many visit actions in a short time. Wait a few minutes, then try again."
                .into(),
        });
    }
    Ok(())
}
pub(super) async fn authorize(
    req: &HttpRequest,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(Value, Value)> {
    let raw = capability(req).ok_or_else(denied)?;
    let hash = hash_token(raw);
    let address: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT room_id,organization_id FROM telehealth_participants WHERE token_hash=$1",
    )
    .bind(&hash)
    .fetch_optional(&mut **tx)
    .await?;
    let (room_id, org) = address.ok_or_else(denied)?;
    // Match the organization-before-row order used by staff/encounter mutations.
    // Re-read the capability and eligibility after the lock so revocation cannot
    // commit halfway through an authorized signaling/admission transaction.
    crate::scheduling::lock_org(tx, org).await?;
    let room = room(tx, room_id, Some(org)).await?;
    live(&room)?;
    let p:Value=sqlx::query_scalar("SELECT to_jsonb(p) FROM telehealth_participants p WHERE token_hash=$1 AND token_expires_at>clock_timestamp() AND state<>'removed'").bind(&hash).fetch_optional(&mut **tx).await?.ok_or_else(denied)?;
    if !eligible(tx, &room, &p).await? {
        return Err(denied());
    }
    Ok((room, p))
}
pub(super) async fn participants(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
) -> Result<Vec<Value>> {
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(p) FROM telehealth_participants p WHERE room_id=$1 ORDER BY created_at,id",
    )
    .bind(uid(room, "id")?)
    .fetch_all(&mut **tx)
    .await?;
    let mut result = Vec::new();
    for p in rows {
        if eligible(tx, room, &p).await? {
            result.push(p);
        }
    }
    Ok(result)
}
pub(super) async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    action: &str,
    room: &Value,
) -> Result<()> {
    access::audit(
        tx,
        actor,
        action,
        "telehealth_rooms",
        Some(uid(room, "id")?),
    )
    .await
}
