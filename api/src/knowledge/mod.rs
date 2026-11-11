//! Derived approved-record search; canonical clinical records remain authoritative.
mod sources;
mod sync;
use crate::{
    access::{self, Actor},
    records, ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
pub(crate) use sync::refresh;
use uuid::Uuid;

pub(crate) const KINDS: &[&str] = &[
    "note",
    "form_response",
    "treatment_plan",
    "plan_review",
    "transcript_excerpt",
];
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct KnowledgeChunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub source_kind: String,
    pub source_id: Uuid,
    pub source_version: Uuid,
    pub source_hash: String,
    pub title: String,
    pub text: String,
    pub source_date: String,
    pub episode_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub classification: String,
    pub ordinal: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Document {
    id: Uuid,
    organization_id: Uuid,
    client_id: Uuid,
    source_kind: String,
    source_id: Uuid,
    source_version: Uuid,
    source_hash: String,
    title: String,
    body: String,
    source_date: String,
    episode_id: Option<Uuid>,
    session_id: Option<Uuid>,
    classification: String,
}
const PROJECTION:&str="(to_jsonb(d)-ARRAY['id','body','indexed_at']) || jsonb_build_object('id',c.id,'document_id',d.id,'ordinal',c.ordinal,'text',c.text)";
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/clients/{id}/knowledge", web::get().to(search))
        .route("/clients/{id}/knowledge", web::post().to(rebuild))
        .route(
            "/clients/{id}/knowledge/sources/{document}",
            web::get().to(source),
        );
}
fn response(value: Value) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(value)
}
pub(crate) async fn authorize(
    req: &HttpRequest,
    tx: &mut Transaction<'_, Postgres>,
    org: Uuid,
    client: Uuid,
) -> Result<Actor> {
    let actor = access::lock_mutation(req, tx, org, "notes").await?;
    check_patient(tx, &actor, client).await?;
    Ok(actor)
}
async fn check_patient(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: Uuid,
) -> Result<()> {
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
        return Err(ApiError::denied());
    }
    let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff WHERE id=$1 AND organization_id=$2 AND active AND role=$3)").bind(actor.id).bind(actor.org).bind(&actor.role).fetch_one(&mut **tx).await?;
    if !current {
        return Err(ApiError::denied());
    }
    let row = records::get(tx, "clients", client, actor.org, false).await?;
    records::scope(tx, actor, "clients", &row).await
}
pub(crate) async fn resolve(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: Uuid,
    ids: &[Uuid],
) -> Result<Vec<KnowledgeChunk>> {
    if ids.is_empty() || ids.len() > 12 || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err(ApiError::invalid(
            "Select 1–12 distinct approved evidence excerpts",
        ));
    }
    refresh(tx, actor, client).await?;
    let sql=format!("SELECT {PROJECTION} FROM knowledge_chunks c JOIN knowledge_documents d ON d.id=c.document_id WHERE c.organization_id=$1 AND c.client_id=$2 AND c.id=ANY($3)");
    let rows: Vec<Value> = sqlx::query_scalar(&sql)
        .bind(actor.org)
        .bind(client)
        .bind(ids)
        .fetch_all(&mut **tx)
        .await?;
    let mut out = Vec::new();
    for v in rows {
        out.push(
            serde_json::from_value::<KnowledgeChunk>(v)
                .map_err(|_| ApiError::conflict("The indexed source could not be read"))?,
        );
    }
    if out.len() != ids.len() {
        return Err(ApiError::missing());
    }
    out.sort_by_key(|s| ids.iter().position(|id| *id == s.id).unwrap_or(usize::MAX));
    Ok(out)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    q: Option<String>,
    episode_id: Option<Uuid>,
    kind: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}
