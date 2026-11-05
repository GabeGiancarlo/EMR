//! Export contract tests against real PostgreSQL and the configured Actix HTTP routes.
//! Synthetic fixtures exist only in a uniquely named schema inside nexus_test.
use actix_web::{
    http::{header, StatusCode},
    test, web, App,
};
use emr_api::{configure, migrate, AppState};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

const ACCOUNTING: &str = "/api/exports/accounting.csv?from=2026-06-30&to=2026-07-01";
const CLAIMS: &str = "/api/exports/claims.csv?from=2026-06-30&to=2026-07-01";
const ACCOUNTING_HEADER: &str =
    "\"received_on\",\"source\",\"entry_count\",\"cash_received_cents\",\"adjustment_cents\"\r\n";
const CLAIMS_HEADER: &str = "\"export_purpose\",\"claim_id\",\"client_id\",\"policy_id\",\"session_id\",\"authorization_id\",\"service_code\",\"diagnosis_code\",\"units\",\"amount_cents\",\"status\",\"external_reference\",\"denial_reason\",\"void_reason\",\"created_at_utc\",\"updated_at_utc\"\r\n";

struct Download {
    status: StatusCode,
    body: String,
    headers: header::HeaderMap,
}

async fn download(pool: &PgPool, path: &str, actor: Option<&str>) -> Download {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
            .configure(configure),
    )
    .await;
    let mut req = test::TestRequest::get().uri(path);
    if let Some(actor) = actor {
        req = req.insert_header(("x-nexus-actor", actor));
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
    Download {
        status,
        body,
        headers,
    }
}

fn assert_download(result: &Download, filename: &str) {
    assert_eq!(result.status, StatusCode::OK, "{}", result.body);
    assert_eq!(
        result.headers.get(header::CONTENT_TYPE).unwrap(),
        "text/csv; charset=utf-8"
    );
    assert_eq!(
        result
            .headers
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap(),
        format!("attachment; filename=\"{filename}\"")
    );
    assert_eq!(
        result.headers.get(header::CACHE_CONTROL).unwrap(),
        "no-store, private"
    );
    assert_eq!(result.headers.get(header::PRAGMA).unwrap(), "no-cache");
    assert_eq!(
        result.headers.get("x-content-type-options").unwrap(),
        "nosniff"
    );
}

async fn audit_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit_events")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn staff(pool: &PgPool, org: Uuid, role: &str, active: bool) -> String {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO staff(id,organization_id,name,email,role,active) VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(id)
    .bind(org)
    .bind(format!("Export test {role}"))
    .bind(format!("{id}@example.invalid"))
    .bind(role)
    .bind(active)
    .execute(pool)
    .await
    .unwrap();
    id.to_string()
}

struct Tenant {
    org: Uuid,
    facility: Uuid,
    admin: String,
}

async fn tenant(pool: &PgPool) -> Tenant {
    let org = Uuid::new_v4();
    let facility = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO organizations(id,name) VALUES($1,'Disposable export test organization')",
    )
    .bind(org)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO facilities(id,organization_id,name) VALUES($1,$2,'Disposable export test facility')")
        .bind(facility).bind(org).execute(pool).await.unwrap();
    Tenant {
        org,
        facility,
        admin: staff(pool, org, "admin", true).await,
    }
}

struct ClaimFixture {
    claim: Uuid,
    private_values: Vec<String>,
}

