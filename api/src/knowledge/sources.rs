use super::*;
use crate::telehealth::recording::transcripts::excerpts;
fn id(row: &Value, key: &str) -> Result<Uuid> {
    records::uuid(row, key)
}
fn optional(row: &Value, key: &str) -> Result<Option<Uuid>> {
    if row[key].is_null() {
        Ok(None)
    } else {
        Ok(Some(id(row, key)?))
    }
}
fn text(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap_or_default().to_owned()
}
fn document(
    actor: &Actor,
    client: Uuid,
    kind: &str,
    row: &Value,
    title: String,
    body: String,
    date: &str,
) -> Result<Document> {
    if body.trim().is_empty() || body.len() > 200000 || body.contains('\0') {
        return Err(ApiError::conflict("An approved source is empty or exceeds the current 200000-byte indexing limit. No partial index was published."));
    }
    let source_id = id(row, "id")?;
    let version = if kind == "transcript_excerpt" {
        id(row, "version_id")?
    } else {
        source_id
    };
    let episode_id = optional(row, "episode_id")?;
    let session_id = optional(row, "session_id")?;
    let source_date = chrono::DateTime::parse_from_rfc3339(row[date].as_str().unwrap_or_default())
        .map_err(|_| ApiError::conflict("An approved source has no valid source date"))?
        .to_rfc3339();
    let source_hash=sync::hash(&json!({"body":body,"title":title,"date":source_date,"episode":episode_id,"session":session_id,"version":version}).to_string());
    let id = sync::stable_id(&format!(
        "{}:{client}:{kind}:{source_id}:{version}:{source_hash}",
        actor.org
    ));
    Ok(Document {
        id,
        organization_id: actor.org,
        client_id: client,
        source_kind: kind.into(),
        source_id,
        source_version: version,
        source_hash,
        title,
        body,
        source_date,
        episode_id,
        session_id,
        classification: "clinical".into(),
    })
}
async fn rows(
    tx: &mut Transaction<'_, Postgres>,
    sql: &str,
    actor: &Actor,
    client: Uuid,
) -> Result<Vec<Value>> {
    let rows = sqlx::query_scalar(sql)
        .bind(actor.org)
        .bind(client)
        .fetch_all(&mut **tx)
        .await?;
    if rows.len() > 1000 {
        return Err(ApiError::conflict("This patient exceeds the current source indexing limit. No partial index was published."));
    }
    Ok(rows)
}
fn unavailable(error: &ApiError) -> bool {
    matches!(
        error.status,
        actix_web::http::StatusCode::FORBIDDEN
            | actix_web::http::StatusCode::CONFLICT
            | actix_web::http::StatusCode::NOT_FOUND
    )
}
pub(super) async fn collect(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: Uuid,
) -> Result<Vec<Document>> {
    let mut out = Vec::new();
    let notes=rows(tx,"SELECT to_jsonb(n) FROM notes n WHERE n.organization_id=$1 AND n.client_id=$2 AND n.status='signed' AND NOT EXISTS(SELECT 1 FROM notes newer WHERE newer.amends_note_id=n.id AND newer.organization_id=$1 AND newer.client_id=$2 AND newer.status='signed') ORDER BY n.id LIMIT 1001",actor,client).await?;
    for n in notes {
        let mut n = crate::note_lineage::resolve(tx, actor, &n).await?.row;
        n["episode_id"] = json!(crate::note_lineage::episode(tx, actor, &n).await?);
        if let Some(form) = optional(&n, "form_response_id")? {
            let f = records::get(tx, "form_responses", form, actor.org, false).await?;
            if let Err(e) = excerpts::validate_form(tx, actor, &f).await {
                if unavailable(&e) {
                    continue;
                }
                return Err(e);
            }
        }
        out.push(document(
            actor,
            client,
            "note",
            &n,
            text(&n, "title"),
            text(&n, "body"),
            "signed_at",
        )?);
    }
    let forms=rows(tx,"SELECT to_jsonb(f) FROM form_responses f WHERE f.organization_id=$1 AND f.client_id=$2 AND f.status='signed' AND NOT EXISTS(SELECT 1 FROM form_responses newer WHERE newer.amends_response_id=f.id AND newer.organization_id=$1 AND newer.client_id=$2 AND newer.status='signed') ORDER BY f.id LIMIT 1001",actor,client).await?;
    for f in forms {
        if let Err(e) = excerpts::validate_form(tx, actor, &f).await {
            if unavailable(&e) {
                continue;
            }
            return Err(e);
        }
        let fields = f["template_snapshot"]["fields"]
            .as_array()
            .ok_or_else(|| ApiError::conflict("An approved form has no valid template snapshot"))?;
        let mut parts = Vec::new();
        for field in fields {
            let key = field["key"].as_str().unwrap_or_default();
            let answer = &f["answers"][key];
            if !answer.is_null() {
                let answer = answer
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| answer.to_string());
                if !answer.trim().is_empty() {
                    parts.push(format!(
                        "{}: {}",
                        field["label"].as_str().unwrap_or(key),
                        answer
                    ));
                }
            }
        }
        if parts.is_empty() {
            continue;
        }
        out.push(document(
            actor,
            client,
            "form_response",
            &f,
            f["template_snapshot"]["name"]
                .as_str()
                .unwrap_or("Signed form")
                .into(),
            parts.join("\n"),
            "signed_at",
        )?);
    }
    let plans=rows(tx,"SELECT to_jsonb(p) FROM treatment_plans p WHERE p.organization_id=$1 AND p.client_id=$2 AND p.signed_at IS NOT NULL AND p.status IN ('active','completed') ORDER BY p.id LIMIT 1001",actor,client).await?;
    for p in plans {
        let goals:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(g) FROM goals g WHERE plan_id=$1 AND organization_id=$2 ORDER BY created_at,id").bind(id(&p,"id")?).bind(actor.org).fetch_all(&mut **tx).await?;
        let mut parts = vec![
            format!("Plan status: {}", text(&p, "status")),
            format!("Presenting needs: {}", text(&p, "presenting_needs")),
            format!("Strengths: {}", text(&p, "strengths")),
        ];
        for g in goals {
            parts.push(format!(
                "Goal: {}\nMeasure: {}\nBaseline: {}\nTarget: {}\nTarget date: {}",
                text(&g, "description"),
                text(&g, "measure"),
                text(&g, "baseline"),
                text(&g, "target"),
                g["target_date"].as_str().unwrap_or("Unspecified")
            ));
        }
        out.push(document(
            actor,
            client,
            "treatment_plan",
            &p,
            text(&p, "title"),
            parts.join("\n\n"),
            "signed_at",
        )?);
    }
    let reviews=rows(tx,"SELECT to_jsonb(r) || jsonb_build_object('episode_id',p.episode_id,'plan_title',p.title) FROM plan_reviews r JOIN treatment_plans p ON p.id=r.plan_id AND p.organization_id=r.organization_id AND p.client_id=r.client_id WHERE r.organization_id=$1 AND r.client_id=$2 AND p.signed_at IS NOT NULL AND p.status IN ('active','completed') ORDER BY r.id LIMIT 1001",actor,client).await?;
    for r in reviews {
        let mut parts = vec![text(&r, "summary")];
        if let Some(progress) = r["goal_progress"].as_array() {
            for entry in progress {
                let goal = r["goal_snapshot"]
                    .as_array()
                    .and_then(|goals| goals.iter().find(|g| g["id"] == entry["goal_id"]));
                parts.push(format!(
                    "Goal: {}\nStatus: {}\nObservation: {}",
                    goal.and_then(|g| g["description"].as_str())
                        .unwrap_or("Referenced goal"),
                    text(entry, "status"),
                    text(entry, "observation")
                ));
            }
        }
        out.push(document(
            actor,
            client,
            "plan_review",
            &r,
            format!("Review: {}", text(&r, "plan_title")),
            parts.join("\n\n"),
            "created_at",
        )?);
    }
    let excerpts=rows(tx,"SELECT to_jsonb(e) FROM transcript_excerpts e JOIN transcripts t ON t.id=e.transcript_id JOIN transcript_versions v ON v.id=e.version_id WHERE e.organization_id=$1 AND e.client_id=$2 AND t.current_version_id=e.version_id AND v.state='reviewed' ORDER BY e.id LIMIT 1001",actor,client).await?;
    for e in excerpts {
        match excerpts::source(tx, actor, id(&e, "id")?).await {
            Ok(_) => {}
            Err(e) if unavailable(&e) => continue,
            Err(e) => return Err(e),
        };
        out.push(document(
            actor,
            client,
            "transcript_excerpt",
            &e,
            "Reviewed patient transcript excerpt".into(),
            text(&e, "text"),
            "created_at",
        )?);
    }
    Ok(out)
}
