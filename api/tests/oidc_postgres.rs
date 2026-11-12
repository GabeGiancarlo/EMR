//! Hosted-mode requests end to end: a bearer token from the test provider must map
//! to exactly one active staff row, and the development header must count for nothing.
#[path = "support/oidc.rs"]
mod oidc;

use actix_web::{http::StatusCode, test, web, App};
use emr_api::{configure, db, migrate, AppState};
use oidc::{Provider, SigningKey};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Call<'a> {
    method: &'a str,
    path: &'a str,
    token: Option<&'a str>,
    actor_header: Option<Uuid>,
    body: Option<Value>,
    scheme: &'a str,
}
impl<'a> Call<'a> {
    fn get(path: &'a str) -> Self {
        Self {
            method: "GET",
            path,
            token: None,
            actor_header: None,
            body: None,
            scheme: "Bearer",
        }
    }
    fn post(path: &'a str, body: Value) -> Self {
        Self {
            method: "POST",
            body: Some(body),
            ..Self::get(path)
        }
    }
    fn token(mut self, token: &'a str) -> Self {
        self.token = Some(token);
        self
    }
    fn scheme(mut self, scheme: &'a str) -> Self {
        self.scheme = scheme;
        self
    }
    fn claiming(mut self, staff: Uuid) -> Self {
        self.actor_header = Some(staff);
        self
    }
}

