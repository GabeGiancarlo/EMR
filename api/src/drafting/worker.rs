use super::*;
use sqlx::{PgConnection, PgPool};
use std::time::Duration;

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
async fn failed(pool: &PgPool, id: Uuid, code: &str) -> Result<()> {
    let mut tx = pool.begin().await?;
    let row:Option<Value>=sqlx::query_scalar("UPDATE clinical_draft_jobs SET state='failed',error_code=$2,completed_at=clock_timestamp() WHERE id=$1 AND state IN ('queued','running') RETURNING to_jsonb(clinical_draft_jobs)").bind(id).bind(code).fetch_optional(&mut *tx).await?;
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
            "generation_failed",
            "clinical_draft_jobs",
            Some(id),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn process(pool: &PgPool, owner: &mut PgConnection, id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
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
        tokio::select! {out=&mut generation=>break out?,_=interval.tick()=>{sqlx::query("SELECT 1").execute(&mut *owner).await?;let mut tx=pool.begin().await?;authorized(&mut tx,id).await?;tx.commit().await?;}}
    };
    let mut tx = pool.begin().await?;
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
pub(crate) async fn start(pool: PgPool) {
    let Ok(connection) = pool.acquire().await else {
        log::error!("Clinical drafting worker connection unavailable");
        return;
    };
    let mut owner = connection.detach();
    if !sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock(734903222)")
        .fetch_one(&mut owner)
        .await
        .unwrap_or(false)
    {
        return;
    }
    let interrupted =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM clinical_draft_jobs WHERE state='running'")
            .fetch_all(&pool)
            .await;
    let Ok(interrupted) = interrupted else {
        log::error!("Clinical drafting recovery failed");
        return;
    };
    for id in interrupted {
        if failed(&pool, id, "worker_interrupted").await.is_err() {
            log::error!("Clinical drafting recovery could not be saved");
            return;
        }
    }
    loop {
        if sqlx::query("SELECT 1").execute(&mut owner).await.is_err() {
            log::error!("Clinical drafting worker ownership lost");
            return;
        }
        match sqlx::query_scalar::<_,Uuid>("SELECT id FROM clinical_draft_jobs WHERE state='queued' ORDER BY created_at,id LIMIT 1").fetch_optional(&pool).await{
   Ok(Some(id))=>{if let Err(e)=process(&pool,&mut owner,id).await{if failed(&pool,id,e.code).await.is_err(){log::error!("Clinical drafting failure could not be saved");}}},
   Ok(None)=>{},Err(_)=>log::error!("Clinical drafting queue unavailable")
  }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
