use super::*;
use crate::media_transcription::Recognition;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Duration;

async fn authorized(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(Actor, Value, Value)> {
    let (org, rid): (Uuid, Uuid) =
        sqlx::query_as("SELECT organization_id,room_id FROM transcription_jobs WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(ApiError::missing)?;
    crate::scheduling::lock_org(tx, org).await?;
    let room = room(tx, rid, Some(org)).await?;
    let job = job(tx, id).await?;
    if !matches!(job["state"].as_str(), Some("queued" | "running")) {
        return Err(ApiError::conflict(
            "This processing job is no longer active",
        ));
    }
    let value: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND organization_id=$2 AND active",
    )
    .bind(uid(&job, "requested_by")?)
    .bind(org)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::denied)?;
    let actor = Actor {
        id: uid(&value, "id")?,
        org,
        role: value["role"].as_str().unwrap_or_default().to_owned(),
        value,
    };
    manager_scope(tx, &actor, &room).await?;
    processing_consents(
        tx,
        &room,
        uid(&job, "recording_id")?,
        Some(&job["consent_snapshot"]),
    )
    .await?;
    Ok((actor, room, job))
}
async fn failed(pool: &PgPool, id: Uuid, code: &str) -> Result<()> {
    let mut tx = pool.begin().await?;
    let row:Option<Value>=sqlx::query_scalar("UPDATE transcription_jobs SET state='failed',error_code=$2,completed_at=clock_timestamp() WHERE id=$1 AND state IN ('queued','running') RETURNING to_jsonb(transcription_jobs)").bind(id).bind(code).fetch_optional(&mut *tx).await?;
    if let Some(row) = row {
        let actor = Actor {
            id: uid(&row, "requested_by")?,
            org: uid(&row, "organization_id")?,
            role: String::new(),
            value: Value::Null,
        };
        access::audit(
            &mut tx,
            &actor,
            "processing_failed",
            "transcription_jobs",
            Some(id),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn guarded(
    pool: &PgPool,
    owner: &mut sqlx::PgConnection,
    id: Uuid,
    bytes: &[u8],
) -> Result<Recognition> {
    let recognition = crate::media_transcription::recognize(bytes);
    tokio::pin!(recognition);
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {out=&mut recognition=>return out,_=interval.tick()=>{sqlx::query("SELECT 1").execute(&mut *owner).await?;let mut tx=pool.begin().await?;authorized(&mut tx,id).await?;tx.commit().await?;}}
    }
}
async fn publish(
    pool: &PgPool,
    id: Uuid,
    recognized: Recognition,
    source_hash: String,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let (actor, room, job) = authorized(&mut tx, id).await?;
    let doc = Uuid::new_v4();
    let version = Uuid::new_v4();
    let segments:Vec<Value>=recognized.segments.iter().map(|s|json!({"id":Uuid::new_v4(),"start_ms":s.start_ms,"end_ms":s.end_ms,"text":s.text,"uncertain":s.uncertain,"speaker_participant_id":null,"speaker_verified":false,"patient_id":null,"context_verified":false,"excluded":false,"review_note":if s.uncertain{"Recognition uncertainty: verify wording and timing against the recording"}else{""}})).collect();
    if serde_json::to_vec(&segments)
        .map_err(|_| ApiError::invalid("Invalid recognition segments"))?
        .len()
        > 900000
    {
        return Err(ApiError {
            status: actix_web::http::StatusCode::PAYLOAD_TOO_LARGE,
            code: "transcript_too_large",
            message: "The recognized text exceeds the review limit. Use a shorter recording."
                .into(),
        });
    }
    sqlx::query("INSERT INTO transcripts(id,organization_id,room_id,recording_id,track_id,job_id,duration_ms,engine,model_label,model_sha256,source_sha256,recognized_segments,current_version_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)").bind(doc).bind(actor.org).bind(uid(&room,"id")?).bind(uid(&job,"recording_id")?).bind(uid(&job,"track_id")?).bind(id).bind(recognized.duration_ms).bind(recognized.engine).bind(recognized.model_label).bind(recognized.model_sha256).bind(source_hash).bind(json!(segments)).bind(version).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO transcript_versions(id,organization_id,transcript_id,number,segments,reason,created_by) VALUES($1,$2,$3,1,$4,'Initial local recognition; clinician review required',$5)").bind(version).bind(actor.org).bind(doc).bind(json!(segments)).bind(actor.id).execute(&mut *tx).await?;
    let changed = sqlx::query(
        "UPDATE transcription_jobs SET state='ready',completed_at=clock_timestamp() WHERE id=$1 AND state='running'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?.rows_affected();
    if changed != 1 {
        return Err(ApiError::conflict(
            "This processing job is no longer running",
        ));
    }
    access::audit(&mut tx, &actor, "recognized", "transcripts", Some(doc)).await?;
    tx.commit().await?;
    Ok(())
}
async fn process(pool: &PgPool, owner: &mut sqlx::PgConnection, id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    let (actor, room, job) = authorized(&mut tx, id).await?;
    if job["state"] != "queued" {
        return Err(ApiError::conflict("This job was already claimed"));
    }
    let track = track(&mut tx, &room, uid(&job, "track_id")?).await?;
    let bytes = super::super::tracks::decrypt_track(&mut tx, &room, &track).await?;
    let changed = sqlx::query(
        "UPDATE transcription_jobs SET state='running',started_at=clock_timestamp() WHERE id=$1 AND state='queued'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?.rows_affected();
    if changed != 1 {
        return Err(ApiError::conflict(
            "This processing job is no longer queued",
        ));
    }
    access::audit(
        &mut tx,
        &actor,
        "processing_started",
        "transcription_jobs",
        Some(id),
    )
    .await?;
    tx.commit().await?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let recognized = guarded(pool, owner, id, &bytes).await?;
    publish(pool, id, recognized, hash).await
}
pub(crate) async fn start(pool: PgPool) {
    // A dedicated session owns this database's worker; its drop releases the lock.
    let Ok(connection) = pool.acquire().await else {
        log::error!("Transcription worker connection unavailable");
        return;
    };
    let mut owner = connection.detach();
    let locked = sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock(734903221)")
        .fetch_one(&mut owner)
        .await
        .unwrap_or(false);
    if !locked {
        return;
    }
    let interrupted: Vec<Uuid> =
        match sqlx::query_scalar("SELECT id FROM transcription_jobs WHERE state='running'")
            .fetch_all(&pool)
            .await
        {
            Ok(ids) => ids,
            Err(_) => {
                log::error!("Transcription recovery query failed");
                return;
            }
        };
    for id in interrupted {
        if failed(&pool, id, "worker_interrupted").await.is_err() {
            log::error!("Transcription recovery audit failed");
            return;
        }
    }
    loop {
        if sqlx::query("SELECT 1").execute(&mut owner).await.is_err() {
            log::error!("Transcription worker ownership connection lost");
            return;
        }
        if let Ok(_permit) = crate::speech::recognition_permit() {
            let next=sqlx::query_scalar::<_,Uuid>("SELECT id FROM transcription_jobs WHERE state='queued' ORDER BY created_at,id LIMIT 1").fetch_optional(&pool).await;
            match next {
                Ok(Some(id)) => {
                    if let Err(e) = process(&pool, &mut owner, id).await {
                        if failed(&pool, id, e.code).await.is_err() {
                            log::error!("Transcription failure state could not be stored");
                        }
                    }
                }
                Ok(None) => {}
                Err(_) => log::error!("Transcription queue unavailable"),
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
