//! Real database integration test. Uses an isolated, explicitly named disposable database.
use actix_web::{
    http::{Method, StatusCode},
    test, web, App,
};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn attach_loaded_updated_at(
    pool: &PgPool,
    method: &Method,
    path: &str,
    body: Value,
) -> Value {
    if *method != Method::PATCH {
        return body;
    }
    let Some(mut object) = body.as_object().cloned() else {
        return body;
    };
    // An explicit token, including a deliberately stale one or null, is preserved.
    if object.contains_key("_expected_updated_at") {
        return body;
    }
    let Some(rest) = path.strip_prefix("/api/") else {
        return body;
    };
    let mut parts = rest.split('/');
    let Some(resource) = parts.next() else {
        return body;
    };
    let Some(id) = parts.next() else {
        return body;
    };
    if parts.next().is_some() || !resource.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return body;
    }
    let Ok(id) = Uuid::parse_str(id) else {
        return body;
    };
    let sql = format!("SELECT to_jsonb(updated_at) FROM {resource} WHERE id=$1");
    let updated_at: Option<Value> = sqlx::query_scalar(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    let Some(updated_at) = updated_at.filter(|value| value.is_string()) else {
        return body;
    };
    object.insert("_expected_updated_at".into(), updated_at);
    Value::Object(object)
}
async fn request(
    pool: &PgPool,
    method: Method,
    path: &str,
    actor: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let body = attach_loaded_updated_at(pool, &method, path, body).await;
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
    if let Some(id) = actor {
        req = req.insert_header(("x-nexus-actor", id));
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let body = test::read_body(response).await;
    (
        status,
        serde_json::from_slice(&body).expect("JSON response"),
    )
}
async fn create(pool: &PgPool, actor: &str, resource: &str, body: Value) -> Value {
    let (status, value) = request(
        pool,
        Method::POST,
        &format!("/api/{resource}"),
        Some(actor),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{resource}: {value}");
    value
}

#[actix_web::test]
#[ignore = "Requires TEST_DATABASE_URL ending in /nexus_test; resets that disposable schema"]
async fn persistent_workflows_and_boundaries() {
    let url =
        std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL for isolated database");
    assert!(
        url.ends_with("/nexus_test"),
        "Refusing to reset any database except nexus_test"
    );
    let pool = PgPool::connect(&url).await.unwrap();
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(&pool)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let (_, empty) = request(&pool, Method::GET, "/api/bootstrap", None, json!({})).await;
    assert_eq!(empty["setup_required"], true);
    let(status,setup)=request(&pool,Method::POST,"/api/setup",None,json!({"organization_name":"Integration test organization","facility_name":"Isolated test facility","admin_name":"Test administrator","admin_email":"admin@example.invalid"})).await;
    assert_eq!(status, StatusCode::CREATED, "{setup}");
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let client=create(&pool,actor,"clients",json!({"facility_id":facility,"first_name":"Test","last_name":"Client","birth_date":"1990-01-01","primary_clinician_id":actor})).await;
    let episode=create(&pool,actor,"episodes",json!({"client_id":client["id"],"facility_id":facility,"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let session=create(&pool,actor,"sessions",json!({"client_id":client["id"],"episode_id":episode["id"],"clinician_id":actor,"starts_at":"2026-02-01T14:00:00Z","ends_at":"2026-02-01T15:00:00Z","kind":"individual","status":"completed"})).await;
    let note=create(&pool,actor,"notes",json!({"client_id":client["id"],"session_id":session["id"],"title":"Test progress","body":"Synthetic integration-test note, never used in live workspace."})).await;
    let note_id = note["id"].as_str().unwrap();
    let (status, signed) = request(
        &pool,
        Method::POST,
        &format!("/api/notes/{note_id}/sign"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["status"], "signed");
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/notes/{note_id}"),
        Some(actor),
        json!({"body":"Overwrite signed history"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let(status,amendment)=request(&pool,Method::POST,&format!("/api/notes/{note_id}/amend"),Some(actor),json!({"body":"Corrected synthetic entry","reason":"Correct the original test observation"})).await;
    assert_eq!(status, StatusCode::OK, "{amendment}");
    assert_eq!(amendment["amends_note_id"], note["id"]);
    let policy=create(&pool,actor,"policies",json!({"client_id":client["id"],"payer_name":"Test payer","member_id":"TEST-ONLY","effective_date":"2026-01-01"})).await;
    let authorization=create(&pool,actor,"authorizations",json!({"client_id":client["id"],"policy_id":policy["id"],"authorization_number":"TEST-AUTH","service_code":"TEST","units_approved":1,"starts_on":"2026-01-01","ends_on":"2026-12-31"})).await;
    let claim=create(&pool,actor,"claims",json!({"client_id":client["id"],"session_id":session["id"],"policy_id":policy["id"],"authorization_id":authorization["id"],"service_code":"TEST","diagnosis_code":"TEST","amount_cents":10000})).await;
    let claim_id = claim["id"].as_str().unwrap();
    let (status, readiness) = request(
        &pool,
        Method::GET,
        &format!("/api/claims/{claim_id}/readiness"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{readiness}");
    assert_eq!(readiness["ready"], true, "{readiness}");
    assert!(readiness.get("summary").is_none());
    let checklist = readiness["checklist"].as_array().expect("checklist");
    for code in [
        "session_completed",
        "session_ended",
        "signed_note",
        "policy_coverage",
        "service_code",
        "diagnosis_code",
        "authorization_coverage",
        "authorization_units",
    ] {
        let item = checklist
            .iter()
            .find(|item| item["code"] == code)
            .unwrap_or_else(|| panic!("{code} missing in {readiness}"));
        assert_eq!(item["passed"], true, "{item}");
        assert!(!item["title"].as_str().unwrap_or_default().is_empty());
        assert!(!item["message"].as_str().unwrap_or_default().is_empty());
        assert!(matches!(
            item["owner_role"].as_str(),
            Some("clinical" | "billing")
        ));
        assert!(item.get("body").is_none());
    }
    assert!(!readiness
        .to_string()
        .contains("Synthetic integration-test note"));
    let (status, ready) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{claim_id}/ready"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    let (status, submitted) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{claim_id}/record-submission"),
        Some(actor),
        json!({"external_reference":"OUTSIDE-TEST"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");
    let payment_a = json!({"claim_id":claim["id"],"amount_cents":6000,"received_on":"2026-02-02","reference":"TEST-PAY-A","source":"payer"});
    let payment_b = json!({"claim_id":claim["id"],"amount_cents":6000,"received_on":"2026-02-02","reference":"TEST-PAY-B","source":"payer"});
    let (a, b) = tokio::join!(
        request(&pool, Method::POST, "/api/payments", Some(actor), payment_a),
        request(&pool, Method::POST, "/api/payments", Some(actor), payment_b)
    );
    assert!(
        (a.0 == StatusCode::CREATED && b.0 == StatusCode::CONFLICT)
            || (b.0 == StatusCode::CREATED && a.0 == StatusCode::CONFLICT),
        "Concurrent overpayment: {a:?} {b:?}"
    );
    let billing=create(&pool,actor,"staff",json!({"name":"Test billing","email":"billing@example.invalid","role":"billing","facility_id":facility})).await;
    let (status, finance) = request(
        &pool,
        Method::GET,
        "/api/bootstrap",
        Some(billing["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(finance["notes"], json!([]));
    assert_eq!(finance["claims"].as_array().unwrap().len(), 1);
    let (status, _) = request(
        &pool,
        Method::GET,
        "/api/notes",
        Some(billing["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, billing_readiness) = request(
        &pool,
        Method::GET,
        &format!("/api/claims/{claim_id}/readiness"),
        Some(billing["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{billing_readiness}");
    assert_eq!(billing_readiness["ready"], true);
    assert!(!billing_readiness
        .to_string()
        .contains("Synthetic integration-test note"));
    let support = create(
        &pool,
        actor,
        "staff",
        json!({"name":"Test support","email":"support@example.invalid","role":"support","facility_id":facility}),
    )
    .await;
    let support_id = support["id"].as_str().unwrap();
    let (status, _) = request(
        &pool,
        Method::GET,
        &format!("/api/claims/{claim_id}/readiness"),
        Some(support_id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{claim_id}/ready"),
        Some(support_id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let unsigned_session = create(
        &pool,
        actor,
        "sessions",
        json!({"client_id":client["id"],"episode_id":episode["id"],"clinician_id":actor,"starts_at":"2026-02-01T18:00:00Z","ends_at":"2026-02-01T19:00:00Z","kind":"individual","status":"completed"}),
    )
    .await;
    let blocked = create(
        &pool,
        actor,
        "claims",
        json!({"client_id":client["id"],"session_id":unsigned_session["id"],"policy_id":policy["id"],"service_code":"TEST","diagnosis_code":"TEST","amount_cents":500}),
    )
    .await;
    let blocked_id = blocked["id"].as_str().unwrap();
    let (status, failed) = request(
        &pool,
        Method::GET,
        &format!("/api/claims/{blocked_id}/readiness"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{failed}");
    assert_eq!(failed["ready"], false);
    let note_rule = failed["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "signed_note")
        .expect("signed note rule");
    assert_eq!(note_rule["passed"], false);
    assert_eq!(note_rule["owner_role"], "clinical");
    assert_eq!(
        failed["checklist"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == "session_completed")
            .unwrap()["passed"],
        true
    );
    let (status, blocked_ready) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{blocked_id}/ready"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{blocked_ready}");
    assert!(blocked_ready["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("signed session note"));
    let (status, _) = request(&pool, Method::GET, "/api/clients", None, json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = request(
        &pool,
        Method::GET,
        "/api/clients",
        Some(&Uuid::new_v4().to_string()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let other_org = Uuid::new_v4();
    let other_client = Uuid::new_v4();
    let other_facility = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Other isolated org')")
        .bind(other_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO facilities(id,organization_id,name) VALUES($1,$2,'Other facility')")
        .bind(other_facility)
        .bind(other_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date) VALUES($1,$2,$3,'Other','Test','1990-01-01')").bind(other_client).bind(other_org).bind(other_facility).execute(&pool).await.unwrap();
    let(status,_)=request(&pool,Method::POST,"/api/policies",Some(actor),json!({"client_id":other_client,"payer_name":"No","member_id":"No","effective_date":"2026-01-01"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/clients/{other_client}"),
        Some(actor),
        json!({"first_name":"Forbidden"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, workspace) =
        request(&pool, Method::GET, "/api/bootstrap", Some(actor), json!({})).await;
    assert_eq!(workspace["clients"].as_array().unwrap().len(), 1);
    assert!(workspace["audit"].as_array().unwrap().len() > 10);
    assert_eq!(workspace["metrics"]["outstanding_cents"], 4000);
    // Reversals retain original receipts and serialize with concurrent new receipts.
    let posted = if a.0 == StatusCode::CREATED {
        &a.1
    } else {
        &b.1
    };
    let payment_id = posted["id"].as_str().unwrap();
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/payments/{payment_id}/reverse"),
        Some(actor),
        json!({"reason":"Correct an erroneous test receipt","reversed_on":"2026-02-01"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, reversal) = request(
        &pool,
        Method::POST,
        &format!("/api/payments/{payment_id}/reverse"),
        Some(actor),
        json!({"reason":"Correct an erroneous test receipt","reversed_on":"2026-02-03"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reversal}");
    assert_eq!(reversal["amount_cents"], 6000);
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/payments/{payment_id}/reverse"),
        Some(actor),
        json!({"reason":"Duplicate reversal must be rejected","reversed_on":"2026-02-03"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, reversed_workspace) =
        request(&pool, Method::GET, "/api/bootstrap", Some(actor), json!({})).await;
    assert_eq!(reversed_workspace["metrics"]["outstanding_cents"], 10000);
    assert_eq!(reversed_workspace["metrics"]["cash_received_cents"], 0);
    let replacement_payment=create(&pool,actor,"payments",json!({"claim_id":claim["id"],"amount_cents":10000,"received_on":"2026-02-04","reference":"FULL-CORRECTED-TEST","source":"payer"})).await;
    let reverse_path = format!(
        "/api/payments/{}/reverse",
        replacement_payment["id"].as_str().unwrap()
    );
    let (reverse_result, new_result) = tokio::join!(
        request(
            &pool,
            Method::POST,
            &reverse_path,
            Some(actor),
            json!({"reason":"Exercise reverse and receipt serialization","reversed_on":"2026-02-05"})
        ),
        request(
            &pool,
            Method::POST,
            "/api/payments",
            Some(actor),
            json!({"claim_id":claim["id"],"amount_cents":5000,"received_on":"2026-02-05","reference":"CONCURRENT-CORRECTION","source":"payer"})
        )
    );
    assert_eq!(reverse_result.0, StatusCode::OK, "{reverse_result:?}");
    assert!(
        matches!(new_result.0, StatusCode::CREATED | StatusCode::CONFLICT),
        "{new_result:?}"
    );
    let (_, balanced) = request(&pool, Method::GET, "/api/bootstrap", Some(actor), json!({})).await;
    let net = balanced["metrics"]["cash_received_cents"].as_i64().unwrap();
    assert!(net == 0 || net == 5000);
    assert_eq!(balanced["metrics"]["outstanding_cents"], 10000 - net);
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{claim_id}/void"),
        Some(actor),
        json!({"reason":"Submitted claims may not be voided locally"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Source records behind signed notes and billed claims cannot silently change.
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/sessions/{}", session["id"].as_str().unwrap()),
        Some(actor),
        json!({"starts_at":"2026-02-01T12:00:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/policies/{}", policy["id"].as_str().unwrap()),
        Some(actor),
        json!({"member_id":"CHANGED"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Explicit read grants cannot become write access, and revocation takes effect.
    let grant=create(&pool,actor,"exceptions",json!({"staff_id":billing["id"],"resource":"notes","reason":"Test minimum read-only access","expires_at":"2099-01-01T00:00:00Z"})).await;
    let (status, _) = request(
        &pool,
        Method::GET,
        "/api/notes",
        Some(billing["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &pool,
        Method::POST,
        "/api/notes",
        Some(billing["id"].as_str().unwrap()),
        json!({"client_id":client["id"],"title":"Forbidden","body":"No write grant"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/exceptions/{}", grant["id"].as_str().unwrap()),
        Some(actor),
        json!({"revoked":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &pool,
        Method::GET,
        "/api/notes",
        Some(billing["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let clinician=create(&pool,actor,"staff",json!({"name":"Test clinician","email":"clinician@example.invalid","role":"clinician","facility_id":facility})).await;
    let clinician_id = clinician["id"].as_str().unwrap();
    create(&pool,actor,"exceptions",json!({"staff_id":clinician["id"],"resource":"claims","access_mode":"write","reason":"Test retained assignment boundary","expires_at":"2099-01-01T00:00:00Z"})).await;
    let (_, restricted) = request(
        &pool,
        Method::GET,
        "/api/claims",
        Some(clinician_id),
        json!({}),
    )
    .await;
    assert_eq!(restricted, json!([]));
    let(status,_)=request(&pool,Method::POST,"/api/claims",Some(clinician_id),json!({"client_id":client["id"],"session_id":session["id"],"policy_id":policy["id"],"service_code":"NOT-ASSIGNED","diagnosis_code":"TEST","amount_cents":100})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let task=create(&pool,actor,"tasks",json!({"assignee_id":clinician["id"],"title":"Transfer this in-progress task","status":"in_progress"})).await;
    let (status, offboarded) = request(
        &pool,
        Method::POST,
        &format!("/api/staff/{clinician_id}/offboard"),
        Some(actor),
        json!({"transfer_to":actor}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{offboarded}");
    assert_eq!(offboarded["active"], false);
    let (_, tasks) = request(&pool, Method::GET, "/api/tasks", Some(actor), json!({})).await;
    assert_eq!(
        tasks
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == task["id"])
            .unwrap()["assignee_id"],
        actor
    );
    let (status, _) = request(
        &pool,
        Method::GET,
        "/api/bootstrap",
        Some(clinician_id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Structured forms freeze templates, enforce required fields, and retain evidence.
    let template=create(&pool,actor,"templates",json!({"name":"Test progress form","kind":"progress","fields":[{"key":"observation","label":"Observation","type":"textarea","required":true}]})).await;
    let response=create(&pool,actor,"form_responses",json!({"template_id":template["id"],"client_id":client["id"],"episode_id":episode["id"],"session_id":session["id"]})).await;
    let response_id = response["id"].as_str().unwrap();
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/form_responses/{response_id}/review"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let(status,filled)=request(&pool,Method::PATCH,&format!("/api/form_responses/{response_id}"),Some(actor),json!({"answers":{"observation":"Reported one practice"},"source_type":"transcript","source_excerpt":"Participant reported one practice today.","field_provenance":{"observation":{"excerpt":"reported one practice"}}})).await;
    assert_eq!(status, StatusCode::OK, "{filled}");
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/templates/{}", template["id"].as_str().unwrap()),
        Some(actor),
        json!({"fields":[{"key":"different","label":"New version field","type":"text"}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, reviewed) = request(
        &pool,
        Method::POST,
        &format!("/api/form_responses/{response_id}/review"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reviewed}");
    assert_eq!(
        reviewed["template_snapshot"]["fields"][0]["key"],
        "observation"
    );
    let (status, signed_response) = request(
        &pool,
        Method::POST,
        &format!("/api/form_responses/{response_id}/sign"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{signed_response}");
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/form_responses/{response_id}"),
        Some(actor),
        json!({"answers":{"observation":"overwrite"}}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, generated) = request(
        &pool,
        Method::POST,
        &format!("/api/form_responses/{response_id}/create-note"),
        Some(actor),
        json!({"title":"Test signed form note"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{generated}");
    assert_eq!(generated["form_response_id"], response["id"]);
    assert_eq!(generated["body"], "Observation: Reported one practice");
    assert_eq!(generated["status"], "draft");
    let (status, amended) = request(
        &pool,
        Method::POST,
        &format!("/api/form_responses/{response_id}/amend"),
        Some(actor),
        json!({"reason":"Correct the transcribed observation"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{amended}");
    assert_eq!(amended["amends_response_id"], response["id"]);
    assert_eq!(amended["status"], "draft");
    // Treatment plan attestation preserves definitions and snapshots progress.
    let plan=create(&pool,actor,"treatment_plans",json!({"client_id":client["id"],"episode_id":episode["id"],"title":"Test recovery plan","presenting_needs":"Synthetic integration scenario","strengths":"Test-only strengths"})).await;
    let plan_id = plan["id"].as_str().unwrap();
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/treatment_plans/{plan_id}"),
        Some(actor),
        json!({"status":"active"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/treatment_plans/{plan_id}/activate"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let goal=create(&pool,actor,"goals",json!({"plan_id":plan["id"],"description":"Practice a chosen coping strategy","measure":"Self-reported practice count","baseline":"0 practices","target":"3 practices"})).await;
    let (status, active_plan) = request(
        &pool,
        Method::POST,
        &format!("/api/treatment_plans/{plan_id}/activate"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{active_plan}");
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/goals/{}", goal["id"].as_str().unwrap()),
        Some(actor),
        json!({"target":"Overwrite history"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let review=create(&pool,actor,"plan_reviews",json!({"plan_id":plan["id"],"summary":"Test progress observation","goal_progress":[{"goal_id":goal["id"],"status":"on_track","observation":"One reported practice in isolated fixture"}]})).await;
    assert_eq!(review["goal_snapshot"][0]["target"], "3 practices");
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/plan_reviews/{}", review["id"].as_str().unwrap()),
        Some(actor),
        json!({"summary":"Overwrite review"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, revised) = request(
        &pool,
        Method::POST,
        &format!("/api/treatment_plans/{plan_id}/revise"),
        Some(actor),
        json!({"reason":"Adjust the plan after a new assessment"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    let (status, new_active) = request(
        &pool,
        Method::POST,
        &format!(
            "/api/treatment_plans/{}/activate",
            revised["id"].as_str().unwrap()
        ),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{new_active}");
    // A shared group produces a real attendance-linked session, enforces capacity,
    // and preserves signed clinical/billing evidence.
    let group=create(&pool,actor,"groups",json!({"facility_id":facility,"clinician_id":actor,"title":"Test skills group","kind":"skills","starts_at":"2026-02-02T14:00:00Z","ends_at":"2026-02-02T15:00:00Z","capacity":1})).await;
    let attendance=create(&pool,actor,"group_attendance",json!({"group_id":group["id"],"client_id":client["id"],"episode_id":episode["id"],"status":"attended"})).await;
    assert!(attendance["session_id"].as_str().is_some());
    let other_clinician=create(&pool,actor,"staff",json!({"name":"Other group clinician","email":"other-clinician@example.invalid","role":"clinician","facility_id":facility})).await;
    let other_clinician_id = other_clinician["id"].as_str().unwrap();
    let own_client=create(&pool,actor,"clients",json!({"facility_id":facility,"first_name":"Group","last_name":"Test","birth_date":"1990-01-01","primary_clinician_id":other_clinician["id"]})).await;
    let own_episode=create(&pool,actor,"episodes",json!({"client_id":own_client["id"],"facility_id":facility,"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let other_group=create(&pool,actor,"groups",json!({"facility_id":facility,"clinician_id":actor,"title":"Facilitator scope test","kind":"skills","starts_at":"2026-02-02T16:00:00Z","ends_at":"2026-02-02T17:00:00Z","capacity":3})).await;
    let(status,_)=request(&pool,Method::POST,"/api/group_attendance",Some(other_clinician_id),json!({"group_id":other_group["id"],"client_id":own_client["id"],"episode_id":own_episode["id"]})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let other_attendance=create(&pool,actor,"group_attendance",json!({"group_id":other_group["id"],"client_id":own_client["id"],"episode_id":own_episode["id"]})).await;
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!(
            "/api/group_attendance/{}",
            other_attendance["id"].as_str().unwrap()
        ),
        Some(other_clinician_id),
        json!({"status":"attended"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = request(
        &pool,
        Method::POST,
        "/api/group_attendance",
        Some(actor),
        json!({"group_id":group["id"],"client_id":client["id"],"episode_id":episode["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let group_note=create(&pool,actor,"notes",json!({"client_id":client["id"],"session_id":attendance["session_id"],"title":"Test group participation","body":"Test-only individual observation"})).await;
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/notes/{}/sign", group_note["id"].as_str().unwrap()),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!(
            "/api/group_attendance/{}",
            attendance["id"].as_str().unwrap()
        ),
        Some(actor),
        json!({"status":"no_show"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, completed) = request(
        &pool,
        Method::POST,
        &format!("/api/groups/{}/complete", group["id"].as_str().unwrap()),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    let corrective_claim=create(&pool,actor,"claims",json!({"client_id":client["id"],"session_id":attendance["session_id"],"policy_id":policy["id"],"service_code":"GROUP-TEST","diagnosis_code":"TEST","amount_cents":2000})).await;
    let corrective_id = corrective_claim["id"].as_str().unwrap();
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{corrective_id}/ready"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{corrective_id}/return-to-draft"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &pool,
        Method::PATCH,
        &format!("/api/claims/{corrective_id}"),
        Some(actor),
        json!({"amount_cents":1500}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, voided) = request(
        &pool,
        Method::POST,
        &format!("/api/claims/{corrective_id}/void"),
        Some(actor),
        json!({"reason":"Replace incorrect unsubmitted test claim"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{voided}");
    assert_eq!(voided["status"], "voided");
    create(&pool,actor,"claims",json!({"client_id":client["id"],"session_id":attendance["session_id"],"policy_id":policy["id"],"service_code":"GROUP-TEST","diagnosis_code":"TEST","amount_cents":1700})).await;
    let(status,discharged)=request(&pool,Method::POST,&format!("/api/episodes/{}/discharge",episode["id"].as_str().unwrap()),Some(actor),json!({"end_date":"2026-02-03","discharge_reason":"Completed isolated integration scenario"})).await;
    assert_eq!(status, StatusCode::OK, "{discharged}");
    // Two editors share the originally loaded version. The stale write must not land or be audited.
    let draft = create(
        &pool,
        actor,
        "tasks",
        json!({"title":"Concurrent draft","description":"Original unsaved-safe text"}),
    )
    .await;
    let draft_id = draft["id"].as_str().unwrap();
    let stale = draft["updated_at"].clone();
    let (status, first) = request(
        &pool,
        Method::PATCH,
        &format!("/api/tasks/{draft_id}"),
        Some(actor),
        json!({"title":"First editor","description":"Kept","_expected_updated_at":stale}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["title"], "First editor");
    assert_ne!(first["updated_at"], stale);
    let (status, second) = request(
        &pool,
        Method::PATCH,
        &format!("/api/tasks/{draft_id}"),
        Some(actor),
        json!({"title":"Second editor overwrite","description":"Lost","_expected_updated_at":stale}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{second}");
    assert_eq!(second["error"]["code"], "conflict");
    let message = second["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    assert!(message.contains("reload"), "{message}");
    assert!(
        message.contains("preserve") && message.contains("unsaved"),
        "{message}"
    );
    let (_, tasks_after) = request(&pool, Method::GET, "/api/tasks", Some(actor), json!({})).await;
    let stored = tasks_after
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == draft["id"])
        .unwrap();
    assert_eq!(stored["title"], "First editor");
    assert_eq!(stored["description"], "Kept");
    assert_eq!(stored["updated_at"], first["updated_at"]);
    let updates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE record_id=$1 AND action='update'",
    )
    .bind(Uuid::parse_str(draft_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(updates, 1, "stale PATCH must not append an update audit");
    let (status, missing_token) = request(
        &pool,
        Method::PATCH,
        &format!("/api/tasks/{draft_id}"),
        Some(actor),
        json!({"title":"No token","_expected_updated_at":Value::Null}),
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "{missing_token}");
    assert_eq!(missing_token["error"]["code"], "precondition_required");
    let updates_after_missing: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE record_id=$1 AND action='update'",
    )
    .bind(Uuid::parse_str(draft_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(updates_after_missing, 1);
    let (status, injected) = request(
        &pool,
        Method::PATCH,
        &format!("/api/tasks/{draft_id}"),
        Some(actor),
        json!({"description":"Helper supplied current version"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{injected}");
    assert_eq!(injected["title"], "First editor");
    assert_eq!(injected["description"], "Helper supplied current version");
    verify_facility_service_dates(&pool, actor).await;
    verify_admin_clinical_offboarding(&pool, actor, facility).await;
    verify_assignment_waits_for_offboarding(&pool, actor, facility).await;
    verify_role_changes_preserve_clinical_assignments(&pool, actor, facility).await;
    pool.close().await;
}

async fn verify_role_changes_preserve_clinical_assignments(
    pool: &PgPool,
    actor: &str,
    facility: &Value,
) {
    let client = create(pool, actor, "clients", json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Role transition","birth_date":"1990-01-01"})).await;
    let episode = create(pool, actor, "episodes", json!({"client_id":client["id"],"facility_id":facility,"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let future = chrono::Utc::now() + chrono::Duration::days(2);
    for kind in ["client", "session", "group"] {
        let staff = create(pool, actor, "staff", json!({"name":format!("Synthetic {kind} clinician"),"email":format!("role-{kind}@example.invalid"),"role":"clinician"})).await;
        let staff_id = staff["id"].as_str().unwrap();
        let (resource, assignment, field) = match kind {
            "client" => {
                let (status, row) = request(pool, Method::PATCH, &format!("/api/clients/{}",client["id"].as_str().unwrap()), Some(actor), json!({"primary_clinician_id":staff_id})).await;
                assert_eq!(status, StatusCode::OK, "{row}");
                ("clients", row, "primary_clinician_id")
            },
            "session" => ("sessions", create(pool, actor, "sessions", json!({"client_id":client["id"],"episode_id":episode["id"],"clinician_id":staff_id,"kind":"individual","starts_at":future,"ends_at":future+chrono::Duration::hours(1)})).await, "clinician_id"),
            _ => ("groups", create(pool, actor, "groups", json!({"facility_id":facility,"clinician_id":staff_id,"title":"Synthetic role guard group","kind":"skills","starts_at":future,"ends_at":future+chrono::Duration::hours(1),"capacity":4})).await, "clinician_id"),
        };
        let id = Uuid::parse_str(staff_id).unwrap();
        let audit_before: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource='staff' AND action='update' AND record_id=$1").bind(id).fetch_one(pool).await.unwrap();
        for nonclinical in ["billing", "executive", "support"] {
            let (status, body) = request(
                pool,
                Method::PATCH,
                &format!("/api/staff/{staff_id}"),
                Some(actor),
                json!({"role":nonclinical}),
            )
            .await;
            assert_eq!(status, StatusCode::CONFLICT, "{kind}/{nonclinical}: {body}");
        }
        let preserved: Value = sqlx::query_scalar("SELECT to_jsonb(s) FROM staff s WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(preserved["role"], staff["role"]);
        assert_eq!(preserved["updated_at"], staff["updated_at"]);
        let audit_after: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource='staff' AND action='update' AND record_id=$1").bind(id).fetch_one(pool).await.unwrap();
        assert_eq!(audit_before, audit_after);
        let (status, body) = request(
            pool,
            Method::PATCH,
            &format!("/api/staff/{staff_id}"),
            Some(actor),
            json!({"role":"clinical_lead"}),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "Clinical role changes remain allowed: {body}"
        );
        let (status, body) = request(
            pool,
            Method::PATCH,
            &format!("/api/{resource}/{}", assignment["id"].as_str().unwrap()),
            Some(actor),
            json!({field:actor}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "Explicit reassignment: {body}");
        let (status, body) = request(
            pool,
            Method::PATCH,
            &format!("/api/staff/{staff_id}"),
            Some(actor),
            json!({"role":"billing"}),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "Role change after reassignment: {body}"
        );
    }
}

async fn verify_admin_clinical_offboarding(pool: &PgPool, actor: &str, facility: &Value) {
    let departing = create(pool, actor, "staff", json!({"name":"Departing clinical administrator","email":"departing-admin@example.invalid","role":"admin"})).await;
    let billing = create(pool, actor, "staff", json!({"name":"Billing replacement","email":"replacement-billing@example.invalid","role":"billing"})).await;
    let client = create(pool, actor, "clients", json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Offboarding","birth_date":"1990-01-01","primary_clinician_id":departing["id"]})).await;
    let departing_id = departing["id"].as_str().unwrap();
    let path = format!("/api/staff/{departing_id}/offboard");
    let (status, body) = request(
        pool,
        Method::POST,
        &path,
        Some(actor),
        json!({"transfer_to":billing["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let assigned: Uuid = sqlx::query_scalar("SELECT primary_clinician_id FROM clients WHERE id=$1")
        .bind(Uuid::parse_str(client["id"].as_str().unwrap()).unwrap())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(assigned.to_string(), departing_id);
    let (status, body) = request(
        pool,
        Method::POST,
        &path,
        Some(actor),
        json!({"transfer_to":actor}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let assigned: Uuid = sqlx::query_scalar("SELECT primary_clinician_id FROM clients WHERE id=$1")
        .bind(Uuid::parse_str(client["id"].as_str().unwrap()).unwrap())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(assigned.to_string(), actor);
}

async fn verify_assignment_waits_for_offboarding(pool: &PgPool, actor: &str, facility: &Value) {
    let departing = create(pool, actor, "staff", json!({"name":"Concurrent departing clinician","email":"concurrent-departing@example.invalid","role":"clinician"})).await;
    let id = Uuid::parse_str(departing["id"].as_str().unwrap()).unwrap();
    // Hold the same row lock used by offboarding, then attempt a new assignment.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM staff WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let pending = request(
        pool,
        Method::POST,
        "/api/clients",
        Some(actor),
        json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Concurrent assignment","birth_date":"1990-01-01","primary_clinician_id":id}),
    );
    tokio::pin!(pending);
    tokio::select! {
        result = &mut pending => panic!("Assignment escaped the offboarding lock: {result:?}"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
    }
    sqlx::query("UPDATE staff SET active=false WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(5), pending)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("active"));
    let assigned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM clients WHERE primary_clinician_id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(assigned, 0);
}

async fn verify_facility_service_dates(pool: &PgPool, actor: &str) {
    let (status, _) = request(
        pool,
        Method::POST,
        "/api/facilities",
        Some(actor),
        json!({"name":"Invalid timezone QA", "timezone":"Not/A_Zone"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let facility = create(
        pool,
        actor,
        "facilities",
        json!({"name":"Pacific QA", "timezone":"America/Los_Angeles"}),
    )
    .await;
    let client = create(pool, actor, "clients", json!({"facility_id":facility["id"], "first_name":"Timezone", "last_name":"QA", "birth_date":"1990-01-01"})).await;
    let episode = create(pool, actor, "episodes", json!({"client_id":client["id"], "facility_id":facility["id"], "level_of_care":"outpatient", "start_date":"2026-02-01"})).await;
    let future = chrono::Utc::now() + chrono::Duration::days(1);
    let (status, _) = request(pool, Method::POST, "/api/sessions", Some(actor), json!({"client_id":client["id"], "episode_id":episode["id"], "clinician_id":actor, "kind":"individual", "starts_at":future, "ends_at":future+chrono::Duration::hours(1), "status":"completed"})).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "Future care cannot be recorded as completed"
    );
    // Feb 2 in UTC is still Feb 1 at this facility. Coverage is intentionally only one day.
    let session = create(pool, actor, "sessions", json!({"client_id":client["id"], "episode_id":episode["id"], "clinician_id":actor, "kind":"individual", "starts_at":"2026-02-02T01:00:00Z", "ends_at":"2026-02-02T02:00:00Z", "status":"completed"})).await;
    let note = create(pool, actor, "notes", json!({"client_id":client["id"], "session_id":session["id"], "title":"Timezone QA", "body":"Synthetic boundary verification only."})).await;
    let (status, _) = request(
        pool,
        Method::POST,
        &format!("/api/notes/{}/sign", note["id"].as_str().unwrap()),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let policy = create(pool, actor, "policies", json!({"client_id":client["id"], "payer_name":"QA", "member_id":"QA", "effective_date":"2026-02-01", "termination_date":"2026-02-01"})).await;
    let auth = create(pool, actor, "authorizations", json!({"client_id":client["id"], "policy_id":policy["id"], "authorization_number":"QA-PACIFIC", "service_code":"QA", "units_approved":1, "starts_on":"2026-02-01", "ends_on":"2026-02-01"})).await;
    let claim = create(pool, actor, "claims", json!({"client_id":client["id"], "policy_id":policy["id"], "authorization_id":auth["id"], "session_id":session["id"], "service_code":"QA", "diagnosis_code":"QA", "amount_cents":100})).await;
    let pacific_id = claim["id"].as_str().unwrap();
    let (status, readiness) = request(
        pool,
        Method::GET,
        &format!("/api/claims/{pacific_id}/readiness"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{readiness}");
    assert_eq!(readiness["ready"], true, "{readiness}");
    assert_eq!(
        readiness["checklist"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == "policy_coverage")
            .unwrap()["passed"],
        true
    );
    assert_eq!(
        readiness["checklist"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == "authorization_coverage")
            .unwrap()["passed"],
        true
    );
    let (status, response) = request(
        pool,
        Method::POST,
        &format!("/api/claims/{pacific_id}/ready"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Facility date must determine both policy and authorization coverage: {response}"
    );
    let late_session = create(pool, actor, "sessions", json!({"client_id":client["id"], "episode_id":episode["id"], "clinician_id":actor, "kind":"individual", "starts_at":"2026-02-02T03:00:00Z", "ends_at":"2026-02-02T04:00:00Z", "status":"completed"})).await;
    let late_note = create(pool, actor, "notes", json!({"client_id":client["id"], "session_id":late_session["id"], "title":"Late coverage QA", "body":"Synthetic boundary verification only."})).await;
    let (status, _) = request(
        pool,
        Method::POST,
        &format!("/api/notes/{}/sign", late_note["id"].as_str().unwrap()),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let late_policy = create(pool, actor, "policies", json!({"client_id":client["id"], "payer_name":"QA late", "member_id":"QA-LATE", "effective_date":"2026-02-02"})).await;
    let late_claim = create(pool, actor, "claims", json!({"client_id":client["id"], "policy_id":late_policy["id"], "session_id":late_session["id"], "service_code":"QA", "diagnosis_code":"QA", "amount_cents":100})).await;
    let late_id = late_claim["id"].as_str().unwrap();
    let (status, failed_policy) = request(
        pool,
        Method::GET,
        &format!("/api/claims/{late_id}/readiness"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{failed_policy}");
    assert_eq!(failed_policy["ready"], false);
    let policy_rule = failed_policy["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "policy_coverage")
        .unwrap();
    assert_eq!(policy_rule["passed"], false);
    assert_eq!(policy_rule["owner_role"], "billing");
    let (status, denied_policy) = request(
        pool,
        Method::POST,
        &format!("/api/claims/{late_id}/ready"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{denied_policy}");
    assert!(denied_policy["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("policy"));
    let reserved_session = create(pool, actor, "sessions", json!({"client_id":client["id"], "episode_id":episode["id"], "clinician_id":actor, "kind":"individual", "starts_at":"2026-02-02T05:00:00Z", "ends_at":"2026-02-02T06:00:00Z", "status":"completed"})).await;
    let reserved_note = create(pool, actor, "notes", json!({"client_id":client["id"], "session_id":reserved_session["id"], "title":"Reserved units QA", "body":"Synthetic boundary verification only."})).await;
    let (status, _) = request(
        pool,
        Method::POST,
        &format!("/api/notes/{}/sign", reserved_note["id"].as_str().unwrap()),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let over_claim = create(pool, actor, "claims", json!({"client_id":client["id"], "policy_id":policy["id"], "authorization_id":auth["id"], "session_id":reserved_session["id"], "service_code":"QA", "diagnosis_code":"QA", "amount_cents":100})).await;
    let over_id = over_claim["id"].as_str().unwrap();
    let (status, failed_units) = request(
        pool,
        Method::GET,
        &format!("/api/claims/{over_id}/readiness"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{failed_units}");
    assert_eq!(failed_units["ready"], false);
    let units_rule = failed_units["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "authorization_units")
        .unwrap();
    assert_eq!(units_rule["passed"], false, "{units_rule}");
    assert_eq!(units_rule["owner_role"], "billing");
    assert_eq!(
        failed_units["checklist"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == "authorization_coverage")
            .unwrap()["passed"],
        true
    );
    let (status, denied_units) = request(
        pool,
        Method::POST,
        &format!("/api/claims/{over_id}/ready"),
        Some(actor),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{denied_units}");
    assert!(denied_units["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("authorized units"));
}
