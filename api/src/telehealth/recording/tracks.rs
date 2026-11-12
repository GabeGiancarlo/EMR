use super::*;
use sqlx::Row;
const MAX_TRACK_BYTES: i64 = 256 * 1024 * 1024;
const MAX_CHUNK_BYTES: usize = 8 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Open {
    connection_id: Uuid,
    mime_type: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    expected_chunks: i32,
    complete: bool,
}
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Payload,
) -> Result<HttpResponse> {
    // The invitation is checked before any of the body is read.
    known_invitation(&req, &state.pool).await?;
    let body: Open = payload::json(&req, body, payload::VISIT_JSON_LIMIT).await?;
    let mut tx = state.pool.begin().await?;
    let (room, p) = authorize(&req, &mut tx).await?;
    super::super::visitor::connection(&p, body.connection_id)?;
    event_budget(&mut tx, &room, uid(&p, "id")?).await?;
    let r: Value = sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM recordings r WHERE id=$1 AND room_id=$2 FOR UPDATE",
    )
    .bind(id.into_inner())
    .bind(uid(&room, "id")?)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(ApiError::missing)?;
    let r = refresh(&mut tx, &room, r).await?;
    if r["state"] != "recording" {
        tx.commit().await?;
        return Err(ApiError::conflict("Recording is not active"));
    }
    let accepted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM recording_participants WHERE recording_id=$1 AND participant_id=$2 AND decision='accepted')").bind(uid(&r,"id")?).bind(uid(&p,"id")?).fetch_one(&mut *tx).await?;
    if !accepted || p["state"] != "admitted" {
        return Err(ApiError::denied());
    }
    let mime = body.mime_type.to_ascii_lowercase().replace(' ', "");
    let supported = if r["mode"] == "audio" {
        matches!(mime.as_str(), "audio/webm" | "audio/webm;codecs=opus")
    } else {
        matches!(
            mime.as_str(),
            "video/webm" | "video/webm;codecs=vp8,opus" | "video/webm;codecs=vp9,opus"
        )
    };
    if !supported {
        return Err(ApiError::invalid(
            "This browser recording format is not supported by the selected policy",
        ));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM recording_tracks WHERE recording_id=$1")
            .bind(uid(&r, "id")?)
            .fetch_one(&mut *tx)
            .await?;
    if count >= 256 {
        return Err(ApiError::conflict(
            "This recording has reached its track limit. Finish and request another recording",
        ));
    }
    let id = Uuid::new_v4();
    let (token, hash) = fresh_token();
    sqlx::query("INSERT INTO recording_tracks(id,organization_id,recording_id,participant_id,connection_id,mime_type,token_hash) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(id).bind(uid(&room,"organization_id")?).bind(uid(&r,"id")?).bind(uid(&p,"id")?).bind(body.connection_id).bind(&mime).bind(hash).execute(&mut *tx).await?;
    event(
        &mut tx,
        &room,
        Some(uid(&p, "id")?),
        None,
        "recording_track_opened",
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(
        json!({"track_id":id,"upload_token":token,"mime_type":mime}),
    ))
}
/// Run before an upload route reads its request body: the caller must present the
/// upload token of this track. One indexed lookup on a pooled connection that is
/// returned before the body is read. It grants nothing; [`upload_access`] still makes
/// every check, under the organization lock, once the body has arrived.
async fn known_upload(req: &HttpRequest, pool: &sqlx::PgPool, id: Uuid) -> Result<()> {
    let raw = capability(req).ok_or_else(ApiError::denied)?;
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM recording_tracks WHERE id=$1 AND token_hash=$2)",
    )
    .bind(id)
    .bind(hash_token(raw))
    .fetch_one(pool)
    .await?;
    if known {
        Ok(())
    } else {
        Err(ApiError::denied())
    }
}
async fn upload_access(
    req: &HttpRequest,
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(Value, Value, Value)> {
    let raw = capability(req).ok_or_else(ApiError::denied)?;
    let address:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT r.room_id,t.organization_id FROM recording_tracks t JOIN recordings r ON r.id=t.recording_id WHERE t.id=$1 AND t.token_hash=$2").bind(id).bind(hash_token(raw)).fetch_optional(&mut **tx).await?;
    let (room_id, org) = address.ok_or_else(ApiError::denied)?;
    crate::scheduling::lock_org(tx, org).await?;
    let room = room(tx, room_id, Some(org)).await?;
    let track:Value=sqlx::query_scalar("SELECT to_jsonb(t)-'token_hash' FROM recording_tracks t WHERE id=$1 AND token_hash=$2 FOR UPDATE").bind(id).bind(hash_token(raw)).fetch_optional(&mut **tx).await?.ok_or_else(ApiError::denied)?;
    let r: Value =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM recordings r WHERE id=$1 FOR UPDATE")
            .bind(uid(&track, "recording_id")?)
            .fetch_one(&mut **tx)
            .await?;
    let r = refresh(tx, &room, r).await?;
    let policy_active: bool =
        sqlx::query_scalar("SELECT active FROM recording_policies WHERE id=$1")
            .bind(uid(&r, "policy_id")?)
            .fetch_one(&mut **tx)
            .await?;
    if !policy_active {
        return Err(ApiError::denied());
    }
    let members = recording_members(tx, uid(&r, "id")?).await?;
    if members.is_empty() || members.iter().any(|m| m["decision"] != "accepted") {
        return Err(ApiError::denied());
    }
    for m in &members {
        if !consent_current(tx, &room, m).await? {
            return Err(ApiError::denied());
        }
    }
    let p: Value =
        sqlx::query_scalar("SELECT to_jsonb(p) FROM telehealth_participants p WHERE id=$1")
            .bind(uid(&track, "participant_id")?)
            .fetch_one(&mut **tx)
            .await?;
    if !eligible(tx, &room, &p).await? || p["connection_id"] != track["connection_id"] {
        return Err(ApiError::denied());
    }
    let grace = matches!(r["state"].as_str(), Some("paused" | "ended"))
        && time(&r, "stopped_at").is_some_and(|t| Utc::now() <= t + chrono::Duration::seconds(30));
    if r["state"] != "recording" && !grace {
        return Err(ApiError::denied());
    }
    Ok((room, r, track))
}
pub async fn upload(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, i32)>,
    body: web::Payload,
) -> Result<HttpResponse> {
    let (id, sequence) = path.into_inner();
    // The upload token is checked before any of the media is read, so only the
    // holder of this track's token can make the server buffer a piece.
    known_upload(&req, &state.pool, id).await?;
    if !(0..3600).contains(&sequence) {
        return Err(ApiError::invalid(
            "A media piece must be 1 byte to 8 MiB with a valid sequence",
        ));
    }
    let body = payload::bytes(&req, body, MAX_CHUNK_BYTES).await?;
    if body.is_empty() {
        return Err(ApiError::invalid(
            "A media piece must be 1 byte to 8 MiB with a valid sequence",
        ));
    }
    if sequence == 0 && !body.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Err(ApiError::invalid(
            "The first media piece must start a WebM recording",
        ));
    }
    let mut tx = state.pool.begin().await?;
    let (room, r, track) = match upload_access(&req, &mut tx, id).await {
        Ok(v) => v,
        Err(e) => {
            tx.commit().await?;
            return Err(e);
        }
    };
    let sha = crate::sha256_hex(&body);
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT plain_sha256 FROM recording_chunks WHERE track_id=$1 AND sequence=$2",
    )
    .bind(id)
    .bind(sequence)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(previous) = previous {
        if previous != sha {
            return Err(ApiError::conflict(
                "This media sequence was already stored with different bytes",
            ));
        }
        tx.commit().await?;
        return Ok(no_store(json!({"stored":true,"duplicate":true})));
    }
    if track["state"] != "open" {
        return Err(ApiError::conflict("This media track is already closed"));
    }
    let next: i64 = sqlx::query_scalar("SELECT count(*) FROM recording_chunks WHERE track_id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if next != i64::from(sequence) {
        return Err(ApiError::conflict(
            "Upload media pieces in sequence without gaps",
        ));
    }
    let total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(byte_count),0)::bigint FROM recording_tracks WHERE recording_id=$1",
    )
    .bind(uid(&r, "id")?)
    .fetch_one(&mut *tx)
    .await?;
    if track["byte_count"].as_i64().unwrap_or(0) + body.len() as i64 > MAX_TRACK_BYTES
        || total + body.len() as i64 > 1024 * 1024 * 1024
    {
        return Err(ApiError::conflict(
            "This recording has reached its storage limit. Stop and review the saved media",
        ));
    }
    let sealed = crypto::seal(
        uid(&room, "organization_id")?,
        uid(&r, "id")?,
        id,
        sequence,
        &body,
    )?;
    sqlx::query("INSERT INTO recording_chunks(track_id,sequence,key_id,nonce,ciphertext,plain_sha256,plain_length) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(id).bind(sequence).bind(sealed.key_id).bind(sealed.nonce).bind(sealed.ciphertext).bind(sealed.sha256).bind(body.len() as i32).execute(&mut *tx).await?;
    sqlx::query("UPDATE recording_tracks SET byte_count=byte_count+$2 WHERE id=$1")
        .bind(id)
        .bind(body.len() as i64)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(no_store(json!({"stored":true,"sequence":sequence})))
}
pub async fn finish(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Payload,
) -> Result<HttpResponse> {
    let id = id.into_inner();
    // The upload token is checked before any of the body is read.
    known_upload(&req, &state.pool, id).await?;
    let body: Finish = payload::json(&req, body, payload::VISIT_JSON_LIMIT).await?;
    if !(0..=3600).contains(&body.expected_chunks) {
        return Err(ApiError::invalid("Invalid media piece count"));
    }
    let mut tx = state.pool.begin().await?;
    let (room, _, track) = match upload_access(&req, &mut tx, id).await {
        Ok(v) => v,
        Err(e) => {
            tx.commit().await?;
            return Err(e);
        }
    };
    let next = if body.complete {
        "complete"
    } else {
        "incomplete"
    };
    if track["state"] != "open" {
        if track["state"] == next && track["expected_chunks"] == body.expected_chunks {
            tx.commit().await?;
            return Ok(no_store(json!({"state":next})));
        }
        return Err(ApiError::conflict(
            "This media track has already been finalized",
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM recording_chunks WHERE track_id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if body.complete && (count == 0 || count != i64::from(body.expected_chunks)) {
        return Err(ApiError::conflict(
            "All media pieces must be stored before a track is marked complete",
        ));
    }
    sqlx::query("UPDATE recording_tracks SET state=$2,expected_chunks=$3,finished_at=clock_timestamp() WHERE id=$1").bind(id).bind(next).bind(body.expected_chunks).execute(&mut *tx).await?;
    event(
        &mut tx,
        &room,
        Some(uid(&track, "participant_id")?),
        None,
        &format!("recording_track_{next}"),
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(json!({"state":next})))
}
pub(super) async fn cleanup(tx: &mut Transaction<'_, Postgres>, room: Uuid) -> Result<()> {
    sqlx::query("UPDATE recording_tracks t SET state='incomplete',finished_at=clock_timestamp() FROM recordings r WHERE t.recording_id=r.id AND r.room_id=$1 AND t.state='open' AND (r.state IN ('interrupted','declined') OR (r.state IN ('paused','ended') AND r.stopped_at<clock_timestamp()-interval '30 seconds'))").bind(room).execute(&mut **tx).await?;
    Ok(())
}
pub async fn list(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room) =
        super::super::manager::managed(&req, &state, id.into_inner(), &mut tx).await?;
    if let Some(r) = current(&mut tx, uid(&room, "id")?).await? {
        refresh(&mut tx, &room, r).await?;
    }
    cleanup(&mut tx, uid(&room, "id")?).await?;
    let records: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM recordings r WHERE room_id=$1 ORDER BY created_at DESC,id",
    )
    .bind(uid(&room, "id")?)
    .fetch_all(&mut *tx)
    .await?;
    let tracks:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(t)-'token_hash'||jsonb_build_object('label',p.label,'role',CASE WHEN p.staff_id IS NULL THEN 'client' ELSE 'host' END) FROM recording_tracks t JOIN recordings r ON r.id=t.recording_id JOIN telehealth_participants p ON p.id=t.participant_id WHERE r.room_id=$1 ORDER BY t.created_at,t.id").bind(uid(&room,"id")?).fetch_all(&mut *tx).await?;
    access::audit(
        &mut tx,
        &actor,
        "read",
        "recordings",
        Some(uid(&room, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(json!({"recordings":records,"tracks":tracks})))
}
pub async fn playback(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid, Uuid)>,
) -> Result<HttpResponse> {
    let (room_id, rid, tid) = path.into_inner();
    let mut tx = state.pool.begin().await?;
    let (actor, room) = super::super::manager::managed(&req, &state, room_id, &mut tx).await?;
    let track:Value=sqlx::query_scalar("SELECT to_jsonb(t)-'token_hash' FROM recording_tracks t JOIN recordings r ON r.id=t.recording_id WHERE t.id=$1 AND r.id=$2 AND r.room_id=$3").bind(tid).bind(rid).bind(room_id).fetch_optional(&mut *tx).await?.ok_or_else(ApiError::missing)?;
    if track["state"] != "complete" {
        return Err(ApiError::conflict(
            "This recording track is incomplete and cannot be presented as a complete recording",
        ));
    }
    let bytes = decrypt_track(&mut tx, &room, &track).await?;
    access::audit(&mut tx, &actor, "playback", "recording_tracks", Some(tid)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok()
        .insert_header((
            "Content-Type",
            track["mime_type"]
                .as_str()
                .unwrap_or("application/octet-stream"),
        ))
        .insert_header(("Cache-Control", "no-store"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Referrer-Policy", "no-referrer"))
        .body(bytes))
}

pub(super) async fn decrypt_track(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    track: &Value,
) -> Result<Vec<u8>> {
    let tid = uid(track, "id")?;
    let rid = uid(track, "recording_id")?;
    if track["state"] != "complete" {
        return Err(ApiError::conflict("Only complete media can be processed"));
    }
    if !track["byte_count"]
        .as_i64()
        .is_some_and(|n| (1..=MAX_TRACK_BYTES).contains(&n))
    {
        return Err(ApiError::conflict(
            "The stored track has an invalid byte count",
        ));
    }
    let chunks=sqlx::query("SELECT sequence,key_id,nonce,ciphertext,plain_sha256,plain_length FROM recording_chunks WHERE track_id=$1 ORDER BY sequence").bind(tid).fetch_all(&mut **tx).await?;
    if chunks.is_empty() || chunks.len() as i64 != track["expected_chunks"].as_i64().unwrap_or(-1) {
        return Err(ApiError::conflict(
            "The stored media sequence is incomplete",
        ));
    }
    let mut bytes = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let seq: i32 = chunk.try_get("sequence")?;
        if seq != index as i32 {
            return Err(ApiError::conflict("The stored media sequence has a gap"));
        }
        let plain = crypto::open(
            uid(room, "organization_id")?,
            rid,
            tid,
            seq,
            chunk.try_get("key_id")?,
            chunk.try_get("nonce")?,
            chunk.try_get("ciphertext")?,
        )?;
        let length: i32 = chunk.try_get("plain_length")?;
        let hash: String = chunk.try_get("plain_sha256")?;
        if plain.len() != length as usize || crate::sha256_hex(&plain) != hash {
            return Err(ApiError::conflict(
                "The stored media failed its integrity check",
            ));
        }
        if bytes.len().saturating_add(plain.len()) > MAX_TRACK_BYTES as usize {
            return Err(ApiError::conflict(
                "The stored track exceeds its media limit",
            ));
        }
        bytes.extend_from_slice(&plain);
    }
    if bytes.len() as i64 != track["byte_count"].as_i64().unwrap_or(-1) {
        return Err(ApiError::conflict(
            "The stored media byte count does not match its saved manifest",
        ));
    }
    Ok(bytes)
}
