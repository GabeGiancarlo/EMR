use super::*;
use serde::Serialize;
use std::collections::HashSet;
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Segment {
    pub id: Uuid,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub speaker_participant_id: Option<Uuid>,
    pub speaker_verified: bool,
    pub patient_id: Option<Uuid>,
    pub context_verified: bool,
    pub uncertain: bool,
    pub excluded: bool,
    pub review_note: String,
}
fn validate(segments: &[Segment], duration: i64, people: &[Value], approve: bool) -> Result<()> {
    if segments.len() > 10000
        || serde_json::to_vec(segments)
            .map_err(|_| ApiError::invalid("Invalid segments"))?
            .len()
            > 900000
    {
        return Err(ApiError::invalid(
            "Transcript review exceeds its segment or text limit",
        ));
    }
    let mut ids = HashSet::new();
    let mut previous = -1;
    let mut included = 0;
    for s in segments {
        if !ids.insert(s.id)
            || s.start_ms < 0
            || s.end_ms < s.start_ms
            || s.end_ms > duration
            || s.start_ms < previous
            || s.text.len() > 20000
            || s.text.contains('\0')
            || s.review_note.len() > 2000
        {
            return Err(ApiError::invalid(
                "Segments need unique identifiers, ordered valid timestamps and bounded text",
            ));
        }
        previous = s.start_ms;
        let person = s
            .speaker_participant_id
            .and_then(|id| people.iter().find(|p| p["id"] == id.to_string()));
        if (s.speaker_participant_id.is_some() && person.is_none())
            || (s.speaker_verified && person.is_none())
        {
            return Err(ApiError::invalid(
                "Choose a speaker from this recording's participants",
            ));
        }
        if let Some(client) = s.patient_id {
            if !people.iter().any(|p| p["client_id"] == client.to_string())
                || person.is_some_and(|p| {
                    p["client_id"].is_string() && p["client_id"] != client.to_string()
                })
            {
                return Err(ApiError::invalid(
                    "A client's speech cannot be attributed to another patient's chart",
                ));
            }
        } else if s.context_verified {
            return Err(ApiError::invalid(
                "Choose the patient before verifying patient context",
            ));
        }
        if !s.excluded {
            included += 1;
            if approve
                && (s.text.trim().is_empty()
                    || s.end_ms <= s.start_ms
                    || !s.speaker_verified
                    || s.uncertain
                    || (s.patient_id.is_some() && !s.context_verified))
            {
                return Err(ApiError::conflict("Resolve uncertainty and verify the speaker and any patient context for every included segment, or explicitly exclude it"));
            }
        }
    }
    if approve && included == 0 {
        return Err(ApiError::conflict(
            "There are no reviewed speech segments to approve",
        ));
    }
    Ok(())
}
async fn current(tx: &mut Transaction<'_, Postgres>, doc: &Value, expected: Uuid) -> Result<Value> {
    if uid(doc, "current_version_id")? != expected {
        return Err(ApiError::conflict("Another transcript version was saved. Review the latest version before applying your changes"));
    }
    Ok(sqlx::query_scalar(
        "SELECT to_jsonb(v) FROM transcript_versions v WHERE id=$1 AND transcript_id=$2 FOR UPDATE",
    )
    .bind(expected)
    .bind(uid(doc, "id")?)
    .fetch_one(&mut **tx)
    .await?)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionQuery {
    version_id: Option<Uuid>,
}
pub async fn read(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    q: web::Query<VersionQuery>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, _, doc) = scoped_doc(&req, &state, &mut tx, id.into_inner()).await?;
    let result = detail(&mut tx, &actor, &doc, q.version_id).await?;
    access::audit(
        &mut tx,
        &actor,
        "read",
        "transcripts",
        Some(uid(&doc, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(result))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    expected_version_id: Uuid,
    reason: String,
    segments: Vec<Segment>,
}
pub async fn save(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Json<Save>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room, mut doc) = scoped_doc(&req, &state, &mut tx, id.into_inner()).await?;
    authorize_processing(&mut tx, &room, &doc).await?;
    let before = current(&mut tx, &doc, body.expected_version_id).await?;
    if !(10..=2000).contains(&body.reason.trim().chars().count()) {
        return Err(ApiError::invalid(
            "Explain this revision in 10–2000 characters",
        ));
    }
    validate(
        &body.segments,
        doc["duration_ms"].as_i64().unwrap_or(0),
        &participants_for(&mut tx, &doc).await?,
        false,
    )?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO transcript_versions(id,organization_id,transcript_id,number,parent_id,segments,reason,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(id).bind(actor.org).bind(uid(&doc,"id")?).bind(before["number"].as_i64().unwrap_or(0) as i32+1).bind(body.expected_version_id).bind(json!(body.segments)).bind(body.reason.trim()).bind(actor.id).execute(&mut *tx).await?;
    sqlx::query("UPDATE transcripts SET current_version_id=$2 WHERE id=$1")
        .bind(uid(&doc, "id")?)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    doc["current_version_id"] = json!(id);
    access::audit(&mut tx, &actor, "revise", "transcript_versions", Some(id)).await?;
    let out = detail(&mut tx, &actor, &doc, None).await?;
    tx.commit().await?;
    Ok(no_store(out))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approve {
    expected_version_id: Uuid,
    attested: bool,
}
pub async fn approve(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Json<Approve>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room, doc) = scoped_doc(&req, &state, &mut tx, id.into_inner()).await?;
    authorize_processing(&mut tx, &room, &doc).await?;
    let before = current(&mut tx, &doc, body.expected_version_id).await?;
    if before["state"] != "draft" || !body.attested {
        return Err(ApiError::conflict(
            "Explicitly attest to the review of this current draft",
        ));
    }
    let segments: Vec<Segment> = serde_json::from_value(before["segments"].clone())
        .map_err(|_| ApiError::invalid("Invalid stored transcript"))?;
    validate(
        &segments,
        doc["duration_ms"].as_i64().unwrap_or(0),
        &participants_for(&mut tx, &doc).await?,
        true,
    )?;
    sqlx::query("UPDATE transcript_versions SET state='reviewed',reviewed_by=$2,reviewed_at=clock_timestamp() WHERE id=$1").bind(body.expected_version_id).bind(actor.id).execute(&mut *tx).await?;
    access::audit(
        &mut tx,
        &actor,
        "approve",
        "transcript_versions",
        Some(body.expected_version_id),
    )
    .await?;
    let out = detail(&mut tx, &actor, &doc, None).await?;
    tx.commit().await?;
    Ok(no_store(out))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Excerpt {
    version_id: Uuid,
    client_id: Uuid,
    segment_ids: Vec<Uuid>,
}
pub async fn excerpt(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Json<Excerpt>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room, doc) = scoped_doc(&req, &state, &mut tx, id.into_inner()).await?;
    authorize_processing(&mut tx, &room, &doc).await?;
    let before = current(&mut tx, &doc, body.version_id).await?;
    if before["state"] != "reviewed" {
        return Err(ApiError::conflict(
            "Approve the current transcript before creating a patient excerpt",
        ));
    }
    let client = records::get(&mut tx, "clients", body.client_id, actor.org, false).await?;
    records::scope(&mut tx, &actor, "clients", &client).await?;
    let ids: HashSet<Uuid> = body.segment_ids.iter().copied().collect();
    if ids.is_empty() || ids.len() != body.segment_ids.len() {
        return Err(ApiError::invalid("Choose unique reviewed segments"));
    }
    let segments: Vec<Segment> = serde_json::from_value(before["segments"].clone())
        .map_err(|_| ApiError::invalid("Invalid stored transcript"))?;
    let selected: Vec<&Segment> = segments.iter().filter(|s| ids.contains(&s.id)).collect();
    if selected.len() != ids.len()
        || selected.iter().any(|s| {
            s.excluded
                || s.uncertain
                || !s.speaker_verified
                || !s.context_verified
                || s.patient_id != Some(body.client_id)
        })
    {
        return Err(ApiError::denied());
    }
    let text = selected
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() || text.len() > 100000 {
        return Err(ApiError::invalid(
            "The selected excerpt is empty or too long",
        ));
    }
    // The attendance row is the canonical group-to-patient session link. A
    // separate session that happens to mention this group is not that visit.
    let session:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT s.id,s.episode_id FROM sessions s WHERE s.organization_id=$1 AND s.client_id=$2 AND (s.id=$3 OR ($3::uuid IS NULL AND EXISTS(SELECT 1 FROM group_attendance a WHERE a.organization_id=$1 AND a.group_id=$4 AND a.client_id=$2 AND a.session_id=s.id AND a.episode_id=s.episode_id)))").bind(actor.org).bind(body.client_id).bind(room["session_id"].as_str().and_then(|s|Uuid::parse_str(s).ok())).bind(room["group_id"].as_str().and_then(|s|Uuid::parse_str(s).ok())).fetch_optional(&mut *tx).await?;
    let (session, episode) = session.ok_or_else(ApiError::denied)?;
    let row:Value=sqlx::query_scalar("INSERT INTO transcript_excerpts(id,organization_id,transcript_id,version_id,client_id,episode_id,session_id,segment_ids,text,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING to_jsonb(transcript_excerpts)").bind(Uuid::new_v4()).bind(actor.org).bind(uid(&doc,"id")?).bind(body.version_id).bind(body.client_id).bind(episode).bind(session).bind(&body.segment_ids).bind(text).bind(actor.id).fetch_one(&mut *tx).await?;
    access::audit(
        &mut tx,
        &actor,
        "create",
        "transcript_excerpts",
        Some(uid(&row, "id")?),
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(row))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn segment() -> Segment {
        Segment {
            id: Uuid::new_v4(),
            start_ms: 0,
            end_ms: 1000,
            text: "Synthetic reviewed phrase".into(),
            speaker_participant_id: None,
            speaker_verified: false,
            patient_id: None,
            context_verified: false,
            uncertain: false,
            excluded: false,
            review_note: String::new(),
        }
    }
    #[test]
    fn approval_requires_real_review_and_cannot_cross_patient_speakers() {
        let person = Uuid::new_v4();
        let client = Uuid::new_v4();
        let another = Uuid::new_v4();
        let people = vec![
            json!({"id":person,"client_id":client}),
            json!({"id":Uuid::new_v4(),"client_id":another}),
        ];
        let mut s = segment();
        assert!(validate(&[s.clone()], 2000, &people, true).is_err());
        s.speaker_participant_id = Some(person);
        s.speaker_verified = true;
        s.patient_id = Some(client);
        s.context_verified = true;
        assert!(validate(&[s.clone()], 2000, &people, true).is_ok());
        s.patient_id = Some(another);
        assert!(validate(&[s.clone()], 2000, &people, false).is_err());
        s.patient_id = Some(client);
        s.uncertain = true;
        assert!(validate(&[s], 2000, &people, true).is_err());
    }
    #[test]
    fn timestamps_identifiers_and_empty_approval_are_checked() {
        let s = segment();
        assert!(validate(&[s.clone(), s.clone()], 2000, &[], false).is_err());
        assert!(validate(&[s], 500, &[], false).is_err());
        assert!(validate(&[], 1000, &[], true).is_err());
        assert!(validate(&[], 1000, &[], false).is_ok());
    }
}