async fn seed_claim(pool: &PgPool, tenant: &Tenant, marker: &str) -> ClaimFixture {
    let client = Uuid::new_v4();
    let episode = Uuid::new_v4();
    let session = Uuid::new_v4();
    let policy = Uuid::new_v4();
    let authorization = Uuid::new_v4();
    let claim = Uuid::new_v4();
    let actor = Uuid::parse_str(&tenant.admin).unwrap();
    let name = format!("PRIVATE_CLIENT_{marker}");
    let member = format!("PRIVATE_MEMBER_{marker}");
    let email = format!("private_{marker}@example.invalid");
    let phone = format!("PRIVATE_PHONE_{marker}");
    let narrative = format!("PRIVATE_NOTE_{marker}");
    sqlx::query("INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date,email,phone,primary_clinician_id) VALUES($1,$2,$3,$4,'Synthetic','1990-01-01',$5,$6,$7)")
        .bind(client).bind(tenant.org).bind(tenant.facility).bind(&name).bind(&email).bind(&phone).bind(actor)
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date) VALUES($1,$2,$3,$4,'iop','2026-01-01')")
        .bind(episode).bind(tenant.org).bind(client).bind(tenant.facility).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind,status) VALUES($1,$2,$3,$4,$5,'2026-06-01T12:00:00Z','2026-06-01T13:00:00Z','individual','completed')")
        .bind(session).bind(tenant.org).bind(client).bind(episode).bind(actor).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO notes(id,organization_id,client_id,session_id,author_id,title,body) VALUES($1,$2,$3,$4,$5,'Synthetic export test note',$6)")
        .bind(Uuid::new_v4()).bind(tenant.org).bind(client).bind(session).bind(actor).bind(&narrative).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO policies(id,organization_id,client_id,payer_name,member_id,effective_date) VALUES($1,$2,$3,'Synthetic payer',$4,'2026-01-01')")
        .bind(policy).bind(tenant.org).bind(client).bind(&member).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO authorizations(id,organization_id,client_id,policy_id,authorization_number,service_code,units_approved,starts_on,ends_on) VALUES($1,$2,$3,$4,'PRIVATE_AUTH','TEST',10,'2026-01-01','2026-12-31')")
        .bind(authorization).bind(tenant.org).bind(client).bind(policy).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO claims(id,organization_id,client_id,policy_id,session_id,authorization_id,service_code,diagnosis_code,units,amount_cents,status,external_reference,denial_reason,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,'TEST','TEST-DX',3,25000,'denied',$7,$8,'2026-07-01T23:59:59Z','2026-07-02T01:00:00Z')")
        .bind(claim).bind(tenant.org).bind(client).bind(policy).bind(session).bind(authorization)
        .bind("  =HYPERLINK(\"https://example.invalid\")")
        .bind("Reason, \"quoted\"\r\ncontinued")
        .execute(pool).await.unwrap();
    ClaimFixture {
        claim,
        private_values: vec![
            name,
            member,
            email,
            phone,
            narrative,
            client.to_string(),
            policy.to_string(),
            session.to_string(),
            authorization.to_string(),
            claim.to_string(),
        ],
    }
}

async fn payment(
    pool: &PgPool,
    tenant: &Tenant,
    claim: Uuid,
    day: &str,
    source: &str,
    cents: i64,
) -> String {
    let reference = format!("PRIVATE_PAYMENT_{}", Uuid::new_v4());
    sqlx::query("INSERT INTO payments(id,organization_id,claim_id,received_on,source,amount_cents,reference) VALUES($1,$2,$3,$4::text::date,$5,$6,$7)")
        .bind(Uuid::new_v4()).bind(tenant.org).bind(claim).bind(day).bind(source).bind(cents).bind(&reference)
        .execute(pool).await.unwrap();
    reference
}

async fn reverse_payment(pool: &PgPool, tenant: &Tenant, cents: i64, day: &str) -> String {
    let reason = format!("PRIVATE_REVERSAL_{}", Uuid::new_v4());
    let result = sqlx::query("INSERT INTO payment_reversals(id,organization_id,payment_id,claim_id,amount_cents,reversed_on,reason,actor_id) SELECT $1,organization_id,id,claim_id,amount_cents,$2::text::date,$3,$4 FROM payments WHERE organization_id=$5 AND amount_cents=$6")
        .bind(Uuid::new_v4()).bind(day).bind(&reason).bind(Uuid::parse_str(&tenant.admin).unwrap()).bind(tenant.org).bind(cents)
        .execute(pool).await.unwrap();
    assert_eq!(result.rows_affected(), 1);
    reason
}

