use super::*;
use crate::AppState;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    connection_id: Uuid,
    after: Option<i64>,
    to: Option<Uuid>,
    to_connection: Option<Uuid>,
    payload: Option<Value>,
    stroke: Option<Value>,
    clear: Option<bool>,
}
fn configuration() -> Result<Value> {
    let raw = std::env::var("NEXUS_ICE_SERVERS_JSON").unwrap_or_else(|_| "[]".into());
    parse_configuration(&raw)
}
fn parse_configuration(raw: &str) -> Result<Value> {
    let servers: Value = serde_json::from_str(raw)
        .map_err(|_| ApiError::conflict("The media connectivity configuration is invalid"))?;
    let list = servers
        .as_array()
        .filter(|list| list.len() <= 8)
        .ok_or_else(|| ApiError::conflict("The media connectivity configuration is invalid"))?;
    for server in list {
        let urls = match &server["urls"] {
            Value::String(s) => vec![s.as_str()],
            Value::Array(items) if items.len() <= 8 && items.iter().all(Value::is_string) => {
                items.iter().filter_map(Value::as_str).collect()
            }
            _ => Vec::new(),
        };
        if urls.is_empty()
            || urls.iter().any(|url| {
                url.len() > 500
                    || url.chars().any(char::is_whitespace)
                    || !(url.starts_with("turn:")
                        || url.starts_with("turns:")
                        || url.starts_with("stun:")
                        || url.starts_with("stuns:"))
            })
        {
            return Err(ApiError::conflict(
                "The media connectivity configuration is invalid",
            ));
        }
        if server.as_object().is_none_or(|o| {
            o.keys()
                .any(|k| !matches!(k.as_str(), "urls" | "username" | "credential"))
                || ["username", "credential"].iter().any(|key| {
                    o.get(*key)
                        .is_some_and(|v| v.as_str().is_none_or(|s| s.len() > 1024))
                })
        }) {
            return Err(ApiError::conflict(
                "The media connectivity configuration is invalid",
            ));
        }
    }
    Ok(servers)
}
fn payload_valid(payload: &Value) -> bool {
    let Some(object) = payload.as_object() else {
        return false;
    };
    if object.len() != 1 {
        return false;
    }
    if let Some(desc) = object.get("description") {
        return desc.as_object().is_some_and(|o| o.len() == 2)
            && matches!(desc["type"].as_str(), Some("offer" | "answer"))
            && desc["sdp"]
                .as_str()
                .is_some_and(|s| s.starts_with("v=0") && s.len() <= 65536);
    }
    if let Some(candidate) = object.get("candidate") {
        if candidate.is_null() {
            return true;
        }
        return candidate.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                matches!(
                    k.as_str(),
                    "candidate" | "sdpMid" | "sdpMLineIndex" | "usernameFragment"
                )
            })
        }) && candidate["candidate"]
            .as_str()
            .is_some_and(|s| s.len() <= 4096)
            && (candidate["sdpMid"].is_null()
                || candidate["sdpMid"].as_str().is_some_and(|s| s.len() <= 128))
            && (candidate["usernameFragment"].is_null()
                || candidate["usernameFragment"]
                    .as_str()
                    .is_some_and(|s| s.len() <= 256))
            && (candidate["sdpMLineIndex"].is_null()
                || candidate["sdpMLineIndex"]
                    .as_u64()
                    .is_some_and(|n| n <= 100));
    }
    false
}
fn validate_stroke(stroke: &Value) -> Result<()> {
    uid(stroke, "id")?;
    if !matches!(
        stroke["color"].as_str(),
        Some("#245b50" | "#3574c4" | "#b35d32" | "#75519c")
    ) {
        return Err(ApiError::invalid("Choose a whiteboard color"));
    }
    let points = stroke["points"]
        .as_array()
        .filter(|p| !p.is_empty() && p.len() <= 256)
        .ok_or_else(|| ApiError::invalid("A stroke needs 1 to 256 points"))?;
    for point in points {
        if !point.as_array().is_some_and(|p| {
            p.len() == 2
                && p.iter().all(|n| {
                    n.as_f64()
                        .is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n))
                })
        }) {
            return Err(ApiError::invalid(
                "Whiteboard points must be normalized coordinates",
            ));
        }
    }
    Ok(())
}
fn connection(p: &Value, id: Uuid) -> Result<()> {
    if p["connection_id"] != id.to_string()
        || !matches!(p["state"].as_str(), Some("waiting" | "admitted"))
        || !online(p)
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "visit_connection",
            message: "This visit connection ended or expired. Rejoin the waiting room.".into(),
        });
    }
    Ok(())
}
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    room: &Value,
    p: &Value,
    after: i64,
    connected: bool,
) -> Result<Value> {
    let people = participants(tx, room).await?;
    let host_present = people
        .iter()
        .any(|row| !row["staff_id"].is_null() && row["state"] == "admitted" && online(row));
    let is_host = !p["staff_id"].is_null();
    let admitted = connected && p["state"] == "admitted";
    let visible: Vec<Value> = people
        .iter()
        .filter(|row| {
            connected
                && (is_host
                    || (admitted && host_present && row["state"] == "admitted" && online(row)))
                || row["id"] == p["id"]
        })
        .map(public_participant)
        .collect();
    let mut signals: Vec<Value> = if admitted && host_present {
        sqlx::query_scalar("SELECT jsonb_build_object('id',s.id,'from_id',s.from_id,'from_connection',s.from_connection,'payload',s.payload) FROM telehealth_signals s JOIN telehealth_participants p ON p.id=s.from_id WHERE s.room_id=$1 AND s.to_id=$2 AND s.to_connection=$3 AND s.id>$4 AND s.expires_at>clock_timestamp() AND p.connection_id=s.from_connection AND p.state='admitted' AND p.last_seen_at>clock_timestamp()-interval '20 seconds' ORDER BY s.id LIMIT 256").bind(uid(room,"id")?).bind(uid(p,"id")?).bind(uid(p,"connection_id")?).bind(after).fetch_all(&mut **tx).await?
    } else {
        Vec::new()
    };
    let cursor = signals
        .last()
        .and_then(|s| s["id"].as_i64())
        .unwrap_or(after);
    signals.retain(|signal| {
        people
            .iter()
            .any(|person| person["id"] == signal["from_id"])
    });
    let ice = if admitted && host_present {
        configuration()?
    } else {
        json!([])
    };
    Ok(
        json!({"room":{"id":room["id"],"state":if room["ended_at"].is_null(){"open"}else{"ended"},"expires_at":room["expires_at"]},"self":public_participant(p),"participants":visible,"signals":signals,"cursor":cursor,"host_present":host_present&&connected,"board":if admitted&&host_present {room["board"].clone()}else{json!([])},"connectivity":if ice.as_array().is_some_and(|a|!a.is_empty()){"configured"}else{"local"},"ice_servers":ice}),
    )
}
pub async fn inspect(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse> {
    let mut tx = state.pool.begin().await?;
    let (room, p) = authorize(&req, &mut tx).await?;
    let out = snapshot(&mut tx, &room, &p, 0, false).await?;
    tx.commit().await?;
    Ok(no_store(out))
}
pub async fn action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Command>,
) -> Result<HttpResponse> {
    let action = path.into_inner();
    if !matches!(
        action.as_str(),
        "join" | "poll" | "signal" | "leave" | "board" | "admit" | "remove" | "end"
    ) {
        return Err(ApiError::missing());
    }
    let mut tx = state.pool.begin().await?;
    let (room, mut p) = authorize(&req, &mut tx).await?;
    let id = uid(&p, "id")?;
    let room_id = uid(&room, "id")?;
    if action == "join" {
        if online(&p)
            && matches!(p["state"].as_str(), Some("waiting" | "admitted"))
            && p["connection_id"] != body.connection_id.to_string()
        {
            return Err(ApiError{status:StatusCode::CONFLICT,code:"visit_busy",message:"This invitation is already connected in another window. Leave that visit, or wait 20 seconds after closing it, before joining here.".into()});
        }
        let same = online(&p) && p["connection_id"] == body.connection_id.to_string();
        let next = if !p["staff_id"].is_null() || p["state"] == "admitted" {
            "admitted"
        } else {
            "waiting"
        };
        p=sqlx::query_scalar("UPDATE telehealth_participants SET connection_id=$2,state=$3,last_seen_at=clock_timestamp() WHERE id=$1 RETURNING to_jsonb(telehealth_participants)").bind(id).bind(body.connection_id).bind(next).fetch_one(&mut *tx).await?;
        if !same {
            event(
                &mut tx,
                &room,
                Some(id),
                None,
                if next == "waiting" {
                    "waiting"
                } else {
                    "connected"
                },
            )
            .await?;
        }
    } else {
        connection(&p, body.connection_id)?;
    }
    if matches!(action.as_str(), "admit" | "remove" | "end") {
        if p["staff_id"].is_null() || p["state"] != "admitted" {
            return Err(ApiError::denied());
        }
        let staff_id = uid(&p, "staff_id")?;
        if action == "end" {
            sqlx::query(
                "UPDATE telehealth_rooms SET ended_at=clock_timestamp(),ended_by=$2 WHERE id=$1",
            )
            .bind(room_id)
            .bind(staff_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE telehealth_participants SET state='removed',last_seen_at=NULL WHERE room_id=$1").bind(room_id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM telehealth_signals WHERE room_id=$1")
                .bind(room_id)
                .execute(&mut *tx)
                .await?;
            event(&mut tx, &room, None, Some(staff_id), "ended").await?;
        } else {
            let target = body
                .to
                .ok_or_else(|| ApiError::invalid("Select a participant"))?;
            if target == id {
                return Err(ApiError::invalid("Use Leave to exit your own connection"));
            }
            let target: Value = sqlx::query_scalar(
                "SELECT to_jsonb(p) FROM telehealth_participants p WHERE id=$1 AND room_id=$2",
            )
            .bind(target)
            .bind(room_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(ApiError::missing)?;
            if action == "admit"
                && (target["state"] != "waiting"
                    || !online(&target)
                    || !eligible(&mut tx, &room, &target).await?)
            {
                return Err(ApiError::conflict(
                    "Only an eligible participant currently waiting can be admitted",
                ));
            }
            let next = if action == "admit" {
                "admitted"
            } else {
                "removed"
            };
            sqlx::query("UPDATE telehealth_participants SET state=$2 WHERE id=$1")
                .bind(uid(&target, "id")?)
                .bind(next)
                .execute(&mut *tx)
                .await?;
            event(
                &mut tx,
                &room,
                Some(uid(&target, "id")?),
                Some(staff_id),
                next,
            )
            .await?;
        }
        sqlx::query("INSERT INTO audit_events(id,organization_id,actor_id,action,resource,record_id) VALUES($1,$2,$3,$4,'telehealth_rooms',$5)").bind(Uuid::new_v4()).bind(uid(&room,"organization_id")?).bind(staff_id).bind(&action).bind(room_id).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(no_store(json!({"ok":true,"attendance_changed":false})));
    }
    if action == "leave" {
        sqlx::query(
            "UPDATE telehealth_participants SET state='left',last_seen_at=NULL WHERE id=$1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        event(&mut tx, &room, Some(id), None, "left").await?;
        tx.commit().await?;
        return Ok(no_store(json!({"state":"left"})));
    }
    if matches!(action.as_str(), "poll" | "join") {
        p=sqlx::query_scalar("UPDATE telehealth_participants SET last_seen_at=clock_timestamp() WHERE id=$1 RETURNING to_jsonb(telehealth_participants)").bind(id).fetch_one(&mut *tx).await?;
        let after = body.after.unwrap_or(0);
        if after < 0 {
            return Err(ApiError::invalid("Invalid signal cursor"));
        }
        sqlx::query(
            "DELETE FROM telehealth_signals WHERE room_id=$1 AND expires_at<=clock_timestamp()",
        )
        .bind(room_id)
        .execute(&mut *tx)
        .await?;
        let out = snapshot(&mut tx, &room, &p, after, true).await?;
        tx.commit().await?;
        return Ok(no_store(out));
    }
    let people = participants(&mut tx, &room).await?;
    if p["state"] != "admitted"
        || !people
            .iter()
            .any(|p| !p["staff_id"].is_null() && p["state"] == "admitted" && online(p))
    {
        return Err(ApiError::denied());
    }
    if action == "signal" {
        let target = body.to.ok_or_else(|| ApiError::invalid("Select a peer"))?;
        let generation = body
            .to_connection
            .ok_or_else(|| ApiError::invalid("Peer connection is required"))?;
        if target == id
            || !people.iter().any(|p| {
                p["id"] == target.to_string()
                    && p["connection_id"] == generation.to_string()
                    && p["state"] == "admitted"
                    && online(p)
            })
        {
            return Err(ApiError::denied());
        }
        let payload = body
            .payload
            .as_ref()
            .filter(|v| payload_valid(v))
            .ok_or_else(|| ApiError::invalid("Invalid WebRTC signaling payload"))?;
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM telehealth_signals WHERE from_id=$1 AND expires_at>clock_timestamp()").bind(id).fetch_one(&mut *tx).await?;
        if count >= 2000 {
            return Err(ApiError {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "visit_rate",
                message: "Too many connection attempts. Pause before reconnecting.".into(),
            });
        }
        sqlx::query("INSERT INTO telehealth_signals(room_id,from_id,to_id,from_connection,to_connection,payload) VALUES($1,$2,$3,$4,$5,$6)").bind(room_id).bind(id).bind(target).bind(body.connection_id).bind(generation).bind(payload).execute(&mut *tx).await?;
    } else if action == "board" {
        let mut board = room["board"].as_array().cloned().unwrap_or_default();
        if body.clear == Some(true) {
            if p["staff_id"].is_null() {
                return Err(ApiError::denied());
            }
            board.clear();
            event(&mut tx, &room, Some(id), None, "whiteboard_cleared").await?;
        } else {
            let stroke = body
                .stroke
                .as_ref()
                .ok_or_else(|| ApiError::invalid("A whiteboard stroke is required"))?;
            validate_stroke(stroke)?;
            if board.len() >= 200 {
                return Err(ApiError::conflict(
                    "The board is full. Ask the clinician to clear it before drawing more.",
                ));
            }
            if !board.iter().any(|s| s["id"] == stroke["id"]) {
                board.push(json!({"id":stroke["id"],"author_id":p["id"],"color":stroke["color"],"points":stroke["points"]}));
            }
        }
        sqlx::query("UPDATE telehealth_rooms SET board=$2 WHERE id=$1")
            .bind(room_id)
            .bind(json!(board))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(no_store(json!({"ok":true})))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connectivity_configuration_rejects_mixed_types_and_extra_credentials() {
        assert!(parse_configuration("[]").is_ok());
        assert!(parse_configuration(r#"[{"urls":["turn:relay.example.invalid:3478"],"username":"visit","credential":"temporary"}]"#).is_ok());
        for raw in [
            r#"[{"urls":["stun:relay.example.invalid",17]}]"#,
            r#"[{"urls":"stun:relay.example.invalid","credential":42}]"#,
            r#"[{"urls":"https://relay.example.invalid"}]"#,
            r#"[{"urls":"stun:relay.example.invalid","extra":"value"}]"#,
        ] {
            assert!(parse_configuration(raw).is_err(), "{raw}");
        }
    }
    #[test]
    fn signaling_is_bounded_and_typed() {
        assert!(payload_valid(
            &json!({"description":{"type":"offer","sdp":"v=0\r\n"}})
        ));
        assert!(payload_valid(&json!({"candidate":null})));
        assert!(!payload_valid(
            &json!({"description":{"type":"rollback","sdp":"v=0"}})
        ));
        assert!(!payload_valid(
            &json!({"candidate":{"candidate":"x","command":"ignored"}})
        ));
        assert!(!payload_valid(
            &json!({"description":{"type":"answer","sdp":"v=0"},"candidate":null})
        ));
    }
    #[test]
    fn board_has_no_arbitrary_html_or_outside_coordinates() {
        let id = Uuid::new_v4();
        assert!(validate_stroke(
            &json!({"id":id,"color":"#245b50","points":[[0.0,1.0],[0.2,0.4]]})
        )
        .is_ok());
        assert!(
            validate_stroke(&json!({"id":id,"color":"#245b50","points":[[0.0,1.01]]})).is_err()
        );
        assert!(validate_stroke(
            &json!({"id":id,"color":"url(javascript:x)","points":[[0.0,1.0]]})
        )
        .is_err());
    }
}
