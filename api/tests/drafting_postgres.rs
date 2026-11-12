//! Durable proposal review/clinical-note boundaries. Model output here is an explicit fixture.
mod support;
use actix_web::{
    http::{Method, StatusCode},
    test, web, App,
};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;
async fn send(
    pool: &PgPool,
    actor: Option<&str>,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let mut req = test::TestRequest::default()
        .method(method)
        .uri(path)
        .set_json(body);
    if let Some(a) = actor {
        req = req.insert_header(("x-nexus-actor", a));
    }
    let res = test::call_service(&app, req.to_request()).await;
    let status = res.status();
    let bytes = test::read_body(res).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn post(pool: &PgPool, actor: &str, path: &str, body: Value) -> Value {
    let (s, v) = send(pool, Some(actor), Method::POST, path, body).await;
    assert!(s.is_success(), "{path} {s}: {v}");
    v
}
async fn fixture_ready(pool: &PgPool, job: &Value, chunk: &Value) {
    let output = json!({"model":"synthetic-engine-fixture","model_sha256":"a".repeat(64),"prompt_version":"synthetic-fixture","sections":[{"key":"observation","label":"Observation","statements":[{"text":"The synthetic appointment was reported for Monday.","citations":[{"source_id":chunk["id"],"quote":"Synthetic observation: the appointment is on Monday."}]}]}],"unresolved":["No delivery of care is established by this synthetic fixture."]});
    let id = Uuid::parse_str(job["id"].as_str().unwrap()).unwrap();
    sqlx::query(
        "UPDATE clinical_draft_jobs SET state='running',started_at=clock_timestamp() WHERE id=$1",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE clinical_draft_jobs SET state='ready',output=$2,completed_at=clock_timestamp() WHERE id=$1").bind(id).bind(output).execute(pool).await.unwrap();
}
#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn proposals_require_current_scoped_evidence_and_explicit_review_before_note_creation() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let schema = format!("nexus_drafting_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let search = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |c, _| {
            let s = search.clone();
            Box::pin(async move {
                sqlx::query(&s).execute(c).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    std::env::set_var("NEXUS_LLM_MODEL", std::env::current_exe().unwrap());
    std::env::set_var("NEXUS_LLM_PORT", "8093");
    std::env::set_var("NEXUS_LLM_ALIAS", "synthetic-model-fixture");
    let(s,setup)=send(&pool,None,Method::POST,"/api/setup",json!({"organization_name":"Synthetic draft QA","facility_name":"Synthetic facility","admin_name":"Synthetic reviewer","admin_email":"draft@example.invalid"})).await;
    assert!(s.is_success(), "{setup}");
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let client=post(&pool,actor,"/api/clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Draft","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let client_id = client["id"].as_str().unwrap();
    let note=post(&pool,actor,"/api/notes",json!({"client_id":client_id,"title":"Synthetic source","body":"Synthetic observation: the appointment is on Monday."})).await;
    post(
        &pool,
        actor,
        &format!("/api/notes/{}/sign", note["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    let template=post(&pool,actor,"/api/templates",json!({"name":"Synthetic clinical form","kind":"progress","fields":[{"key":"observation","label":"Observation","type":"textarea","required":true}]})).await;
    let knowledge_url = format!("/api/clients/{client_id}/knowledge");
    let (s, k) = send(&pool, Some(actor), Method::GET, &knowledge_url, Value::Null).await;
    assert_eq!(s, StatusCode::OK, "{k}");
    let chunk = &k["results"][0];
    assert!(chunk["id"].is_string(), "{k}");
    let episode = support::legacy_episode(&pool, actor, json!({"client_id":client_id,"facility_id":facility,"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let url = format!("/api/clients/{client_id}/clinical-drafts");
    let body = json!({"template_id":template["id"],"source_ids":[chunk["id"]],"episode_id":episode["id"],"session_id":null,"title":"Synthetic reviewed proposal","purpose":"Summarize the explicitly synthetic appointment evidence."});
    let job = post(&pool, actor, &url, body.clone()).await;
    assert_eq!(job["state"], "queued");
    assert_eq!(
        send(&pool, Some(actor), Method::POST, &url, body.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    let job_url = format!("/api/clinical-drafts/{}", job["id"].as_str().unwrap());
    let billing = post(
        &pool,
        actor,
        "/api/staff",
        json!({"name":"Synthetic biller","email":"bill@example.invalid","role":"billing"}),
    )
    .await;
    assert_eq!(
        send(
            &pool,
            billing["id"].as_str(),
            Method::GET,
            &job_url,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let clinician=post(&pool,actor,"/api/staff",json!({"name":"Synthetic other clinician","email":"other@example.invalid","role":"clinician"})).await;
    assert_eq!(
        send(
            &pool,
            clinician["id"].as_str(),
            Method::GET,
            &job_url,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let job_id = Uuid::parse_str(job["id"].as_str().unwrap()).unwrap();
    assert!(
        sqlx::query("UPDATE clinical_draft_jobs SET purpose='replaced inputs' WHERE id=$1")
            .bind(job_id)
            .execute(&pool)
            .await
            .is_err()
    );
    post(&pool, actor, &format!("{job_url}/cancel"), json!({})).await;
    assert!(
        sqlx::query("UPDATE clinical_draft_jobs SET state='queued' WHERE id=$1")
            .bind(job_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let ready = post(&pool, actor, &url, body.clone()).await;
    fixture_ready(&pool, &ready, chunk).await;
    let ready_url = format!("/api/clinical-drafts/{}", ready["id"].as_str().unwrap());
    let review = json!({"decision":"accept","reason":"Checked the synthetic evidence and historical context.","attested":true,"decisions":[{"section_key":"observation","index":0,"decision":"accept","reason":""}]});
    let mut unreviewed = review.clone();
    unreviewed["attested"] = json!(false);
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::POST,
            &format!("{ready_url}/review"),
            unreviewed
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut unknown = review.clone();
    unknown["decisions"][0]["section_key"] = json!("invented");
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::POST,
            &format!("{ready_url}/review"),
            unknown
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let accepted = post(&pool, actor, &format!("{ready_url}/review"), review.clone()).await;
    assert_eq!(accepted["note"]["status"], "draft");
    assert_eq!(accepted["note"]["clinical_draft_job_id"], ready["id"]);
    assert_eq!(accepted["note"]["client_id"], client["id"]);
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::POST,
            &format!("{ready_url}/review"),
            review.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let ready_id = Uuid::parse_str(ready["id"].as_str().unwrap()).unwrap();
    assert!(
        sqlx::query("UPDATE clinical_draft_jobs SET output='{}' WHERE id=$1")
            .bind(ready_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM clinical_draft_reviews WHERE job_id=$1")
            .bind(ready_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let pending = post(&pool, actor, &url, body.clone()).await;
    fixture_ready(&pool, &pending, chunk).await;
    // A database-linked unrelated note cannot borrow the acceptance of the actual reviewed note.
    let unrelated = post(&pool,actor,"/api/notes",json!({"client_id":client_id,"title":"Unrelated synthetic note","body":"Unrelated evidence."})).await;
    sqlx::query("UPDATE notes SET clinical_draft_job_id=$1 WHERE id=$2")
        .bind(ready_id)
        .bind(Uuid::parse_str(unrelated["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, denied) = send(
        &pool,
        Some(actor),
        Method::POST,
        &format!("/api/notes/{}/sign", unrelated["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{denied}");
    post(
        &pool,
        actor,
        &format!(
            "/api/notes/{}/sign",
            accepted["note"]["id"].as_str().unwrap()
        ),
        json!({}),
    )
    .await;
    let derived = post(&pool,actor,&format!("/api/notes/{}/amend",accepted["note"]["id"].as_str().unwrap()),json!({"reason":"Synthetic clarification retains reviewed evidence.","body":"Synthetic clarified observation from Monday."})).await;
    assert_eq!(derived["clinical_draft_job_id"], ready["id"]);
    // Simulate an older amendment created before source links were copied.
    sqlx::query("UPDATE notes SET clinical_draft_job_id=NULL WHERE id=$1")
        .bind(Uuid::parse_str(derived["id"].as_str().unwrap()).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    post(
        &pool,
        actor,
        &format!("/api/notes/{}/sign", derived["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    let (_, indexed) = send(
        &pool,
        Some(actor),
        Method::GET,
        &format!(
            "{knowledge_url}?episode_id={}",
            episode["id"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert!(
        indexed["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["source_id"] == derived["id"] && r["episode_id"] == episode["id"]),
        "{indexed}"
    );
    let next_derived = post(&pool,actor,&format!("/api/notes/{}/amend",derived["id"].as_str().unwrap()),json!({"reason":"Synthetic second clarification for source recheck.","body":"Synthetic further clarification."})).await;
    assert_eq!(next_derived["clinical_draft_job_id"], ready["id"]);
    let amendment=post(&pool,actor,&format!("/api/notes/{}/amend",note["id"].as_str().unwrap()),json!({"reason":"Synthetic correction changes the appointment day.","body":"Synthetic correction: the appointment is on Tuesday."})).await;
    post(
        &pool,
        actor,
        &format!("/api/notes/{}/sign", amendment["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::POST,
            &format!(
                "/api/clinical-drafts/{}/review",
                pending["id"].as_str().unwrap()
            ),
            review
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::POST,
            &format!("/api/notes/{}/sign", next_derived["id"].as_str().unwrap()),
            json!({})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, historical) = send(&pool, Some(actor), Method::GET, &ready_url, Value::Null).await;
    assert_eq!(historical["output"]["model"], "synthetic-engine-fixture");
    for key in ["NEXUS_LLM_MODEL", "NEXUS_LLM_PORT", "NEXUS_LLM_ALIAS"] {
        std::env::remove_var(key);
    }
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
