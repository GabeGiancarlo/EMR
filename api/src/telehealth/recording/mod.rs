//! Explicit participant agreement, encrypted media storage and scoped playback.
mod crypto;
mod tracks;
pub(crate) mod transcripts;
use super::*;
use crate::{payload, AppState};
use serde::Deserialize;
pub(super) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.configure(transcripts::configure);
    cfg.route("/recording-policies", web::get().to(policies))
        .route("/recording-policies", web::post().to(create_policy))
        .route(
            "/recording-policies/{id}/retire",
            web::post().to(retire_policy),
        )
        .route(
            "/telehealth/visit/recordings/{action}",
            web::post().to(action),
        )
        .route(
            "/telehealth/visit/recordings/{id}/tracks",
            web::post().to(tracks::create),
        )
        // The handler reads the body itself, after checking the upload token, and
        // applies the 8 MiB limit there.
        .service(
            web::resource("/recording-tracks/{id}/chunks/{sequence}")
                .route(web::put().to(tracks::upload))
                .default_service(web::to(crate::method_not_allowed)),
        )
        .route(
            "/recording-tracks/{id}/finish",
            web::post().to(tracks::finish),
        )
        .route(
            "/telehealth/rooms/{id}/recordings",
            web::get().to(tracks::list),
        )
        .route(
            "/telehealth/rooms/{room}/recordings/{recording}/tracks/{track}",
            web::get().to(tracks::playback),
        );
}
async fn policies_for(tx: &mut Transaction<'_, Postgres>, org: Uuid) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar("SELECT to_jsonb(p) FROM recording_policies p WHERE organization_id=$1 ORDER BY created_at DESC,id").bind(org).fetch_all(&mut **tx).await?)
}
pub async fn policies(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
        return Err(ApiError::denied());
    }
    let mut tx = state.pool.begin().await?;
    let rows = policies_for(&mut tx, actor.org).await?;
    access::audit(&mut tx, &actor, "read", "recording_policies", None).await?;
    tx.commit().await?;
    Ok(no_store(
        json!({"configured":crypto::configured(),"policies":rows}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    title: String,
    purpose: String,
    retention_days: i32,
    allow_video: bool,
}
pub async fn create_policy(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<Policy>,
) -> Result<HttpResponse> {
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, initial.org, "templates").await?;
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead") {
        return Err(ApiError::denied());
    }
    if !(1..=120).contains(&body.title.trim().chars().count())
        || !(10..=2000).contains(&body.purpose.trim().chars().count())
        || !(1..=3650).contains(&body.retention_days)
    {
        return Err(ApiError::invalid("Provide a title, a clear recording purpose and a retention review period of 1–3650 days"));
    }
    let id = Uuid::new_v4();
    let row:Value=sqlx::query_scalar("INSERT INTO recording_policies(id,organization_id,title,purpose,retention_days,allow_video,created_by) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING to_jsonb(recording_policies)").bind(id).bind(actor.org).bind(body.title.trim()).bind(body.purpose.trim()).bind(body.retention_days).bind(body.allow_video).bind(actor.id).fetch_one(&mut *tx).await?;
    access::audit(&mut tx, &actor, "create", "recording_policies", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(row))
}
pub async fn retire_policy(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, initial.org, "templates").await?;
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead") {
        return Err(ApiError::denied());
    }
    let id = id.into_inner();
    let count = sqlx::query(
        "UPDATE recording_policies SET active=false WHERE id=$1 AND organization_id=$2",
    )
    .bind(id)
    .bind(actor.org)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if count == 0 {
        return Err(ApiError::missing());
    }
    access::audit(&mut tx, &actor, "retire", "recording_policies", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(json!({"retired":true})))
}
async fn current(tx: &mut Transaction<'_, Postgres>, room: Uuid) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar("SELECT to_jsonb(r) FROM recordings r WHERE room_id=$1 ORDER BY created_at DESC,id DESC LIMIT 1 FOR UPDATE").bind(room).fetch_optional(&mut **tx).await?)
}
async fn recording_members(
    tx: &mut Transaction<'_, Postgres>,
    recording: Uuid,
) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar("SELECT to_jsonb(m)||jsonb_build_object('label',p.label,'staff_id',p.staff_id,'client_id',p.client_id) FROM recording_participants m JOIN telehealth_participants p ON p.id=m.participant_id WHERE m.recording_id=$1 ORDER BY p.created_at,p.id").bind(recording).fetch_all(&mut **tx).await?)
}
async fn consent(
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    client: Uuid,
    selected: Option<Uuid>,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar("SELECT to_jsonb(c) FROM consents c JOIN clients p ON p.id=c.client_id JOIN facilities f ON f.id=p.facility_id WHERE c.organization_id=$1 AND c.client_id=$2 AND c.kind='recording' AND c.revoked_on IS NULL AND c.granted_on<=(clock_timestamp() AT TIME ZONE f.timezone)::date AND (c.expires_on IS NULL OR c.expires_on>=(clock_timestamp() AT TIME ZONE f.timezone)::date) AND ($3::uuid IS NULL OR c.id=$3) ORDER BY c.granted_on DESC,c.id LIMIT 1").bind(org).bind(client).bind(selected).fetch_optional(&mut **tx).await?)
}
async fn consent_current(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    m: &Value,
) -> Result<bool> {
    if let Some(client) = m["client_id"].as_str() {
        let Some(id) = m["consent_id"]
            .as_str()
            .and_then(|v| Uuid::parse_str(v).ok())
        else {
            return Ok(false);
        };
        let Some(current) = consent(
            tx,
            uid(room, "organization_id")?,
            Uuid::parse_str(client).map_err(|_| ApiError::denied())?,
            Some(id),
        )
        .await?
        else {
            return Ok(false);
        };
        // Editing the consent's scope/dates also requires a new recording request.
        return Ok(current == m["consent_snapshot"]);
    }
    Ok(true)
}
async fn set_state(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    r: &Value,
    state: &str,
    reason: &str,
    actor: Option<Uuid>,
    participant: Option<Uuid>,
) -> Result<Value> {
    let row:Value=sqlx::query_scalar("UPDATE recordings SET state=$2,reason=$3,started_at=CASE WHEN $2='recording' THEN COALESCE(started_at,clock_timestamp()) ELSE started_at END,stopped_at=CASE WHEN $2 IN ('paused','ended','declined','interrupted') THEN clock_timestamp() ELSE NULL END WHERE id=$1 RETURNING to_jsonb(recordings)").bind(uid(r,"id")?).bind(state).bind(reason).fetch_one(&mut **tx).await?;
    event(tx, room, participant, actor, &format!("recording_{state}")).await?;
    if matches!(state, "interrupted" | "declined") {
        sqlx::query("UPDATE recording_tracks SET state='incomplete',finished_at=clock_timestamp() WHERE recording_id=$1 AND state='open'")
            .bind(uid(r, "id")?).execute(&mut **tx).await?;
    }
    Ok(row)
}
async fn refresh(tx: &mut Transaction<'_, Postgres>, room: &Value, mut r: Value) -> Result<Value> {
    if !matches!(
        r["state"].as_str(),
        Some("requested" | "recording" | "paused")
    ) {
        return Ok(r);
    }
    if !room["ended_at"].is_null() {
        r = set_state(
            tx,
            room,
            &r,
            "ended",
            "The clinician ended the visit",
            None,
            None,
        )
        .await?;
        // Grace begins at the actual room end, never at a later upload attempt.
        sqlx::query("UPDATE recordings SET stopped_at=$2 WHERE id=$1")
            .bind(uid(&r, "id")?)
            .bind(time(room, "ended_at"))
            .execute(&mut **tx)
            .await?;
        r["stopped_at"] = room["ended_at"].clone();
        return Ok(r);
    }
    let policy_active: bool =
        sqlx::query_scalar("SELECT active FROM recording_policies WHERE id=$1")
            .bind(uid(&r, "policy_id")?)
            .fetch_one(&mut **tx)
            .await?;
    let members = recording_members(tx, uid(&r, "id")?).await?;
    let people = participants(tx, room).await?;
    let admitted: Vec<&Value> = people
        .iter()
        .filter(|p| p["state"] == "admitted" && online(p))
        .collect();
    let mut reason = if !policy_active {
        "The recording policy was retired"
    } else if live(room).is_err() {
        "The visit expired"
    } else if admitted.len() != members.len() || !admitted.iter().any(|p| p["staff_id"].is_string())
    {
        "The connected participant group changed"
    } else {
        ""
    };
    if reason.is_empty() {
        for m in &members {
            if !admitted.iter().any(|p| p["id"] == m["participant_id"])
                || !consent_current(tx, room, m).await?
            {
                reason = "Participant access or recording consent changed";
                break;
            }
        }
    }
    if !reason.is_empty() {
        r = set_state(tx, room, &r, "interrupted", reason, None, None).await?;
    }
    Ok(r)
}
pub(super) async fn context(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    p: &Value,
    connected: bool,
) -> Result<Value> {
    if !connected || p["state"] != "admitted" {
        return Ok(json!({"configured":crypto::configured(),"policies":[],"current":null}));
    }
    let policies = if p["staff_id"].is_string() {
        policies_for(tx, uid(room, "organization_id")?).await?
    } else {
        Vec::new()
    };
    let Some(r) = current(tx, uid(room, "id")?).await? else {
        return Ok(json!({"configured":crypto::configured(),"policies":policies,"current":null}));
    };
    let r = refresh(tx, room, r).await?;
    let members = recording_members(tx, uid(&r, "id")?).await?;
    // A newly admitted person sees that recording stopped, but not the old roster.
    let enrolled = members.iter().any(|m| m["participant_id"] == p["id"]);
    let mut visible = Vec::new();
    if enrolled || p["staff_id"].is_string() {
        for m in members {
            visible.push(json!({"participant_id":m["participant_id"],"label":m["label"],"role":if m["staff_id"].is_string(){"host"}else{"client"},"decision":m["decision"],"eligible":consent_current(tx,room,&m).await?}));
        }
    }
    let out = json!({"id":r["id"],"state":r["state"],"mode":r["mode"],"purpose":r["policy_snapshot"]["purpose"],"policy_title":r["policy_snapshot"]["title"],"retention_days":r["policy_snapshot"]["retention_days"],"reason":r["reason"],"participants":visible,"created_at":r["created_at"],"started_at":r["started_at"],"stopped_at":r["stopped_at"]});
    Ok(json!({"configured":crypto::configured(),"policies":policies,"current":out}))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    connection_id: Uuid,
    policy_id: Option<Uuid>,
    mode: Option<String>,
}
pub async fn action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Payload,
) -> Result<HttpResponse> {
    let action = path.into_inner();
    if !matches!(
        action.as_str(),
        "request" | "accept" | "decline" | "withdraw" | "start" | "pause" | "resume" | "stop"
    ) {
        return Err(ApiError::missing());
    }
    // The invitation is checked before any of the body is read.
    known_invitation(&req, &state.pool).await?;
    let body: Command = payload::json(&req, body, payload::VISIT_JSON_LIMIT).await?;
    let mut tx = state.pool.begin().await?;
    let (room, p) = authorize(&req, &mut tx).await?;
    super::visitor::connection(&p, body.connection_id)?;
    if p["state"] != "admitted" {
        return Err(ApiError::denied());
    }
    let staff = p["staff_id"].as_str().and_then(|v| Uuid::parse_str(v).ok());
    let pid = uid(&p, "id")?;
    if !matches!(action.as_str(), "accept" | "decline" | "withdraw") && staff.is_none() {
        return Err(ApiError::denied());
    }
    let prior = match current(&mut tx, uid(&room, "id")?).await? {
        Some(r) => Some(refresh(&mut tx, &room, r).await?),
        None => None,
    };
    if action == "request" {
        if !crypto::configured() {
            return Err(ApiError::conflict(
                "Configure the local media vault before requesting a recording",
            ));
        }
        if prior.as_ref().is_some_and(|r| {
            matches!(
                r["state"].as_str(),
                Some("requested" | "recording" | "paused")
            )
        }) {
            return Err(ApiError::conflict(
                "Finish the current recording request first",
            ));
        }
        let policy_id = body
            .policy_id
            .ok_or_else(|| ApiError::invalid("Choose a recording policy"))?;
        let policy:Value=sqlx::query_scalar("SELECT to_jsonb(p) FROM recording_policies p WHERE id=$1 AND organization_id=$2 AND active").bind(policy_id).bind(uid(&room,"organization_id")?).fetch_optional(&mut *tx).await?.ok_or_else(ApiError::missing)?;
        let mode = body.mode.as_deref().unwrap_or("");
        if mode != "audio" && (mode != "video" || policy["allow_video"] != true) {
            return Err(ApiError::invalid("Choose an allowed recording mode"));
        }
        let people = participants(&mut tx, &room).await?;
        let people: Vec<Value> = people
            .into_iter()
            .filter(|p| p["state"] == "admitted" && online(p))
            .collect();
        if !people.iter().any(|p| p["client_id"].is_string()) {
            return Err(ApiError::conflict(
                "Admit the client before requesting recording",
            ));
        }
        let rid = Uuid::new_v4();
        let r:Value=sqlx::query_scalar("INSERT INTO recordings(id,organization_id,room_id,policy_id,policy_snapshot,mode,created_by,retain_until) VALUES($1,$2,$3,$4,$5,$6,$7,clock_timestamp()+make_interval(days=>$8)) RETURNING to_jsonb(recordings)").bind(rid).bind(uid(&room,"organization_id")?).bind(uid(&room,"id")?).bind(policy_id).bind(&policy).bind(mode).bind(staff).bind(policy["retention_days"].as_i64().unwrap_or(1) as i32).fetch_one(&mut *tx).await?;
        for person in people {
            let consent = if let Some(client) = person["client_id"].as_str() {
                Some(consent(&mut tx,uid(&room,"organization_id")?,Uuid::parse_str(client).map_err(|_|ApiError::denied())?,None).await?.ok_or_else(||ApiError::conflict("Every client needs a current recording consent before a request can be sent"))?)
            } else {
                None
            };
            let cid = consent
                .as_ref()
                .and_then(|c| c["id"].as_str())
                .and_then(|id| Uuid::parse_str(id).ok());
            sqlx::query("INSERT INTO recording_participants(recording_id,room_id,participant_id,consent_id,consent_snapshot,organization_id) VALUES($1,$2,$3,$4,$5,$6)").bind(rid).bind(uid(&room,"id")?).bind(uid(&person,"id")?).bind(cid).bind(consent).bind(uid(&room,"organization_id")?).execute(&mut *tx).await?;
        }
        event(&mut tx, &room, Some(pid), staff, "recording_requested").await?;
        let _ = r;
    } else {
        let r = prior.ok_or_else(ApiError::missing)?;
        if matches!(
            r["state"].as_str(),
            Some("ended" | "declined" | "interrupted")
        ) {
            tx.commit().await?;
            return Err(ApiError::conflict(
                "This recording has stopped. Request a new recording if appropriate",
            ));
        }
        let rid = uid(&r, "id")?;
        if matches!(action.as_str(), "accept" | "decline" | "withdraw") {
            let decision = match action.as_str() {
                "accept" => "accepted",
                "decline" => "declined",
                _ => "withdrawn",
            };
            let old:Option<String>=sqlx::query_scalar("SELECT decision FROM recording_participants WHERE recording_id=$1 AND participant_id=$2").bind(rid).bind(pid).fetch_optional(&mut *tx).await?;
            let allowed = match action.as_str() {
                "accept" | "decline" => {
                    r["state"] == "requested" && old.as_deref() == Some("pending")
                }
                _ => old.as_deref() == Some("accepted"),
            };
            if !allowed {
                return Err(ApiError::conflict(
                    "This recording decision is no longer available",
                ));
            }
            sqlx::query("UPDATE recording_participants SET decision=$3,decided_at=clock_timestamp() WHERE recording_id=$1 AND participant_id=$2").bind(rid).bind(pid).bind(decision).execute(&mut *tx).await?;
            event(
                &mut tx,
                &room,
                Some(pid),
                staff,
                &format!("recording_{decision}"),
            )
            .await?;
            if action != "accept" {
                set_state(
                    &mut tx,
                    &room,
                    &r,
                    if action == "decline" {
                        "declined"
                    } else {
                        "interrupted"
                    },
                    "A participant declined or withdrew recording agreement",
                    staff,
                    Some(pid),
                )
                .await?;
            }
        } else {
            let members = recording_members(&mut tx, rid).await?;
            let next = match action.as_str() {
                "start" | "resume" => {
                    if (action == "start" && r["state"] != "requested")
                        || (action == "resume" && r["state"] != "paused")
                        || members.iter().any(|m| m["decision"] != "accepted")
                    {
                        return Err(ApiError::conflict(
                            "Every participant must accept before capture begins",
                        ));
                    }
                    if action == "resume" {
                        tracks::cleanup(&mut tx, uid(&room, "id")?).await?;
                        let open: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM recording_tracks WHERE recording_id=$1 AND state='open')")
                            .bind(rid).fetch_one(&mut *tx).await?;
                        if open {
                            return Err(ApiError::conflict("The previous recording segment is still saving. Wait before resuming"));
                        }
                    }
                    "recording"
                }
                "pause" => {
                    if r["state"] != "recording" {
                        return Err(ApiError::conflict("Only an active recording can be paused"));
                    }
                    "paused"
                }
                _ => "ended",
            };
            set_state(
                &mut tx,
                &room,
                &r,
                next,
                if next == "recording" {
                    ""
                } else {
                    "The clinician stopped capture"
                },
                staff,
                Some(pid),
            )
            .await?;
        }
    }
    let out = context(&mut tx, &room, &p, true).await?;
    tx.commit().await?;
    Ok(no_store(out))
}