async fn send(state: &web::Data<AppState>, call: Call<'_>) -> (StatusCode, Value, Option<String>) {
    let app = test::init_service(App::new().app_data(state.clone()).configure(configure)).await;
    let mut req = test::TestRequest::default()
        .method(call.method.parse().unwrap())
        .uri(call.path);
    if let Some(token) = call.token {
        req = req.insert_header(("authorization", format!("{} {token}", call.scheme)));
    }
    if let Some(staff) = call.actor_header {
        req = req.insert_header(("x-nexus-actor", staff.to_string()));
    }
    if let Some(body) = call.body {
        req = req.set_json(body);
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let challenge = response
        .headers()
        .get("www-authenticate")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = test::read_body(response).await;
    (
        status,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
        challenge,
    )
}

async fn disposable_pool(prefix: &str) -> PgPool {
    sized_pool(prefix, PgPoolOptions::new().max_connections(4)).await
}

/// A migrated pool on a new schema of the disposable database, built from `options`
/// so a test can choose the pool size and timeouts it is about.
async fn sized_pool(prefix: &str, options: PgPoolOptions) -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(database, "nexus_test");
    let schema = format!("{prefix}_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    // The schema travels in the connection options, so it applies to every pooled
    // connection without replacing the pool's own connection setup.
    let connect: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
    let pool = options
        .connect_with(connect.options([("search_path", schema.as_str())]))
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

async fn try_staff(
    pool: &PgPool,
    org: Uuid,
    email: &str,
    active: bool,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role,active) VALUES($1,$2,'Synthetic sign-in test operator',$3,'admin',$4)")
        .bind(id).bind(org).bind(email).bind(active).execute(pool).await?;
    Ok(id)
}

fn unique_violation(result: Result<Uuid, sqlx::Error>) -> bool {
    matches!(result, Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505"))
}

async fn organization(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,$2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn staff(pool: &PgPool, org: Uuid, email: &str, role: &str, active: bool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role,active) VALUES($1,$2,'Synthetic sign-in test operator',$3,$4,$5)")
        .bind(id).bind(org).bind(email).bind(role).bind(active).execute(pool).await.unwrap();
    id
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn hosted_requests_act_only_as_the_signed_in_staff_member() {
    let pool = disposable_pool("nexus_oidc_test").await;
    let org = organization(&pool, "Synthetic sign-in test").await;
    let other_org = organization(&pool, "Synthetic neighbouring tenant").await;
    let admin = staff(&pool, org, "admin@example.invalid", "admin", true).await;
    let clinician = staff(&pool, org, "Clinician@Example.invalid", "clinician", true).await;
    staff(&pool, org, "former@example.invalid", "admin", false).await;
    // The same address in two tenants could not be resolved to one person, so the
    // database refuses a second active holder, in either tenant and in any case.
    staff(&pool, org, "shared@example.invalid", "admin", true).await;
    for (tenant, address) in [
        (other_org, "shared@example.invalid"),
        (other_org, "Shared@Example.INVALID"),
        (org, "SHARED@example.invalid"),
    ] {
        assert!(
            unique_violation(try_staff(&pool, tenant, address, true).await),
            "{address}"
        );
    }
    // Only active staff hold an address: a former member's can be given to someone
    // new, and it then signs in as that person alone.
    try_staff(&pool, org, "reused@example.invalid", false)
        .await
        .unwrap();
    let successor = try_staff(&pool, other_org, "Reused@Example.invalid", true)
        .await
        .unwrap();
    let outsider = staff(&pool, other_org, "outsider@example.invalid", "admin", true).await;

    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    let state = web::Data::new(AppState::oidc(
        pool.clone(),
        Arc::new(provider.connect().await),
    ));
    let token = |email: &str| key.sign(&provider.claims(email));

    // A valid token for an active staff member opens that member's workspace.
    let clinician_token = token("clinician@example.invalid");
    let (status, body, _) = send(&state, Call::get("/api/bootstrap").token(&clinician_token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["authentication"], "oidc");
    assert_eq!(body["actor"]["id"], clinician.to_string());
    assert_eq!(body["actor"]["role"], "clinician");
    assert_eq!(body["organizations"].as_array().unwrap().len(), 1);
    assert_eq!(body["organizations"][0]["id"], org.to_string());
    // Role policy still applies to the resolved staff member.
    assert_eq!(body["permissions"]["staff"]["write"], false);
    assert_eq!(body["permissions"]["claims"]["read"], false);

    // The authorization scheme name is case-insensitive; another scheme is not a sign-in.
    for scheme in ["bearer", "BEARER", "BeArEr"] {
        let call = Call::get("/api/bootstrap")
            .token(&clinician_token)
            .scheme(scheme);
        let (status, body, _) = send(&state, call).await;
        assert_eq!(status, StatusCode::OK, "{scheme}");
        assert_eq!(body["actor"]["id"], clinician.to_string());
    }
    let call = Call::get("/api/bootstrap")
        .token(&clinician_token)
        .scheme("Basic");
    assert_eq!(send(&state, call).await.0, StatusCode::UNAUTHORIZED);

    let (status, body, _) = send(
        &state,
        Call::get("/api/bootstrap").token(&token("reused@example.invalid")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["actor"]["id"], successor.to_string());
    assert_eq!(body["organizations"][0]["id"], other_org.to_string());

    // Unknown, inactive and unverified accounts get one identical refusal.
    let mut unverified = provider.claims("admin@example.invalid");
    unverified["email_verified"] = json!(false);
    let refusals = [
        ("unknown user", token("nobody@example.invalid")),
        ("inactive user", token("former@example.invalid")),
        ("unverified email", key.sign(&unverified)),
    ];
    let mut bodies = Vec::new();
    for (name, refused) in &refusals {
        let (status, body, _) = send(&state, Call::get("/api/bootstrap").token(refused)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{name}: {body}");
        assert_eq!(body["error"]["code"], "no_access", "{name}");
        bodies.push(body);
    }
    assert!(bodies.iter().all(|body| body == &bodies[0]));
    assert!(!bodies[0].to_string().contains("example.invalid"));
    assert!(bodies[0].get("staff").is_none());

    // No token, a broken token and an expired token are all "sign in again".
    let mut expired = provider.claims("admin@example.invalid");
    expired["exp"] = json!(jsonwebtoken::get_current_timestamp() - 3600);
    let expired = key.sign(&expired);
    let impostor = SigningKey::generate("key-1").sign(&provider.claims("admin@example.invalid"));
    for (name, call) in [
        ("no token", Call::get("/api/bootstrap")),
        ("garbage", Call::get("/api/bootstrap").token("not-a-token")),
        ("expired", Call::get("/api/bootstrap").token(&expired)),
        (
            "foreign signature",
            Call::get("/api/bootstrap").token(&impostor),
        ),
        ("staff list", Call::get("/api/staff")),
        (
            "write",
            Call::post("/api/tasks", json!({"title": "Synthetic"})),
        ),
    ] {
        let (status, body, challenge) = send(&state, call).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{name}: {body}");
        assert_eq!(body["error"]["code"], "unauthenticated", "{name}");
        assert_eq!(challenge.as_deref(), Some("Bearer"), "{name}");
        // The operator picker's listing is never served when hosted.
        assert!(body.get("staff").is_none() && body.get("organizations").is_none());
    }

    // The development header is ignored: alone it proves nothing...
    for claimed in [admin, outsider] {
        let (status, _, _) = send(&state, Call::get("/api/bootstrap").claiming(claimed)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _, _) = send(
            &state,
            Call::post("/api/tasks", json!({"title": "Synthetic"})).claiming(claimed),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    // ...and beside a valid token it cannot change who the caller is.
    let (status, body, _) = send(
        &state,
        Call::get("/api/bootstrap")
            .token(&clinician_token)
            .claiming(admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actor"]["id"], clinician.to_string());
    let (status, body, _) = send(
        &state,
        Call::post(
            "/api/staff",
            json!({"name": "Synthetic escalation", "email": "new@example.invalid", "role": "admin"}),
        )
        .token(&clinician_token)
        .claiming(admin),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _, _) = send(
        &state,
        Call::get("/api/bootstrap")
            .token(&clinician_token)
            .claiming(outsider),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Writes are attributed to the token's staff member, through the same
    // organization lock and audit transaction as before.
    let admin_token = token("admin@example.invalid");
    let (status, created, _) = send(
        &state,
        Call::post(
            "/api/tasks",
            json!({"title": "Synthetic hosted-mode task", "priority": "normal", "status": "open"}),
        )
        .token(&admin_token)
        .claiming(clinician),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let recorded: Option<Uuid> = sqlx::query_scalar(
        "SELECT actor_id FROM audit_events WHERE resource='tasks' AND action='create' AND record_id=$1",
    )
    .bind(Uuid::parse_str(created["id"].as_str().unwrap()).unwrap())
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(recorded, Some(admin));

    // Tenant scope: the neighbouring tenant's administrator sees only that tenant.
    let outsider_token = token("outsider@example.invalid");
    let (status, body, _) = send(&state, Call::get("/api/tasks").token(&outsider_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);

    // First-run setup is closed when hosted, signed in or not.
    let setup = json!({"organization_name": "Takeover", "facility_name": "Takeover",
        "admin_name": "Takeover", "admin_email": "takeover@example.invalid"});
    let (status, _, _) = send(&state, Call::post("/api/setup", setup.clone())).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = send(&state, Call::post("/api/setup", setup).token(&admin_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let organizations: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(organizations, 2);

    // Public surface: sign-in configuration and health, with nothing about staff.
    let (status, body, _) = send(&state, Call::get("/api/auth/config")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mode"], "oidc");
    assert_eq!(body["issuer"], provider.issuer);
    let (status, body, _) = send(&state, Call::get("/healthz")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authentication"], "oidc");
    assert_eq!(body["database"], "connected");
    // No worker was started in this test process; the key is still reported.
    assert_eq!(body["workers"], json!({}));
    let (status, body, _) = send(&state, Call::get("/livez")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"status": "alive"}));

    // Patient visit routes keep their own invitation token and are not staff routes:
    // a staff token is not a visit invitation, and no invitation is a visit refusal.
    let (status, body, _) = send(&state, Call::get("/api/telehealth/visit")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "visit_access");
    let (status, body, _) = send(
        &state,
        Call::get("/api/telehealth/visit").token(&admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "visit_access");

    // A deactivated account loses access on its next request.
    sqlx::query("UPDATE staff SET active=false WHERE id=$1")
        .bind(clinician)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body, _) = send(&state, Call::get("/api/bootstrap").token(&clinician_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, bodies[0]);
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn local_mode_is_unchanged_and_does_not_accept_bearer_identity() {
    let pool = disposable_pool("nexus_oidc_local_test").await;
    let org = organization(&pool, "Synthetic local test").await;
    let admin = staff(&pool, org, "admin@example.invalid", "admin", true).await;
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    let token = key.sign(&provider.claims("admin@example.invalid"));
    let state = web::Data::new(AppState::local(pool.clone()));

    let (status, body, _) = send(&state, Call::get("/api/auth/config")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"mode": "local"}));
    // Without the header the local operator picker is listed, as before.
    for call in [
        Call::get("/api/bootstrap"),
        Call::get("/api/bootstrap").token(&token),
    ] {
        let (status, body, _) = send(&state, call).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["authentication"], "deferred-local-only");
        assert!(body["actor"].is_null());
        assert_eq!(body["staff"].as_array().unwrap().len(), 1);
    }
    let (status, body, _) = send(&state, Call::get("/api/bootstrap").claiming(admin)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actor"]["id"], admin.to_string());
    // A bearer token is not an identity in local mode.
    let (status, body, _) = send(&state, Call::get("/api/staff").token(&token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
}

/// The staff a test acts as in local mode.
async fn local_call(
    state: &web::Data<AppState>,
    method: &str,
    path: &str,
    actor: Uuid,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let call = Call {
        method,
        body,
        ..Call::get(path)
    }
    .claiming(actor);
    let (status, body, _) = send(state, call).await;
    (status, body)
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn a_request_never_needs_a_second_pooled_connection() {
    // One connection, and a short wait for it. A handler that opens a transaction and
    // then asks the pool for another connection cannot be served: it would wait out
    // the acquire timeout and fail. With ten connections the same fault is ten
    // concurrent requests each holding one and waiting for another.
    let options = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3));
    let pool = sized_pool("nexus_pool_test", options).await;
    let org = organization(&pool, "Synthetic pool test").await;
    let admin = staff(&pool, org, "admin@example.invalid", "admin", true).await;
    // Not an administrator, so every permission is looked up in the database.
    let support = staff(&pool, org, "support@example.invalid", "support", true).await;
    let state = web::Data::new(AppState::local(pool.clone()));

    let began = Instant::now();
    for actor in [support, admin] {
        let (status, body) = local_call(&state, "GET", "/api/bootstrap", actor, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["actor"]["id"], actor.to_string());
    }
    let (_, body) = local_call(&state, "GET", "/api/bootstrap", support, None).await;
    assert_eq!(body["permissions"]["tasks"]["read"], true);
    assert_eq!(body["permissions"]["notes"]["read"], false);
    assert_eq!(body["permissions"]["staff"]["write"], false);

    // The other handlers that used to re-read the operator from the pool while
    // holding a transaction. Each now gets past that point to its ordinary answer.
    let nobody = Uuid::new_v4();
    for path in [
        format!("/api/clients/{nobody}/form-requirements"),
        format!("/api/clinical-drafts/{nobody}"),
    ] {
        let (status, body) = local_call(&state, "GET", &path, admin, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    }
    for path in [
        format!("/api/clinical-drafts/{nobody}/cancel"),
        format!("/api/clinical-drafts/{nobody}/review"),
    ] {
        let body = json!({"decision": "reject", "reason": "Synthetic pool regression", "attested": true, "decisions": []});
        let (status, body) = local_call(&state, "POST", &path, admin, Some(body)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    }
    // Nothing above waited for a connection that could never come.
    assert!(
        began.elapsed() < Duration::from_secs(3),
        "{:?}",
        began.elapsed()
    );

    // Several at once still complete: each waits its turn for the one connection
    // and none holds it while waiting for another.
    let calls = (0..6).map(|_| local_call(&state, "GET", "/api/bootstrap", support, None));
    for (status, body) in futures_join(calls).await {
        assert_eq!(status, StatusCode::OK, "{body}");
    }
}

/// Await every future concurrently on this thread and collect the answers.
async fn futures_join<F: std::future::Future>(futures: impl Iterator<Item = F>) -> Vec<F::Output> {
    let mut pending: Vec<_> = futures.map(Box::pin).collect();
    let mut answers: Vec<Option<F::Output>> = pending.iter().map(|_| None).collect();
    std::future::poll_fn(|context| {
        for (slot, future) in answers.iter_mut().zip(pending.iter_mut()) {
            if slot.is_none() {
                if let std::task::Poll::Ready(answer) = future.as_mut().poll(context) {
                    *slot = Some(answer);
                }
            }
        }
        if answers.iter().all(Option::is_some) {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    })
    .await;
    answers.into_iter().flatten().collect()
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn request_connections_time_out_and_dedicated_ones_do_not() {
    let settings = db::PoolSettings {
        max_connections: 2,
        statement_timeout: Some(Duration::from_millis(1500)),
        lock_timeout: Some(Duration::from_millis(400)),
    };
    let pool = sized_pool("nexus_timeout_test", db::pool_options(&settings)).await;
    let show = |name: &'static str| format!("SELECT current_setting('{name}')");
    let statement: String = sqlx::query_scalar(&show("statement_timeout"))
        .fetch_one(&pool)
        .await
        .unwrap();
    let lock: String = sqlx::query_scalar(&show("lock_timeout"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((statement.as_str(), lock.as_str()), ("1500ms", "400ms"));

    // A statement that overruns is cancelled, and reported as a retryable 503.
    let began = Instant::now();
    let slow = sqlx::query("SELECT pg_sleep(10)").execute(&pool).await;
    assert!(began.elapsed() < Duration::from_secs(5));
    let error = emr_api::ApiError::from(slow.unwrap_err());
    assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error.code, "database_timeout");

    // A dedicated connection carries neither limit.
    let mut owner = db::dedicated(&pool).await.unwrap();
    for name in ["statement_timeout", "lock_timeout"] {
        let value: String = sqlx::query_scalar(&show(name))
            .fetch_one(&mut owner)
            .await
            .unwrap();
        assert_eq!(value, "0", "{name}");
    }

    // A lock held elsewhere fails the waiting request quickly instead of pinning it.
    sqlx::query("SELECT pg_advisory_lock(918240977)")
        .execute(&mut owner)
        .await
        .unwrap();
    let began = Instant::now();
    let blocked = sqlx::query("SELECT pg_advisory_lock(918240977)")
        .execute(&pool)
        .await;
    assert!(began.elapsed() < Duration::from_secs(3));
    let error = emr_api::ApiError::from(blocked.unwrap_err());
    assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error.code, "database_busy");

    // The pool does not count the dedicated connection: both of its two connections
    // can still be taken while that one is open.
    let _first = pool.acquire().await.unwrap();
    let _second = pool.acquire().await.unwrap();
    sqlx::query("SELECT 1").execute(&mut owner).await.unwrap();

    // With every pooled connection in use, a request is refused after the acquire
    // timeout instead of waiting indefinitely; readiness answers sooner still.
    let state = web::Data::new(AppState::local(pool.clone()));
    let began = Instant::now();
    let (status, body, _) = send(&state, Call::get("/healthz")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["error"]["code"], "database_unavailable");
    assert_eq!(body["database"], "unavailable");
    assert!(
        began.elapsed() < Duration::from_secs(4),
        "{:?}",
        began.elapsed()
    );
    // Liveness does not depend on the database at all.
    let began = Instant::now();
    let (status, _, _) = send(&state, Call::get("/livez")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(began.elapsed() < Duration::from_millis(500));
    let began = Instant::now();
    let (status, body, _) = send(&state, Call::get("/api/bootstrap")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["error"]["code"], "database_unavailable");
    assert!(began.elapsed() < db::ACQUIRE_TIMEOUT + Duration::from_secs(3));
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn staff_email_is_normalized_on_write_and_unique_among_active_staff() {
    let pool = disposable_pool("nexus_staff_email_test").await;
    let state = web::Data::new(AppState::local(pool.clone()));
    let setup = json!({"organization_name": "Synthetic email test", "facility_name": "Synthetic",
        "admin_name": "Synthetic administrator", "admin_email": "  Admin@Example.INVALID "});
    let (status, created, _) = send(&state, Call::post("/api/setup", setup)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["actor"]["email"], "admin@example.invalid");
    let admin = Uuid::parse_str(created["actor"]["id"].as_str().unwrap()).unwrap();

    let new_staff =
        |email: &str| json!({"name": "Synthetic clinician", "email": email, "role": "clinician"});
    let (status, clinician) = local_call(
        &state,
        "POST",
        "/api/staff",
        admin,
        Some(new_staff(" Clinician@Example.Invalid ")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{clinician}");
    assert_eq!(clinician["email"], "clinician@example.invalid");
    let stored: String = sqlx::query_scalar("SELECT email FROM staff WHERE id=$1")
        .bind(Uuid::parse_str(clinician["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "clinician@example.invalid");

    // The same person in another spelling is a conflict, not a second account.
    for email in ["clinician@example.invalid", "CLINICIAN@EXAMPLE.INVALID"] {
        let (status, body) =
            local_call(&state, "POST", "/api/staff", admin, Some(new_staff(email))).await;
        assert_eq!(status, StatusCode::CONFLICT, "{email}: {body}");
        assert_eq!(body["error"]["code"], "conflict");
        assert!(!body.to_string().contains("example.invalid"));
    }
    for email in [
        "",
        "not-an-address",
        "two@signs@example.invalid",
        "a b@example.invalid",
    ] {
        let (status, body) =
            local_call(&state, "POST", "/api/staff", admin, Some(new_staff(email))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{email:?}: {body}");
        assert_eq!(body["error"]["code"], "validation");
    }

    // An edit is normalized too, and cannot take an address someone active holds.
    let patch =
        |email: &str| json!({"email": email, "_expected_updated_at": clinician["updated_at"]});
    let path = format!("/api/staff/{}", clinician["id"].as_str().unwrap());
    let (status, body) = local_call(
        &state,
        "PATCH",
        &path,
        admin,
        Some(patch("Admin@example.invalid")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = local_call(
        &state,
        "PATCH",
        &path,
        admin,
        Some(patch("  Renamed@Example.Invalid")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["email"], "renamed@example.invalid");
    let staff_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM staff")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(staff_rows, 2);
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn malformed_requests_are_answered_in_the_error_envelope() {
    let pool = disposable_pool("nexus_envelope_test").await;
    let org = organization(&pool, "Synthetic envelope test").await;
    let admin = staff(&pool, org, "admin@example.invalid", "admin", true).await;
    let state = web::Data::new(AppState::local(pool.clone()));
    let app = test::init_service(App::new().app_data(state.clone()).configure(configure)).await;
    let actor = ("x-nexus-actor", admin.to_string());

    let cases = [
        // (method, path, content type, body, expected status, expected code)
        ("GET", "/nowhere", None, "", 404, "not_found"),
        ("DELETE", "/api/staff", None, "", 404, "not_found"),
        (
            "GET",
            "/api/speech/transcribe",
            None,
            "",
            405,
            "method_not_allowed",
        ),
        (
            "POST",
            "/api/tasks",
            Some("application/json"),
            "{\"title\":",
            400,
            "validation",
        ),
        (
            "POST",
            "/api/tasks",
            Some("text/plain"),
            "{}",
            400,
            "validation",
        ),
        (
            "POST",
            "/api/telehealth/rooms",
            Some("application/json"),
            "{\"session\":1}",
            400,
            "validation",
        ),
        (
            "GET",
            "/api/telehealth/rooms/not-a-uuid",
            None,
            "",
            404,
            "not_found",
        ),
        (
            "GET",
            "/api/telehealth/rooms?session_id=not-a-uuid",
            None,
            "",
            400,
            "validation",
        ),
    ];
    for (method, path, content_type, body, status, code) in cases {
        let mut request = test::TestRequest::default()
            .method(method.parse().unwrap())
            .uri(path)
            .insert_header(actor.clone())
            .set_payload(body);
        if let Some(content_type) = content_type {
            request = request.insert_header(("content-type", content_type));
        }
        let response = test::call_service(&app, request.to_request()).await;
        assert_eq!(response.status().as_u16(), status, "{method} {path}");
        let content = response.headers().get("content-type").unwrap().clone();
        assert_eq!(content, "application/json", "{method} {path}");
        let answer: Value = test::read_body_json(response).await;
        assert_eq!(answer["error"]["code"], code, "{method} {path}: {answer}");
        let text = answer.to_string().to_lowercase();
        for leak in [
            "serde",
            "uuid",
            "line 1",
            "column",
            "expected",
            "session_id",
            "missing field",
        ] {
            assert!(!text.contains(leak), "{method} {path} leaks {leak}: {text}");
        }
    }
}