#[actix_web::test]
#[ignore = "Requires TEST_DATABASE_URL ending in /nexus_test; creates and removes only a unique test schema"]
async fn exports_enforce_roles_scope_projection_and_audit() {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("Set TEST_DATABASE_URL for the disposable database");
    assert!(
        url.ends_with("/nexus_test"),
        "Refusing to create test fixtures outside nexus_test"
    );
    let control = PgPool::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(database, "nexus_test");
    // The name has a fixed prefix and hex-only suffix. It cannot target public or user schemas.
    let schema = format!("nexus_export_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let search_path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            let search_path = search_path.clone();
            Box::pin(async move {
                sqlx::query(&search_path).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();

    let first = tenant(&pool).await;
    let billing = staff(&pool, first.org, "billing", true).await;
    let executive = staff(&pool, first.org, "executive", true).await;
    let clinician = staff(&pool, first.org, "clinician", true).await;
    let support = staff(&pool, first.org, "support", true).await;
    let lead = staff(&pool, first.org, "clinical_lead", true).await;
    let inactive = staff(&pool, first.org, "admin", false).await;

    let empty = download(&pool, ACCOUNTING, Some(&first.admin)).await;
    assert_download(&empty, "nexus-accounting-2026-06-30-to-2026-07-01.csv");
    assert_eq!(empty.body, ACCOUNTING_HEADER);
    let empty = download(&pool, CLAIMS, Some(&first.admin)).await;
    assert_download(
        &empty,
        "nexus-claims-internal-review-2026-06-30-to-2026-07-01.csv",
    );
    assert_eq!(empty.body, CLAIMS_HEADER);
    assert_eq!(audit_count(&pool).await, 2);

    let unknown = Uuid::new_v4().to_string();
    for actor in [
        None,
        Some(""),
        Some("not-a-uuid"),
        Some(unknown.as_str()),
        Some(inactive.as_str()),
    ] {
        for path in [ACCOUNTING, CLAIMS] {
            let result = download(&pool, path, actor).await;
            assert_eq!(result.status, StatusCode::FORBIDDEN, "{}", result.body);
        }
    }
    // An exception can grant individual resource reads but cannot broaden bulk export policy.
    for actor in [&clinician, &support, &lead, &executive] {
        for resource in ["claims", "payments"] {
            sqlx::query("INSERT INTO exceptions(id,organization_id,staff_id,resource,reason,expires_at,access_mode) VALUES($1,$2,$3,$4,'Synthetic export restriction test',now()+interval '1 day','read')")
                .bind(Uuid::new_v4()).bind(first.org).bind(Uuid::parse_str(actor).unwrap()).bind(resource).execute(&pool).await.unwrap();
        }
        assert_eq!(
            download(&pool, CLAIMS, Some(actor)).await.status,
            StatusCode::FORBIDDEN
        );
    }
    for actor in [&clinician, &support, &lead] {
        assert_eq!(
            download(&pool, ACCOUNTING, Some(actor)).await.status,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        audit_count(&pool).await,
        2,
        "Denied export requests must not be logged as completed exports"
    );

    let fixture = seed_claim(&pool, &first, "FIRST").await;
    let mut private_values = fixture.private_values;
    for (day, source, cents) in [
        ("2026-06-30", "payer", 1000),
        ("2026-06-30", "payer", 2500),
        ("2026-06-30", "client", 750),
        ("2026-06-30", "adjustment", 300),
        ("2026-07-01", "payer", 4000),
        ("2026-06-29", "payer", 88888),
        ("2026-07-02", "payer", 77777),
    ] {
        private_values.push(payment(&pool, &first, fixture.claim, day, source, cents).await);
    }
    // Reversals are booked on their own date, including a receipt from an earlier period.
    for (cents, day) in [
        (1000, "2026-07-01"),
        (300, "2026-07-01"),
        (88888, "2026-07-01"),
        (2500, "2026-07-02"),
    ] {
        private_values.push(reverse_payment(&pool, &first, cents, day).await);
    }
    let other = tenant(&pool).await;
    let other_fixture = seed_claim(&pool, &other, "OTHER").await;
    payment(
        &pool,
        &other,
        other_fixture.claim,
        "2026-06-30",
        "payer",
        99999,
    )
    .await;
    reverse_payment(&pool, &other, 99999, "2026-07-01").await;

    let expected = format!("{ACCOUNTING_HEADER}\"2026-06-30\",\"adjustment\",\"1\",\"0\",\"300\"\r\n\"2026-06-30\",\"client\",\"1\",\"750\",\"0\"\r\n\"2026-06-30\",\"payer\",\"2\",\"3500\",\"0\"\r\n\"2026-07-01\",\"adjustment\",\"1\",\"0\",\"-300\"\r\n\"2026-07-01\",\"payer\",\"3\",\"-85888\",\"0\"\r\n");
    for actor in [&first.admin, &billing, &executive] {
        let result = download(&pool, ACCOUNTING, Some(actor)).await;
        assert_download(&result, "nexus-accounting-2026-06-30-to-2026-07-01.csv");
        assert_eq!(result.body, expected);
        for private in &private_values {
            assert!(
                !result.body.contains(private),
                "Accounting exposed a private source value"
            );
        }
    }
    for actor in [&first.admin, &billing] {
        let result = download(&pool, CLAIMS, Some(actor)).await;
        assert_download(
            &result,
            "nexus-claims-internal-review-2026-06-30-to-2026-07-01.csv",
        );
        assert!(result.body.starts_with(CLAIMS_HEADER));
        assert_eq!(
            result
                .body
                .matches("internal_review_not_payer_submission")
                .count(),
            1
        );
        assert!(result.body.contains(&fixture.claim.to_string()));
        assert!(!result.body.contains(&other_fixture.claim.to_string()));
        assert!(result
            .body
            .contains("\"TEST\",\"TEST-DX\",\"3\",\"25000\",\"denied\""));
        assert!(result
            .body
            .contains("\"'  =HYPERLINK(\"\"https://example.invalid\"\")\""));
        assert!(result
            .body
            .contains("\"Reason, \"\"quoted\"\"\r\ncontinued\""));
        assert!(result.body.contains("2026-07-01T23:59:59+00:00"));
        assert!(result.body.contains("2026-07-02T01:00:00+00:00"));
    }
    let result = download(&pool, ACCOUNTING, Some(&other.admin)).await;
    assert_eq!(
        result.body,
        format!("{ACCOUNTING_HEADER}\"2026-06-30\",\"payer\",\"1\",\"99999\",\"0\"\r\n\"2026-07-01\",\"payer\",\"1\",\"-99999\",\"0\"\r\n")
    );
    let result = download(&pool, CLAIMS, Some(&other.admin)).await;
    assert!(result.body.contains(&other_fixture.claim.to_string()));
    assert!(!result.body.contains(&fixture.claim.to_string()));
    let before_invalid = audit_count(&pool).await;
    assert_eq!(before_invalid, 9);

    for path in [
        "/api/exports/accounting.csv",
        "/api/exports/accounting.csv?from=2026-07-01",
        "/api/exports/accounting.csv?from=2026-07-02&to=2026-07-01",
        "/api/exports/accounting.csv?from=2024-01-01&to=2025-01-01",
        "/api/exports/accounting.csv?from=2026-02-30&to=2026-03-01",
        "/api/exports/accounting.csv?from=2026-06-30&to=2026-07-01&organization_id=other",
        "/api/exports/claims.csv?from=2026-07-01",
    ] {
        assert_eq!(
            download(&pool, path, Some(&first.admin)).await.status,
            StatusCode::BAD_REQUEST,
            "{path}"
        );
    }
    assert_eq!(audit_count(&pool).await, before_invalid);

    // Filtering is by UTC claim creation, not service date or updated_at.
    let outside = download(
        &pool,
        "/api/exports/claims.csv?from=2026-07-02&to=2026-07-02",
        Some(&first.admin),
    )
    .await;
    assert_eq!(outside.status, StatusCode::OK);
    assert_eq!(outside.body, CLAIMS_HEADER);
    let audits: Vec<(Uuid, Uuid, String, String, Option<Uuid>)> = sqlx::query_as("SELECT organization_id,actor_id,action,resource,record_id FROM audit_events ORDER BY occurred_at")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(audits.len(), 10);
    for (org, actor, action, resource, record_id) in &audits {
        assert!(record_id.is_none());
        assert!(matches!(
            (action.as_str(), resource.as_str()),
            ("export_accounting_csv", "payments") | ("export_claims_csv_internal_review", "claims")
        ));
        let expected_org: Uuid =
            sqlx::query_scalar("SELECT organization_id FROM staff WHERE id=$1")
                .bind(actor)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(*org, expected_org);
    }
    assert!(audits
        .iter()
        .any(|(_, actor, action, _, _)| actor.to_string() == executive
            && action == "export_accounting_csv"));

    // Force the audit insert to fail: no downloadable success may escape that transaction.
    sqlx::raw_sql("CREATE FUNCTION fail_export_audit_test() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Synthetic audit failure'; END $$; CREATE TRIGGER fail_export_audit_test BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION fail_export_audit_test();")
        .execute(&pool).await.unwrap();
    for path in [ACCOUNTING, CLAIMS] {
        let result = download(&pool, path, Some(&first.admin)).await;
        assert_eq!(result.status, StatusCode::CONFLICT);
        assert!(!result.headers.contains_key(header::CONTENT_DISPOSITION));
    }
    assert_eq!(audit_count(&pool).await, 10);
    let unchanged: (String, String) =
        sqlx::query_as("SELECT status,external_reference FROM claims WHERE id=$1")
            .bind(fixture.claim)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        unchanged,
        (
            "denied".into(),
            "  =HYPERLINK(\"https://example.invalid\")".into()
        )
    );

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}

/// Inclusive UTC start and exclusive UTC end for from=2026-08-01&to=2026-08-01.
/// Mirrors claims() in exports.rs: start = from 00:00:00Z, end = (to + 1 day) 00:00:00Z.
const CAP_WINDOW_START: &str = "2026-08-01T00:00:00Z";
const CAP_WINDOW_END: &str = "2026-08-02T00:00:00Z";
const CAP_IN_WINDOW: &str = CAP_WINDOW_START;
const CAP_BEFORE_WINDOW: &str = "2026-07-31T23:59:59Z";
const CAP_EXPORT: &str = "/api/exports/claims.csv?from=2026-08-01&to=2026-08-01";
const CAP_FILENAME: &str = "nexus-claims-internal-review-2026-08-01-to-2026-08-01.csv";
const CAP_DIAGNOSIS: &str = "DXPLAIN";
const CAP_REFERENCE: &str = "CAPCOHORT";
const CAP_CENTS: i64 = 100;
const CAP_RENDERED_AT: &str = "2026-08-01T00:00:00+00:00";
const CAP_REJECTION: &str =
    "More than 10000 claims match. Narrow the export dates; no partial export was generated.";

/// Deterministic synthetic id. Index 1 is 00000001-0000-4000-8000-000000000001.
/// Index 10000 is 00002710-0000-4000-8000-000000002710, matching PostgreSQL to_hex.
fn cohort_claim_id(index: i32) -> Uuid {
    assert!((1..=10_000).contains(&index));
    Uuid::parse_str(&format!("{index:08x}-0000-4000-8000-{index:012x}")).unwrap()
}

fn assert_exact_set(
    actual: &std::collections::BTreeSet<String>,
    expected: &std::collections::BTreeSet<String>,
    label: &str,
) {
    let missing: Vec<_> = expected.difference(actual).take(5).cloned().collect();
    let extra: Vec<_> = actual.difference(expected).take(5).cloned().collect();
    assert!(
        missing.is_empty() && extra.is_empty() && actual.len() == expected.len(),
        "{label} mismatch actual={} expected={} missing={missing:?} extra={extra:?}",
        actual.len(),
        expected.len()
    );
}

fn plain_csv_table(body: &str) -> Vec<Vec<String>> {
    assert!(
        body.ends_with("\r\n"),
        "claims export records must be CRLF-delimited"
    );
    assert!(
        !body.contains("\r\n\r\n"),
        "plain synthetic fields must not produce a blank record"
    );
    body.split("\r\n")
        .filter(|line| !line.is_empty())
        .map(|line| {
            assert!(!line.contains('\n') && !line.contains('\r'));
            line.split(',')
                .map(|cell| {
                    assert!(
                        cell.len() >= 2 && cell.starts_with('"') && cell.ends_with('"'),
                        "expected a fully quoted plain field, got {cell:?}"
                    );
                    let inner = &cell[1..cell.len() - 1];
                    assert!(
                        !inner.contains('"'),
                        "plain field contained a quote: {cell:?}"
                    );
                    inner.to_string()
                })
                .collect()
        })
        .collect()
}

struct CapChain {
    client: Uuid,
    policy: Uuid,
    session: Uuid,
    authorization: Uuid,
}

async fn cap_chain(pool: &PgPool, tenant: &Tenant) -> CapChain {
    let client = Uuid::new_v4();
    let episode = Uuid::new_v4();
    let session = Uuid::new_v4();
    let policy = Uuid::new_v4();
    let authorization = Uuid::new_v4();
    let actor = Uuid::parse_str(&tenant.admin).unwrap();
    sqlx::query("INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date,email,phone,primary_clinician_id) VALUES($1,$2,$3,'Synthetic','Cohort','1990-01-01',$4,'0000000000',$5)")
        .bind(client).bind(tenant.org).bind(tenant.facility).bind(format!("{client}@example.invalid")).bind(actor)
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date) VALUES($1,$2,$3,$4,'iop','2026-01-01')")
        .bind(episode).bind(tenant.org).bind(client).bind(tenant.facility)
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind,status) VALUES($1,$2,$3,$4,$5,'2026-06-01T12:00:00Z','2026-06-01T13:00:00Z','individual','completed')")
        .bind(session).bind(tenant.org).bind(client).bind(episode).bind(actor)
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO policies(id,organization_id,client_id,payer_name,member_id,effective_date) VALUES($1,$2,$3,'Synthetic payer',$4,'2026-01-01')")
        .bind(policy).bind(tenant.org).bind(client).bind(format!("SYN{client}"))
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO authorizations(id,organization_id,client_id,policy_id,authorization_number,service_code,units_approved,starts_on,ends_on) VALUES($1,$2,$3,$4,'SYNTHAUTH','TEST',10,'2026-01-01','2026-12-31')")
        .bind(authorization).bind(tenant.org).bind(client).bind(policy)
        .execute(pool).await.unwrap();
    CapChain {
        client,
        policy,
        session,
        authorization,
    }
}

async fn insert_plain_claim(
    pool: &PgPool,
    tenant: &Tenant,
    chain: &CapChain,
    claim: Uuid,
    service_code: &str,
    reference: &str,
    created_at: &str,
) {
    let inserted = sqlx::query(
        r#"
        INSERT INTO claims(
            id, organization_id, client_id, policy_id, session_id, authorization_id,
            service_code, diagnosis_code, units, amount_cents, status,
            external_reference, denial_reason, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, 1, $9, 'draft', $10, '',
            $11::text::timestamptz, $11::text::timestamptz
        )
        "#,
    )
    .bind(claim)
    .bind(tenant.org)
    .bind(chain.client)
    .bind(chain.policy)
    .bind(chain.session)
    .bind(chain.authorization)
    .bind(service_code)
    .bind(CAP_DIAGNOSIS)
    .bind(CAP_CENTS)
    .bind(reference)
    .bind(created_at)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(inserted.rows_affected(), 1);
}

async fn insert_live_cohort(
    pool: &PgPool,
    tenant: &Tenant,
    chain: &CapChain,
    count: i32,
    created_at: &str,
    service_prefix: &str,
    reference: &str,
) {
    // Distinct live service codes satisfy one_live_service_claim
    // (organization_id, session_id, nexus_service_code_v1(service_code)) WHERE status <> 'voided'.
    // $1 org, $2 client, $3 policy, $4 session, $5 authorization,
    // $6 service prefix, $7 diagnosis, $8 amount_cents, $9 reference,
    // $10 created_at, $11 count. Units and status stay plain literals 1 and draft.
    let inserted = sqlx::query(
        r#"
        INSERT INTO claims(
            id, organization_id, client_id, policy_id, session_id, authorization_id,
            service_code, diagnosis_code, units, amount_cents, status,
            external_reference, denial_reason, created_at, updated_at
        )
        SELECT
            (lpad(to_hex(g), 8, '0') || '-0000-4000-8000-' || lpad(to_hex(g), 12, '0'))::uuid,
            $1,
            $2,
            $3,
            $4,
            $5,
            $6 || lpad(g::text, 5, '0'),
            $7,
            1,
            $8,
            'draft',
            $9,
            '',
            $10::text::timestamptz,
            $10::text::timestamptz
        FROM generate_series(1, $11::int) AS series(g)
        "#,
    )
    .bind(tenant.org)
    .bind(chain.client)
    .bind(chain.policy)
    .bind(chain.session)
    .bind(chain.authorization)
    .bind(service_prefix)
    .bind(CAP_DIAGNOSIS)
    .bind(CAP_CENTS)
    .bind(reference)
    .bind(created_at)
    .bind(count)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(inserted.rows_affected(), count as u64);
}

async fn claims_in_window(pool: &PgPool, org: Uuid) -> i64 {
    sqlx::query_scalar(
        r#"SELECT count(*) FROM claims WHERE organization_id=$1 AND created_at >= $2::text::timestamptz AND created_at < $3::text::timestamptz"#,
    )
    .bind(org)
    .bind(CAP_WINDOW_START)
    .bind(CAP_WINDOW_END)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn claim_total(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM claims")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 10000 authorized in-window claims download; the 10001st matching claim rejects
/// the whole export. Out-of-window same-tenant rows and in-window other-tenant rows
/// must not count toward the cap. Rejection writes no CSV and no audit row.
#[actix_web::test]
#[ignore = "Requires TEST_DATABASE_URL ending in /nexus_test; creates and removes only a unique test schema"]
async fn claims_export_accepts_exactly_10000_and_rejects_10001_without_partial_csv_or_audit() {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("Set TEST_DATABASE_URL for the disposable database");
    assert!(
        url.ends_with("/nexus_test"),
        "Refusing to create test fixtures outside nexus_test"
    );
    let control = PgPool::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(database, "nexus_test");
    let schema_prefix = "nexus_export_cap_";
    let schema = format!("{schema_prefix}{}", Uuid::new_v4().simple());
    assert_eq!(schema.len(), schema_prefix.len() + 32);
    assert!(schema.starts_with(schema_prefix));
    assert!(schema[schema_prefix.len()..]
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit()));
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let search_path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            let search_path = search_path.clone();
            Box::pin(async move {
                sqlx::query(&search_path).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();

    let first = tenant(&pool).await;
    let other = tenant(&pool).await;
    let chain = cap_chain(&pool, &first).await;
    let other_chain = cap_chain(&pool, &other).await;
    insert_live_cohort(
        &pool,
        &first,
        &chain,
        10_000,
        CAP_IN_WINDOW,
        "C",
        CAP_REFERENCE,
    )
    .await;
    let before = Uuid::new_v4();
    let after = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    insert_plain_claim(
        &pool,
        &first,
        &chain,
        before,
        "OUTB",
        "OUTRANGE",
        CAP_BEFORE_WINDOW,
    )
    .await;
    insert_plain_claim(
        &pool,
        &first,
        &chain,
        after,
        "OUTA",
        "OUTRANGE",
        CAP_WINDOW_END,
    )
    .await;
    insert_plain_claim(
        &pool,
        &other,
        &other_chain,
        foreign,
        "OT0001",
        "OTHERTENANT",
        CAP_IN_WINDOW,
    )
    .await;

    let stored_first: Uuid = sqlx::query_scalar(
        "SELECT id FROM claims WHERE organization_id=$1 AND service_code='C00001'",
    )
    .bind(first.org)
    .fetch_one(&pool)
    .await
    .unwrap();
    let stored_last: Uuid = sqlx::query_scalar(
        "SELECT id FROM claims WHERE organization_id=$1 AND service_code='C10000'",
    )
    .bind(first.org)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored_first, cohort_claim_id(1));
    assert_eq!(stored_last, cohort_claim_id(10_000));
    // 10000 in window plus 3 excluded rows. A cap query that forgot organization_id
    // or the created_at bounds would see more than 10000 and reject this download.
    assert_eq!(claim_total(&pool).await, 10_003);
    assert_eq!(claims_in_window(&pool, first.org).await, 10_000);
    assert_eq!(claims_in_window(&pool, other.org).await, 1);
    assert_eq!(audit_count(&pool).await, 0);

    let success = download(&pool, CAP_EXPORT, Some(&first.admin)).await;
    assert_download(&success, CAP_FILENAME);
    assert_eq!(success.body.lines().count(), 10_001);
    assert_eq!(success.body.matches("\r\n").count(), 10_001);
    assert_eq!(success.body.matches('\n').count(), 10_001);
    assert_eq!(success.body.matches('\r').count(), 10_001);
    assert!(success.body.starts_with(CLAIMS_HEADER));
    assert_eq!(success.body.matches(r#""CAPCOHORT""#).count(), 10_000);
    assert!(!success.body.contains("100.00"));
    assert!(!success.body.contains(&before.to_string()));
    assert!(!success.body.contains(&after.to_string()));
    assert!(!success.body.contains(&foreign.to_string()));
    assert!(!success.body.contains(&other_chain.client.to_string()));
    assert!(!success.body.contains(&other_chain.policy.to_string()));
    assert!(!success.body.contains(&other_chain.session.to_string()));
    assert!(!success
        .body
        .contains(&other_chain.authorization.to_string()));
    assert!(!success.body.contains("OUTRANGE"));
    assert!(!success.body.contains("OTHERTENANT"));
    assert!(!success.body.contains(r#""OUTB""#));
    assert!(!success.body.contains(r#""OUTA""#));
    assert!(!success.body.contains(r#""OT0001""#));
    assert!(!success.body.contains(r#""C10001""#));

    let table = plain_csv_table(&success.body);
    assert_eq!(table.len(), 10_001);
    let expected_header: Vec<String> = CLAIMS_HEADER
        .trim_end_matches("\r\n")
        .split(',')
        .map(|cell| cell.trim_matches('"').to_string())
        .collect();
    assert_eq!(table[0], expected_header);
    let mut ids = std::collections::BTreeSet::new();
    let mut codes = std::collections::BTreeSet::new();
    for row in table.iter().skip(1) {
        assert_eq!(row.len(), 16, "{row:?}");
        assert_eq!(row[0], "internal_review_not_payer_submission");
        assert_eq!(row[2], chain.client.to_string());
        assert_eq!(row[3], chain.policy.to_string());
        assert_eq!(row[4], chain.session.to_string());
        assert_eq!(row[5], chain.authorization.to_string());
        assert_eq!(row[7], CAP_DIAGNOSIS);
        assert_eq!(row[8], "1");
        assert_eq!(row[9], CAP_CENTS.to_string());
        assert_eq!(row[10], "draft");
        assert_eq!(row[11], CAP_REFERENCE);
        assert_eq!(row[12], "");
        assert_eq!(row[13], "");
        assert_eq!(row[14], CAP_RENDERED_AT);
        assert_eq!(row[15], row[14]);
        ids.insert(row[1].clone());
        codes.insert(row[6].clone());
    }
    let expected_ids: std::collections::BTreeSet<String> = (1..=10_000)
        .map(|index| cohort_claim_id(index).to_string())
        .collect();
    let expected_codes: std::collections::BTreeSet<String> =
        (1..=10_000).map(|index| format!("C{index:05}")).collect();
    assert_exact_set(&ids, &expected_ids, "claim ids");
    assert_exact_set(&codes, &expected_codes, "service codes");

    let audit: (Uuid, Uuid, String, String, Option<Uuid>) = sqlx::query_as(
        "SELECT organization_id,actor_id,action,resource,record_id FROM audit_events",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit.0, first.org);
    assert_eq!(audit.1, Uuid::parse_str(&first.admin).unwrap());
    assert_eq!(audit.2, "export_claims_csv_internal_review");
    assert_eq!(audit.3, "claims");
    assert!(audit.4.is_none());
    let unchanged: (i64, String) =
        sqlx::query_as("SELECT amount_cents,status FROM claims WHERE id=$1")
            .bind(stored_first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unchanged, (CAP_CENTS, "draft".to_string()));
    assert_eq!(claim_total(&pool).await, 10_003);

    let overflow = Uuid::new_v4();
    insert_plain_claim(
        &pool,
        &first,
        &chain,
        overflow,
        "C10001",
        "CAPOVERFLOW",
        CAP_IN_WINDOW,
    )
    .await;
    assert_eq!(claims_in_window(&pool, first.org).await, 10_001);
    assert_eq!(claim_total(&pool).await, 10_004);
    let rejected = download(&pool, CAP_EXPORT, Some(&first.admin)).await;
    // Same ApiError::invalid mapping already asserted as 400 for malformed export windows.
    assert_eq!(
        rejected.status,
        StatusCode::BAD_REQUEST,
        "{}",
        rejected.body
    );
    assert!(rejected.body.contains(CAP_REJECTION), "{}", rejected.body);
    assert!(!rejected
        .body
        .contains("internal_review_not_payer_submission"));
    assert!(!rejected.body.contains("CAPCOHORT"));
    assert!(!rejected.body.contains("CAPOVERFLOW"));
    assert!(!rejected.body.contains("C00001"));
    assert!(!rejected.body.contains("C10001"));
    assert!(!rejected.body.contains(&overflow.to_string()));
    assert!(!rejected.body.starts_with(CLAIMS_HEADER));
    assert!(
        !rejected.headers.contains_key(header::CONTENT_DISPOSITION),
        "cap rejection must not offer a download"
    );
    let content_type = rejected
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(
        !content_type.to_ascii_lowercase().starts_with("text/csv"),
        "cap rejection must not be served as CSV, content-type {content_type}"
    );
    assert_eq!(audit_count(&pool).await, 1);
    let still_audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action='export_claims_csv_internal_review' AND resource='claims' AND record_id IS NULL AND organization_id=$1 AND actor_id=$2",
    )
    .bind(first.org)
    .bind(Uuid::parse_str(&first.admin).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(still_audited, 1);
    assert_eq!(claims_in_window(&pool, first.org).await, 10_001);
    assert_eq!(claims_in_window(&pool, other.org).await, 1);
    assert_eq!(claim_total(&pool).await, 10_004);
    let still: (i64, String) = sqlx::query_as("SELECT amount_cents,status FROM claims WHERE id=$1")
        .bind(overflow)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(still, (CAP_CENTS, "draft".to_string()));

    assert_eq!(schema.len(), schema_prefix.len() + 32);
    assert!(schema.starts_with(schema_prefix));
    assert!(schema[schema_prefix.len()..]
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit()));
    pool.close().await;
    // Only this test's uniquely named schema is removed.
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
