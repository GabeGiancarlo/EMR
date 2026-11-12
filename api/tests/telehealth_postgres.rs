//! Real telehealth authorization and signaling persistence, never ordinary workspace data.
mod support;
use actix_web::{http::StatusCode, test, web, App};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;
async fn send(
    pool: &PgPool,
    actor: Option<&str>,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let mut req = if method == "GET" {
        test::TestRequest::get()
    } else {
        test::TestRequest::post()
    };
    req = req.uri(path).set_json(body);
    if let Some(actor) = actor {
        req = req.insert_header(("x-nexus-actor", actor));
    }
    if let Some(token) = token {
        req = req.insert_header(("authorization", format!("Bearer {token}")));
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn create(pool: &PgPool, actor: &str, resource: &str, body: Value) -> Value {
    let (status, row) = send(
        pool,
        Some(actor),
        None,
        "POST",
        &format!("/api/{resource}"),
        body,
    )
    .await;
    assert!(status.is_success(), "Create {resource}: {row}");
    row
}
async fn visit(
    pool: &PgPool,
    token: &str,
    action: &str,
    connection: Uuid,
    mut body: Value,
) -> (StatusCode, Value) {
    body["connection_id"] = json!(connection);
    send(
        pool,
        None,
        Some(token),
        "POST",
        &format!("/api/telehealth/visit/{action}"),
        body,
    )
    .await
}
#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn encounter_participation_and_signaling_enforce_current_access() {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(actual, "nexus_test");
    let schema = format!("nexus_telehealth_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |conn, _| {
            let path = path.clone();
            Box::pin(async move {
                sqlx::query(&path).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let (status,setup)=send(&pool,None,None,"POST","/api/setup",json!({"organization_name":"Synthetic telehealth test","facility_name":"Synthetic facility","admin_name":"Synthetic clinician","admin_email":"telehealth@example.invalid"})).await;
    assert_eq!(status, StatusCode::CREATED);
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let client=create(&pool,actor,"clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Participant","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let episode=support::legacy_episode(&pool,actor,json!({"facility_id":facility,"client_id":client["id"],"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let session=support::legacy_session(&pool,actor,json!({"client_id":client["id"],"episode_id":episode["id"],"clinician_id":actor,"kind":"individual","starts_at":"2030-01-01T09:00:00Z","ends_at":"2030-01-01T10:00:00Z"})).await;
    let (room, concurrent_room) = tokio::join!(
        create(
            &pool,
            actor,
            "telehealth/rooms",
            json!({"session_id":session["id"]})
        ),
        create(
            &pool,
            actor,
            "telehealth/rooms",
            json!({"session_id":session["id"]})
        )
    );
    assert_eq!(
        room["id"], concurrent_room["id"],
        "Concurrent creates reuse the same room"
    );
    let room_id = room["id"].as_str().unwrap();
    let repeated = create(
        &pool,
        actor,
        "telehealth/rooms",
        json!({"session_id":session["id"]}),
    )
    .await;
    assert_eq!(room["id"], repeated["id"]);
    let billing=create(&pool,actor,"staff",json!({"name":"Synthetic billing","email":"billing-visit@example.invalid","role":"billing"})).await;
    let (status, _) = send(
        &pool,
        Some(billing["id"].as_str().unwrap()),
        None,
        "GET",
        &format!("/api/telehealth/rooms/{room_id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let host = create(
        &pool,
        actor,
        &format!("telehealth/rooms/{room_id}/host"),
        json!({}),
    )
    .await;
    let guest = create(
        &pool,
        actor,
        &format!("telehealth/rooms/{room_id}/invite"),
        json!({"client_id":client["id"]}),
    )
    .await;
    let host_token = host["token"].as_str().unwrap();
    let guest_token = guest["token"].as_str().unwrap();
    assert_eq!(host_token.len(), 64);
    assert_ne!(host_token, guest_token);
    let hash: String =
        sqlx::query_scalar("SELECT token_hash FROM telehealth_participants WHERE id=$1")
            .bind(Uuid::parse_str(guest["participant"]["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(hash, guest_token);
    let (_, inspection) = send(
        &pool,
        None,
        Some(guest_token),
        "GET",
        "/api/telehealth/visit",
        json!({}),
    )
    .await;
    assert_eq!(inspection["participants"].as_array().unwrap().len(), 1);
    assert!(!inspection.to_string().contains("client_id"));
    assert!(!inspection.to_string().contains("Synthetic clinician"));
    assert!(!inspection.to_string().contains("token_hash"));
    let hc = Uuid::new_v4();
    let gc1 = Uuid::new_v4();
    let gc2 = Uuid::new_v4();
    let (left, right) = tokio::join!(
        visit(&pool, guest_token, "join", gc1, json!({})),
        visit(&pool, guest_token, "join", gc2, json!({}))
    );
    assert!(
        (left.0 == StatusCode::OK && right.0 == StatusCode::CONFLICT)
            || (right.0 == StatusCode::OK && left.0 == StatusCode::CONFLICT),
        "Only one concurrent window may claim this invitation"
    );
    let (gc, (status, waiting)) = if left.0 == StatusCode::OK {
        (gc1, left)
    } else {
        (gc2, right)
    };
    assert_eq!(status, StatusCode::OK);
    assert_eq!(waiting["self"]["state"], "waiting");
    assert_eq!(waiting["host_present"], false);
    let guest_id = &guest["participant"]["id"];
    let host_id = &host["participant"]["id"];
    let (status, _) = visit(
        &pool,
        guest_token,
        "signal",
        gc,
        json!({"to":host_id,"to_connection":hc,"payload":{"candidate":null}}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, joined) = visit(&pool, host_token, "join", hc, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(joined["self"]["state"], "admitted");
    let (status, _) = visit(&pool, guest_token, "admit", gc, json!({"to":guest_id})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = visit(&pool, host_token, "admit", hc, json!({"to":guest_id})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, admitted) = visit(&pool, guest_token, "poll", gc, json!({"after":0})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(admitted["participants"].as_array().unwrap().len(), 2);
    assert_eq!(admitted["host_present"], true);
    assert_eq!(admitted["ice_servers"], json!([]));
    let(status,_)=visit(&pool,host_token,"signal",hc,json!({"to":guest_id,"to_connection":gc,"payload":{"description":{"type":"offer","sdp":"v=0\r\n"}}})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, poll) = visit(&pool, guest_token, "poll", gc, json!({"after":0})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(poll["signals"].as_array().unwrap().len(), 1);
    assert_eq!(poll["signals"][0]["from_connection"], hc.to_string());
    let (_, next) = visit(
        &pool,
        guest_token,
        "poll",
        gc,
        json!({"after":poll["cursor"]}),
    )
    .await;
    assert_eq!(next["signals"], json!([]));
    let (status, _) = visit(&pool, guest_token, "join", Uuid::new_v4(), json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let stroke = json!({"id":Uuid::new_v4(),"color":"#245b50","points":[[0.1,0.2],[0.2,0.3]]});
    let (status, _) = visit(&pool, guest_token, "board", gc, json!({"stroke":stroke})).await;
    assert_eq!(status, StatusCode::OK);
    let (_, board) = visit(&pool, host_token, "poll", hc, json!({})).await;
    assert_eq!(board["board"].as_array().unwrap().len(), 1);
    let (status, _) = visit(&pool, guest_token, "board", gc, json!({"clear":true})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = visit(&pool, host_token, "board", hc, json!({"clear":true})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = visit(&pool, guest_token, "end", gc, json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Absence of the host closes the cooperative media graph, not the clinical encounter.
    let (status, _) = visit(&pool, host_token, "leave", hc, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let (_, paused) = visit(&pool, guest_token, "poll", gc, json!({})).await;
    assert_eq!(paused["host_present"], false);
    assert_eq!(paused["signals"], json!([]));
    assert_eq!(paused["participants"].as_array().unwrap().len(), 1);
    visit(&pool, host_token, "join", hc, json!({})).await;
    let (status, _) = visit(&pool, host_token, "remove", hc, json!({"to":guest_id})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = visit(&pool, guest_token, "poll", gc, json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let replacement = create(
        &pool,
        actor,
        &format!("telehealth/rooms/{room_id}/invite"),
        json!({"client_id":client["id"]}),
    )
    .await;
    assert_eq!(replacement["participant"]["id"], *guest_id);
    assert_ne!(replacement["token"], guest["token"]);
    let (status, _) = send(
        &pool,
        None,
        Some(guest_token),
        "GET",
        "/api/telehealth/visit",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = visit(&pool, host_token, "end", hc, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let stored: String = sqlx::query_scalar("SELECT status FROM sessions WHERE id=$1")
        .bind(Uuid::parse_str(session["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "scheduled");
    let (status, _) = visit(&pool, host_token, "poll", hc, json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, history) = send(
        &pool,
        Some(actor),
        None,
        "GET",
        &format!("/api/telehealth/rooms/{room_id}"),
        json!({}),
    )
    .await;
    assert!(history["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["action"] == "ended"));
    assert!(!history.to_string().contains("token_hash"));
    assert!(!history.to_string().contains(guest_token));
    let immutable = sqlx::query("UPDATE telehealth_events SET action='changed' WHERE room_id=$1")
        .bind(Uuid::parse_str(room_id).unwrap())
        .execute(&pool)
        .await;
    assert!(immutable.is_err());
    // Facilitating a shared group grants only its minimum invitation roster,
    // not chart access to participants assigned to another primary clinician.
    let facilitator=create(&pool,actor,"staff",json!({"name":"Synthetic facilitator","email":"facilitator@example.invalid","role":"clinician","facility_id":facility})).await;
    let fid = facilitator["id"].as_str().unwrap();
    let group=support::legacy_group(&pool,actor,json!({"facility_id":facility,"clinician_id":fid,"title":"Synthetic shared encounter","kind":"skills","starts_at":"2030-01-02T09:00:00Z","ends_at":"2030-01-02T10:00:00Z","capacity":4})).await;
    let attendance = create(
        &pool,
        actor,
        "group_attendance",
        json!({"group_id":group["id"],"client_id":client["id"],"episode_id":episode["id"]}),
    )
    .await;
    let gr = create(
        &pool,
        fid,
        "telehealth/rooms",
        json!({"group_id":group["id"]}),
    )
    .await;
    let grid = gr["id"].as_str().unwrap();
    let (status, roster) = send(
        &pool,
        Some(fid),
        None,
        "GET",
        &format!("/api/telehealth/rooms/{grid}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(roster["invitees"][0]["id"], client["id"]);
    assert_eq!(roster["invitees"][0].as_object().unwrap().len(), 3);
    let (status, visible_clients) =
        send(&pool, Some(fid), None, "GET", "/api/clients", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !visible_clients
            .to_string()
            .contains(client["id"].as_str().unwrap()),
        "Group roster access does not expose the patient's chart"
    );
    let (status, _) = send(
        &pool,
        Some(actor),
        None,
        "POST",
        "/api/telehealth/rooms",
        json!({"session_id":attendance["session_id"]}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "A child session cannot open a duplicate shared room"
    );
    let gh = create(
        &pool,
        fid,
        &format!("telehealth/rooms/{grid}/host"),
        json!({}),
    )
    .await;
    let gg = create(
        &pool,
        fid,
        &format!("telehealth/rooms/{grid}/invite"),
        json!({"client_id":client["id"]}),
    )
    .await;
    let ght = gh["token"].as_str().unwrap();
    let ggt = gg["token"].as_str().unwrap();
    let ghid = &gh["participant"]["id"];
    let ggid = &gg["participant"]["id"];
    let ghc = Uuid::new_v4();
    let ggc = Uuid::new_v4();
    assert_eq!(
        visit(&pool, ght, "join", ghc, json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        visit(&pool, ggt, "join", ggc, json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        visit(&pool, ght, "admit", ghc, json!({"to":ggid})).await.0,
        StatusCode::OK
    );
    // Cross-room targets, stale generations and invite rotation while connected fail.
    assert_eq!(
        visit(
            &pool,
            ght,
            "signal",
            ghc,
            json!({"to":guest_id,"to_connection":gc,"payload":{"candidate":null}})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        visit(
            &pool,
            ght,
            "signal",
            ghc,
            json!({"to":ggid,"to_connection":Uuid::new_v4(),"payload":{"candidate":null}})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &pool,
            Some(fid),
            None,
            "POST",
            &format!("/api/telehealth/rooms/{grid}/invite"),
            json!({"client_id":client["id"]})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // A timed-out browser gets a fresh generation; old queued signals stay hidden.
    assert_eq!(
        visit(
            &pool,
            ght,
            "signal",
            ghc,
            json!({"to":ggid,"to_connection":ggc,"payload":{"candidate":null}})
        )
        .await
        .0,
        StatusCode::OK
    );
    let guest_uuid = Uuid::parse_str(ggid.as_str().unwrap()).unwrap();
    sqlx::query("UPDATE telehealth_participants SET last_seen_at=clock_timestamp()-interval '30 seconds' WHERE id=$1").bind(guest_uuid).execute(&pool).await.unwrap();
    let new_gc = Uuid::new_v4();
    let (status, rejoined) = visit(&pool, ggt, "join", new_gc, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rejoined["self"]["state"], "admitted");
    assert_eq!(rejoined["signals"], json!([]));
    assert_eq!(
        visit(&pool, ggt, "poll", ggc, json!({})).await.0,
        StatusCode::CONFLICT
    );
    // Expiring the host removes both its signals and peer presence immediately
    // from the next authorized snapshot, even if its heartbeat is still fresh.
    assert_eq!(
        visit(
            &pool,
            ght,
            "signal",
            ghc,
            json!({"to":ggid,"to_connection":new_gc,"payload":{"candidate":null}})
        )
        .await
        .0,
        StatusCode::OK
    );
    let host_uuid = Uuid::parse_str(ghid.as_str().unwrap()).unwrap();
    sqlx::query("UPDATE telehealth_participants SET token_expires_at=clock_timestamp()-interval '1 second' WHERE id=$1").bind(host_uuid).execute(&pool).await.unwrap();
    assert_eq!(
        visit(&pool, ght, "poll", ghc, json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, expired) = visit(&pool, ggt, "poll", new_gc, json!({})).await;
    assert_eq!(expired["host_present"], false);
    assert_eq!(expired["signals"], json!([]));
    sqlx::query("UPDATE telehealth_participants SET token_expires_at=clock_timestamp()+interval '1 hour' WHERE id=$1").bind(host_uuid).execute(&pool).await.unwrap();
    let staff_uuid = Uuid::parse_str(fid).unwrap();
    sqlx::query("UPDATE staff SET active=false WHERE id=$1")
        .bind(staff_uuid)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        visit(&pool, ght, "poll", ghc, json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, offboarded) = visit(&pool, ggt, "poll", new_gc, json!({})).await;
    assert_eq!(offboarded["host_present"], false);
    assert_eq!(offboarded["signals"], json!([]));
    sqlx::query("UPDATE staff SET active=true WHERE id=$1")
        .bind(staff_uuid)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE groups SET clinician_id=$2 WHERE id=$1")
        .bind(Uuid::parse_str(group["id"].as_str().unwrap()).unwrap())
        .bind(Uuid::parse_str(actor).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        visit(&pool, ght, "poll", ghc, json!({})).await.0,
        StatusCode::FORBIDDEN,
        "Reassignment revokes the former facilitator"
    );
    sqlx::query("UPDATE episodes SET status='discharged',end_date='2030-01-03',discharge_reason='Synthetic test' WHERE id=$1").bind(Uuid::parse_str(episode["id"].as_str().unwrap()).unwrap()).execute(&pool).await.unwrap();
    assert_eq!(
        visit(&pool, ggt, "poll", new_gc, json!({})).await.0,
        StatusCode::FORBIDDEN,
        "Discharge revokes client participation"
    );
    // Foreign organization operators cannot inspect this room or issue access.
    let other_org = Uuid::new_v4();
    let other_actor = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Synthetic other organization')")
        .bind(other_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role) VALUES($1,$2,'Synthetic other admin','other@example.invalid','admin')").bind(other_actor).bind(other_org).execute(&pool).await.unwrap();
    assert_eq!(
        send(
            &pool,
            Some(&other_actor.to_string()),
            None,
            "GET",
            &format!("/api/telehealth/rooms/{grid}"),
            json!({})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &pool,
            Some(&other_actor.to_string()),
            None,
            "POST",
            &format!("/api/telehealth/rooms/{grid}/host"),
            json!({})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}

/// A request with an exact body and headers, for what the JSON helper cannot send.
async fn raw(
    pool: &PgPool,
    method: &str,
    path: &str,
    token: Option<&str>,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let mut req = test::TestRequest::default()
        .method(method.parse().unwrap())
        .uri(path)
        .set_payload(body);
    if let Some(token) = token {
        req = req.insert_header(("authorization", format!("Bearer {token}")));
    }
    if let Some(content_type) = content_type {
        req = req.insert_header(("content-type", content_type));
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn visit_routes_check_the_token_before_the_body_and_bound_event_growth() {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(actual, "nexus_test");
    let schema = format!("nexus_visit_limits_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |conn, _| {
            let path = path.clone();
            Box::pin(async move {
                sqlx::query(&path).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let (status,setup)=send(&pool,None,None,"POST","/api/setup",json!({"organization_name":"Synthetic visit limits test","facility_name":"Synthetic facility","admin_name":"Synthetic clinician","admin_email":"visit-limits@example.invalid"})).await;
    assert_eq!(status, StatusCode::CREATED);
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let client=create(&pool,actor,"clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Participant","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let episode=support::legacy_episode(&pool,actor,json!({"facility_id":facility,"client_id":client["id"],"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let session=support::legacy_session(&pool,actor,json!({"client_id":client["id"],"episode_id":episode["id"],"clinician_id":actor,"kind":"individual","starts_at":"2030-01-01T09:00:00Z","ends_at":"2030-01-01T10:00:00Z"})).await;
    let room = create(
        &pool,
        actor,
        "telehealth/rooms",
        json!({"session_id":session["id"]}),
    )
    .await;
    let guest = create(
        &pool,
        actor,
        &format!("telehealth/rooms/{}/invite", room["id"].as_str().unwrap()),
        json!({"client_id":client["id"]}),
    )
    .await;
    let token = guest["token"].as_str().unwrap();
    let participant = Uuid::parse_str(guest["participant"]["id"].as_str().unwrap()).unwrap();
    let json_type = Some("application/json");
    let stranger = "0".repeat(64);
    let malformed = b"{\"connection_id\":".to_vec();
    // Larger than a visit body may be, smaller than the staff JSON limit.
    let oversized = format!(
        "{{\"connection_id\":\"{}\",\"payload\":\"{}\"}}",
        Uuid::new_v4(),
        "x".repeat(300 * 1024)
    )
    .into_bytes();

    // Without a real invitation the answer is the invitation refusal, whatever the
    // body is: the body was never read, so its faults are never reported.
    for path in [
        "/api/telehealth/visit/join",
        "/api/telehealth/visit/recordings/accept",
        &format!("/api/telehealth/visit/recordings/{}/tracks", Uuid::new_v4()),
    ] {
        for credential in [None, Some(stranger.as_str()), Some("short")] {
            for body in [malformed.clone(), oversized.clone()] {
                let (status, answer) = raw(&pool, "POST", path, credential, json_type, body).await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {answer}");
                assert_eq!(answer["error"]["code"], "visit_access", "{path}");
            }
        }
        // With one, the body is read and judged.
        let (status, answer) = raw(
            &pool,
            "POST",
            path,
            Some(token),
            json_type,
            malformed.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {answer}");
        assert_eq!(answer["error"]["code"], "validation", "{path}");
        let (status, answer) = raw(
            &pool,
            "POST",
            path,
            Some(token),
            json_type,
            oversized.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{path}: {answer}");
        assert_eq!(answer["error"]["code"], "payload_too_large", "{path}");
        let (status, answer) = raw(
            &pool,
            "POST",
            path,
            Some(token),
            Some("text/plain"),
            b"{}".to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {answer}");
    }

    // Recording uploads: an unknown track or token is refused before the media is
    // read. Nine mebibytes is over the piece limit, so reading it first would have
    // answered 413 instead.
    let nine_mebibytes = vec![0u8; 9 * 1024 * 1024];
    let track = Uuid::new_v4();
    for credential in [None, Some(stranger.as_str())] {
        let (status, answer) = raw(
            &pool,
            "PUT",
            &format!("/api/recording-tracks/{track}/chunks/0"),
            credential,
            None,
            nine_mebibytes.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{answer}");
        assert_eq!(answer["error"]["code"], "forbidden");
        let (status, answer) = raw(
            &pool,
            "POST",
            &format!("/api/recording-tracks/{track}/finish"),
            credential,
            json_type,
            malformed.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{answer}");
    }
    let (status, answer) = raw(
        &pool,
        "GET",
        &format!("/api/recording-tracks/{track}/chunks/0"),
        None,
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(answer["error"]["code"], "method_not_allowed");

    // Joining and leaving in a loop writes two kept events a time. It is refused
    // once the participant has caused sixty in ten minutes, and nothing more is
    // written after that.
    let events = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM telehealth_events WHERE participant_id=$1",
        )
        .bind(participant)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let mut refused_after = None;
    for cycle in 0..40 {
        let connection = Uuid::new_v4();
        let (status, answer) = visit(&pool, token, "join", connection, json!({})).await;
        if status != StatusCode::OK {
            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{answer}");
            assert_eq!(answer["error"]["code"], "visit_rate");
            refused_after = Some(cycle);
            break;
        }
        let (status, answer) = visit(&pool, token, "leave", connection, json!({})).await;
        assert_eq!(status, StatusCode::OK, "{answer}");
    }
    let cycles = refused_after.expect("the join and leave loop was never refused");
    assert!((29..=30).contains(&cycles), "refused after {cycles} cycles");
    let written = events().await;
    assert!((60..=61).contains(&written), "{written} events");
    for _ in 0..5 {
        let (status, _) = visit(&pool, token, "join", Uuid::new_v4(), json!({})).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }
    assert_eq!(events().await, written);
    // The refusal changed nothing about the participant either.
    let state: String = sqlx::query_scalar("SELECT state FROM telehealth_participants WHERE id=$1")
        .bind(participant)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "left");
    // Reading the visit is still possible; only new events are refused.
    let (status, _) = send(
        &pool,
        None,
        Some(token),
        "GET",
        "/api/telehealth/visit",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
