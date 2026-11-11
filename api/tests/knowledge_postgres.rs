//! Derived approved-record search exercised against an isolated PostgreSQL schema.
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

#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn search_preserves_scope_approval_history_and_stable_evidence_identifiers() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let schema = format!("nexus_knowledge_test_{}", Uuid::new_v4().simple());
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
    let (status,setup)=send(&pool,None,Method::POST,"/api/setup",json!({"organization_name":"Synthetic knowledge QA","facility_name":"Synthetic facility","admin_name":"Synthetic reviewer","admin_email":"knowledge@example.invalid"})).await;
    assert!(status.is_success(), "{setup}");
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let client=post(&pool,actor,"/api/clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Knowledge","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let other=post(&pool,actor,"/api/clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Other","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let cid = client["id"].as_str().unwrap();
    let path = format!("/api/clients/{cid}/knowledge");
    let (_, empty) = send(&pool, Some(actor), Method::GET, &path, Value::Null).await;
    assert_eq!(empty["total"], 0, "{empty}");
    let note=post(&pool,actor,"/api/notes",json!({"client_id":cid,"title":"Synthetic appointment","body":"Synthetic appointment on Monday. No medication change reported."})).await;
    let (_, draft) = send(&pool, Some(actor), Method::GET, &path, Value::Null).await;
    assert_eq!(draft["total"], 0);
    post(
        &pool,
        actor,
        &format!("/api/notes/{}/sign", note["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    let (status, index) = send(
        &pool,
        Some(actor),
        Method::GET,
        &format!("{path}?q=Monday&kind=note&limit=1"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{index}");
    assert_eq!(index["total"], 1, "{index}");
    let chunk = index["results"][0].clone();
    assert_eq!(chunk["source_id"], note["id"]);
    assert_eq!(chunk["source_kind"], "note");
    let source = format!("{path}/sources/{}", chunk["document_id"].as_str().unwrap());
    let (_, detail) = send(&pool, Some(actor), Method::GET, &source, Value::Null).await;
    assert_eq!(detail["document"]["body"], note["body"]);
    assert_eq!(detail["chunks"][0]["id"], chunk["id"]);
    post(&pool, actor, &path, json!({})).await;
    let (_, again) = send(&pool, Some(actor), Method::GET, &path, Value::Null).await;
    assert_eq!(again["results"][0]["id"], chunk["id"]);
    let (_, page) = send(
        &pool,
        Some(actor),
        Method::GET,
        &format!("{path}?limit=1&offset=1"),
        Value::Null,
    )
    .await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["results"], json!([]));
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::GET,
            &format!("{path}?kind=invented"),
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::GET,
            &format!(
                "/api/clients/{}/knowledge/sources/{}",
                other["id"].as_str().unwrap(),
                chunk["document_id"].as_str().unwrap()
            ),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for (role, email) in [
        ("billing", "bill@example.invalid"),
        ("executive", "exec@example.invalid"),
        ("clinician", "other@example.invalid"),
    ] {
        let staff = post(
            &pool,
            actor,
            "/api/staff",
            json!({"name":"Synthetic other operator","email":email,"role":role}),
        )
        .await;
        assert_eq!(
            send(&pool, staff["id"].as_str(), Method::GET, &path, Value::Null)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(
                &pool,
                staff["id"].as_str(),
                Method::GET,
                &source,
                Value::Null
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(&pool, staff["id"].as_str(), Method::POST, &path, json!({}))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    // The original stays searchable until its amendment is actually signed.
    let amendment=post(&pool,actor,&format!("/api/notes/{}/amend",note["id"].as_str().unwrap()),json!({"reason":"Corrected synthetic appointment weekday","body":"Synthetic appointment on Tuesday. No medication change reported."})).await;
    let (_, still) = send(&pool, Some(actor), Method::GET, &path, Value::Null).await;
    assert_eq!(still["results"][0]["id"], chunk["id"]);
    post(
        &pool,
        actor,
        &format!("/api/notes/{}/sign", amendment["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(
        send(&pool, Some(actor), Method::GET, &source, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, old) = send(
        &pool,
        Some(actor),
        Method::GET,
        &format!("{path}?q=Monday"),
        Value::Null,
    )
    .await;
    assert_eq!(old["total"], 0);
    let (_, new) = send(
        &pool,
        Some(actor),
        Method::GET,
        &format!("{path}?q=Tuesday"),
        Value::Null,
    )
    .await;
    assert_eq!(new["total"], 1);
    assert_eq!(new["results"][0]["source_id"], amendment["id"]);
    assert_ne!(new["results"][0]["id"], chunk["id"]);
    // Goal wording and review observations are indexed only after plan activation.
    let episode=support::legacy_episode(&pool,actor,json!({"facility_id":facility,"client_id":cid,"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let plan=post(&pool,actor,"/api/treatment_plans",json!({"client_id":cid,"episode_id":episode["id"],"title":"Synthetic recovery plan","presenting_needs":"Synthetic scheduling support","strengths":"Synthetic organization"})).await;
    let goal=post(&pool,actor,"/api/goals",json!({"plan_id":plan["id"],"description":"Synthetic attendance goal","measure":"Synthetic weekly visits","baseline":"Zero","target":"Two"})).await;
    let planpath = format!("/api/treatment_plans/{}", plan["id"].as_str().unwrap());
    post(&pool, actor, &format!("{planpath}/activate"), json!({})).await;
    post(&pool,actor,"/api/plan_reviews",json!({"plan_id":plan["id"],"summary":"Synthetic progress observation","goal_progress":[{"goal_id":goal["id"],"status":"on_track","observation":"Synthetic attendance discussed"}]})).await;
    let (_, plans) = send(
        &pool,
        Some(actor),
        Method::GET,
        &format!("{path}?episode_id={}", episode["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(plans["total"], 2, "{plans}");
    assert!(plans["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["source_kind"] == "plan_review"
            && r["text"]
                .as_str()
                .unwrap()
                .contains("Synthetic attendance goal")));
    let replacement = post(
        &pool,
        actor,
        &format!("{planpath}/revise"),
        json!({"reason":"Synthetic replacement plan review"}),
    )
    .await;
    post(
        &pool,
        actor,
        &format!(
            "/api/treatment_plans/{}/activate",
            replacement["id"].as_str().unwrap()
        ),
        json!({}),
    )
    .await;
    let (_, revised) = send(
        &pool,
        Some(actor),
        Method::GET,
        &format!("{path}?episode_id={}", episode["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(revised["total"], 1, "{revised}");
    assert_eq!(revised["results"][0]["source_id"], replacement["id"]);
    // Queries and clinical text are not copied into audit events.
    let audits: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(a) FROM audit_events a WHERE resource='knowledge_documents'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!audits.is_empty());
    assert!(!serde_json::to_string(&audits).unwrap().contains("Monday"));
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