async fn search(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    query: web::Query<Query>,
) -> Result<HttpResponse> {
    let client = id.into_inner();
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = authorize(&req, &mut tx, initial.org, client).await?;
    let q = query.q.as_deref().unwrap_or("").trim();
    let kind = query.kind.as_deref().unwrap_or("");
    let limit = query.limit.unwrap_or(25);
    let offset = query.offset.unwrap_or(0);
    if q.len() > 1000
        || q.contains('\0')
        || (!kind.is_empty() && !KINDS.contains(&kind))
        || !(1..=50).contains(&limit)
        || !(0..=10000).contains(&offset)
    {
        return Err(ApiError::invalid("Use a supported source type, up to 1000 search bytes, 1–50 results and an offset within 10000"));
    }
    if let Some(ep) = query.episode_id {
        let e = records::get(&mut tx, "episodes", ep, actor.org, false).await?;
        if e["client_id"] != client.to_string() {
            return Err(ApiError::denied());
        }
    }
    refresh(&mut tx, &actor, client).await?;
    let filter="c.organization_id=$1 AND c.client_id=$2 AND ($3='' OR c.text_search @@ websearch_to_tsquery('english',$3)) AND ($4='' OR d.source_kind=$4) AND ($5::uuid IS NULL OR d.episode_id=$5)";
    let total:i64=sqlx::query_scalar(&format!("SELECT count(*) FROM knowledge_chunks c JOIN knowledge_documents d ON d.id=c.document_id WHERE {filter}")).bind(actor.org).bind(client).bind(q).bind(kind).bind(query.episode_id).fetch_one(&mut *tx).await?;
    let sql=format!("SELECT {PROJECTION} FROM knowledge_chunks c JOIN knowledge_documents d ON d.id=c.document_id WHERE {filter} ORDER BY CASE WHEN $3='' THEN 0 ELSE ts_rank_cd(c.text_search,websearch_to_tsquery('english',$3)) END DESC,d.source_date DESC,d.id,c.ordinal LIMIT $6 OFFSET $7");
    let results: Vec<Value> = sqlx::query_scalar(&sql)
        .bind(actor.org)
        .bind(client)
        .bind(q)
        .bind(kind)
        .bind(query.episode_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await?;
    let (documents, chunks) = counts(&mut tx, actor.org, client).await?;
    access::audit(
        &mut tx,
        &actor,
        "search",
        "knowledge_documents",
        Some(client),
    )
    .await?;
    tx.commit().await?;
    Ok(response(
        json!({"client_id":client,"indexed_at":chrono::Utc::now(),"document_count":documents,"chunk_count":chunks,"total":total,"limit":limit,"offset":offset,"results":results}),
    ))
}
async fn counts(tx: &mut Transaction<'_, Postgres>, org: Uuid, client: Uuid) -> Result<(i64, i64)> {
    Ok(sqlx::query_as("SELECT (SELECT count(*) FROM knowledge_documents WHERE organization_id=$1 AND client_id=$2),(SELECT count(*) FROM knowledge_chunks WHERE organization_id=$1 AND client_id=$2)").bind(org).bind(client).fetch_one(&mut **tx).await?)
}
async fn rebuild(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let client = id.into_inner();
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = authorize(&req, &mut tx, initial.org, client).await?;
    refresh(&mut tx, &actor, client).await?;
    let (documents, chunks) = counts(&mut tx, actor.org, client).await?;
    access::audit(
        &mut tx,
        &actor,
        "refresh",
        "knowledge_documents",
        Some(client),
    )
    .await?;
    tx.commit().await?;
    Ok(response(
        json!({"client_id":client,"document_count":documents,"chunk_count":chunks,"indexed_at":chrono::Utc::now()}),
    ))
}
async fn source(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse> {
    let (client, id) = path.into_inner();
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = authorize(&req, &mut tx, initial.org, client).await?;
    refresh(&mut tx, &actor, client).await?;
    let document:Value=sqlx::query_scalar("SELECT to_jsonb(d) FROM knowledge_documents d WHERE id=$1 AND organization_id=$2 AND client_id=$3").bind(id).bind(actor.org).bind(client).fetch_optional(&mut *tx).await?.ok_or_else(ApiError::missing)?;
    let chunks:Vec<Value>=sqlx::query_scalar(&format!("SELECT {PROJECTION} FROM knowledge_chunks c JOIN knowledge_documents d ON d.id=c.document_id WHERE d.id=$1 AND d.organization_id=$2 AND d.client_id=$3 ORDER BY c.ordinal")).bind(id).bind(actor.org).bind(client).fetch_all(&mut *tx).await?;
    access::audit(&mut tx, &actor, "read", "knowledge_documents", Some(id)).await?;
    tx.commit().await?;
    Ok(response(json!({"document":document,"chunks":chunks})))
}
