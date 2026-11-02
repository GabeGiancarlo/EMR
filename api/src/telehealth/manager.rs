use super::*;
use crate::AppState;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    session_id: Option<Uuid>,
    group_id: Option<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    client_id: Uuid,
}

pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<Create>,
) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(
        &req,
        &mut tx,
        actor.org,
        if body.group_id.is_some() {
            "groups"
        } else {
            "sessions"
        },
    )
    .await?;
    let care = encounter(&mut tx, &actor, body.session_id, body.group_id).await?;
    if care["status"] != "scheduled" {
        return Err(ApiError::conflict(
            "Telehealth requires a scheduled encounter",
        ));
    }
    // Expired rooms retain their history, while a fresh room receives new credentials.
    sqlx::query("UPDATE telehealth_rooms SET ended_at=clock_timestamp() WHERE organization_id=$1 AND ended_at IS NULL AND expires_at<=clock_timestamp() AND (session_id=$2 OR group_id=$3)").bind(actor.org).bind(body.session_id).bind(body.group_id).execute(&mut *tx).await?;
    let existing:Option<Value>=sqlx::query_scalar("SELECT to_jsonb(r) FROM telehealth_rooms r WHERE organization_id=$1 AND ended_at IS NULL AND (session_id=$2 OR group_id=$3)").bind(actor.org).bind(body.session_id).bind(body.group_id).fetch_optional(&mut *tx).await?;
    let room = if let Some(row) = existing {
        row
    } else {
        let row:Value=sqlx::query_scalar("INSERT INTO telehealth_rooms(id,organization_id,session_id,group_id,created_by,expires_at) VALUES($1,$2,$3,$4,$5,clock_timestamp()+interval '4 hours') RETURNING to_jsonb(telehealth_rooms)").bind(Uuid::new_v4()).bind(actor.org).bind(body.session_id).bind(body.group_id).bind(actor.id).fetch_one(&mut *tx).await?;
        event(&mut tx, &row, None, Some(actor.id), "created").await?;
        audit(&mut tx, &actor, "create", &row).await?;
        row
    };
    tx.commit().await?;
    Ok(no_store(room))
}
async fn managed(
    req: &HttpRequest,
    _state: &AppState,
    id: Uuid,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(Actor, Value)> {
    let actor_id = req
        .headers()
        .get("x-nexus-actor")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(ApiError::denied)?;
    let org: Uuid = sqlx::query_scalar("SELECT organization_id FROM staff WHERE id=$1 AND active")
        .bind(actor_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(ApiError::denied)?;
    let actor = access::lock_mutation(req, tx, org, "sessions").await?;
    let room = room(tx, id, Some(actor.org)).await?;
    manager_scope(tx, &actor, &room).await?;
    Ok((actor, room))
}
pub async fn read(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room) = managed(&req, &state, id.into_inner(), &mut tx).await?;
    let mut people = Vec::new();
    let rows:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(p)-'token_hash' FROM telehealth_participants p WHERE room_id=$1 ORDER BY created_at").bind(uid(&room,"id")?).fetch_all(&mut *tx).await?;
    for row in rows {
        let mut p = public_participant(&row);
        p["client_id"] = row["client_id"].clone();
        p["staff_id"] = row["staff_id"].clone();
        p["eligible"] = json!(eligible(&mut tx, &room, &row).await?);
        people.push(p);
    }
    let events: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM telehealth_events e WHERE room_id=$1 ORDER BY occurred_at,id",
    )
    .bind(uid(&room, "id")?)
    .fetch_all(&mut *tx)
    .await?;
    audit(&mut tx, &actor, "read", &room).await?;
    let invitees:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',c.id,'first_name',c.first_name,'last_name',c.last_name) FROM clients c WHERE c.organization_id=$1 AND (EXISTS(SELECT 1 FROM sessions s JOIN episodes e ON e.id=s.episode_id WHERE s.id=$2 AND s.client_id=c.id AND s.status='scheduled' AND e.status='active') OR EXISTS(SELECT 1 FROM group_attendance a JOIN episodes e ON e.id=a.episode_id JOIN groups g ON g.id=a.group_id WHERE a.group_id=$3 AND a.client_id=c.id AND a.status IN ('scheduled','attended') AND e.status='active' AND g.status='scheduled')) ORDER BY c.last_name,c.first_name,c.id").bind(actor.org).bind(room["session_id"].as_str().and_then(|s|Uuid::parse_str(s).ok())).bind(room["group_id"].as_str().and_then(|s|Uuid::parse_str(s).ok())).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(no_store(
        json!({"room":room,"participants":people,"events":events,"invitees":invitees}),
    ))
}
async fn issue(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    room: &Value,
    client: Option<Uuid>,
) -> Result<Value> {
    live(room)?;
    let care = manager_scope(tx, actor, room).await?;
    if care["status"] != "scheduled" {
        return Err(ApiError::conflict("The encounter is no longer scheduled"));
    }
    let label = if let Some(client) = client {
        let person = records::get(tx, "clients", client, actor.org, false).await?;
        if room["session_id"].is_string() {
            if care["client_id"] != client.to_string() {
                return Err(ApiError::denied());
            }
        } else {
            let enrolled:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM group_attendance a JOIN episodes e ON e.id=a.episode_id WHERE a.organization_id=$1 AND a.group_id=$2 AND a.client_id=$3 AND a.status IN ('scheduled','attended') AND e.status='active')").bind(actor.org).bind(uid(room,"group_id")?).bind(client).fetch_one(&mut **tx).await?;
            if !enrolled {
                return Err(ApiError::conflict(
                    "Invite an eligible participant enrolled in this group",
                ));
            }
        }
        let first = person["first_name"].as_str().unwrap_or("Participant");
        let last = person["last_name"]
            .as_str()
            .and_then(|s| s.chars().next())
            .map(|c| format!(" {c}."))
            .unwrap_or_default();
        format!("{first}{last}")
            .chars()
            .take(100)
            .collect::<String>()
    } else {
        actor.value["name"]
            .as_str()
            .unwrap_or("Clinician")
            .chars()
            .take(100)
            .collect()
    };
    let staff = if client.is_none() {
        Some(actor.id)
    } else {
        None
    };
    let old:Option<Value>=sqlx::query_scalar("SELECT to_jsonb(p) FROM telehealth_participants p WHERE room_id=$1 AND (client_id=$2 OR staff_id=$3)").bind(uid(room,"id")?).bind(client).bind(staff).fetch_optional(&mut **tx).await?;
    if old
        .as_ref()
        .is_some_and(|p| online(p) && matches!(p["state"].as_str(), Some("admitted" | "waiting")))
    {
        return Err(ApiError::conflict("This participant is connected. Remove or leave the current visit before issuing another invitation."));
    }
    let (token, hash) = fresh_token();
    let participant = if let Some(old) = old {
        sqlx::query_scalar::<_,Value>("UPDATE telehealth_participants SET token_hash=$2,token_expires_at=$3,label=$4,state='invited',connection_id=NULL,last_seen_at=NULL WHERE id=$1 RETURNING to_jsonb(telehealth_participants)").bind(uid(&old,"id")?).bind(hash).bind(time(room,"expires_at")).bind(label).fetch_one(&mut **tx).await?
    } else {
        sqlx::query_scalar::<_,Value>("INSERT INTO telehealth_participants(id,organization_id,room_id,staff_id,client_id,label,token_hash,token_expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING to_jsonb(telehealth_participants)").bind(Uuid::new_v4()).bind(actor.org).bind(uid(room,"id")?).bind(staff).bind(client).bind(label).bind(hash).bind(time(room,"expires_at")).fetch_one(&mut **tx).await?
    };
    if !eligible(tx, room, &participant).await? {
        return Err(ApiError::conflict(
            "This person is no longer eligible for this encounter",
        ));
    }
    event(
        tx,
        room,
        Some(uid(&participant, "id")?),
        Some(actor.id),
        "invited",
    )
    .await?;
    audit(tx, actor, "invite", room).await?;
    Ok(
        json!({"participant":public_participant(&participant),"token":token,"expires_at":room["expires_at"]}),
    )
}
pub async fn invite(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    body: web::Json<Invite>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room) = managed(&req, &state, id.into_inner(), &mut tx).await?;
    let out = issue(&mut tx, &actor, &room, Some(body.client_id)).await?;
    tx.commit().await?;
    Ok(no_store(out))
}
pub async fn host(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room) = managed(&req, &state, id.into_inner(), &mut tx).await?;
    let out = issue(&mut tx, &actor, &room, None).await?;
    tx.commit().await?;
    Ok(no_store(out))
}
pub async fn admission(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid, String)>,
) -> Result<HttpResponse> {
    let (id, participant, action) = path.into_inner();
    if !matches!(action.as_str(), "admit" | "remove") {
        return Err(ApiError::missing());
    }
    let mut tx = state.pool.begin().await?;
    let (actor, room) = managed(&req, &state, id, &mut tx).await?;
    live(&room)?;
    let p: Value = sqlx::query_scalar(
        "SELECT to_jsonb(p) FROM telehealth_participants p WHERE id=$1 AND room_id=$2",
    )
    .bind(participant)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(ApiError::missing)?;
    if action == "admit"
        && (p["state"] != "waiting" || !online(&p) || !eligible(&mut tx, &room, &p).await?)
    {
        return Err(ApiError::conflict(
            "Only an eligible participant currently waiting can be admitted",
        ));
    }
    let state = if action == "admit" {
        "admitted"
    } else {
        "removed"
    };
    sqlx::query("UPDATE telehealth_participants SET state=$2 WHERE id=$1")
        .bind(participant)
        .bind(state)
        .execute(&mut *tx)
        .await?;
    event(&mut tx, &room, Some(participant), Some(actor.id), state).await?;
    audit(&mut tx, &actor, &action, &room).await?;
    tx.commit().await?;
    Ok(no_store(json!({"state":state})))
}
pub async fn end(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (actor, room) = managed(&req, &state, id.into_inner(), &mut tx).await?;
    if room["ended_at"].is_null() {
        sqlx::query(
            "UPDATE telehealth_rooms SET ended_at=clock_timestamp(),ended_by=$2 WHERE id=$1",
        )
        .bind(uid(&room, "id")?)
        .bind(actor.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE telehealth_participants SET state='removed',last_seen_at=NULL WHERE room_id=$1",
        )
        .bind(uid(&room, "id")?)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM telehealth_signals WHERE room_id=$1")
            .bind(uid(&room, "id")?)
            .execute(&mut *tx)
            .await?;
        event(&mut tx, &room, None, Some(actor.id), "ended").await?;
        audit(&mut tx, &actor, "end", &room).await?;
    }
    tx.commit().await?;
    Ok(no_store(
        json!({"state":"ended","attendance_changed":false}),
    ))
}
