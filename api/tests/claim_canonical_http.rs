//! HTTP integration coverage for canonical claim identity.
//!
//! Unique schema inside disposable nexus_test only. No public or walkthrough reset.
//! Catalog rows are SQL fixtures. Claim writes, voids, authorization matching, and
//! readiness go through the real HTTP routes and are checked in persisted rows.
mod support;

use actix_web::{http::StatusCode, test, web, App};
use chrono::{Duration as ChronoDuration, Utc};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use uuid::Uuid;

struct Fixture {
    control: PgPool,
    pool: PgPool,
    schema: String,
    actor: String,
    org: Uuid,
    facility_id: String,
}

async fn send(
    pool: &PgPool,
    actor: Option<&str>,
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
    let mut builder = match method {
        "POST" => test::TestRequest::post(),
        "PATCH" => test::TestRequest::patch(),
        "GET" => test::TestRequest::get(),
        other => panic!("unsupported method {other}"),
    }
    .uri(path)
    .set_json(body);
    if let Some(actor) = actor.filter(|value| !value.is_empty()) {
        builder = builder.insert_header(("x-nexus-actor", actor));
    }
    let response = test::call_service(&app, builder.to_request()).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw": String::from_utf8_lossy(&bytes).into_owned()}));
    (status, value)
}

async fn create(pool: &PgPool, actor: &str, resource: &str, body: Value) -> Value {
    let (status, row) = send(pool, Some(actor), "POST", &format!("/api/{resource}"), body).await;
    assert_eq!(status, StatusCode::CREATED, "{resource}: {row}");
    row
}

async fn patch(
    pool: &PgPool,
    actor: &str,
    resource: &str,
    row: &Value,
    mut body: Value,
) -> (StatusCode, Value) {
    assert!(
        row["updated_at"].is_string(),
        "loaded updated_at missing: {row}"
    );
    assert!(
        body.get("_expected_updated_at").is_none(),
        "token must come from the loaded row"
    );
    body["_expected_updated_at"] = row["updated_at"].clone();
    send(
        pool,
        Some(actor),
        "PATCH",
        &format!("/api/{resource}/{}", row["id"].as_str().unwrap()),
        body,
    )
    .await
}

fn uid(value: &Value) -> Uuid {
    Uuid::parse_str(value["id"].as_str().unwrap()).unwrap()
}

fn cents(row: &Value) -> i64 {
    row["amount_cents"]
        .as_i64()
        .unwrap_or_else(|| panic!("amount_cents must be integer cents: {row}"))
}

fn error_message(body: &Value) -> String {
    body["error"]["message"].as_str().unwrap_or("").to_string()
}

fn assert_same_tenant(row: &Value, org: Uuid, client_id: &Value) {
    let org = org.to_string();
    assert_eq!(row["organization_id"].as_str(), Some(org.as_str()), "{row}");
    assert_eq!(&row["client_id"], client_id, "{row}");
}

fn assert_canonical_conflict(status: StatusCode, body: &Value) {
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "conflict", "{body}");
    let message = error_message(body).to_lowercase();
    assert!(
        !message.contains("reload"),
        "canonical collision must not be reported as a stale write: {body}"
    );
}

fn checklist<'a>(report: &'a Value, code: &str) -> &'a Value {
    report["checklist"]
        .as_array()
        .unwrap_or_else(|| panic!("checklist missing: {report}"))
        .iter()
        .find(|item| item["code"] == code)
        .unwrap_or_else(|| panic!("{code} missing in {report}"))
}

