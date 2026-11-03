//! Synthetic transport bytes in a unique disposable schema; not a media decoder test.
mod support;
use actix_web::{
    http::{Method, StatusCode},
    test, web, App,
};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;
async fn raw(
    pool: &PgPool,
    actor: Option<&str>,
    token: Option<&str>,
    method: Method,
    path: &str,
    body: Vec<u8>,
    json_body: bool,
) -> (StatusCode, Vec<u8>) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
            .configure(configure),
    )
    .await;
    let mut req = test::TestRequest::default()
        .method(method)
        .uri(path)
        .set_payload(body)
        .insert_header((
            "content-type",
            if json_body {
                "application/json"
            } else {
                "application/octet-stream"
            },
        ));
    if let Some(a) = actor {
        req = req.insert_header(("x-nexus-actor", a));
    }
    if let Some(t) = token {
        req = req.insert_header(("authorization", format!("Bearer {t}")));
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    (status, test::read_body(response).await.to_vec())
}
async fn send(
    pool: &PgPool,
    actor: Option<&str>,
    token: Option<&str>,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let (status, bytes) = raw(
        pool,
        actor,
        token,
        method,
        path,
        serde_json::to_vec(&body).unwrap(),
        true,
    )
    .await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn create(pool: &PgPool, actor: &str, resource: &str, body: Value) -> Value {
    let (s, v) = send(
        pool,
        Some(actor),
        None,
        Method::POST,
        &format!("/api/{resource}"),
        body,
    )
    .await;
    assert!(s.is_success(), "{resource}: {s} {v}");
    v
}
async fn visit(
    pool: &PgPool,
    token: &str,
    c: Uuid,
    path: &str,
    mut body: Value,
) -> (StatusCode, Value) {
    body["connection_id"] = json!(c);
    send(
        pool,
        None,
        Some(token),
        Method::POST,
        &format!("/api/telehealth/visit/{path}"),
        body,
    )
    .await
}
#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique schema"]
async fn recording_requires_every_agreement_and_preserves_encrypted_ordered_media() {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(actual, "nexus_test");
    let schema = format!("nexus_recordings_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |c, _| {
            let path = path.clone();
            Box::pin(async move {
                sqlx::query(&path).execute(c).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    // Process-local test encryption key only, never a real workspace key.
    std::env::set_var("NEXUS_MEDIA_KEY_HEX", "11".repeat(32));
    let(s,setup)=send(&pool,None,None,Method::POST,"/api/setup",json!({"organization_name":"Synthetic recording QA","facility_name":"Synthetic facility","admin_name":"Synthetic clinician","admin_email":"media@example.invalid"})).await;
    assert_eq!(s, StatusCode::CREATED);
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let client=create(&pool,actor,"clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Recording","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let episode=support::legacy_episode(&pool,actor,json!({"facility_id":facility,"client_id":client["id"],"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let session=support::legacy_session(&pool,actor,json!({"client_id":client["id"],"episode_id":episode["id"],"clinician_id":actor,"kind":"individual","starts_at":"2030-01-01T09:00:00Z","ends_at":"2030-01-01T10:00:00Z"})).await;
    let room = create(
        &pool,
        actor,
        "telehealth/rooms",
        json!({"session_id":session["id"]}),
    )
    .await;
    let room_id = room["id"].as_str().unwrap();
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
    let ht = host["token"].as_str().unwrap();
    let gt = guest["token"].as_str().unwrap();
    let hc = Uuid::new_v4();
    let gc = Uuid::new_v4();
    assert_eq!(
        visit(&pool, ht, hc, "join", json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        visit(&pool, gt, gc, "join", json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        visit(
            &pool,
            ht,
            hc,
            "admit",
            json!({"to":guest["participant"]["id"]})
        )
        .await
        .0,
        StatusCode::OK
    );
    let policy=create(&pool,actor,"recording-policies",json!({"title":"Synthetic QA policy","purpose":"Synthetic capture for transport verification only.","retention_days":14,"allow_video":false})).await;
    let request = json!({"policy_id":policy["id"],"mode":"audio"});
    assert_eq!(
        visit(&pool, gt, gc, "recordings/request", request.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        visit(
            &pool,
            ht,
            hc,
            "recordings/request",
            json!({"policy_id":policy["id"],"mode":"video"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        visit(&pool, ht, hc, "recordings/request", request.clone())
            .await
            .0,
        StatusCode::CONFLICT,
        "No recording without the chart consent"
    );
    let consent=create(&pool,actor,"consents",json!({"client_id":client["id"],"kind":"recording","scope":"Synthetic local recording test","granted_on":"2026-01-01"})).await;
    let (s, requested) = visit(&pool, ht, hc, "recordings/request", request.clone()).await;
    assert_eq!(s, StatusCode::OK, "{requested}");
    assert_eq!(
        requested["current"]["participants"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        visit(&pool, ht, hc, "recordings/start", json!({})).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        visit(&pool, ht, hc, "recordings/accept", json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        visit(&pool, gt, gc, "recordings/decline", json!({}))
            .await
            .1["current"]["state"],
        "declined"
    );
    let (_, still_in_call) = visit(&pool, gt, gc, "poll", json!({})).await;
    assert_eq!(still_in_call["self"]["state"], "admitted");
    assert_eq!(still_in_call["recording"]["current"]["state"], "declined");
    let (s, requested) = visit(&pool, ht, hc, "recordings/request", request.clone()).await;
    assert_eq!(s, StatusCode::OK);
    let rid = requested["current"]["id"].as_str().unwrap();
    for (token, c) in [(ht, hc), (gt, gc)] {
        assert_eq!(
            visit(&pool, token, c, "recordings/accept", json!({}))
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        visit(&pool, ht, hc, "recordings/start", json!({})).await.1["current"]["state"],
        "recording"
    );
    let (s, track) = visit(
        &pool,
        ht,
        hc,
        &format!("recordings/{rid}/tracks"),
        json!({"mime_type":"audio/webm;codecs=opus"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{track}");
    let tid = track["track_id"].as_str().unwrap();
    let ut = track["upload_token"].as_str().unwrap();
    let bytes0 = [
        vec![0x1a, 0x45, 0xdf, 0xa3],
        b"synthetic recording transport".to_vec(),
    ]
    .concat();
    let bytes1 = b"ordered synthetic continuation".to_vec();
    let chunk_url = format!("/api/recording-tracks/{tid}/chunks/");
    let finish_url = format!("/api/recording-tracks/{tid}/finish");
    assert_eq!(
        raw(
            &pool,
            None,
            Some(ht),
            Method::PUT,
            &format!("{chunk_url}0"),
            bytes0.clone(),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "Visit capability cannot upload as a track capability"
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(ut),
            Method::PUT,
            &format!("{chunk_url}1"),
            bytes1.clone(),
            false
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(ut),
            Method::PUT,
            &format!("{chunk_url}0"),
            bytes0.clone(),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(ut),
            Method::PUT,
            &format!("{chunk_url}0"),
            bytes0.clone(),
            false
        )
        .await
        .0,
        StatusCode::OK,
        "Identical retry is idempotent"
    );
    let mut changed = bytes0.clone();
    changed.push(8);
    assert_eq!(
        raw(
            &pool,
            None,
            Some(ut),
            Method::PUT,
            &format!("{chunk_url}0"),
            changed,
            false
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &pool,
            None,
            Some(ut),
            Method::POST,
            &finish_url,
            json!({"expected_chunks":2,"complete":true})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(ut),
            Method::PUT,
            &format!("{chunk_url}1"),
            bytes1.clone(),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &pool,
            None,
            Some(ut),
            Method::POST,
            &finish_url,
            json!({"expected_chunks":2,"complete":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    let cipher: Vec<u8> = sqlx::query_scalar(
        "SELECT ciphertext FROM recording_chunks WHERE track_id=$1 AND sequence=0",
    )
    .bind(Uuid::parse_str(tid).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(cipher, bytes0);
    assert!(!cipher.windows(9).any(|b| b == b"synthetic"));
    let token_hash: String =
        sqlx::query_scalar("SELECT token_hash FROM recording_tracks WHERE id=$1")
            .bind(Uuid::parse_str(tid).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(token_hash, ut);
    let playback_url = format!("/api/telehealth/rooms/{room_id}/recordings/{rid}/tracks/{tid}");
    let (s, restored) = raw(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &playback_url,
        Vec::new(),
        false,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(restored, [bytes0.clone(), bytes1.clone()].concat());
    let billing=create(&pool,actor,"staff",json!({"name":"Synthetic billing","email":"media-billing@example.invalid","role":"billing"})).await;
    assert_eq!(
        raw(
            &pool,
            Some(billing["id"].as_str().unwrap()),
            None,
            Method::GET,
            &playback_url,
            Vec::new(),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, archive) = send(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &format!("/api/telehealth/rooms/{room_id}/recordings"),
        json!({}),
    )
    .await;
    assert!(!archive.to_string().contains("token_hash"));
    assert!(!archive.to_string().contains(ut));
    assert_eq!(archive["tracks"][0]["state"], "complete");
    // A normal pause permits the last media event; withdrawal rejects new bytes.
    let (s, gt2) = visit(
        &pool,
        gt,
        gc,
        &format!("recordings/{rid}/tracks"),
        json!({"mime_type":"audio/webm"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let t2 = gt2["track_id"].as_str().unwrap();
    let u2 = gt2["upload_token"].as_str().unwrap();
    assert_eq!(
        visit(&pool, ht, hc, "recordings/pause", json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        visit(&pool, ht, hc, "recordings/resume", json!({})).await.0,
        StatusCode::CONFLICT,
        "An unfinished segment must flush before capture resumes"
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(u2),
            Method::PUT,
            &format!("/api/recording-tracks/{t2}/chunks/0"),
            bytes0.clone(),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &pool,
            None,
            Some(u2),
            Method::POST,
            &format!("/api/recording-tracks/{t2}/finish"),
            json!({"expected_chunks":1,"complete":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        visit(&pool, ht, hc, "recordings/resume", json!({})).await.0,
        StatusCode::OK
    );
    let (_, t3) = visit(
        &pool,
        ht,
        hc,
        &format!("recordings/{rid}/tracks"),
        json!({"mime_type":"audio/webm"}),
    )
    .await;
    let tid3 = t3["track_id"].as_str().unwrap();
    let ut3 = t3["upload_token"].as_str().unwrap();
    assert_eq!(
        visit(&pool, gt, gc, "recordings/withdraw", json!({}))
            .await
            .1["current"]["state"],
        "interrupted"
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(ut3),
            Method::PUT,
            &format!("/api/recording-tracks/{tid3}/chunks/0"),
            bytes0.clone(),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        visit(&pool, gt, gc, "poll", json!({})).await.1["self"]["state"],
        "admitted"
    );
    // Existing media remains a retained care artifact; a withdrawal stops capture,
    // not an implicit destructive operation or chart erasure.
    assert_eq!(
        raw(
            &pool,
            Some(actor),
            None,
            Method::GET,
            &playback_url,
            Vec::new(),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, r4) = visit(&pool, ht, hc, "recordings/request", request).await;
    assert_eq!(r4["current"]["state"], "requested");
    for (token, c) in [(ht, hc), (gt, gc)] {
        assert_eq!(
            visit(&pool, token, c, "recordings/accept", json!({}))
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        visit(&pool, ht, hc, "recordings/start", json!({})).await.0,
        StatusCode::OK
    );
    sqlx::query("UPDATE consents SET revoked_on=current_date WHERE id=$1")
        .bind(Uuid::parse_str(consent["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (_, revoked) = visit(&pool, ht, hc, "poll", json!({})).await;
    assert_eq!(revoked["recording"]["current"]["state"], "interrupted");
    let unchanged: String = sqlx::query_scalar("SELECT status FROM sessions WHERE id=$1")
        .bind(Uuid::parse_str(session["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unchanged, "scheduled");
    assert!(
        sqlx::query("UPDATE recording_chunks SET plain_length=1 WHERE track_id=$1")
            .bind(Uuid::parse_str(tid).unwrap())
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE recording_policies SET purpose='Changed terms' WHERE id=$1")
            .bind(Uuid::parse_str(policy["id"].as_str().unwrap()).unwrap())
            .execute(&pool)
            .await
            .is_err()
    );
    create(&pool,actor,"consents",json!({"client_id":client["id"],"kind":"recording","scope":"Fresh synthetic agreement after revocation","granted_on":"2026-01-01"})).await;
    let (_, final_request) = visit(
        &pool,
        ht,
        hc,
        "recordings/request",
        json!({"policy_id":policy["id"],"mode":"audio"}),
    )
    .await;
    let final_id = final_request["current"]["id"].as_str().unwrap();
    for (token, c) in [(ht, hc), (gt, gc)] {
        assert_eq!(
            visit(&pool, token, c, "recordings/accept", json!({}))
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        visit(&pool, ht, hc, "recordings/start", json!({})).await.0,
        StatusCode::OK
    );
    let (_, tail) = visit(
        &pool,
        ht,
        hc,
        &format!("recordings/{final_id}/tracks"),
        json!({"mime_type":"audio/webm"}),
    )
    .await;
    let tail_id = tail["track_id"].as_str().unwrap();
    let tail_token = tail["upload_token"].as_str().unwrap();
    assert_eq!(
        visit(&pool, ht, hc, "end", json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(tail_token),
            Method::PUT,
            &format!("/api/recording-tracks/{tail_id}/chunks/0"),
            bytes0.clone(),
            false
        )
        .await
        .0,
        StatusCode::OK,
        "A normal room end allows the final media event"
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!(
                "/api/recording-policies/{}/retire",
                policy["id"].as_str().unwrap()
            ),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        raw(
            &pool,
            None,
            Some(tail_token),
            Method::PUT,
            &format!("/api/recording-tracks/{tail_id}/chunks/1"),
            bytes0.clone(),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "Retiring a policy also closes the final-upload grace"
    );
    sqlx::query("UPDATE sessions SET status='completed' WHERE id=$1")
        .bind(Uuid::parse_str(session["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (history_status, history) = send(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &format!(
            "/api/telehealth/rooms?session_id={}",
            session["id"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert_eq!(history_status, StatusCode::OK);
    assert!(!history["rooms"].as_array().unwrap().is_empty());
    assert_eq!(
        raw(
            &pool,
            Some(actor),
            None,
            Method::GET,
            &playback_url,
            Vec::new(),
            false
        )
        .await
        .0,
        StatusCode::OK,
        "Completed encounters retain their authorized recording archive"
    );
    std::env::remove_var("NEXUS_MEDIA_KEY_HEX");
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
