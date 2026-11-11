//! Recover source links from immutable amendment ancestry, including older amendments.
use crate::{access::Actor, records, ApiError, Result};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct NoteLineage {
    pub row: Value,
    pub ids: Vec<Uuid>,
}

pub(crate) async fn resolve(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    note: &Value,
) -> Result<NoteLineage> {
    let mut result = NoteLineage {
        row: note.clone(),
        ids: Vec::new(),
    };
    let mut current = note.clone();
    loop {
        let id = records::uuid(&current, "id")?;
        if result.ids.len() >= 100 || result.ids.contains(&id) {
            return Err(ApiError::conflict(
                "The note amendment history is cyclic or exceeds the 100-record review limit",
            ));
        }
        if current["organization_id"] != actor.org.to_string()
            || current["client_id"] != note["client_id"]
            || (!result.ids.is_empty() && current["status"] != "signed")
        {
            return Err(ApiError::conflict(
                "The note amendment history has inconsistent patient or signature context",
            ));
        }
        result.ids.push(id);
        for key in ["session_id", "form_response_id", "clinical_draft_job_id"] {
            if result.row[key].is_null() {
                result.row[key] = current[key].clone();
            } else if !current[key].is_null() && result.row[key] != current[key] {
                return Err(ApiError::conflict(
                    "The note amendment history has conflicting source links",
                ));
            }
        }
        if current["amends_note_id"].is_null() {
            return Ok(result);
        }
        current = records::get(
            tx,
            "notes",
            records::uuid(&current, "amends_note_id")?,
            actor.org,
            false,
        )
        .await?;
    }
}

pub(crate) async fn episode(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    note: &Value,
) -> Result<Option<Uuid>> {
    let mut episode = None;
    for (key, table) in [
        ("session_id", "sessions"),
        ("form_response_id", "form_responses"),
        ("clinical_draft_job_id", "clinical_draft_jobs"),
    ] {
        if note[key].is_null() {
            continue;
        }
        // Table names are fixed above, never supplied by a request.
        let row: Option<Value> = sqlx::query_scalar(&format!(
            "SELECT to_jsonb(r) FROM {table} r WHERE id=$1 AND organization_id=$2 AND client_id=$3"
        ))
        .bind(records::uuid(note, key)?)
        .bind(actor.org)
        .bind(records::uuid(note, "client_id")?)
        .fetch_optional(&mut **tx)
        .await?;
        let row = row.ok_or_else(|| {
            ApiError::conflict("The note source is not in the same patient record")
        })?;
        if !row["episode_id"].is_null() {
            let id = records::uuid(&row, "episode_id")?;
            if episode.is_some_and(|saved| saved != id) {
                return Err(ApiError::conflict(
                    "The note sources have conflicting episodes",
                ));
            }
            episode = Some(id);
        }
    }
    Ok(episode)
}
