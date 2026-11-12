use super::*;
use crate::workers::{Exit, Status};
use sqlx::{Connection, PgConnection, PgPool};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

const ADVISORY_LOCK_KEY: i64 = 734903222;
const TERMINAL_SAVE_BACKOFF: Duration = Duration::from_secs(2);

async fn authorized(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(Actor, Value)> {
    let org: Uuid =
        sqlx::query_scalar("SELECT organization_id FROM clinical_draft_jobs WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(ApiError::missing)?;
    crate::scheduling::lock_org(tx, org).await?;
    let row = job(tx, id).await?;
    if !matches!(row["state"].as_str(), Some("queued" | "running")) {
        return Err(ApiError::conflict("This generation is no longer active"));
    }
    let value: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND organization_id=$2 AND active",
    )
    .bind(uid(&row, "requested_by")?)
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
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
        return Err(ApiError::denied());
    }
    let client = records::get(tx, "clients", uid(&row, "client_id")?, org, false).await?;
    records::scope(tx, &actor, "clients", &client).await?;
    current_sources(tx, &actor, &row).await?;
    Ok((actor, row))
}
async fn write_terminal(tx: &mut Transaction<'_, Postgres>, id: Uuid, code: &str) -> Result<()> {
    // The id lookup does not lock the job row. Organization lock comes first.
    let org = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM clinical_draft_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(org) = org else {
        return Ok(());
    };
    crate::scheduling::lock_org(tx, org).await?;
    let row: Option<Value> = sqlx::query_scalar("UPDATE clinical_draft_jobs SET state='failed',error_code=$2,completed_at=clock_timestamp() WHERE id=$1 AND state IN ('queued','running') RETURNING to_jsonb(clinical_draft_jobs)")
        .bind(id)
        .bind(code)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(row) = row {
        let actor = Actor {
            id: uid(&row, "requested_by")?,
            org: uid(&row, "organization_id")?,
            role: String::new(),
            value: Value::Null,
        };
        access::audit(
            tx,
            &actor,
            "generation_failed",
            "clinical_draft_jobs",
            Some(id),
        )
        .await?;
    }
    Ok(())
}
async fn failed(owner: &mut PgConnection, id: Uuid, code: &str) -> Result<()> {
    let mut tx = owner.begin().await?;
    match write_terminal(&mut tx, id, code).await {
        Ok(()) => {
            tx.commit().await?;
            Ok(())
        }
        Err(error) => {
            let _ = tx.rollback().await;
            Err(error)
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
#[must_use]
enum TerminalPersist {
    Saved,
    Retry,
    OwnershipLost,
}
/// Terminal write on the advisory-lock session. Retry keeps the same id/code.
/// OwnershipLost means this session is unusable; callers must not write via the pool.
async fn persist_terminal(owner: &mut PgConnection, id: Uuid, code: &str) -> TerminalPersist {
    if failed(owner, id, code).await.is_ok() {
        return TerminalPersist::Saved;
    }
    // This probe does not fence another connection. It only reports whether the
    // session that holds the advisory lock can still run a query.
    if same_session_unusable(owner).await {
        TerminalPersist::OwnershipLost
    } else {
        TerminalPersist::Retry
    }
}
async fn same_session_unusable(owner: &mut PgConnection) -> bool {
    sqlx::query("SELECT 1").execute(&mut *owner).await.is_err()
}
async fn owned_interrupted(owner: &mut PgConnection) -> Result<VecDeque<Uuid>> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM clinical_draft_jobs WHERE state='running' ORDER BY created_at, id",
    )
    .fetch_all(&mut *owner)
    .await?;
    Ok(VecDeque::from(ids))
}
async fn next_queued(owner: &mut PgConnection) -> Result<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM clinical_draft_jobs WHERE state='queued' ORDER BY created_at, id LIMIT 1",
    )
    .fetch_optional(&mut *owner)
    .await?)
}
async fn process(pool: &PgPool, owner: &mut PgConnection, id: Uuid) -> Result<()> {
    let mut tx = crate::db::begin_background(pool).await?;
    let (actor, row) = authorized(&mut tx, id).await?;
    if row["state"] != "queued" {
        return Err(ApiError::conflict("This generation was already claimed"));
    }
    let saved: Vec<KnowledgeChunk> = serde_json::from_value(row["sources"].clone())
        .map_err(|_| ApiError::invalid("Invalid stored evidence"))?;
    let input = DraftInput {
        purpose: row["purpose"].as_str().unwrap_or_default().to_owned(),
        template: row["template_snapshot"].clone(),
        sources: saved
            .iter()
            .map(|s| DraftSource {
                id: s.id,
                title: s.title.clone(),
                text: s.text.clone(),
                source_date: s.source_date.clone(),
            })
            .collect(),
    };
    let changed=sqlx::query("UPDATE clinical_draft_jobs SET state='running',started_at=clock_timestamp() WHERE id=$1 AND state='queued'").bind(id).execute(&mut *tx).await?.rows_affected();
    if changed != 1 {
        return Err(ApiError::conflict("This generation is no longer queued"));
    }
    access::audit(
        &mut tx,
        &actor,
        "generation_started",
        "clinical_draft_jobs",
        Some(id),
    )
    .await?;
    tx.commit().await?;
    let generation = clinical_model::generate(input);
    tokio::pin!(generation);
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    let output = loop {
        tokio::select! {out=&mut generation=>break out?,_=interval.tick()=>{sqlx::query("SELECT 1").execute(&mut *owner).await?;let mut tx=crate::db::begin_background(pool).await?;authorized(&mut tx,id).await?;tx.commit().await?;}}
    };
    let mut tx = crate::db::begin_background(pool).await?;
    let (actor, _) = authorized(&mut tx, id).await?;
    let changed=sqlx::query("UPDATE clinical_draft_jobs SET state='ready',output=$2,completed_at=clock_timestamp() WHERE id=$1 AND state='running'").bind(id).bind(json!(output)).execute(&mut *tx).await?.rows_affected();
    if changed != 1 {
        return Err(ApiError::conflict("This generation is no longer running"));
    }
    access::audit(
        &mut tx,
        &actor,
        "generation_ready",
        "clinical_draft_jobs",
        Some(id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
/// One run of the worker: take ownership of the queue, then work it until the owning
/// session is lost. The supervisor in `crate::workers` decides what happens next.
pub(crate) async fn run(pool: PgPool, status: Arc<Status>) -> Exit {
    // Recovery reads and terminal writes use this advisory-lock session only.
    // A pooled running scan is not a fence after this connection dies.
    let Ok(mut owner) = crate::db::dedicated(&pool).await else {
        log::error!("Clinical drafting worker connection unavailable");
        return Exit::ConnectionUnavailable;
    };
    let locked = sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .fetch_one(&mut owner)
        .await;
    match locked {
        Ok(true) => {}
        Ok(false) => {
            // Another session owns the queue. Close this one now instead of leaving
            // the server to notice a dropped socket.
            let _ = owner.close().await;
            return Exit::LockHeld;
        }
        Err(_) => return Exit::ConnectionUnavailable,
    }
    status.running();
    let mut interrupted: Option<VecDeque<Uuid>> = None;
    let mut unsaved: Option<(Uuid, &'static str)> = None;
    loop {
        if interrupted.is_none() {
            match owned_interrupted(&mut owner).await {
                Ok(ids) => interrupted = Some(ids),
                Err(_) => {
                    if same_session_unusable(&mut owner).await {
                        log::error!("Clinical drafting worker ownership lost");
                        return Exit::OwnershipLost;
                    }
                    log::error!("Clinical drafting recovery query failed");
                    tokio::time::sleep(TERMINAL_SAVE_BACKOFF).await;
                    continue;
                }
            }
        }
        if let Some(ids) = interrupted.as_mut() {
            if let Some(id) = ids.front().copied() {
                match persist_terminal(&mut owner, id, "worker_interrupted").await {
                    TerminalPersist::Saved => {
                        ids.pop_front();
                    }
                    TerminalPersist::Retry => {
                        log::error!("Clinical drafting recovery could not be saved for {id}");
                        tokio::time::sleep(TERMINAL_SAVE_BACKOFF).await;
                    }
                    TerminalPersist::OwnershipLost => {
                        log::error!("Clinical drafting worker ownership lost");
                        return Exit::OwnershipLost;
                    }
                }
                continue;
            }
        }
        if let Some((id, code)) = unsaved {
            match persist_terminal(&mut owner, id, code).await {
                TerminalPersist::Saved => unsaved = None,
                TerminalPersist::Retry => {
                    log::error!("Clinical drafting failure could not be saved for {id} ({code})");
                    tokio::time::sleep(TERMINAL_SAVE_BACKOFF).await;
                    continue;
                }
                TerminalPersist::OwnershipLost => {
                    log::error!("Clinical drafting worker ownership lost");
                    return Exit::OwnershipLost;
                }
            }
        }
        // Do not acquire another job until this session has stored a pending terminal write.
        if same_session_unusable(&mut owner).await {
            log::error!("Clinical drafting worker ownership lost");
            return Exit::OwnershipLost;
        }
        match next_queued(&mut owner).await {
            Ok(Some(id)) => {
                if let Err(error) = process(&pool, &mut owner, id).await {
                    unsaved = Some((id, error.code));
                    continue;
                }
            }
            Ok(None) => {}
            Err(_) => {
                if same_session_unusable(&mut owner).await {
                    log::error!("Clinical drafting worker ownership lost");
                    return Exit::OwnershipLost;
                }
                log::error!("Clinical drafting queue unavailable");
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[cfg(test)]
mod tests {
    include!("worker_terminal_tests.rs");

    #[actix_web::test]
    #[ignore = "Requires isolated nexus_test schemas"]
    async fn draft_run_yields_to_another_owner_and_ends_when_its_session_is_lost() {
        let harness = Harness::create("nexus_draft_supervise")
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let result = crate::workers::testing::ownership_case(
            &harness.pool,
            &harness.control,
            ADVISORY_LOCK_KEY,
            super::run,
        )
        .await;
        let cleanup = harness.cleanup().await;
        result.unwrap();
        cleanup.unwrap();
    }
}