fn past_window(days_ago: i64) -> (String, String) {
    let end = Utc::now() - ChronoDuration::days(days_ago);
    let start = end - ChronoDuration::minutes(50);
    (
        start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        end.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
}

async fn row_json(pool: &PgPool, table: &str, id: Uuid) -> Value {
    assert!(matches!(
        table,
        "claims" | "sessions" | "authorizations" | "notes"
    ));
    let sql = format!("SELECT to_jsonb(t) FROM {table} t WHERE id=$1");
    sqlx::query_scalar(&sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audits_for(pool: &PgPool, resource: &str, id: Uuid) -> Value {
    sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY id), '[]'::jsonb) FROM audit_events a WHERE resource=$1 AND record_id=$2",
    )
    .bind(resource)
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn audit_count(pool: &PgPool, resource: &str, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource=$1 AND record_id=$2")
        .bind(resource)
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn update_audits(pool: &PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE resource='claims' AND record_id=$1 AND action='update'",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn claim_canonical(pool: &PgPool, id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT nexus_service_code_v1(service_code) FROM claims WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn live_canonical(pool: &PgPool, org: Uuid, session_id: Uuid, code: &str) -> i64 {
    sqlx::query_scalar(
        r#"SELECT count(*) FROM claims
           WHERE organization_id=$1 AND session_id=$2 AND status<>'voided'
             AND (nexus_service_code_v1(service_code) COLLATE "C")
               = (nexus_service_code_v1($3) COLLATE "C")"#,
    )
    .bind(org)
    .bind(session_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn listed_claim(fixture: &Fixture, id: &str) -> Value {
    let (status, rows) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "GET",
        "/api/claims",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rows}");
    rows.as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"].as_str() == Some(id))
        .cloned()
        .unwrap_or_else(|| panic!("claim {id} missing from {rows}"))
}

async fn assert_claim_unchanged(fixture: &Fixture, id: Uuid, before: &Value, audits: &Value) {
    let stored = row_json(&fixture.pool, "claims", id).await;
    assert_eq!(
        &stored, before,
        "failed claim write changed the persisted row"
    );
    let current_audits = audits_for(&fixture.pool, "claims", id).await;
    assert_eq!(
        &current_audits, audits,
        "failed claim write changed claim audit history"
    );
    let listed = listed_claim(fixture, &id.to_string()).await;
    assert_eq!(listed["service_code"], before["service_code"]);
    assert_eq!(listed["status"], before["status"]);
    assert_eq!(listed["updated_at"], before["updated_at"]);
    assert_eq!(cents(&listed), cents(before));
}

async fn assert_rejected_code_patch(
    fixture: &Fixture,
    claim: &Value,
    service_code: &str,
    amount: i64,
) {
    let id = uid(claim);
    let before = row_json(&fixture.pool, "claims", id).await;
    let audits = audits_for(&fixture.pool, "claims", id).await;
    let (status, body) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &before,
        json!({"service_code": service_code, "amount_cents": amount}),
    )
    .await;
    assert_canonical_conflict(status, &body);
    assert_claim_unchanged(fixture, id, &before, &audits).await;
    assert_ne!(cents(&before), amount);
}

async fn readiness(fixture: &Fixture, claim_id: &str) -> Value {
    let (status, body) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "GET",
        &format!("/api/claims/{claim_id}/readiness"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let checks = body["checklist"].as_array().expect("checklist");
    let all_passed = checks.iter().all(|item| item["passed"] == true);
    assert_eq!(body["ready"], all_passed, "{body}");
    assert!(!body.to_string().contains("Not a patient record"), "{body}");
    body
}

async fn metrics(fixture: &Fixture) -> Value {
    let (status, body) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "GET",
        "/api/bootstrap",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for key in ["unbilled_cents", "outstanding_cents", "cash_received_cents"] {
        assert!(
            body["metrics"][key].is_i64(),
            "{key} not integer cents: {body}"
        );
    }
    body["metrics"].clone()
}

async fn assert_no_payments(pool: &PgPool) {
    let payments: i64 = sqlx::query_scalar("SELECT count(*) FROM payments")
        .fetch_one(pool)
        .await
        .unwrap();
    let reversals: i64 = sqlx::query_scalar("SELECT count(*) FROM payment_reversals")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(payments, 0);
    assert_eq!(reversals, 0);
}

fn claim_body(client: &Value, policy: &Value, session: &Value, code: &str, amount: i64) -> Value {
    json!({
        "client_id": client["id"],
        "policy_id": policy["id"],
        "session_id": session["id"],
        "service_code": code,
        "diagnosis_code": "F41.1",
        "amount_cents": amount,
        "units": 1
    })
}

async fn create_claim(
    fixture: &Fixture,
    client: &Value,
    policy: &Value,
    session: &Value,
    code: &str,
    amount: i64,
) -> Value {
    let row = create(
        &fixture.pool,
        &fixture.actor,
        "claims",
        claim_body(client, policy, session, code, amount),
    )
    .await;
    assert_same_tenant(&row, fixture.org, &client["id"]);
    assert_eq!(row["session_id"], session["id"]);
    assert_eq!(row["status"], "draft");
    assert_eq!(row["diagnosis_code"], "F41.1");
    assert_eq!(cents(&row), amount);
    let stored = row_json(&fixture.pool, "claims", uid(&row)).await;
    assert_eq!(stored["updated_at"], row["updated_at"]);
    assert_eq!(update_audits(&fixture.pool, uid(&row)).await, 0);
    row
}

async fn open_fixture() -> Fixture {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(
        url.ends_with("/nexus_test"),
        "Refusing to use any database except nexus_test"
    );
    let control = PgPool::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(database, "nexus_test");
    let schema = format!("nexus_claim_canonical_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(3)
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
    let current: Option<String> = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        current.as_deref(),
        Some(schema.as_str()),
        "refusing to migrate outside the unique test schema"
    );
    let isolated: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(isolated, "nexus_test");
    tokio::time::timeout(Duration::from_secs(90), migrate(&pool))
        .await
        .expect("migration timed out")
        .unwrap();
    let identity: bool =
        sqlx::query_scalar("SELECT to_regprocedure('nexus_service_code_v1(text)') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        identity,
        "service-code identity function missing after migration"
    );
    let index: bool =
        sqlx::query_scalar("SELECT to_regclass('one_live_service_claim') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(index, "canonical live claim index missing after migration");
    let (status, setup) = send(
        &pool,
        None,
        "POST",
        "/api/setup",
        json!({
            "organization_name": "Synthetic canonical claims",
            "facility_name": "Synthetic canonical facility",
            "admin_name": "Synthetic canonical admin",
            "admin_email": "canonical-claims@example.invalid"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{setup}");
    let actor = setup["actor"]["id"].as_str().unwrap().to_string();
    let org = Uuid::parse_str(setup["actor"]["organization_id"].as_str().unwrap()).unwrap();
    let facility_id = setup["facility"]["id"].as_str().unwrap().to_string();
    Fixture {
        control,
        pool,
        schema,
        actor,
        org,
        facility_id,
    }
}

async fn finish(fixture: Fixture) {
    assert!(fixture.schema.starts_with("nexus_claim_canonical_"));
    assert!(!fixture.schema.contains('"'));
    let Fixture {
        control,
        pool,
        schema,
        ..
    } = fixture;
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}

async fn synthetic_client(fixture: &Fixture, last_name: &str) -> Value {
    let row = create(
        &fixture.pool,
        &fixture.actor,
        "clients",
        json!({
            "facility_id": fixture.facility_id,
            "first_name": "Synthetic",
            "last_name": last_name,
            "birth_date": "1990-01-01",
            "primary_clinician_id": fixture.actor
        }),
    )
    .await;
    assert_eq!(row["organization_id"], fixture.org.to_string());
    assert!(Uuid::parse_str(row["id"].as_str().unwrap()).is_ok());
    row
}

async fn synthetic_episode(fixture: &Fixture, client: &Value) -> Value {
    support::legacy_episode(
        &fixture.pool,
        &fixture.actor,
        json!({
            "client_id": client["id"],
            "facility_id": fixture.facility_id,
            "level_of_care": "iop",
            "start_date": "2000-01-01"
        }),
    )
    .await
}

async fn legacy_completed_session(
    fixture: &Fixture,
    client: &Value,
    episode: &Value,
    days_ago: i64,
) -> Value {
    let (starts_at, ends_at) = past_window(days_ago);
    let session = support::legacy_session(
        &fixture.pool,
        &fixture.actor,
        json!({
            "client_id": client["id"],
            "episode_id": episode["id"],
            "clinician_id": fixture.actor,
            "starts_at": starts_at,
            "ends_at": ends_at,
            "kind": "individual",
            "status": "completed"
        }),
    )
    .await;
    let service_id: Option<Uuid> =
        sqlx::query_scalar("SELECT program_service_id FROM sessions WHERE id=$1")
            .bind(uid(&session))
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(
        service_id, None,
        "canonical code tests require a legacy encounter"
    );
    session
}

async fn synthetic_policy(fixture: &Fixture, client: &Value, member: &str) -> Value {
    create(
        &fixture.pool,
        &fixture.actor,
        "policies",
        json!({
            "client_id": client["id"],
            "payer_name": "Synthetic payer",
            "member_id": member,
            "effective_date": "2000-01-01",
            "status": "active"
        }),
    )
    .await
}

async fn sign_note(fixture: &Fixture, client: &Value, session: &Value) -> Value {
    let note = create(
        &fixture.pool,
        &fixture.actor,
        "notes",
        json!({
            "client_id": client["id"],
            "session_id": session["id"],
            "title": "Synthetic readiness note",
            "body": "Synthetic canonical readiness note. Not a patient record."
        }),
    )
    .await;
    let (status, signed) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        &format!("/api/notes/{}/sign", note["id"].as_str().unwrap()),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["status"], "signed");
    signed
}

async fn assert_signed_note_immutable(fixture: &Fixture, signed: &Value) {
    let current = row_json(&fixture.pool, "notes", uid(signed)).await;
    let audits = audits_for(&fixture.pool, "notes", uid(signed)).await;
    let (status, denied) = patch(
        &fixture.pool,
        &fixture.actor,
        "notes",
        &current,
        json!({"body": "Overwrite signed history"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{denied}");
    let stored = row_json(&fixture.pool, "notes", uid(signed)).await;
    assert_eq!(stored, current, "rejected edit changed signed note history");
    assert_eq!(
        audits_for(&fixture.pool, "notes", uid(signed)).await,
        audits
    );
}

async fn insert_program(pool: &PgPool, org: Uuid, facility: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    let code = format!("SYN-{}", &id.simple().to_string()[..8]);
    sqlx::query("INSERT INTO care_programs(id,organization_id,facility_id,code,name,level_of_care,min_age,capacity,active) VALUES($1,$2,$3,$4,'Synthetic canonical program','iop',18,8,true)")
        .bind(id)
        .bind(org)
        .bind(facility)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn insert_service(pool: &PgPool, org: Uuid, program: Uuid, code: &str, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO program_services(id,organization_id,program_id,code_system,service_code,name,session_kind,min_minutes,default_minutes,max_minutes,active) VALUES($1,$2,$3,'hcpcs',$4,$5,'individual',30,50,60,true)")
        .bind(id)
        .bind(org)
        .bind(program)
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn coverage_matches(report: &Value, expected: bool) {
    let item = checklist(report, "authorization_coverage");
    assert_eq!(item["passed"], expected, "{item}");
    assert_eq!(item["owner_role"], "billing", "{item}");
    assert!(item.get("body").is_none(), "{item}");
    let message = item["message"].as_str().unwrap_or("");
    if expected {
        assert!(message.contains("SYN-UNICODE-1"), "{item}");
        assert!(message.contains("service code"), "{item}");
    } else {
        assert!(message.contains("service code"), "{item}");
        assert!(message.contains("rather than"), "{item}");
    }
    assert_eq!(
        checklist(report, "authorization_units")["passed"],
        true,
        "{report}"
    );
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn claim_patch_canonical_collision_leaves_row_and_audit_unchanged() {
    let fixture = open_fixture().await;
    let client = synthetic_client(&fixture, "Collision").await;
    let episode = synthetic_episode(&fixture, &client).await;
    let policy = synthetic_policy(&fixture, &client, "COLLISION-1").await;

    let ascii_session = legacy_completed_session(&fixture, &client, &episode, 8).await;
    let kept = create_claim(&fixture, &client, &policy, &ascii_session, "H2019", 1500).await;
    let ascii_other = create_claim(&fixture, &client, &policy, &ascii_session, "T1015", 2200).await;
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, uid(&ascii_session), "H2019").await,
        1
    );
    assert_rejected_code_patch(&fixture, &ascii_other, " h2019 ", 9999).await;
    assert_rejected_code_patch(&fixture, &ascii_other, "H2019\u{00A0}", 9998).await;
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, uid(&ascii_session), "h2019").await,
        1
    );
    assert_eq!(
        cents(&row_json(&fixture.pool, "claims", uid(&kept)).await),
        1500
    );
    assert_eq!(
        cents(&row_json(&fixture.pool, "claims", uid(&ascii_other)).await),
        2200
    );

    let unicode_session = legacy_completed_session(&fixture, &client, &episode, 9).await;
    let folded = create_claim(
        &fixture,
        &client,
        &policy,
        &unicode_session,
        "stra\u{00DF}e",
        1800,
    )
    .await;
    let unicode_other =
        create_claim(&fixture, &client, &policy, &unicode_session, "T1015", 1900).await;
    assert_rejected_code_patch(&fixture, &unicode_other, "STRASSE", 9997).await;
    assert_rejected_code_patch(
        &fixture,
        &unicode_other,
        "\u{00A0}stra\u{00DF}e\u{00A0}",
        9996,
    )
    .await;
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, uid(&unicode_session), "STRASSE").await,
        1
    );

    let folded_id = uid(&folded);
    let before_self = row_json(&fixture.pool, "claims", folded_id).await;
    let (status, updated) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &before_self,
        json!({"service_code": " STRASSE "}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(cents(&updated), 1800);
    assert_eq!(updated["diagnosis_code"], "F41.1");
    assert_eq!(updated["session_id"], unicode_session["id"]);
    assert_eq!(updated["status"], "draft");
    assert_eq!(
        claim_canonical(&fixture.pool, folded_id).await.as_deref(),
        Some("STRASSE")
    );
    assert_eq!(update_audits(&fixture.pool, folded_id).await, 1);
    assert_eq!(
        live_canonical(
            &fixture.pool,
            fixture.org,
            uid(&unicode_session),
            "stra\u{00DF}e"
        )
        .await,
        1
    );
    assert_rejected_code_patch(&fixture, &unicode_other, "stra\u{00DF}e", 9995).await;
    assert_eq!(
        cents(&row_json(&fixture.pool, "claims", uid(&ascii_other)).await),
        2200
    );
    assert_eq!(
        cents(&row_json(&fixture.pool, "claims", folded_id).await),
        1800
    );

    let metrics = metrics(&fixture).await;
    assert_eq!(metrics["unbilled_cents"], 1500 + 2200 + 1800 + 1900);
    assert_eq!(metrics["outstanding_cents"], 0);
    assert_eq!(metrics["cash_received_cents"], 0);
    assert_no_payments(&fixture.pool).await;
    finish(fixture).await;
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn void_and_replace_reuses_canonical_service_code() {
    let fixture = open_fixture().await;
    let client = synthetic_client(&fixture, "Void").await;
    let episode = synthetic_episode(&fixture, &client).await;
    let session = legacy_completed_session(&fixture, &client, &episode, 6).await;
    let policy = synthetic_policy(&fixture, &client, "VOID-1").await;
    let session_id = uid(&session);

    let original = create_claim(&fixture, &client, &policy, &session, "stra\u{00DF}e", 1500).await;
    let sibling = create_claim(&fixture, &client, &policy, &session, "T1015", 2500).await;
    let original_id = uid(&original);
    let sibling_id = uid(&sibling);
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, session_id, "STRASSE").await,
        1
    );
    let sibling_before = row_json(&fixture.pool, "claims", sibling_id).await;
    let original_audits_before = audit_count(&fixture.pool, "claims", original_id).await;

    let (status, voided) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        &format!("/api/claims/{original_id}/void"),
        json!({"reason": "Replace incorrect unsubmitted test claim"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{voided}");
    assert_eq!(voided["status"], "voided");
    assert_eq!(cents(&voided), 1500);
    assert_eq!(voided["service_code"], original["service_code"]);
    let voided_row = row_json(&fixture.pool, "claims", original_id).await;
    assert_eq!(voided_row["status"], "voided");
    assert_eq!(voided_row["service_code"], original["service_code"]);
    assert_eq!(cents(&voided_row), 1500);
    assert_eq!(voided_row["diagnosis_code"], "F41.1");
    assert_eq!(
        audit_count(&fixture.pool, "claims", original_id).await,
        original_audits_before + 1
    );
    assert_eq!(
        voided_row["void_reason"],
        "Replace incorrect unsubmitted test claim"
    );
    let void_audits: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE resource='claims' AND record_id=$1 AND action='void'")
        .bind(original_id).fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(void_audits, 1);
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, session_id, "stra\u{00DF}e").await,
        0
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", sibling_id).await,
        sibling_before
    );

    let replacement = create_claim(&fixture, &client, &policy, &session, " STRASSE ", 1700).await;
    let replacement_id = uid(&replacement);
    assert_ne!(replacement_id, original_id);
    assert_eq!(replacement["status"], "draft");
    assert_eq!(
        claim_canonical(&fixture.pool, replacement_id)
            .await
            .as_deref(),
        Some("STRASSE")
    );
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, session_id, "STRASSE").await,
        1
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", original_id).await,
        voided_row
    );

    let (status, duplicate) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        "/api/claims",
        claim_body(&client, &policy, &session, "stra\u{00DF}e", 1100),
    )
    .await;
    assert_canonical_conflict(status, &duplicate);
    let stray: i64 = sqlx::query_scalar("SELECT count(*) FROM claims WHERE amount_cents = 1100")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(stray, 0);
    assert_eq!(
        row_json(&fixture.pool, "claims", original_id).await,
        voided_row
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", replacement_id).await["amount_cents"],
        replacement["amount_cents"]
    );

    assert_rejected_code_patch(&fixture, &sibling, "STRASSE", 1).await;
    assert_eq!(
        cents(&row_json(&fixture.pool, "claims", sibling_id).await),
        2500
    );

    let (status, voided_replacement) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        &format!("/api/claims/{replacement_id}/void"),
        json!({"reason": "Replace incorrect unsubmitted test claim"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{voided_replacement}");
    assert_eq!(voided_replacement["status"], "voided");
    assert_eq!(cents(&voided_replacement), 1700);
    assert_eq!(
        voided_replacement["service_code"],
        replacement["service_code"]
    );
    let replacement_voided = row_json(&fixture.pool, "claims", replacement_id).await;
    assert_eq!(replacement_voided["status"], "voided");
    assert_eq!(cents(&replacement_voided), 1700);
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, session_id, "STRASSE").await,
        0
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", original_id).await,
        voided_row
    );

    let sibling_loaded = row_json(&fixture.pool, "claims", sibling_id).await;
    let (status, reused) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &sibling_loaded,
        json!({"service_code": " STRASSE "}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reused}");
    assert_eq!(cents(&reused), 2500);
    assert_eq!(reused["status"], "draft");
    assert_eq!(
        claim_canonical(&fixture.pool, sibling_id).await.as_deref(),
        Some("STRASSE")
    );
    assert_eq!(
        live_canonical(&fixture.pool, fixture.org, session_id, "stra\u{00DF}e").await,
        1
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", original_id).await,
        voided_row
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", replacement_id).await,
        replacement_voided
    );

    let (status, edit_voided) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &voided_row,
        json!({"service_code": "T1015", "amount_cents": 9999}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{edit_voided}");
    assert!(
        error_message(&edit_voided).contains("draft"),
        "{edit_voided}"
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", original_id).await,
        voided_row
    );

    let sibling_after = row_json(&fixture.pool, "claims", sibling_id).await;
    let (status, occupied) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        "/api/claims",
        claim_body(&client, &policy, &session, "STRASSE", 1100),
    )
    .await;
    assert_canonical_conflict(status, &occupied);
    let stray: i64 = sqlx::query_scalar("SELECT count(*) FROM claims WHERE amount_cents = 1100")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(stray, 0);
    assert_eq!(
        row_json(&fixture.pool, "claims", sibling_id).await,
        sibling_after
    );
    let voided_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM claims WHERE organization_id=$1 AND session_id=$2 AND status='voided'",
    )
    .bind(fixture.org)
    .bind(session_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(voided_count, 2);

    let (status, rows) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "GET",
        "/api/claims",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rows}");
    let rows = rows.as_array().unwrap();
    let listed = |id: Uuid| {
        rows.iter()
            .find(|row| row["id"].as_str() == Some(&id.to_string()))
            .unwrap()
    };
    assert_eq!(listed(original_id)["status"], "voided");
    assert_eq!(cents(listed(original_id)), 1500);
    assert_eq!(listed(replacement_id)["status"], "voided");
    assert_eq!(cents(listed(replacement_id)), 1700);
    assert_eq!(listed(sibling_id)["status"], "draft");
    assert_eq!(cents(listed(sibling_id)), 2500);

    let metrics = metrics(&fixture).await;
    assert_eq!(metrics["unbilled_cents"], 2500);
    assert_eq!(metrics["outstanding_cents"], 0);
    assert_eq!(metrics["cash_received_cents"], 0);
    assert_no_payments(&fixture.pool).await;
    finish(fixture).await;
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn unicode_authorization_matching_drives_http_readiness() {
    let fixture = open_fixture().await;
    let client = synthetic_client(&fixture, "Unicode").await;
    let episode = synthetic_episode(&fixture, &client).await;
    let session = legacy_completed_session(&fixture, &client, &episode, 4).await;
    let policy = synthetic_policy(&fixture, &client, "UNICODE-1").await;
    let auth = create(
        &fixture.pool,
        &fixture.actor,
        "authorizations",
        json!({
            "client_id": client["id"],
            "policy_id": policy["id"],
            "authorization_number": "SYN-UNICODE-1",
            "service_code": " stra\u{00DF}e ",
            "units_approved": 5,
            "starts_on": "2000-01-01",
            "ends_on": "2099-12-31",
            "status": "active"
        }),
    )
    .await;
    assert_same_tenant(&auth, fixture.org, &client["id"]);
    let signed = sign_note(&fixture, &client, &session).await;
    let mut body = claim_body(&client, &policy, &session, "STRASSE", 1500);
    body["authorization_id"] = auth["id"].clone();
    let claim = create(&fixture.pool, &fixture.actor, "claims", body).await;
    assert_same_tenant(&claim, fixture.org, &client["id"]);
    assert_eq!(cents(&claim), 1500);
    assert_eq!(claim["status"], "draft");
    let claim_id = uid(&claim);

    let stored_auth: String =
        sqlx::query_scalar("SELECT service_code FROM authorizations WHERE id=$1")
            .bind(uid(&auth))
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    let stored_claim: String = sqlx::query_scalar("SELECT service_code FROM claims WHERE id=$1")
        .bind(claim_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    let same: bool = sqlx::query_scalar(
        r#"SELECT COALESCE((nexus_service_code_v1($1) COLLATE "C") = (nexus_service_code_v1($2) COLLATE "C"), false)"#,
    )
    .bind(&stored_auth)
    .bind(&stored_claim)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert!(
        same,
        "stored auth {stored_auth:?} and claim {stored_claim:?}"
    );
    assert_ne!(stored_auth, stored_claim);

    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    assert!(report["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["code"] != "program_service_code"));
    coverage_matches(&report, true).await;
    assert_eq!(report["ready"], true, "{report}");

    let claim_before_auth_patch = row_json(&fixture.pool, "claims", claim_id).await;
    let claim_audits = audits_for(&fixture.pool, "claims", claim_id).await;
    let (status, auth_folded) = patch(
        &fixture.pool,
        &fixture.actor,
        "authorizations",
        &auth,
        json!({"service_code": "\u{FB01}"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth_folded}");
    assert_eq!(
        row_json(&fixture.pool, "claims", claim_id).await,
        claim_before_auth_patch
    );
    assert_eq!(
        audits_for(&fixture.pool, "claims", claim_id).await,
        claim_audits
    );
    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    coverage_matches(&report, false).await;
    assert_eq!(report["ready"], false, "{report}");

    let (status, claim_fi) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &claim_before_auth_patch,
        json!({"service_code": "FI"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claim_fi}");
    assert_eq!(cents(&claim_fi), 1500);
    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    coverage_matches(&report, true).await;
    assert_eq!(report["ready"], true, "{report}");

    let (status, claim_spaced) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &claim_fi,
        json!({"service_code": " fi "}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claim_spaced}");
    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    coverage_matches(&report, true).await;

    let (status, claim_nbsp) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &claim_spaced,
        json!({"service_code": "\u{00A0}FI\u{00A0}"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claim_nbsp}");
    assert_eq!(
        claim_canonical(&fixture.pool, claim_id).await.as_deref(),
        Some("FI")
    );
    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    coverage_matches(&report, true).await;
    assert_eq!(report["ready"], true, "{report}");

    let (status, claim_other) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &claim_nbsp,
        json!({"service_code": "OTHER"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claim_other}");
    assert_eq!(cents(&claim_other), 1500);
    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    coverage_matches(&report, false).await;
    assert_eq!(report["ready"], false, "{report}");
    let ready_audits = audit_count(&fixture.pool, "claims", claim_id).await;
    let before_ready = row_json(&fixture.pool, "claims", claim_id).await;
    let (status, denied) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        &format!("/api/claims/{claim_id}/ready"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{denied}");
    assert_eq!(
        row_json(&fixture.pool, "claims", claim_id).await,
        before_ready
    );
    assert_eq!(
        audit_count(&fixture.pool, "claims", claim_id).await,
        ready_audits
    );
    assert_eq!(before_ready["status"], "draft");

    let (status, auth_accent) = patch(
        &fixture.pool,
        &fixture.actor,
        "authorizations",
        &auth_folded,
        json!({"service_code": "\u{00E9}"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth_accent}");
    let (status, decomposed) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &claim_other,
        json!({"service_code": "e\u{0301}"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{decomposed}");
    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    coverage_matches(&report, false).await;
    assert_eq!(report["ready"], false, "{report}");

    let (status, accented) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &decomposed,
        json!({"service_code": "\u{00C9}"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accented}");
    assert_eq!(cents(&accented), 1500);
    assert_eq!(
        claim_canonical(&fixture.pool, claim_id).await.as_deref(),
        Some("\u{00C9}")
    );
    let report = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    coverage_matches(&report, true).await;
    assert_eq!(report["ready"], true, "{report}");
    let (status, ready) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        &format!("/api/claims/{claim_id}/ready"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["status"], "ready");
    assert_eq!(cents(&ready), 1500);
    let stored = row_json(&fixture.pool, "claims", claim_id).await;
    assert_eq!(stored["status"], "ready");
    assert_eq!(cents(&stored), 1500);

    assert_signed_note_immutable(&fixture, &signed).await;
    let metrics = metrics(&fixture).await;
    assert_eq!(metrics["unbilled_cents"], 1500);
    assert_eq!(metrics["outstanding_cents"], 0);
    assert_eq!(metrics["cash_received_cents"], 0);
    assert_no_payments(&fixture.pool).await;
    finish(fixture).await;
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn unsigned_encounter_service_change_stales_claim_readiness() {
    let fixture = open_fixture().await;
    let client = synthetic_client(&fixture, "Stale").await;
    let episode = synthetic_episode(&fixture, &client).await;
    let facility = Uuid::parse_str(&fixture.facility_id).unwrap();
    let program = insert_program(&fixture.pool, fixture.org, facility).await;
    sqlx::query("UPDATE episodes SET program_id=$1 WHERE id=$2 AND organization_id=$3")
        .bind(program)
        .bind(uid(&episode))
        .bind(fixture.org)
        .execute(&fixture.pool)
        .await
        .unwrap();
    let service_a = insert_service(
        &fixture.pool,
        fixture.org,
        program,
        "H2019",
        "Synthetic individual A",
    )
    .await;
    let service_b = insert_service(
        &fixture.pool,
        fixture.org,
        program,
        "H0031",
        "Synthetic individual B",
    )
    .await;
    let (starts_at, ends_at) = past_window(5);
    let session = create(
        &fixture.pool,
        &fixture.actor,
        "sessions",
        json!({
            "client_id": client["id"],
            "episode_id": episode["id"],
            "clinician_id": fixture.actor,
            "program_service_id": service_a.to_string(),
            "kind": "individual",
            "starts_at": starts_at,
            "ends_at": ends_at,
            "location": "office",
            "status": "scheduled"
        }),
    )
    .await;
    assert_eq!(session["service_snapshot"]["service_code"], "H2019");
    let (status, completed) = patch(
        &fixture.pool,
        &fixture.actor,
        "sessions",
        &session,
        json!({"status": "completed"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["service_snapshot"]["service_code"], "H2019");
    assert_eq!(
        completed["service_snapshot"]["name"],
        "Synthetic individual A"
    );

    let policy = synthetic_policy(&fixture, &client, "STALE-1").await;
    let claim = create_claim(&fixture, &client, &policy, &completed, "H2019", 1500).await;
    let claim_id = uid(&claim);
    let session_id = uid(&completed);
    let notes_before: i64 = sqlx::query_scalar("SELECT count(*) FROM notes WHERE session_id=$1")
        .bind(session_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(notes_before, 0, "encounter must still be unsigned");

    let initial = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    let program_check = checklist(&initial, "program_service_code");
    assert_eq!(program_check["passed"], true, "{program_check}");
    assert!(
        program_check["message"]
            .as_str()
            .unwrap_or("")
            .contains("saved service code"),
        "{program_check}"
    );
    assert_eq!(
        checklist(&initial, "signed_note")["passed"],
        false,
        "{initial}"
    );
    assert_eq!(
        checklist(&initial, "session_completed")["passed"],
        true,
        "{initial}"
    );
    assert_eq!(initial["ready"], false, "{initial}");

    let claim_before = row_json(&fixture.pool, "claims", claim_id).await;
    let audits_before = audits_for(&fixture.pool, "claims", claim_id).await;
    let updates_before = update_audits(&fixture.pool, claim_id).await;
    let current_session = row_json(&fixture.pool, "sessions", session_id).await;
    let (status, changed) = patch(
        &fixture.pool,
        &fixture.actor,
        "sessions",
        &current_session,
        json!({"program_service_id": service_b.to_string()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["program_service_id"], service_b.to_string());
    assert_eq!(changed["service_snapshot"]["service_code"], "H0031");
    assert_eq!(
        changed["service_snapshot"]["name"],
        "Synthetic individual B"
    );
    let snapshot: Value = sqlx::query_scalar("SELECT service_snapshot FROM sessions WHERE id=$1")
        .bind(session_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(snapshot["service_code"], "H0031");
    assert_eq!(snapshot["organization_id"], fixture.org.to_string());
    assert_eq!(
        row_json(&fixture.pool, "claims", claim_id).await,
        claim_before
    );
    assert_eq!(
        audits_for(&fixture.pool, "claims", claim_id).await,
        audits_before
    );
    assert_eq!(update_audits(&fixture.pool, claim_id).await, updates_before);
    let listed = listed_claim(&fixture, claim["id"].as_str().unwrap()).await;
    assert_eq!(listed["service_code"], "H2019");
    assert_eq!(cents(&listed), 1500);
    assert_eq!(listed["status"], "draft");

    let stale = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    let stale_check = checklist(&stale, "program_service_code");
    assert_eq!(stale_check["passed"], false, "{stale_check}");
    assert_eq!(stale_check["owner_role"], "billing");
    assert_eq!(
        stale_check["message"],
        "Claim service code must match the historical program service code."
    );
    assert_eq!(checklist(&stale, "signed_note")["passed"], false, "{stale}");
    assert_eq!(stale["ready"], false, "{stale}");

    let before_amount = row_json(&fixture.pool, "claims", claim_id).await;
    let amount_audits = audit_count(&fixture.pool, "claims", claim_id).await;
    let (status, amount_denied) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &before_amount,
        json!({"amount_cents": 9999}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{amount_denied}");
    assert!(
        error_message(&amount_denied).contains("historical program service code"),
        "{amount_denied}"
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", claim_id).await,
        before_amount
    );
    assert_eq!(
        audit_count(&fixture.pool, "claims", claim_id).await,
        amount_audits
    );
    assert_eq!(update_audits(&fixture.pool, claim_id).await, 0);

    let signed = sign_note(&fixture, &client, &completed).await;
    let blocked = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    assert_eq!(
        checklist(&blocked, "signed_note")["passed"],
        true,
        "{blocked}"
    );
    assert_eq!(
        checklist(&blocked, "session_completed")["passed"],
        true,
        "{blocked}"
    );
    assert_eq!(
        checklist(&blocked, "session_ended")["passed"],
        true,
        "{blocked}"
    );
    assert_eq!(
        checklist(&blocked, "policy_coverage")["passed"],
        true,
        "{blocked}"
    );
    assert_eq!(
        checklist(&blocked, "service_code")["passed"],
        true,
        "{blocked}"
    );
    assert_eq!(
        checklist(&blocked, "diagnosis_code")["passed"],
        true,
        "{blocked}"
    );
    assert_eq!(
        checklist(&blocked, "authorization_coverage")["passed"],
        true,
        "{blocked}"
    );
    assert_eq!(
        checklist(&blocked, "program_service_code")["passed"],
        false,
        "{blocked}"
    );
    assert_eq!(blocked["ready"], false, "{blocked}");
    let ready_audits = audit_count(&fixture.pool, "claims", claim_id).await;
    let before_ready = row_json(&fixture.pool, "claims", claim_id).await;
    let (status, denied_ready) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        &format!("/api/claims/{claim_id}/ready"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{denied_ready}");
    assert!(
        error_message(&denied_ready).contains("historical program service code"),
        "{denied_ready}"
    );
    assert_eq!(
        row_json(&fixture.pool, "claims", claim_id).await,
        before_ready
    );
    assert_eq!(
        audit_count(&fixture.pool, "claims", claim_id).await,
        ready_audits
    );
    assert_eq!(before_ready["status"], "draft");
    assert_eq!(cents(&before_ready), 1500);
    assert_eq!(before_ready["service_code"], "H2019");

    let (status, corrected) = patch(
        &fixture.pool,
        &fixture.actor,
        "claims",
        &before_ready,
        json!({"service_code": " h0031 ", "amount_cents": 1600}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corrected}");
    assert_eq!(corrected["status"], "draft");
    assert_eq!(cents(&corrected), 1600);
    assert_eq!(corrected["diagnosis_code"], "F41.1");
    assert_eq!(
        claim_canonical(&fixture.pool, claim_id).await.as_deref(),
        Some("H0031")
    );
    assert_eq!(update_audits(&fixture.pool, claim_id).await, 1);
    let matched = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    assert_eq!(
        checklist(&matched, "program_service_code")["passed"],
        true,
        "{matched}"
    );
    assert_eq!(matched["ready"], true, "{matched}");
    let (status, ready) = send(
        &fixture.pool,
        Some(&fixture.actor),
        "POST",
        &format!("/api/claims/{claim_id}/ready"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["status"], "ready");
    assert_eq!(cents(&ready), 1600);
    let ready_row = row_json(&fixture.pool, "claims", claim_id).await;
    assert_eq!(ready_row["status"], "ready");
    assert_eq!(cents(&ready_row), 1600);

    // The signed note already protects this encounter. This assertion does not
    // independently establish the advanced-claim-only locking branch.
    let locked_session = row_json(&fixture.pool, "sessions", session_id).await;
    let locked_session_audits = audits_for(&fixture.pool, "sessions", session_id).await;
    let (status, locked) = patch(
        &fixture.pool,
        &fixture.actor,
        "sessions",
        &locked_session,
        json!({"program_service_id": service_a.to_string()}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{locked}");
    assert!(
        error_message(&locked).contains("cannot be rewritten"),
        "{locked}"
    );
    assert_eq!(
        row_json(&fixture.pool, "sessions", session_id).await,
        locked_session
    );
    assert_eq!(
        audits_for(&fixture.pool, "sessions", session_id).await,
        locked_session_audits
    );
    assert_eq!(row_json(&fixture.pool, "claims", claim_id).await, ready_row);
    let still = readiness(&fixture, claim["id"].as_str().unwrap()).await;
    assert_eq!(still["ready"], true, "{still}");
    assert_eq!(
        checklist(&still, "program_service_code")["passed"],
        true,
        "{still}"
    );

    assert_signed_note_immutable(&fixture, &signed).await;
    let metrics = metrics(&fixture).await;
    assert_eq!(metrics["unbilled_cents"], 1600);
    assert_eq!(metrics["outstanding_cents"], 0);
    assert_eq!(metrics["cash_received_cents"], 0);
    assert_no_payments(&fixture.pool).await;
    finish(fixture).await;
}
