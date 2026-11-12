//! Hosted-mode requests end to end: a bearer token from the test provider must map
//! to exactly one active staff row, and the development header must count for nothing.
#[path = "support/oidc.rs"]
mod oidc;

use actix_web::{http::StatusCode, test, web, App};
use emr_api::{configure, migrate, AppState};
use oidc::{Provider, SigningKey};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::Arc;
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
    pool
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
    // The same address in two tenants cannot be resolved to one person.
    staff(&pool, org, "shared@example.invalid", "admin", true).await;
    staff(&pool, other_org, "shared@example.invalid", "admin", true).await;
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

    // Unknown, inactive, ambiguous and unverified accounts get one identical refusal.
    let mut unverified = provider.claims("admin@example.invalid");
    unverified["email_verified"] = json!(false);
    let refusals = [
        ("unknown user", token("nobody@example.invalid")),
        ("inactive user", token("former@example.invalid")),
        (
            "address shared by two tenants",
            token("shared@example.invalid"),
        ),
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
