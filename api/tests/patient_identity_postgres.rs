//! Registration conflict and identity boundaries against a disposable isolated schema.
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
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
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
async fn registration_preserves_mrn_exact_review_and_patient_scope() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(actual, "nexus_test");
    let schema = format!("nexus_identity_test_{}", Uuid::new_v4().simple());
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
    let(s,setup)=send(&pool,None,Method::POST,"/api/setup",json!({"organization_name":"Synthetic identity QA","facility_name":"Synthetic adult facility","admin_name":"Synthetic registrar","admin_email":"identity@example.invalid"})).await;
    assert_eq!(s, StatusCode::CREATED, "{setup}");
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let body = json!({"first_name":"Ana Maria","last_name":"Sample","birth_date":"1990-01-01","email":"ana@example.invalid","phone":"(212) 555-0100","facility_id":facility,"primary_clinician_id":actor,"pronouns":"she/her","preferred_language":"English","city":"Synthetic city"});
    let first = post(&pool, actor, "/api/clients", body.clone()).await;
    assert!(first["mrn"].as_str().unwrap().starts_with("NX"));
    assert_eq!(first["pronouns"], "she/her");
    let first_id = Uuid::parse_str(first["id"].as_str().unwrap()).unwrap();
    assert!(
        sqlx::query("UPDATE clients SET mrn='rewritten' WHERE id=$1")
            .bind(first_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::PATCH,
            &format!("/api/clients/{first_id}"),
            json!({"_expected_updated_at":first["updated_at"],"mrn":"override"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let matching = json!({"first_name":" ana   maria ","last_name":"SAMPLE","birth_date":"1990-01-01","email":"ANA@example.invalid","phone":"2125550100"});
    let found = post(&pool, actor, "/api/intake/matches", matching.clone()).await;
    assert_eq!(found["matches"].as_array().unwrap().len(), 1);
    assert_eq!(found["matches"][0]["id"], first["id"]);
    assert_eq!(found["matches"][0]["reasons"].as_array().unwrap().len(), 3);
    assert_eq!(found["restricted_matches"], false);
    assert!(found["matches"][0].get("email").is_none());
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::POST,
            "/api/clients",
            body.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut reviewed = body.clone();
    reviewed["_duplicate_review"] = json!({"matched_ids":[first["id"]],"reason":"Reviewed synthetic same-name identities; separate records are intended."});
    let second = post(&pool, actor, "/api/clients", reviewed.clone()).await;
    assert_ne!(second["mrn"], first["mrn"]);
    // A newly introduced match invalidates the earlier review set, even with a valid reason.
    assert_eq!(
        send(&pool, Some(actor), Method::POST, "/api/clients", reviewed)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM patient_duplicate_reviews")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(sqlx::query("DELETE FROM patient_duplicate_reviews")
        .execute(&pool)
        .await
        .is_err());
    // Unchanged identity edits remain usable even where a historical duplicate exists.
    let (s, edited) = send(
        &pool,
        Some(actor),
        Method::PATCH,
        &format!("/api/clients/{first_id}"),
        json!({"_expected_updated_at":first["updated_at"],"city":"Updated synthetic city"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{edited}");
    assert_eq!(edited["mrn"], first["mrn"]);
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::PATCH,
            &format!("/api/clients/{first_id}"),
            json!({"_expected_updated_at":first["updated_at"],"city":"Stale edit"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &pool,
            Some(actor),
            Method::PATCH,
            &format!("/api/clients/{first_id}"),
            json!({"_expected_updated_at":edited["updated_at"],"phone":"212-555-0100"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut exclude = matching.clone();
    exclude["exclude_client_id"] = first["id"].clone();
    let only_other = post(&pool, actor, "/api/intake/matches", exclude).await;
    assert_eq!(only_other["matches"].as_array().unwrap().len(), 1);
    assert_eq!(only_other["matches"][0]["id"], second["id"]);
    let mut other_birth = matching.clone();
    other_birth["birth_date"] = json!("1991-01-01");
    assert_eq!(
        post(&pool, actor, "/api/intake/matches", other_birth).await["matches"],
        json!([])
    );
    let clinician=post(&pool,actor,"/api/staff",json!({"name":"Synthetic panel clinician","email":"panel@example.invalid","role":"clinician"})).await;
    let scoped = post(
        &pool,
        clinician["id"].as_str().unwrap(),
        "/api/intake/matches",
        matching.clone(),
    )
    .await;
    assert_eq!(scoped, json!({"matches":[],"restricted_matches":true}));
    let mut override_hidden = body.clone();
    override_hidden["_duplicate_review"] = json!({"matched_ids":[first["id"],second["id"]],"reason":"Cannot override inaccessible synthetic identity records."});
    assert_eq!(
        send(
            &pool,
            clinician["id"].as_str(),
            Method::POST,
            "/api/clients",
            override_hidden
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for role in ["billing", "executive"] {
        let staff=post(&pool,actor,"/api/staff",json!({"name":"Synthetic restricted operator","email":format!("{role}@example.invalid"),"role":role})).await;
        assert_eq!(
            send(
                &pool,
                staff["id"].as_str(),
                Method::POST,
                "/api/intake/matches",
                matching.clone()
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let support=post(&pool,actor,"/api/staff",json!({"name":"Synthetic admissions operator","email":"support@example.invalid","role":"support"})).await;
    assert_eq!(
        post(
            &pool,
            support["id"].as_str().unwrap(),
            "/api/intake/matches",
            matching.clone()
        )
        .await["matches"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // A different tenant sees no candidate or restricted-match signal.
    let other_org = Uuid::new_v4();
    let other_actor = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Synthetic separate tenant')")
        .bind(other_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role) VALUES($1,$2,'Synthetic separate operator','separate@example.invalid','admin')").bind(other_actor).bind(other_org).execute(&pool).await.unwrap();
    assert_eq!(
        post(
            &pool,
            &other_actor.to_string(),
            "/api/intake/matches",
            matching
        )
        .await,
        json!({"matches":[],"restricted_matches":false})
    );
    // Organization locking prevents simultaneous identical registrations bypassing review.
    let mut concurrent = body.clone();
    concurrent["birth_date"] = json!("1992-02-02");
    let (a, b) = tokio::join!(
        send(
            &pool,
            Some(actor),
            Method::POST,
            "/api/clients",
            concurrent.clone()
        ),
        send(&pool, Some(actor), Method::POST, "/api/clients", concurrent)
    );
    assert!(
        (a.0 == StatusCode::CREATED && b.0 == StatusCode::CONFLICT)
            || (b.0 == StatusCode::CREATED && a.0 == StatusCode::CONFLICT),
        "{a:?} {b:?}"
    );
    let mut oversized = body;
    oversized["preferred_language"] = json!("x".repeat(81));
    assert_eq!(
        send(&pool, Some(actor), Method::POST, "/api/clients", oversized)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
