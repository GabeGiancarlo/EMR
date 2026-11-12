//! Intake cases and identity attestations against an isolated PostgreSQL schema.
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
) -> (StatusCode, Value, Option<String>) {
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
    if let Some(actor) = actor {
        req = req.insert_header(("x-nexus-actor", actor));
    }
    let res = test::call_service(&app, req.to_request()).await;
    let status = res.status();
    let cache = res
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = test::read_body(res).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cache,
    )
}
async fn post(pool: &PgPool, actor: &str, path: &str, body: Value) -> Value {
    let (status, value, _) = send(pool, Some(actor), Method::POST, path, body).await;
    assert!(status.is_success(), "{path} {status}: {value}");
    value
}
fn codes(case: &Value) -> Vec<String> {
    case["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["required"] == true && item["passed"] != true)
        .map(|item| item["code"].as_str().unwrap().to_owned())
        .collect()
}
fn admission_codes(case: &Value) -> Vec<String> {
    case["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            item["code"]
                .as_str()
                .unwrap_or_default()
                .starts_with("admission_")
                && item["passed"] != true
        })
        .map(|item| item["code"].as_str().unwrap().to_owned())
        .collect()
}

#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn intake_identity_checklist_admission_and_scope_invariants() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let schema = format!("nexus_admissions_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let search = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |conn, _| {
            let search = search.clone();
            Box::pin(async move {
                sqlx::query(&search).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let (status, setup, _) = send(&pool, None, Method::POST, "/api/setup", json!({"organization_name":"Synthetic intake QA","facility_name":"Synthetic facility","admin_name":"Synthetic admin","admin_email":"intake-admin@example.invalid"})).await;
    assert!(status.is_success(), "{setup}");
    let admin = setup["actor"]["id"].as_str().unwrap().to_owned();
    let facility = setup["facility"]["id"].as_str().unwrap().to_owned();
    let org: Uuid = sqlx::query_scalar("SELECT organization_id FROM staff WHERE id=$1")
        .bind(Uuid::parse_str(&admin).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let today: chrono::NaiveDate =
        sqlx::query_scalar("SELECT (CURRENT_TIMESTAMP AT TIME ZONE 'America/New_York')::date")
            .fetch_one(&pool)
            .await
            .unwrap();
    let support = post(
        &pool,
        &admin,
        "/api/staff",
        json!({"name":"Synthetic support","email":"intake-support@example.invalid","role":"support"}),
    )
    .await;
    let clinician = post(
        &pool,
        &admin,
        "/api/staff",
        json!({"name":"Synthetic clinician","email":"intake-clinician@example.invalid","role":"clinician"}),
    )
    .await;
    let other = post(
        &pool,
        &admin,
        "/api/staff",
        json!({"name":"Synthetic other clinician","email":"intake-other@example.invalid","role":"clinician"}),
    )
    .await;
    let billing = post(
        &pool,
        &admin,
        "/api/staff",
        json!({"name":"Synthetic billing","email":"intake-billing@example.invalid","role":"billing"}),
    )
    .await;
    let executive = post(
        &pool,
        &admin,
        "/api/staff",
        json!({"name":"Synthetic executive","email":"intake-exec@example.invalid","role":"executive"}),
    )
    .await;
    let client = post(
        &pool,
        &admin,
        "/api/clients",
        json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Intake","birth_date":"1990-01-01","primary_clinician_id":clinician["id"],"email":"patient@example.invalid","phone":"2125550100"}),
    )
    .await;
    let cid = client["id"].as_str().unwrap();
    let mrn = client["mrn"].as_str().unwrap().to_owned();
    assert!(mrn.starts_with("NX"));
    let (denied, _, _) = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/clients/{cid}"),
        json!({"mrn":"NX0000000099","_expected_updated_at":client["updated_at"]}),
    )
    .await;
    assert_eq!(denied, StatusCode::BAD_REQUEST);
    assert!(
        sqlx::query("UPDATE clients SET mrn='NX0000000099' WHERE id=$1")
            .bind(Uuid::parse_str(cid).unwrap())
            .execute(&pool)
            .await
            .is_err()
    );
    let active = support::legacy_episode(
        &pool,
        &admin,
        json!({"facility_id":facility,"client_id":cid,"level_of_care":"outpatient","start_date":"2024-01-01","status":"active"}),
    )
    .await;
    let program_php=post(&pool,&admin,"/api/care-programs",json!({"facility_id":facility,"code":"PHP","name":"Synthetic PHP","level_of_care":"php","min_age":18,"max_age":null,"capacity":100,"active":true,"required_intake_template_ids":[],"required_discharge_template_ids":[],"change_reason":"Configure synthetic intake program"})).await;
    let program_iop=post(&pool,&admin,"/api/care-programs",json!({"facility_id":facility,"code":"IOP","name":"Synthetic IOP","level_of_care":"iop","min_age":18,"max_age":null,"capacity":100,"active":true,"required_intake_template_ids":[],"required_discharge_template_ids":[],"change_reason":"Configure synthetic intake program"})).await;
    let program_outpatient=post(&pool,&admin,"/api/care-programs",json!({"facility_id":facility,"code":"OP","name":"Synthetic OP","level_of_care":"outpatient","min_age":18,"max_age":null,"capacity":100,"active":true,"required_intake_template_ids":[],"required_discharge_template_ids":[],"change_reason":"Configure synthetic intake program"})).await;
    let body = json!({"program_id":program_php["id"],"facility_id":facility,"level_of_care":"php","starts_on":today,"received_on":today,"owner_id":support["id"],"referral_type":"provider","referrer_name":"Synthetic referrer","referrer_contact":"555-0101","referral_reason":"Synthetic referral token qx7"});
    assert_eq!(
        send(
            &pool,
            Some(billing["id"].as_str().unwrap()),
            Method::POST,
            &format!("/api/clients/{cid}/intake"),
            body.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &pool,
            Some(other["id"].as_str().unwrap()),
            Method::POST,
            &format!("/api/clients/{cid}/intake"),
            body.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let created = post(
        &pool,
        clinician["id"].as_str().unwrap(),
        &format!("/api/clients/{cid}/intake"),
        body,
    )
    .await;
    assert_eq!(created["status"], "open");
    assert_eq!(created["events"], json!([]));
    assert!(created["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["code"] == "coverage_information"
            && item["required"] == false
            && item["detail"]
                .as_str()
                .unwrap()
                .contains("not electronically verified")));
    let case_id = created["id"].as_str().unwrap();
    let episode_id = created["episode_id"].as_str().unwrap();
    let outside_owner = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/intake-cases/{case_id}"),
        json!({"owner_id":other["id"],"_expected_updated_at":created["updated_at"]}),
    )
    .await;
    assert_eq!(
        outside_owner.0,
        StatusCode::BAD_REQUEST,
        "{}",
        outside_owner.1
    );
    let (list_status, list, cache) = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await;
    assert_eq!(list_status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(list["cases"][0]["id"], created["id"]);
    let still: String = sqlx::query_scalar("SELECT status FROM clients WHERE id=$1")
        .bind(Uuid::parse_str(cid).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(still, "active");
    let ongoing: String = sqlx::query_scalar("SELECT status FROM episodes WHERE id=$1")
        .bind(Uuid::parse_str(active["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ongoing, "active");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT mrn FROM clients WHERE id=$1")
            .bind(Uuid::parse_str(cid).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        mrn
    );
    assert_eq!(
        send(
            &pool,
            Some(billing["id"].as_str().unwrap()),
            Method::GET,
            &format!("/api/clients/{cid}/intake"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &pool,
            Some(executive["id"].as_str().unwrap()),
            Method::GET,
            &format!("/api/clients/{cid}/intake"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let hidden = send(
        &pool,
        Some(other["id"].as_str().unwrap()),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(!hidden.to_string().contains("qx7"));
    let (pre, _, _) = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/intake-cases/{case_id}"),
        json!({"referrer_name":"Renamed referrer"}),
    )
    .await;
    assert_eq!(pre, StatusCode::PRECONDITION_REQUIRED);
    let stale = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/intake-cases/{case_id}"),
        json!({"referrer_name":"Renamed referrer","_expected_updated_at":"2000-01-01T00:00:00Z"}),
    )
    .await;
    assert_eq!(stale.0, StatusCode::CONFLICT);
    assert!(!stale.1.to_string().contains("qx7"));
    let edited = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/intake-cases/{case_id}"),
        json!({"referrer_name":"Renamed referrer","_expected_updated_at":created["updated_at"]}),
    )
    .await
    .1;
    assert_eq!(edited["referrer_name"], "Renamed referrer");
    assert_eq!(edited["episode_id"], created["episode_id"]);
    let short = send(&pool, Some(&admin), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"screening","reason":"too short","attested":true,"_expected_updated_at":edited["updated_at"]})).await;
    assert_eq!(short.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM intake_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let screening = send(&pool, Some(support["id"].as_str().unwrap()), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"screening","reason":"Screening started for synthetic intake","attested":true,"_expected_updated_at":edited["updated_at"]})).await.1;
    assert_eq!(screening["status"], "screening");
    assert_eq!(screening["events"].as_array().unwrap().len(), 1);
    let skip = send(&pool, Some(&admin), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"admitted","reason":"Skipping the ready state is not allowed","attested":true,"_expected_updated_at":screening["updated_at"]})).await;
    assert_eq!(skip.0, StatusCode::CONFLICT);
    assert!(codes(&screening).contains(&"identity_confirmation".to_string()));
    let attested = post(
        &pool,
        &admin,
        &format!("/api/clients/{cid}/identity-checks"),
        json!({"method":"document_review","outcome":"confirmed","reason":"Synthetic attestation token qx7","attested":true}),
    )
    .await;
    assert_eq!(attested["current"], true);
    let renamed = send(&pool, Some(&admin), Method::PATCH, &format!("/api/clients/{cid}"), json!({"first_name":"Changed","_expected_updated_at":sqlx::query_scalar::<_, Value>("SELECT to_jsonb(c) FROM clients c WHERE id=$1").bind(Uuid::parse_str(cid).unwrap()).fetch_one(&pool).await.unwrap()["updated_at"]})).await.1;
    let after_change = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert_eq!(after_change["identity_checks"][0]["current"], false);
    send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/clients/{cid}"),
        json!({"first_name":"Synthetic","_expected_updated_at":renamed["updated_at"]}),
    )
    .await;
    let restored = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert_eq!(restored["identity_checks"][0]["current"], true);
    post(
        &pool,
        &admin,
        &format!("/api/clients/{cid}/identity-checks"),
        json!({"method":"patient_report","outcome":"unable","reason":"Unable to confirm current identity today","attested":true}),
    )
    .await;
    let unable = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(unable["identity_checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["current"] == false));
    post(
        &pool,
        &admin,
        &format!("/api/clients/{cid}/identity-checks"),
        json!({"method":"prior_record","outcome":"confirmed","reason":"Confirmed again against current identifiers","attested":true}),
    )
    .await;
    sqlx::query("UPDATE clients SET birth_date='2015-01-01' WHERE id=$1")
        .bind(Uuid::parse_str(cid).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let minor = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(codes(&minor["cases"][0])
        .iter()
        .any(|code| code == "adult_eligibility"));
    sqlx::query("UPDATE clients SET birth_date='1990-01-01' WHERE id=$1")
        .bind(Uuid::parse_str(cid).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    post(
        &pool,
        &admin,
        &format!("/api/clients/{cid}/identity-checks"),
        json!({"method":"prior_record","outcome":"confirmed","reason":"Confirmed after restoring the birth date","attested":true}),
    )
    .await;
    let facility_row: Value =
        sqlx::query_scalar("SELECT to_jsonb(f) FROM facilities f WHERE id=$1")
            .bind(Uuid::parse_str(&facility).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/facilities/{facility}"),
        json!({"active":false,"_expected_updated_at":facility_row["updated_at"]}),
    )
    .await;
    let inactive = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(codes(&inactive["cases"][0]).contains(&"active_facility".into()));
    let facility_row: Value =
        sqlx::query_scalar("SELECT to_jsonb(f) FROM facilities f WHERE id=$1")
            .bind(Uuid::parse_str(&facility).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/facilities/{facility}"),
        json!({"active":true,"_expected_updated_at":facility_row["updated_at"]}),
    )
    .await;
    let client_row: Value = sqlx::query_scalar("SELECT to_jsonb(c) FROM clients c WHERE id=$1")
        .bind(Uuid::parse_str(cid).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/clients/{cid}"),
        json!({"primary_clinician_id":null,"_expected_updated_at":client_row["updated_at"]}),
    )
    .await;
    let unassigned = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(codes(&unassigned["cases"][0]).contains(&"primary_clinician".into()));
    let client_row: Value = sqlx::query_scalar("SELECT to_jsonb(c) FROM clients c WHERE id=$1")
        .bind(Uuid::parse_str(cid).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    send(&pool, Some(&admin), Method::PATCH, &format!("/api/clients/{cid}"), json!({"primary_clinician_id":clinician["id"],"_expected_updated_at":client_row["updated_at"]})).await;
    let no_consent = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(codes(&no_consent["cases"][0]).contains(&"treatment_consent".into()));
    post(
        &pool,
        &admin,
        "/api/consents",
        json!({"client_id":cid,"kind":"treatment","scope":"Synthetic treatment consent","granted_on":"2020-01-01"}),
    )
    .await;
    let template = post(
        &pool,
        &admin,
        "/api/templates",
        json!({"name":"Synthetic intake packet","kind":"assessment","applies_to_stage":"intake","applies_to_care_level":"php","required_for_stage":true,"fields":[{"key":"ack","label":"Ack","type":"checkbox","required":true}]}),
    )
    .await;
    let other_level = post(
        &pool,
        &admin,
        "/api/templates",
        json!({"name":"Synthetic other level","kind":"intake","applies_to_stage":"intake","applies_to_care_level":"iop","required_for_stage":true,"fields":[{"key":"ack","label":"Ack","type":"checkbox","required":true}]}),
    )
    .await;
    let stage_other = post(
        &pool,
        &admin,
        "/api/templates",
        json!({"name":"Synthetic active stage","kind":"intake","applies_to_stage":"active","applies_to_care_level":"any","required_for_stage":true,"fields":[{"key":"ack","label":"Ack","type":"checkbox","required":true}]}),
    )
    .await;
    let packet = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    let form_code = format!("intake_form:{}", template["id"].as_str().unwrap());
    assert!(codes(&packet["cases"][0]).contains(&form_code));
    assert!(!codes(&packet["cases"][0])
        .iter()
        .any(|code| code.contains(other_level["id"].as_str().unwrap())
            || code.contains(stage_other["id"].as_str().unwrap())));
    let wrong = post(
        &pool,
        &admin,
        "/api/form_responses",
        json!({"client_id":cid,"episode_id":active["id"],"template_id":template["id"],"answers":{"ack":true},"field_provenance":{},"source_type":"manual","source_excerpt":"Synthetic form source"}),
    )
    .await;
    post(
        &pool,
        &admin,
        &format!(
            "/api/form_responses/{}/review",
            wrong["id"].as_str().unwrap()
        ),
        json!({}),
    )
    .await;
    post(
        &pool,
        &admin,
        &format!("/api/form_responses/{}/sign", wrong["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    let still_missing = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(codes(&still_missing["cases"][0]).contains(&form_code));
    let signed = post(
        &pool,
        &admin,
        "/api/form_responses",
        json!({"client_id":cid,"episode_id":episode_id,"template_id":template["id"],"answers":{"ack":true},"field_provenance":{},"source_type":"manual","source_excerpt":"Synthetic form source"}),
    )
    .await;
    post(
        &pool,
        &admin,
        &format!(
            "/api/form_responses/{}/review",
            signed["id"].as_str().unwrap()
        ),
        json!({}),
    )
    .await;
    post(
        &pool,
        &admin,
        &format!(
            "/api/form_responses/{}/sign",
            signed["id"].as_str().unwrap()
        ),
        json!({}),
    )
    .await;
    let ready_case = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(codes(&ready_case["cases"][0]).is_empty(), "{}", ready_case);
    for item in ready_case["cases"][0]["checklist"].as_array().unwrap() {
        let detail = item["detail"].as_str().unwrap().to_lowercase();
        assert!(
            !detail.contains("legally required") && !detail.contains("clinically necessary"),
            "{detail}"
        );
    }
    let ready = send(&pool, Some(support["id"].as_str().unwrap()), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"ready","reason":"Organization checklist items are complete","attested":true,"_expected_updated_at":ready_case["cases"][0]["updated_at"]})).await;
    assert_eq!(ready.0, StatusCode::OK, "{}", ready.1);
    assert_eq!(ready.1["events"].as_array().unwrap().len(), 2);
    assert_eq!(send(&pool, Some(support["id"].as_str().unwrap()), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"admitted","reason":"Support cannot admit this planned episode","attested":true,"_expected_updated_at":ready.1["updated_at"]})).await.0, StatusCode::FORBIDDEN);
    let planned: String = sqlx::query_scalar("SELECT status FROM episodes WHERE id=$1")
        .bind(Uuid::parse_str(episode_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(planned, "planned");
    let overlap = support::legacy_episode(
        &pool,
        &admin,
        json!({"facility_id":facility,"client_id":cid,"level_of_care":"php","start_date":"2024-02-01","status":"active"}),
    )
    .await;
    let blocked = send(&pool, Some(clinician["id"].as_str().unwrap()), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"admitted","reason":"Admission should wait for the other episode","attested":true,"_expected_updated_at":ready.1["updated_at"]})).await;
    assert_eq!(blocked.0, StatusCode::CONFLICT, "{}", blocked.1);
    assert!(blocked.1["error"]["message"]
        .as_str()
        .unwrap()
        .contains("admission_no_overlapping_active_episode"));
    assert!(!blocked.1.to_string().contains("qx7"));
    sqlx::query("UPDATE episodes SET status='discharged', end_date='2024-03-01', discharge_reason='Synthetic discharge of overlapping care' WHERE id=$1").bind(Uuid::parse_str(overlap["id"].as_str().unwrap()).unwrap()).execute(&pool).await.unwrap();
    let fresh = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{cid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert!(admission_codes(&fresh["cases"][0]).is_empty(), "{}", fresh);
    let admitted = send(&pool, Some(clinician["id"].as_str().unwrap()), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"admitted","reason":"Admit the planned episode after checklist review","attested":true,"_expected_updated_at":fresh["cases"][0]["updated_at"]})).await;
    assert_eq!(admitted.0, StatusCode::OK, "{}", admitted.1);
    assert_eq!(admitted.1["status"], "admitted");
    assert_eq!(admitted.1["events"].as_array().unwrap().len(), 3);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM episodes WHERE id=$1")
            .bind(Uuid::parse_str(episode_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "active"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM episodes WHERE id=$1")
            .bind(Uuid::parse_str(active["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "active"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM clients WHERE id=$1")
            .bind(Uuid::parse_str(cid).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "active"
    );
    let episode_row: Value = sqlx::query_scalar("SELECT to_jsonb(e) FROM episodes e WHERE id=$1")
        .bind(Uuid::parse_str(episode_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let bypass = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/episodes/{episode_id}"),
        json!({"level_of_care":"iop","_expected_updated_at":episode_row["updated_at"]}),
    )
    .await;
    assert_eq!(bypass.0, StatusCode::CONFLICT, "{}", bypass.1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT level_of_care FROM episodes WHERE id=$1")
            .bind(Uuid::parse_str(episode_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "php"
    );
    assert_eq!(send(&pool, Some(&admin), Method::POST, &format!("/api/intake-cases/{case_id}/transition"), json!({"to_status":"withdrawn","reason":"Terminal cases cannot transition again","attested":true,"_expected_updated_at":admitted.1["updated_at"]})).await.0, StatusCode::CONFLICT);
    let audits = sqlx::query_scalar::<_, Value>(
        "SELECT COALESCE(jsonb_agg(to_jsonb(a)), '[]'::jsonb) FROM audit_events a",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!audits.to_string().contains("qx7"));
    assert!(
        sqlx::query("UPDATE intake_events SET reason='rewritten synthetic reason'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM intake_cases")
        .execute(&pool)
        .await
        .is_err());
    let doc_cols: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name IN ('intake_cases','patient_identity_checks') AND column_name ILIKE '%document%'").fetch_one(&pool).await.unwrap();
    assert_eq!(doc_cols, 0);

    let second = post(
        &pool,
        &admin,
        "/api/clients",
        json!({"facility_id":facility,"first_name":"Historical","last_name":"Care","birth_date":"1988-05-05","primary_clinician_id":admin}),
    )
    .await;
    let sid = second["id"].as_str().unwrap();
    let future = today.succ_opt().unwrap();
    let future_case = post(
        &pool,
        &admin,
        &format!("/api/clients/{sid}/intake"),
        json!({"program_id":program_iop["id"],"facility_id":facility,"level_of_care":"iop","starts_on":future,"received_on":today,"owner_id":admin,"referral_type":"self","referrer_name":"","referrer_contact":"","referral_reason":"Future start should not admit"}),
    )
    .await;
    post(
        &pool,
        &admin,
        &format!("/api/clients/{sid}/identity-checks"),
        json!({"method":"patient_report","outcome":"confirmed","reason":"Confirmed historical care patient identity","attested":true}),
    )
    .await;
    post(
        &pool,
        &admin,
        "/api/consents",
        json!({"client_id":sid,"kind":"treatment","scope":"Synthetic treatment consent","granted_on":"2020-01-01"}),
    )
    .await;
    let future_template = post(
        &pool,
        &admin,
        "/api/templates",
        json!({"name":"Synthetic future packet","kind":"intake","applies_to_stage":"any","applies_to_care_level":"any","required_for_stage":true,"fields":[{"key":"ack","label":"Ack","type":"checkbox","required":true}]}),
    )
    .await;
    for required_template in [&future_template, &other_level] {
        let response = post(
        &pool,
        &admin,
        "/api/form_responses",
        json!({"client_id":sid,"episode_id":future_case["episode_id"],"template_id":required_template["id"],"answers":{"ack":true},"field_provenance":{},"source_type":"manual","source_excerpt":"Synthetic form source"}),
    )
    .await;
        post(
            &pool,
            &admin,
            &format!(
                "/api/form_responses/{}/review",
                response["id"].as_str().unwrap()
            ),
            json!({}),
        )
        .await;
        post(
            &pool,
            &admin,
            &format!(
                "/api/form_responses/{}/sign",
                response["id"].as_str().unwrap()
            ),
            json!({}),
        )
        .await;
    }
    let screened = send(&pool, Some(&admin), Method::POST, &format!("/api/intake-cases/{}/transition", future_case["id"].as_str().unwrap()), json!({"to_status":"screening","reason":"Screen the future start case","attested":true,"_expected_updated_at":future_case["updated_at"]})).await.1;
    let future_ready = send(&pool, Some(&admin), Method::POST, &format!("/api/intake-cases/{}/transition", future_case["id"].as_str().unwrap()), json!({"to_status":"ready","reason":"Ready does not require today's start","attested":true,"_expected_updated_at":screened["updated_at"]})).await;
    assert_eq!(future_ready.0, StatusCode::OK, "{}", future_ready.1);
    let not_today = send(&pool, Some(&admin), Method::POST, &format!("/api/intake-cases/{}/transition", future_case["id"].as_str().unwrap()), json!({"to_status":"admitted","reason":"Future start must not activate the episode","attested":true,"_expected_updated_at":future_ready.1["updated_at"]})).await;
    assert_eq!(not_today.0, StatusCode::CONFLICT, "{}", not_today.1);
    let declined = send(&pool, Some(&admin), Method::POST, &format!("/api/intake-cases/{}/transition", future_case["id"].as_str().unwrap()), json!({"to_status":"declined","reason":"Decline and preserve the planned episode","attested":true,"_expected_updated_at":future_ready.1["updated_at"]})).await.1;
    assert_eq!(declined["status"], "declined");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM clients WHERE id=$1")
            .bind(Uuid::parse_str(sid).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "inquiry"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM episodes WHERE id=$1")
            .bind(Uuid::parse_str(future_case["episode_id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "planned"
    );
    let linked: Value = sqlx::query_scalar("SELECT to_jsonb(e) FROM episodes e WHERE id=$1")
        .bind(Uuid::parse_str(future_case["episode_id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let activate = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!(
            "/api/episodes/{}",
            future_case["episode_id"].as_str().unwrap()
        ),
        json!({"status":"active","_expected_updated_at":linked["updated_at"]}),
    )
    .await;
    assert_eq!(activate.0, StatusCode::CONFLICT, "{}", activate.1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM episodes WHERE id=$1")
            .bind(Uuid::parse_str(future_case["episode_id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        "planned"
    );
    let replacement = post(
        &pool,
        &admin,
        &format!("/api/clients/{sid}/intake"),
        json!({"program_id":program_outpatient["id"],"facility_id":facility,"level_of_care":"outpatient","starts_on":today,"received_on":today,"owner_id":admin,"referral_type":"family","referrer_name":"Synthetic family","referrer_contact":"","referral_reason":"A later case remains available after decline"}),
    )
    .await;
    assert_eq!(replacement["status"], "open");
    let patient_now: Value = sqlx::query_scalar("SELECT to_jsonb(c) FROM clients c WHERE id=$1")
        .bind(Uuid::parse_str(sid).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let stage_bypass = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/clients/{sid}"),
        json!({"status":"active","_expected_updated_at":patient_now["updated_at"]}),
    )
    .await;
    assert_eq!(stage_bypass.0, StatusCode::CONFLICT);
    let planned_discharge = send(
        &pool,
        Some(&admin),
        Method::POST,
        &format!(
            "/api/episodes/{}/discharge",
            replacement["episode_id"].as_str().unwrap()
        ),
        json!({"end_date":today,"discharge_reason":"Cannot discharge a pending intake"}),
    )
    .await;
    assert_eq!(
        planned_discharge.0,
        StatusCode::CONFLICT,
        "{}",
        planned_discharge.1
    );
    let assigned = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/intake-cases/{}", replacement["id"].as_str().unwrap()),
        json!({"owner_id":support["id"],"_expected_updated_at":replacement["updated_at"]}),
    )
    .await;
    assert_eq!(assigned.0, StatusCode::OK, "{}", assigned.1);
    let clinical_assignment = send(&pool, Some(&admin), Method::PATCH, &format!("/api/clients/{sid}"), json!({"primary_clinician_id":clinician["id"],"_expected_updated_at":patient_now["updated_at"]})).await;
    assert_eq!(
        clinical_assignment.0,
        StatusCode::OK,
        "{}",
        clinical_assignment.1
    );
    let clinical_owner = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/intake-cases/{}", replacement["id"].as_str().unwrap()),
        json!({"owner_id":clinician["id"],"_expected_updated_at":assigned.1["updated_at"]}),
    )
    .await;
    assert_eq!(clinical_owner.0, StatusCode::OK, "{}", clinical_owner.1);
    let panel_loss = send(&pool, Some(&admin), Method::PATCH, &format!("/api/clients/{sid}"), json!({"primary_clinician_id":admin,"_expected_updated_at":clinical_assignment.1["updated_at"]})).await;
    assert_eq!(panel_loss.0, StatusCode::CONFLICT, "{}", panel_loss.1);
    let restore_owner = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/intake-cases/{}", replacement["id"].as_str().unwrap()),
        json!({"owner_id":support["id"],"_expected_updated_at":clinical_owner.1["updated_at"]}),
    )
    .await;
    assert_eq!(restore_owner.0, StatusCode::OK, "{}", restore_owner.1);
    let restore_assignment = send(&pool, Some(&admin), Method::PATCH, &format!("/api/clients/{sid}"), json!({"primary_clinician_id":admin,"_expected_updated_at":clinical_assignment.1["updated_at"]})).await;
    assert_eq!(
        restore_assignment.0,
        StatusCode::OK,
        "{}",
        restore_assignment.1
    );
    let role_change = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/staff/{}", support["id"].as_str().unwrap()),
        json!({"role":"billing","_expected_updated_at":support["updated_at"]}),
    )
    .await;
    assert_eq!(role_change.0, StatusCode::CONFLICT, "{}", role_change.1);
    let panel_role_change = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/staff/{}", support["id"].as_str().unwrap()),
        json!({"role":"clinician","_expected_updated_at":support["updated_at"]}),
    )
    .await;
    assert_eq!(
        panel_role_change.0,
        StatusCode::CONFLICT,
        "{}",
        panel_role_change.1
    );
    let panel_transfer = send(
        &pool,
        Some(&admin),
        Method::POST,
        &format!("/api/staff/{}/offboard", support["id"].as_str().unwrap()),
        json!({"transfer_to":other["id"]}),
    )
    .await;
    assert_eq!(
        panel_transfer.0,
        StatusCode::BAD_REQUEST,
        "{}",
        panel_transfer.1
    );
    let denied_transfer = send(
        &pool,
        Some(&admin),
        Method::POST,
        &format!("/api/staff/{}/offboard", support["id"].as_str().unwrap()),
        json!({"transfer_to":billing["id"]}),
    )
    .await;
    assert_eq!(
        denied_transfer.0,
        StatusCode::BAD_REQUEST,
        "{}",
        denied_transfer.1
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT active FROM staff WHERE id=$1")
            .bind(Uuid::parse_str(support["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    let transferred = send(
        &pool,
        Some(&admin),
        Method::POST,
        &format!("/api/staff/{}/offboard", support["id"].as_str().unwrap()),
        json!({"transfer_to":admin}),
    )
    .await;
    assert_eq!(transferred.0, StatusCode::OK, "{}", transferred.1);
    let transferred_case: Value =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM intake_cases c WHERE id=$1")
            .bind(Uuid::parse_str(replacement["id"].as_str().unwrap()).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(transferred_case["owner_id"], admin);
    assert_ne!(transferred_case["updated_at"], assigned.1["updated_at"]);
    let transfer_event_count: i64 = sqlx::query_scalar("SELECT count(*) FROM intake_events WHERE intake_case_id=$1 AND from_status='open' AND to_status='open'").bind(Uuid::parse_str(replacement["id"].as_str().unwrap()).unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(transfer_event_count, 1);
    let history = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!("/api/clients/{sid}/intake"),
        Value::Null,
    )
    .await
    .1;
    assert_eq!(history["cases"].as_array().unwrap().len(), 2);
    assert_eq!(history["cases"][0]["id"], replacement["id"]);
    assert_eq!(history["cases"][1]["status"], "declined");

    let limit_client = post(
        &pool,
        &admin,
        "/api/clients",
        json!({"facility_id":facility,"first_name":"Limit","last_name":"History","birth_date":"1981-01-01","primary_clinician_id":admin}),
    )
    .await;
    sqlx::query("WITH eps AS (INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date,status) SELECT gen_random_uuid(),$1,$2,$3,'iop',DATE '2024-01-01','planned' FROM generate_series(1,101) RETURNING id) INSERT INTO intake_cases(id,organization_id,client_id,episode_id,owner_id,referral_type,referrer_name,referrer_contact,referral_reason,received_on,status,disposition_reason) SELECT gen_random_uuid(),$1,$2,id,$4,'other','','','Historical limit case',DATE '2024-01-01','withdrawn','Historical limit disposition reason' FROM eps").bind(org).bind(Uuid::parse_str(limit_client["id"].as_str().unwrap()).unwrap()).bind(Uuid::parse_str(&facility).unwrap()).bind(Uuid::parse_str(&admin).unwrap()).execute(&pool).await.unwrap();
    let overflow = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!(
            "/api/clients/{}/intake",
            limit_client["id"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert_eq!(overflow.0, StatusCode::CONFLICT, "{}", overflow.1);
    assert!(overflow.1["error"]["message"]
        .as_str()
        .unwrap()
        .contains("100 cases"));
    let check_client = post(
        &pool,
        &admin,
        "/api/clients",
        json!({"facility_id":facility,"first_name":"Limit","last_name":"Checks","birth_date":"1982-02-02","primary_clinician_id":admin}),
    )
    .await;
    sqlx::query("INSERT INTO patient_identity_checks(id,organization_id,client_id,actor_id,method,outcome,reason,attested,identity_fingerprint) SELECT gen_random_uuid(),$1,$2,$3,'patient_report','unable','Historical identity attestation limit',true,repeat('a',64) FROM generate_series(1,101)").bind(org).bind(Uuid::parse_str(check_client["id"].as_str().unwrap()).unwrap()).bind(Uuid::parse_str(&admin).unwrap()).execute(&pool).await.unwrap();
    let checks = send(
        &pool,
        Some(&admin),
        Method::GET,
        &format!(
            "/api/clients/{}/intake",
            check_client["id"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert_eq!(checks.0, StatusCode::CONFLICT);
    assert!(checks.1["error"]["message"]
        .as_str()
        .unwrap()
        .contains("100 checks"));
    let other_org = Uuid::new_v4();
    let other_facility = Uuid::new_v4();
    let other_client = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Other synthetic org')")
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
    sqlx::query("INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date) VALUES($1,$2,$3,'Hidden','Patient','1991-01-01')").bind(other_client).bind(other_org).bind(other_facility).execute(&pool).await.unwrap();
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::GET,
            &format!("/api/clients/{other_client}/intake"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let unlinked = support::legacy_episode(
        &pool,
        &admin,
        json!({"facility_id":facility,"client_id":sid,"level_of_care":"outpatient","start_date":"2025-01-01","status":"planned"}),
    )
    .await;
    let unlinked_row: Value = sqlx::query_scalar("SELECT to_jsonb(e) FROM episodes e WHERE id=$1")
        .bind(Uuid::parse_str(unlinked["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let legacy = send(
        &pool,
        Some(&admin),
        Method::PATCH,
        &format!("/api/episodes/{}", unlinked["id"].as_str().unwrap()),
        json!({"status":"active","_expected_updated_at":unlinked_row["updated_at"]}),
    )
    .await;
    assert_eq!(legacy.0, StatusCode::CONFLICT, "{}", legacy.1);
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
