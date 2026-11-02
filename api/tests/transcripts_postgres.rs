//! Isolated review/version/authorization integration. Recognition fixtures are synthetic, not an engine test.
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
#[allow(dead_code)]
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
#[ignore = "Requires isolated nexus_test schemas"]
async fn transcript_review_requires_processing_consent_verified_context_and_immutable_versions() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let schema = format!("nexus_transcript_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\" "))
        .execute(&control)
        .await
        .unwrap();
    let search = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |c, _| {
            let search = search.clone();
            Box::pin(async move {
                sqlx::query(&search).execute(c).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let executable = std::env::current_exe().unwrap();
    for name in [
        "NEXUS_FFMPEG_CLI",
        "NEXUS_WHISPER_CLI",
        "NEXUS_WHISPER_MODEL",
    ] {
        std::env::set_var(name, &executable);
    }
    let(_,setup)=send(&pool,None,None,Method::POST,"/api/setup",json!({"organization_name":"Synthetic transcript QA","facility_name":"Synthetic facility","admin_name":"Synthetic reviewer","admin_email":"transcript@example.invalid"})).await;
    let actor = setup["actor"]["id"].as_str().unwrap();
    let org = Uuid::parse_str(setup["actor"]["organization_id"].as_str().unwrap()).unwrap();
    let actor_id = Uuid::parse_str(actor).unwrap();
    let facility = &setup["facility"]["id"];
    let client=create(&pool,actor,"clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Transcript","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let episode=create(&pool,actor,"episodes",json!({"facility_id":facility,"client_id":client["id"],"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let session=create(&pool,actor,"sessions",json!({"client_id":client["id"],"episode_id":episode["id"],"clinician_id":actor,"kind":"individual","starts_at":"2030-01-01T09:00:00Z","ends_at":"2030-01-01T10:00:00Z"})).await;
    let room = create(
        &pool,
        actor,
        "telehealth/rooms",
        json!({"session_id":session["id"]}),
    )
    .await;
    let room_id = Uuid::parse_str(room["id"].as_str().unwrap()).unwrap();
    let guest = create(
        &pool,
        actor,
        &format!("telehealth/rooms/{room_id}/invite"),
        json!({"client_id":client["id"]}),
    )
    .await;
    let person = Uuid::parse_str(guest["participant"]["id"].as_str().unwrap()).unwrap();
    let policy=create(&pool,actor,"recording-policies",json!({"title":"Synthetic capture","purpose":"Synthetic transcript workflow verification only","retention_days":7,"allow_video":false})).await;
    let rid = Uuid::new_v4();
    let tid = Uuid::new_v4();
    sqlx::query("INSERT INTO recordings(id,organization_id,room_id,policy_id,policy_snapshot,mode,state,created_by,retain_until) VALUES($1,$2,$3,$4,$5,'audio','ended',$6,now()+interval '7 days')").bind(rid).bind(org).bind(room_id).bind(Uuid::parse_str(policy["id"].as_str().unwrap()).unwrap()).bind(policy).bind(actor_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO recording_participants(organization_id,recording_id,room_id,participant_id,decision) VALUES($1,$2,$3,$4,'accepted')").bind(org).bind(rid).bind(room_id).bind(person).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO recording_tracks(id,organization_id,recording_id,participant_id,connection_id,mime_type,token_hash,state,expected_chunks,finished_at) VALUES($1,$2,$3,$4,$5,'audio/webm',$6,'complete',1,now())").bind(tid).bind(org).bind(rid).bind(person).bind(Uuid::new_v4()).bind(Uuid::new_v4().to_string()).execute(&pool).await.unwrap();
    let jobs_url = format!("/api/telehealth/rooms/{room_id}/transcription-jobs");
    let request = json!({"track_id":tid,"purpose":"Review this synthetic recognition source for documentation"});
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &jobs_url,
            request.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let consent=create(&pool,actor,"consents",json!({"client_id":client["id"],"kind":"transcription","scope":"Synthetic processing test only","granted_on":"2026-01-01"})).await;
    let (s, job) = send(
        &pool,
        Some(actor),
        None,
        Method::POST,
        &jobs_url,
        request.clone(),
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{job}");
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &jobs_url,
            request.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let job_id = Uuid::parse_str(job["id"].as_str().unwrap()).unwrap();
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("/api/transcription-jobs/{job_id}/cancel"),
            json!({})
        )
        .await
        .1["state"],
        "cancelled"
    );
    let (_, job) = send(&pool, Some(actor), None, Method::POST, &jobs_url, request).await;
    let job_id = Uuid::parse_str(job["id"].as_str().unwrap()).unwrap();
    let doc = Uuid::new_v4();
    let v1 = Uuid::new_v4();
    let sid = Uuid::new_v4();
    let initial = json!([{"id":sid,"start_ms":0,"end_ms":1000,"text":"Synthetic appointment on Monday.","speaker_participant_id":null,"speaker_verified":false,"patient_id":null,"context_verified":false,"uncertain":true,"excluded":false,"review_note":""}]);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO transcripts(id,organization_id,room_id,recording_id,track_id,job_id,duration_ms,engine,model_label,model_sha256,source_sha256,recognized_segments,current_version_id) VALUES($1,$2,$3,$4,$5,$6,2000,'synthetic-test-fixture','not-a-real-recognizer',$7,$7,$8,$9)").bind(doc).bind(org).bind(room_id).bind(rid).bind(tid).bind(job_id).bind("a".repeat(64)).bind(&initial).bind(v1).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO transcript_versions(id,organization_id,transcript_id,number,segments,reason,created_by) VALUES($1,$2,$3,1,$4,'Explicit synthetic review fixture',$5)").bind(v1).bind(org).bind(doc).bind(&initial).bind(actor_id).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE transcription_jobs SET state='ready' WHERE id=$1")
        .bind(job_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let doc_url = format!("/api/transcripts/{doc}");
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{doc_url}/approve"),
            json!({"expected_version_id":v1,"attested":true})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let billing=create(&pool,actor,"staff",json!({"facility_id":facility,"name":"Synthetic billing","email":"bill-transcript@example.invalid","role":"billing"})).await;
    assert_eq!(
        send(
            &pool,
            billing["id"].as_str(),
            None,
            Method::GET,
            &doc_url,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mut corrected = initial.clone();
    corrected[0]["text"] = json!("Synthetic appointment on Tuesday.");
    corrected[0]["speaker_participant_id"] = json!(person);
    corrected[0]["speaker_verified"] = json!(true);
    corrected[0]["patient_id"] = client["id"].clone();
    corrected[0]["context_verified"] = json!(true);
    corrected[0]["uncertain"] = json!(false);
    corrected[0]["review_note"] = json!("Corrected weekday after checking source.");
    let save = json!({"expected_version_id":v1,"reason":"Corrected weekday and verified speaker and context","segments":corrected});
    let (s, saved) = send(
        &pool,
        Some(actor),
        None,
        Method::POST,
        &format!("{doc_url}/versions"),
        save.clone(),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{saved}");
    let v2 = saved["current_version"]["id"].as_str().unwrap();
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{doc_url}/versions"),
            save
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{doc_url}/approve"),
            json!({"expected_version_id":v2,"attested":false})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{doc_url}/approve"),
            json!({"expected_version_id":v2,"attested":true})
        )
        .await
        .1["current_version"]["state"],
        "reviewed"
    );
    let (s, excerpt) = send(
        &pool,
        Some(actor),
        None,
        Method::POST,
        &format!("{doc_url}/excerpts"),
        json!({"version_id":v2,"client_id":client["id"],"segment_ids":[sid]}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{excerpt}");
    assert_eq!(excerpt["text"], "Synthetic appointment on Tuesday.");
    assert_eq!(excerpt["session_id"], session["id"]);

    let template=create(&pool,actor,"templates",json!({"name":"Synthetic excerpt form","kind":"progress","fields":[{"key":"observation","label":"Observation","type":"textarea","required":true}]})).await;
    let linked = json!({"template_id":template["id"],"client_id":client["id"],"transcript_excerpt_id":excerpt["id"],"answers":{"observation":"Appointment on Tuesday"},"field_provenance":{"observation":{"excerpt":"appointment on Tuesday"}}});
    let other_client=create(&pool,actor,"clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Different","birth_date":"1989-01-01","primary_clinician_id":actor})).await;
    let mut wrong_patient = linked.clone();
    wrong_patient["client_id"] = other_client["id"].clone();
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            "/api/form_responses",
            wrong_patient
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let unassigned=create(&pool,actor,"staff",json!({"facility_id":facility,"name":"Synthetic unassigned clinician","email":"unassigned-transcript@example.invalid","role":"clinician"})).await;
    assert_eq!(
        send(
            &pool,
            unassigned["id"].as_str(),
            None,
            Method::GET,
            &format!(
                "/api/clients/{}/transcript-excerpts",
                client["id"].as_str().unwrap()
            ),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let response = create(&pool, actor, "form_responses", linked.clone()).await;
    assert_eq!(response["session_id"], session["id"]);
    assert_eq!(response["source_excerpt"], excerpt["text"]);
    let form_url = format!("/api/form_responses/{}", response["id"].as_str().unwrap());
    assert_eq!(send(&pool,Some(actor),None,Method::PATCH,&form_url,json!({"_expected_updated_at":response["updated_at"],"source_excerpt":"An invented substitute"})).await.0,StatusCode::BAD_REQUEST);
    let (s, reviewed) = send(
        &pool,
        Some(actor),
        None,
        Method::POST,
        &format!("{form_url}/review"),
        json!({}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{reviewed}");
    let (s, signed) = send(
        &pool,
        Some(actor),
        None,
        Method::POST,
        &format!("{form_url}/sign"),
        json!({}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{signed}");
    let (s, note) = send(
        &pool,
        Some(actor),
        None,
        Method::POST,
        &format!("{form_url}/create-note"),
        json!({"title":"Synthetic reviewed source note"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{note}");
    assert_eq!(note["form_response_id"], response["id"]);
    let knowledge_path = format!("/api/clients/{}/knowledge", client["id"].as_str().unwrap());
    let (status, index) = send(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &knowledge_path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{index}");
    assert_eq!(index["total"], 2, "{index}");
    assert!(index["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["source_kind"] == "transcript_excerpt"));
    assert!(index["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["source_kind"] == "form_response"));
    let (_, options) = send(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &format!(
            "/api/clients/{}/transcript-excerpts",
            client["id"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert_eq!(options["excerpts"].as_array().unwrap().len(), 1);
    assert_eq!(
        send(
            &pool,
            billing["id"].as_str(),
            None,
            Method::GET,
            &format!(
                "/api/clients/{}/transcript-excerpts",
                client["id"].as_str().unwrap()
            ),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let awaiting = create(&pool, actor, "form_responses", linked).await;
    assert!(
        sqlx::query("UPDATE transcript_versions SET segments='[]' WHERE id=$1")
            .bind(Uuid::parse_str(v2).unwrap())
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE transcripts SET recognized_segments='[]' WHERE id=$1")
            .bind(doc)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM transcript_excerpts WHERE id=$1")
        .bind(Uuid::parse_str(excerpt["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .is_err());
    let(s,next)=send(&pool,Some(actor),None,Method::POST,&format!("{doc_url}/versions"),json!({"expected_version_id":v2,"reason":"Second documented correction review","segments":corrected})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(next["current_version"]["state"], "draft");
    assert_eq!(next["excerpts"], json!([]));
    let (_, stale_index) = send(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &knowledge_path,
        Value::Null,
    )
    .await;
    assert_eq!(stale_index["total"], 0, "{stale_index}");
    assert!(
        sqlx::query("UPDATE transcripts SET current_version_id=$2 WHERE id=$1")
            .bind(doc)
            .bind(Uuid::parse_str(v2).unwrap())
            .execute(&pool)
            .await
            .is_err()
    );

    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!(
                "/api/form_responses/{}/review",
                awaiting["id"].as_str().unwrap()
            ),
            json!({})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{form_url}/create-note"),
            json!({"title":"Must not generate from superseded source"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let preserved: String = sqlx::query_scalar("SELECT status FROM form_responses WHERE id=$1")
        .bind(Uuid::parse_str(response["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(preserved, "signed");
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{doc_url}/excerpts"),
            json!({"version_id":v2,"client_id":client["id"],"segment_ids":[sid]})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, history) = send(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &format!("{doc_url}?version_id={v1}"),
        Value::Null,
    )
    .await;
    assert_eq!(
        history["current_version"]["segments"][0]["text"],
        initial[0]["text"]
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{doc_url}/approve"),
            json!({"expected_version_id":next["current_version"]["id"],"attested":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    let(status,current_excerpt)=send(&pool,Some(actor),None,Method::POST,&format!("{doc_url}/excerpts"),json!({"version_id":next["current_version"]["id"],"client_id":client["id"],"segment_ids":[sid]})).await;
    assert_eq!(status, StatusCode::OK);
    let(status,remapped)=send(&pool,Some(actor),None,Method::PATCH,&format!("/api/form_responses/{}",awaiting["id"].as_str().unwrap()),json!({"_expected_updated_at":awaiting["updated_at"],"transcript_excerpt_id":current_excerpt["id"],"source_type":"transcript","source_excerpt":current_excerpt["text"],"answers":{"observation":"Synthetic appointment on Tuesday."},"field_provenance":{"observation":{"excerpt":"Synthetic appointment on Tuesday."}}})).await;
    assert_eq!(status, StatusCode::OK, "{remapped}");
    assert_eq!(
        remapped["answers"]["observation"],
        "Synthetic appointment on Tuesday."
    );
    assert_eq!(remapped["status"], "draft");
    // Reuse this isolated synthetic recording to exercise the group binding.
    // An unrelated session can mention the group; only attendance identifies
    // the patient's actual group visit. No production records are rewritten.
    let group=create(&pool,actor,"groups",json!({"facility_id":facility,"clinician_id":actor,"title":"Synthetic transcript group","kind":"skills","starts_at":"2030-01-03T09:00:00Z","ends_at":"2030-01-03T10:00:00Z","capacity":4})).await;
    let attendance = create(
        &pool,
        actor,
        "group_attendance",
        json!({"group_id":group["id"],"client_id":client["id"],"episode_id":episode["id"]}),
    )
    .await;
    let group_id = Uuid::parse_str(group["id"].as_str().unwrap()).unwrap();
    let session_id = Uuid::parse_str(session["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE sessions SET group_id=$2 WHERE id=$1")
        .bind(session_id)
        .bind(group_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE telehealth_rooms SET session_id=NULL,group_id=$2 WHERE id=$1")
        .bind(room_id)
        .bind(group_id)
        .execute(&pool)
        .await
        .unwrap();
    let(status,group_excerpt)=send(&pool,Some(actor),None,Method::POST,&format!("{doc_url}/excerpts"),json!({"version_id":next["current_version"]["id"],"client_id":client["id"],"segment_ids":[sid]})).await;
    assert_eq!(status, StatusCode::OK, "{group_excerpt}");
    assert_eq!(group_excerpt["session_id"], attendance["session_id"]);
    assert_ne!(group_excerpt["session_id"], session["id"]);
    sqlx::query("UPDATE telehealth_rooms SET group_id=NULL,session_id=$2 WHERE id=$1")
        .bind(room_id)
        .bind(session_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET group_id=NULL WHERE id=$1")
        .bind(session_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE consents SET revoked_on=current_date WHERE id=$1")
        .bind(Uuid::parse_str(consent["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (_, revoked_index) = send(
        &pool,
        Some(actor),
        None,
        Method::GET,
        &knowledge_path,
        Value::Null,
    )
    .await;
    assert_eq!(revoked_index["total"], 0, "{revoked_index}");
    assert_eq!(
        send(
            &pool,
            Some(actor),
            None,
            Method::POST,
            &format!("{doc_url}/approve"),
            json!({"expected_version_id":next["current_version"]["id"],"attested":true})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&pool, Some(actor), None, Method::GET, &doc_url, Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    for name in [
        "NEXUS_FFMPEG_CLI",
        "NEXUS_WHISPER_CLI",
        "NEXUS_WHISPER_MODEL",
    ] {
        std::env::remove_var(name);
    }
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
