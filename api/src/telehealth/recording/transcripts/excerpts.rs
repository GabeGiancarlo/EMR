//! Patient excerpts are immutable derivatives; a form keeps their exact version and context.
use super::*;
use serde_json::Map;

pub(crate) async fn source(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    id: Uuid,
) -> Result<Value> {
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
        return Err(ApiError::denied());
    }
    let excerpt: Value = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM transcript_excerpts e WHERE id=$1 AND organization_id=$2",
    )
    .bind(id)
    .bind(actor.org)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::missing)?;
    let client = records::get(tx, "clients", uid(&excerpt, "client_id")?, actor.org, false).await?;
    records::scope(tx, actor, "clients", &client).await?;
    let doc: Value = sqlx::query_scalar(
        "SELECT to_jsonb(t) FROM transcripts t WHERE id=$1 AND organization_id=$2 FOR UPDATE",
    )
    .bind(uid(&excerpt, "transcript_id")?)
    .bind(actor.org)
    .fetch_one(&mut **tx)
    .await?;
    if doc["current_version_id"] != excerpt["version_id"] {
        return Err(ApiError::conflict("This transcript has a newer version. Review it and select a current patient excerpt before using this form."));
    }
    let state: String = sqlx::query_scalar("SELECT state FROM transcript_versions WHERE id=$1")
        .bind(uid(&excerpt, "version_id")?)
        .fetch_one(&mut **tx)
        .await?;
    if state != "reviewed" {
        return Err(ApiError::conflict(
            "Review the current transcript before using its excerpt",
        ));
    }
    let room = room(tx, uid(&doc, "room_id")?, Some(actor.org)).await?;
    authorize_processing(tx, &room, &doc).await?;
    Ok(excerpt)
}

pub(crate) async fn bind_form(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    data: &mut Map<String, Value>,
    before: Option<&Value>,
) -> Result<()> {
    let link = data
        .get("transcript_excerpt_id")
        .or_else(|| before.map(|v| &v["transcript_excerpt_id"]))
        .cloned()
        .unwrap_or(Value::Null);
    if link.is_null() {
        if before.is_some_and(|old| old["transcript_excerpt_id"].is_string()) {
            return Err(ApiError::conflict(
                "Keep the source link or choose a current excerpt; do not detach provenance",
            ));
        }
        return Ok(());
    }
    let id = link
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| ApiError::invalid("Choose a valid patient excerpt"))?;
    let excerpt = source(tx, actor, id).await?;
    for key in ["client_id", "episode_id", "session_id"] {
        let selected = data.get(key).or_else(|| before.map(|v| &v[key]));
        if selected.is_some_and(|v| !v.is_null() && v != &excerpt[key])
            || before.is_some_and(|v| v[key] != excerpt[key])
        {
            return Err(ApiError::conflict(
                "The excerpt belongs to a different patient, episode, or session",
            ));
        }
        data.insert(key.into(), excerpt[key].clone());
    }
    if data
        .get("source_excerpt")
        .is_some_and(|v| v != &excerpt["text"])
        || data.get("source_type").is_some_and(|v| v != "transcript")
    {
        return Err(ApiError::invalid(
            "Linked source text must exactly match the saved patient excerpt",
        ));
    }
    if before.is_some_and(|v| v["transcript_excerpt_id"] != link) {
        // Never inherit an omitted answer bundle from a different source, and never
        // discard answers explicitly remapped by the caller against the new excerpt.
        if data.contains_key("answers") != data.contains_key("field_provenance") {
            return Err(ApiError::invalid(
                "Provide both answers and field provenance when changing a form source",
            ));
        }
        data.entry("answers".to_owned()).or_insert(json!({}));
        data.entry("field_provenance".to_owned())
            .or_insert(json!({}));
    }
    data.insert("transcript_excerpt_id".into(), link);
    data.insert("source_type".into(), json!("transcript"));
    data.insert("source_excerpt".into(), excerpt["text"].clone());
    Ok(())
}

pub(crate) async fn validate_form(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    form: &Value,
) -> Result<()> {
    if form["transcript_excerpt_id"].is_null() {
        return Ok(());
    }
    let excerpt = source(tx, actor, uid(form, "transcript_excerpt_id")?).await?;
    if ["client_id", "episode_id", "session_id"]
        .iter()
        .any(|key| form[*key] != excerpt[*key])
        || form["source_excerpt"] != excerpt["text"]
        || form["source_type"] != "transcript"
    {
        return Err(ApiError::conflict(
            "The form source does not match its immutable excerpt",
        ));
    }
    Ok(())
}

pub async fn list(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let initial = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, initial.org, "form_responses").await?;
    if !matches!(actor.role.as_str(), "admin" | "clinical_lead" | "clinician") {
        return Err(ApiError::denied());
    }
    let id = id.into_inner();
    let client = records::get(&mut tx, "clients", id, actor.org, false).await?;
    records::scope(&mut tx, &actor, "clients", &client).await?;
    let rows: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(e) FROM transcript_excerpts e JOIN transcripts t ON t.id=e.transcript_id JOIN transcript_versions v ON v.id=e.version_id WHERE e.client_id=$1 AND e.organization_id=$2 AND t.current_version_id=e.version_id AND v.state='reviewed' ORDER BY e.created_at DESC LIMIT 100").bind(id).bind(actor.org).fetch_all(&mut *tx).await?;
    access::audit(&mut tx, &actor, "read", "transcript_excerpts", Some(id)).await?;
    tx.commit().await?;
    Ok(no_store(json!({"excerpts":rows})))
}
