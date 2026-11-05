//! Program-service routing through HTTP after root wires the helpers.
//! Catalog rows and pre-migration bookings are explicit SQL fixtures.
mod support;
use actix_web::{http::StatusCode, test, web, App};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

async fn send(
    pool: &PgPool,
    actor: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
            .configure(configure),
    )
    .await;
    let request = match method {
        "POST" => test::TestRequest::post(),
        "PATCH" => test::TestRequest::patch(),
        _ => test::TestRequest::get(),
    }
    .uri(path)
    .insert_header(("x-nexus-actor", actor))
    .set_json(body)
    .to_request();
    let response = test::call_service(&app, request).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn create(pool: &PgPool, actor: &str, resource: &str, body: Value) -> Value {
    let (status, row) = send(pool, actor, "POST", &format!("/api/{resource}"), body).await;
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
    body["_expected_updated_at"] = row["updated_at"].clone();
    send(
        pool,
        actor,
        "PATCH",
        &format!("/api/{resource}/{}", row["id"].as_str().unwrap()),
        body,
    )
    .await
}
fn uid(value: &Value) -> Uuid {
    Uuid::parse_str(value["id"].as_str().unwrap()).unwrap()
}
fn message(value: &Value) -> String {
    value["error"]["message"].as_str().unwrap_or("").to_string()
}
async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}
async fn insert_program(pool: &PgPool, org: Uuid, facility: Uuid, code: &str, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO care_programs(id,organization_id,facility_id,code,name,level_of_care,min_age,capacity,active) VALUES($1,$2,$3,$4,$5,'iop',18,8,true)")
        .bind(id)
        .bind(org)
        .bind(facility)
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}
#[allow(clippy::too_many_arguments)] // Explicit synthetic catalog fixture dimensions.
async fn insert_service(
    pool: &PgPool,
    org: Uuid,
    program: Uuid,
    code: &str,
    name: &str,
    kind: &str,
    min: i32,
    max: i32,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO program_services(id,organization_id,program_id,code_system,service_code,name,session_kind,min_minutes,default_minutes,max_minutes,active) VALUES($1,$2,$3,'hcpcs',$4,$5,$6,$7,$8,$9,true)")
        .bind(id)
        .bind(org)
        .bind(program)
        .bind(code)
        .bind(name)
        .bind(kind)
        .bind(min)
        .bind(min)
        .bind(max)
        .execute(pool)
        .await
        .unwrap();
    id
}
async fn link_program(pool: &PgPool, episode: &Value, program: Uuid) {
    sqlx::query("UPDATE episodes SET program_id=$1 WHERE id=$2")
        .bind(program)
        .bind(uid(episode))
        .execute(pool)
        .await
        .unwrap();
}
async fn session_snapshot(pool: &PgPool, id: Uuid) -> (Option<Uuid>, Value) {
    let service_id: Option<Uuid> =
        sqlx::query_scalar("SELECT program_service_id FROM sessions WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    let snapshot: Value = sqlx::query_scalar("SELECT service_snapshot FROM sessions WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    (service_id, snapshot)
}
fn session_body(
    client: &Value,
    episode: &Value,
    clinician: &str,
    service: Uuid,
    start: &str,
    end: &str,
    kind: &str,
) -> Value {
    json!({
        "client_id": client["id"],
        "episode_id": episode["id"],
        "clinician_id": clinician,
        "program_service_id": service,
        "kind": kind,
        "starts_at": start,
        "ends_at": end,
        "location": "office"
    })
}

// RecordForm submits all editable fields, formatting unchanged timestamps as
// JavaScript ISO strings rather than PostgreSQL's JSON timestamp spelling.
fn booking_form_body(resource: &str, row: &Value) -> Value {
    let fields: &[&str] = if resource == "sessions" {
        &[
            "client_id",
            "episode_id",
            "clinician_id",
            "program_service_id",
            "kind",
            "starts_at",
            "ends_at",
            "location",
            "status",
        ]
    } else {
        &[
            "facility_id",
            "clinician_id",
            "program_service_id",
            "title",
            "kind",
            "starts_at",
            "ends_at",
            "capacity",
            "location",
        ]
    };
    let mut body = serde_json::Map::new();
    for field in fields {
        let value = if matches!(*field, "starts_at" | "ends_at") {
            let instant = chrono::DateTime::parse_from_rfc3339(row[field].as_str().unwrap())
                .unwrap()
                .with_timezone(&chrono::Utc);
            json!(instant.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        } else {
            row[field].clone()
        };
        body.insert((*field).into(), value);
    }
    Value::Object(body)
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn service_routing_preserves_snapshots_and_rejects_invalid_links() {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(
        url.ends_with("/nexus_test"),
        "Use only the disposable nexus_test database"
    );
    let control = PgPool::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(database, "nexus_test");
    let schema = format!("nexus_program_services_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
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
    migrate(&pool).await.unwrap();
    let (status, setup) = send(
        &pool,
        "",
        "POST",
        "/api/setup",
        json!({
            "organization_name": "Synthetic programs",
            "facility_name": "Synthetic facility",
            "admin_name": "Synthetic admin",
            "admin_email": "programs@example.invalid"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{setup}");
    let actor = setup["actor"]["id"].as_str().unwrap().to_string();
    let org = Uuid::parse_str(setup["actor"]["organization_id"].as_str().unwrap()).unwrap();
    let facility = uid(&setup["facility"]);
    let other_facility = create(
        &pool,
        &actor,
        "facilities",
        json!({"name": "Second facility", "timezone": "America/New_York", "active": true}),
    )
    .await;
    let idle_facility = create(
        &pool,
        &actor,
        "facilities",
        json!({"name": "Idle facility", "timezone": "America/New_York", "active": true}),
    )
    .await;

    async fn client(pool: &PgPool, actor: &str, facility: &Value, name: &str) -> (Value, Value) {
        let row = create(
            pool,
            actor,
            "clients",
            json!({
                "first_name": "Synthetic",
                "last_name": name,
                "birth_date": "1990-01-01",
                "facility_id": facility["id"],
                "primary_clinician_id": actor
            }),
        )
        .await;
        let episode = support::legacy_episode(
            pool,
            actor,
            json!({
                "client_id": row["id"],
                "facility_id": facility["id"],
                "level_of_care": "iop",
                "start_date": "2026-01-01"
            }),
        )
        .await;
        (row, episode)
    }
    let (one, episode_a) = client(&pool, &actor, &setup["facility"], "One").await;
    let (two, episode_b) = client(&pool, &actor, &setup["facility"], "Two").await;
    let (three, legacy_episode) = client(&pool, &actor, &setup["facility"], "Three").await;
    let (four, group_episode) = client(&pool, &actor, &setup["facility"], "Four").await;
    let (five, group_episode_two) = client(&pool, &actor, &setup["facility"], "Five").await;
    let (six, past_episode) = client(&pool, &actor, &setup["facility"], "Six").await;
    let (seven, idle_episode) = client(&pool, &actor, &idle_facility, "Seven").await;

    let program_a = insert_program(&pool, org, facility, "IOP-A", "Program A").await;
    let program_b = insert_program(&pool, org, facility, "IOP-B", "Program B").await;
    let program_idle =
        insert_program(&pool, org, uid(&idle_facility), "IOP-IDLE", "Idle program").await;
    let program_toggle = insert_program(&pool, org, facility, "IOP-TOGGLE", "Toggle program").await;
    link_program(&pool, &episode_a, program_a).await;
    link_program(&pool, &episode_b, program_b).await;
    link_program(&pool, &group_episode, program_a).await;
    link_program(&pool, &group_episode_two, program_a).await;
    link_program(&pool, &past_episode, program_a).await;
    link_program(&pool, &idle_episode, program_idle).await;
    let individual = insert_service(
        &pool,
        org,
        program_a,
        "H2019",
        "Individual therapy",
        "individual",
        30,
        60,
    )
    .await;
    let grouped = insert_service(
        &pool,
        org,
        program_a,
        "H2019G",
        "Group skills",
        "group",
        30,
        60,
    )
    .await;
    let other_program_service = insert_service(
        &pool,
        org,
        program_b,
        "H2019",
        "Other program",
        "individual",
        30,
        60,
    )
    .await;
    let idle_service = insert_service(
        &pool,
        org,
        program_idle,
        "H2019I",
        "Idle service",
        "individual",
        30,
        60,
    )
    .await;
    let toggle_service = insert_service(
        &pool,
        org,
        program_toggle,
        "H2019T",
        "Toggle service",
        "individual",
        30,
        60,
    )
    .await;
    let (toggle_client, toggle_episode) = {
        let (person, episode) = client(&pool, &actor, &setup["facility"], "Toggle").await;
        link_program(&pool, &episode, program_toggle).await;
        (person, episode)
    };

    let sessions_before = count(&pool, "SELECT count(*) FROM sessions").await;
    let (status, missing_service) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        json!({
            "client_id": one["id"],
            "episode_id": episode_a["id"],
            "clinician_id": actor,
            "kind": "individual",
            "starts_at": "2030-03-01T09:00:00Z",
            "ends_at": "2030-03-01T09:50:00Z"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{missing_service}");
    assert!(
        message(&missing_service).contains("program_service_id"),
        "{missing_service}"
    );
    assert_eq!(
        sessions_before,
        count(&pool, "SELECT count(*) FROM sessions").await
    );

    let (status, wrong_kind) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            individual,
            "2030-03-01T09:00:00Z",
            "2030-03-01T09:50:00Z",
            "assessment",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{wrong_kind}");
    assert!(
        message(&wrong_kind).contains("session kind"),
        "{wrong_kind}"
    );

    let (status, cross_program) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            other_program_service,
            "2030-03-01T09:00:00Z",
            "2030-03-01T09:50:00Z",
            "individual",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{cross_program}");
    assert!(
        message(&cross_program).contains("program"),
        "{cross_program}"
    );

    for (start, end, needle) in [
        (
            "2030-03-01T11:00:00Z",
            "2030-03-01T11:29:00Z",
            "outside the service bounds",
        ),
        (
            "2030-03-01T12:00:00Z",
            "2030-03-01T13:01:00Z",
            "outside the service bounds",
        ),
        (
            "2030-03-01T14:00:00Z",
            "2030-03-01T14:29:59Z",
            "outside the service bounds",
        ),
    ] {
        let (status, err) = send(
            &pool,
            &actor,
            "POST",
            "/api/sessions",
            session_body(
                &one,
                &episode_a,
                &actor,
                individual,
                start,
                end,
                "individual",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
        assert!(message(&err).contains(needle), "{err}");
    }
    assert_eq!(
        sessions_before,
        count(&pool, "SELECT count(*) FROM sessions").await
    );

    let booked = create(
        &pool,
        &actor,
        "sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            individual,
            "2030-03-01T09:00:00Z",
            "2030-03-01T09:50:00Z",
            "individual",
        ),
    )
    .await;
    let (stored_service, stored_snapshot) = session_snapshot(&pool, uid(&booked)).await;
    assert_eq!(stored_service, Some(individual));
    assert_eq!(stored_snapshot["name"], "Individual therapy");
    assert_eq!(stored_snapshot["min_minutes"], 30);
    assert_eq!(stored_snapshot["service_code"], "H2019");
    create(
        &pool,
        &actor,
        "sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            individual,
            "2030-03-02T09:00:00Z",
            "2030-03-02T09:30:00Z",
            "individual",
        ),
    )
    .await;
    create(
        &pool,
        &actor,
        "sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            individual,
            "2030-03-02T11:00:00Z",
            "2030-03-02T12:00:00Z",
            "individual",
        ),
    )
    .await;

    let foreign_org = Uuid::new_v4();
    let foreign_actor = Uuid::new_v4();
    let foreign_facility = Uuid::new_v4();
    let foreign_service = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Other synthetic organization')")
        .bind(foreign_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO facilities(id,organization_id,name,timezone,active) VALUES($1,$2,'Foreign facility','America/New_York',true)")
        .bind(foreign_facility)
        .bind(foreign_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role) VALUES($1,$2,'Foreign admin','foreign-programs@example.invalid','admin')")
        .bind(foreign_actor)
        .bind(foreign_org)
        .execute(&pool)
        .await
        .unwrap();
    let foreign_program =
        insert_program(&pool, foreign_org, foreign_facility, "IOP-X", "Foreign").await;
    sqlx::query("INSERT INTO program_services(id,organization_id,program_id,code_system,service_code,name,session_kind,min_minutes,default_minutes,max_minutes,active) VALUES($1,$2,$3,'hcpcs','H2019','Foreign service','individual',30,30,60,true)")
        .bind(foreign_service)
        .bind(foreign_org)
        .bind(foreign_program)
        .execute(&pool)
        .await
        .unwrap();
    let before_foreign = count(&pool, "SELECT count(*) FROM sessions").await;
    let (status, foreign_booking) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            foreign_service,
            "2030-03-03T09:00:00Z",
            "2030-03-03T09:50:00Z",
            "individual",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{foreign_booking}");
    assert_eq!(
        before_foreign,
        count(&pool, "SELECT count(*) FROM sessions").await
    );
    let (status, foreign_patch) = patch(
        &pool,
        &foreign_actor.to_string(),
        "sessions",
        &booked,
        json!({"status": "cancelled"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{foreign_patch}");
    let (status, foreign_list) = send(
        &pool,
        &foreign_actor.to_string(),
        "GET",
        "/api/sessions",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{foreign_list}");
    assert!(
        foreign_list
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["id"] != booked["id"]),
        "{foreign_list}"
    );

    sqlx::query("UPDATE program_services SET name='Renamed therapy', min_minutes=15, default_minutes=15, max_minutes=15, updated_at=clock_timestamp() WHERE id=$1")
        .bind(individual)
        .execute(&pool)
        .await
        .unwrap();
    let (_, after_catalog_edit) = session_snapshot(&pool, uid(&booked)).await;
    assert_eq!(after_catalog_edit["name"], "Individual therapy");
    assert_eq!(after_catalog_edit["min_minutes"], 30);
    let sessions_after_shrink = count(&pool, "SELECT count(*) FROM sessions").await;
    let (status, shrunk_catalog_booking) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            individual,
            "2030-03-04T09:00:00Z",
            "2030-03-04T09:50:00Z",
            "individual",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{shrunk_catalog_booking}");
    assert!(
        message(&shrunk_catalog_booking).contains("outside the service bounds")
            && message(&shrunk_catalog_booking).contains("15 to 15"),
        "{shrunk_catalog_booking}"
    );
    assert_eq!(
        sessions_after_shrink,
        count(&pool, "SELECT count(*) FROM sessions").await
    );
    let revisions: i64 = sqlx::query_scalar("SELECT count(*) FROM program_service_revisions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(revisions, 0, "Booking must not rewrite catalog revisions");
    let (status, live_bounds) = patch(
        &pool,
        &actor,
        "sessions",
        &booked,
        json!({"starts_at": "2030-03-01T09:00:00Z", "ends_at": "2030-03-01T09:15:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{live_bounds}");
    assert!(
        message(&live_bounds).contains("outside the service bounds"),
        "{live_bounds}"
    );
    let (status, historical_ok) = patch(
        &pool,
        &actor,
        "sessions",
        &booked,
        json!({"starts_at": "2030-03-01T09:00:00Z", "ends_at": "2030-03-01T09:40:00Z", "location": "office 2"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{historical_ok}");
    let (still_service, still_snapshot) = session_snapshot(&pool, uid(&booked)).await;
    assert_eq!(still_service, Some(individual));
    assert_eq!(still_snapshot, after_catalog_edit);

    let past = create(
        &pool,
        &actor,
        "sessions",
        session_body(
            &two,
            &episode_b,
            &actor,
            other_program_service,
            "2024-05-01T09:00:00Z",
            "2024-05-01T09:50:00Z",
            "individual",
        ),
    )
    .await;
    sqlx::query(
        "UPDATE program_services SET active=false, updated_at=clock_timestamp() WHERE id=$1",
    )
    .bind(other_program_service)
    .execute(&pool)
    .await
    .unwrap();
    let (status, completed) = patch(
        &pool,
        &actor,
        "sessions",
        &past,
        json!({"status": "completed"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    let (_, completed_snapshot) = session_snapshot(&pool, uid(&past)).await;
    assert_eq!(completed_snapshot["name"], "Other program");
    let future_retired = create(
        &pool,
        &actor,
        "sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            individual,
            "2030-04-01T09:00:00Z",
            "2030-04-01T09:15:00Z",
            "individual",
        ),
    )
    .await;
    let (_, reserved_snapshot) = session_snapshot(&pool, uid(&future_retired)).await;
    assert_eq!(reserved_snapshot["name"], "Renamed therapy");
    assert_eq!(reserved_snapshot["min_minutes"], 15);
    assert_eq!(reserved_snapshot["max_minutes"], 15);
    assert_ne!(reserved_snapshot["name"], after_catalog_edit["name"]);
    sqlx::query(
        "UPDATE program_services SET active=false, updated_at=clock_timestamp() WHERE id=$1",
    )
    .bind(individual)
    .execute(&pool)
    .await
    .unwrap();
    let (status, retired_move) = patch(
        &pool,
        &actor,
        "sessions",
        &future_retired,
        json!({"starts_at": "2030-04-02T09:00:00Z", "ends_at": "2030-04-02T09:50:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{retired_move}");
    assert!(
        message(&retired_move).contains("inactive"),
        "{retired_move}"
    );
    let reserved_before = count(&pool, "SELECT count(*) FROM sessions").await;
    let (status, retired_new) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        session_body(
            &one,
            &episode_a,
            &actor,
            individual,
            "2030-04-03T09:00:00Z",
            "2030-04-03T09:15:00Z",
            "individual",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{retired_new}");
    assert!(
        message(&retired_new).contains("program service")
            && message(&retired_new).contains("inactive"),
        "{retired_new}"
    );
    assert_eq!(
        reserved_before,
        count(&pool, "SELECT count(*) FROM sessions").await
    );
    let mut retired_form = booking_form_body("sessions", &future_retired);
    assert_ne!(retired_form["starts_at"], future_retired["starts_at"]);
    retired_form["status"] = json!("cancelled");
    let (status, retired_complete) =
        patch(&pool, &actor, "sessions", &future_retired, retired_form).await;
    assert_eq!(status, StatusCode::OK, "{retired_complete}");
    let (status, reopened) = patch(
        &pool,
        &actor,
        "sessions",
        &retired_complete,
        json!({"status":"scheduled"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{reopened}");
    assert!(message(&reopened).contains("inactive"), "{reopened}");
    let preserved_status: String = sqlx::query_scalar("SELECT status FROM sessions WHERE id=$1")
        .bind(uid(&retired_complete))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(preserved_status, "cancelled");
    let (status, removal) = patch(
        &pool,
        &actor,
        "sessions",
        &retired_complete,
        json!({"program_service_id": null}),
    )
    .await;
    assert!(status.is_client_error(), "{removal}");
    let (kept, _) = session_snapshot(&pool, uid(&future_retired)).await;
    assert_eq!(kept, Some(individual));

    let legacy_id = Uuid::new_v4();
    sqlx::query("INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind,status,location,service_snapshot) VALUES($1,$2,$3,$4,$5,'2030-08-01T09:00:00Z','2030-08-01T10:00:00Z','individual','scheduled','office','{}')")
        .bind(legacy_id)
        .bind(org)
        .bind(uid(&three))
        .bind(uid(&legacy_episode))
        .bind(Uuid::parse_str(&actor).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let legacy: Value = sqlx::query_scalar("SELECT to_jsonb(s) FROM sessions s WHERE id=$1")
        .bind(legacy_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, legacy_move) = patch(
        &pool,
        &actor,
        "sessions",
        &legacy,
        json!({"starts_at": "2030-08-01T12:00:00Z", "ends_at": "2030-08-01T13:00:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{legacy_move}");
    assert!(
        message(&legacy_move).contains("program service"),
        "{legacy_move}"
    );
    let mut legacy_form = booking_form_body("sessions", &legacy);
    assert_eq!(legacy_form["program_service_id"], Value::Null);
    assert_ne!(legacy_form["starts_at"], legacy["starts_at"]);
    legacy_form["status"] = json!("cancelled");
    let (status, legacy_status) = patch(&pool, &actor, "sessions", &legacy, legacy_form).await;
    assert_eq!(status, StatusCode::OK, "{legacy_status}");
    let (status, legacy_reopen) = patch(
        &pool,
        &actor,
        "sessions",
        &legacy_status,
        json!({"status":"scheduled"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{legacy_reopen}");
    assert!(
        message(&legacy_reopen).contains("program service"),
        "{legacy_reopen}"
    );
    let legacy_service: Option<Uuid> =
        sqlx::query_scalar("SELECT program_service_id FROM sessions WHERE id=$1")
            .bind(legacy_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(legacy_service, None);

    let policy = create(
        &pool,
        &actor,
        "policies",
        json!({
            "client_id": two["id"],
            "payer_name": "Synthetic payer",
            "member_id": "M-1",
            "group_number": "G-1",
            "effective_date": "2020-01-01",
            "status": "active"
        }),
    )
    .await;
    let claims_before = count(&pool, "SELECT count(*) FROM claims").await;
    let (status, mismatch) = send(
        &pool,
        &actor,
        "POST",
        "/api/claims",
        json!({
            "client_id": two["id"],
            "policy_id": policy["id"],
            "session_id": past["id"],
            "service_code": "NOPE",
            "diagnosis_code": "F99",
            "amount_cents": 1500,
            "units": 1
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{mismatch}");
    assert!(
        message(&mismatch).contains("historical program service code"),
        "{mismatch}"
    );
    assert_eq!(
        claims_before,
        count(&pool, "SELECT count(*) FROM claims").await
    );
    let claim = create(
        &pool,
        &actor,
        "claims",
        json!({
            "client_id": two["id"],
            "policy_id": policy["id"],
            "session_id": past["id"],
            "service_code": " h2019 ",
            "diagnosis_code": "F99",
            "amount_cents": 1500,
            "units": 1
        }),
    )
    .await;
    let (status, claim_edit) = patch(
        &pool,
        &actor,
        "claims",
        &claim,
        json!({"service_code": "OTHER"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{claim_edit}");
    // The catalog accepts case variants, so economic uniqueness must do so too.
    let (status, duplicate_code) = send(
        &pool,
        &actor,
        "POST",
        "/api/claims",
        json!({
            "client_id": two["id"], "policy_id": policy["id"],
            "session_id": past["id"], "service_code": "H2019",
            "diagnosis_code": "F99", "amount_cents": 1500, "units": 1
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{duplicate_code}");
    let legacy_policy = create(
        &pool,
        &actor,
        "policies",
        json!({
            "client_id": three["id"],
            "payer_name": "Synthetic payer",
            "member_id": "M-LEGACY",
            "group_number": "G-LEGACY",
            "effective_date": "2020-01-01",
            "status": "active"
        }),
    )
    .await;
    create(
        &pool,
        &actor,
        "claims",
        json!({
            "client_id": three["id"],
            "policy_id": legacy_policy["id"],
            "session_id": legacy["id"],
            "service_code": "FREEFORM",
            "diagnosis_code": "F99",
            "amount_cents": 100,
            "units": 1
        }),
    )
    .await;

    sqlx::query("UPDATE program_services SET active=true, min_minutes=30, default_minutes=30, max_minutes=60, updated_at=clock_timestamp() WHERE id=$1")
        .bind(grouped)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE program_services SET active=true, updated_at=clock_timestamp() WHERE id=$1",
    )
    .bind(individual)
    .execute(&pool)
    .await
    .unwrap();
    let groups_before_kind = count(&pool, "SELECT count(*) FROM groups").await;
    let (status, individual_group) = send(
        &pool,
        &actor,
        "POST",
        "/api/groups",
        json!({
            "facility_id": facility,
            "clinician_id": actor,
            "program_service_id": individual,
            "title": "Wrong kind group",
            "kind": "skills",
            "starts_at": "2030-05-03T09:00:00Z",
            "ends_at": "2030-05-03T09:15:00Z",
            "capacity": 4,
            "location": "group room"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{individual_group}");
    assert!(
        message(&individual_group).contains("group"),
        "{individual_group}"
    );
    assert_eq!(
        groups_before_kind,
        count(&pool, "SELECT count(*) FROM groups").await
    );
    let (status, cross_facility) = send(
        &pool,
        &actor,
        "POST",
        "/api/groups",
        json!({
            "facility_id": other_facility["id"],
            "clinician_id": actor,
            "program_service_id": grouped,
            "title": "Cross facility",
            "kind": "skills",
            "starts_at": "2030-05-04T09:00:00Z",
            "ends_at": "2030-05-04T09:50:00Z",
            "capacity": 4,
            "location": "group room"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{cross_facility}");
    assert!(
        message(&cross_facility).contains("facility"),
        "{cross_facility}"
    );

    let past_group = create(
        &pool,
        &actor,
        "groups",
        json!({
            "facility_id": facility,
            "clinician_id": actor,
            "program_service_id": grouped,
            "title": "Past group",
            "kind": "skills",
            "starts_at": "2024-08-01T10:00:00Z",
            "ends_at": "2024-08-01T10:50:00Z",
            "capacity": 6,
            "location": "group room"
        }),
    )
    .await;
    let past_attendance = create(
        &pool,
        &actor,
        "group_attendance",
        json!({"group_id": past_group["id"], "client_id": six["id"], "episode_id": past_episode["id"]}),
    )
    .await;
    let past_session = Uuid::parse_str(past_attendance["session_id"].as_str().unwrap()).unwrap();
    let (past_service, past_snapshot) = session_snapshot(&pool, past_session).await;
    assert_eq!(past_service, Some(grouped));
    assert_eq!(past_snapshot["service_code"], "H2019G");

    let future_group = create(
        &pool,
        &actor,
        "groups",
        json!({
            "facility_id": facility,
            "clinician_id": actor,
            "program_service_id": grouped,
            "title": "Future group",
            "kind": "skills",
            "starts_at": "2030-05-01T09:00:00Z",
            "ends_at": "2030-05-01T09:50:00Z",
            "capacity": 6,
            "location": "group room"
        }),
    )
    .await;
    sqlx::query("UPDATE program_services SET name='Group renamed', min_minutes=10, default_minutes=10, max_minutes=10, updated_at=clock_timestamp() WHERE id=$1")
        .bind(grouped)
        .execute(&pool)
        .await
        .unwrap();
    let group_snapshot: Value =
        sqlx::query_scalar("SELECT service_snapshot FROM groups WHERE id=$1")
            .bind(uid(&future_group))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(group_snapshot["name"], "Group skills");
    assert_eq!(group_snapshot["max_minutes"], 60);
    let (status, historical_group) = patch(
        &pool,
        &actor,
        "groups",
        &future_group,
        json!({"starts_at": "2030-05-01T13:00:00Z", "ends_at": "2030-05-01T13:50:00Z", "location": "group room 2"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{historical_group}");
    let moved_snapshot: Value =
        sqlx::query_scalar("SELECT service_snapshot FROM groups WHERE id=$1")
            .bind(uid(&future_group))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(moved_snapshot, group_snapshot);
    let future_group = historical_group;
    let (status, shrunk_new) = send(
        &pool,
        &actor,
        "POST",
        "/api/groups",
        json!({
            "facility_id": facility,
            "clinician_id": actor,
            "program_service_id": grouped,
            "title": "Too long for new bounds",
            "kind": "skills",
            "starts_at": "2030-05-02T09:00:00Z",
            "ends_at": "2030-05-02T09:50:00Z",
            "capacity": 4,
            "location": "group room"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{shrunk_new}");
    assert!(message(&shrunk_new).contains("10 to 10"), "{shrunk_new}");
    let short_group = create(
        &pool,
        &actor,
        "groups",
        json!({
            "facility_id": facility,
            "clinician_id": actor,
            "program_service_id": grouped,
            "title": "Current bounds",
            "kind": "skills",
            "starts_at": "2030-05-02T11:00:00Z",
            "ends_at": "2030-05-02T11:10:00Z",
            "capacity": 4,
            "location": "group room"
        }),
    )
    .await;
    let short_snapshot: Value =
        sqlx::query_scalar("SELECT service_snapshot FROM groups WHERE id=$1")
            .bind(uid(&short_group))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(short_snapshot["name"], "Group renamed");
    assert_eq!(short_snapshot["max_minutes"], 10);

    let enrolled = create(
        &pool,
        &actor,
        "group_attendance",
        json!({"group_id": future_group["id"], "client_id": four["id"], "episode_id": group_episode["id"]}),
    )
    .await;
    let enrolled_session = Uuid::parse_str(enrolled["session_id"].as_str().unwrap()).unwrap();
    let (_, enrolled_snapshot) = session_snapshot(&pool, enrolled_session).await;
    assert_eq!(enrolled_snapshot["name"], "Group skills");
    assert_eq!(enrolled_snapshot["max_minutes"], 60);
    let (status, cross_enroll) = send(
        &pool,
        &actor,
        "POST",
        "/api/group_attendance",
        json!({"group_id": future_group["id"], "client_id": two["id"], "episode_id": episode_b["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{cross_enroll}");
    assert!(message(&cross_enroll).contains("program"), "{cross_enroll}");
    let attendance_before = count(&pool, "SELECT count(*) FROM group_attendance").await;
    let (status, legacy_into_group) = send(
        &pool,
        &actor,
        "POST",
        "/api/group_attendance",
        json!({"group_id": future_group["id"], "client_id": three["id"], "episode_id": legacy_episode["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{legacy_into_group}");
    assert_eq!(
        attendance_before,
        count(&pool, "SELECT count(*) FROM group_attendance").await
    );

    let legacy_group = Uuid::new_v4();
    sqlx::query("INSERT INTO groups(id,organization_id,facility_id,clinician_id,title,kind,starts_at,ends_at,capacity,status,location,service_snapshot) VALUES($1,$2,$3,$4,'Legacy group','skills','2030-06-01T09:00:00Z','2030-06-01T10:00:00Z',4,'scheduled','group room','{}')")
        .bind(legacy_group)
        .bind(org)
        .bind(facility)
        .bind(Uuid::parse_str(&actor).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let legacy_attendance = create(
        &pool,
        &actor,
        "group_attendance",
        json!({"group_id": legacy_group, "client_id": three["id"], "episode_id": legacy_episode["id"]}),
    )
    .await;
    let legacy_child = Uuid::parse_str(legacy_attendance["session_id"].as_str().unwrap()).unwrap();
    let (legacy_child_service, legacy_child_snapshot) = session_snapshot(&pool, legacy_child).await;
    assert_eq!(legacy_child_service, None);
    assert_eq!(legacy_child_snapshot, json!({}));
    let legacy_group_row: Value =
        sqlx::query_scalar("SELECT to_jsonb(g) FROM groups g WHERE id=$1")
            .bind(legacy_group)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut legacy_group_form = booking_form_body("groups", &legacy_group_row);
    legacy_group_form["title"] = json!("Reviewed legacy group");
    let (status, legacy_group_saved) = patch(
        &pool,
        &actor,
        "groups",
        &legacy_group_row,
        legacy_group_form,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{legacy_group_saved}");
    assert_eq!(legacy_group_saved["program_service_id"], Value::Null);
    let mut legacy_group_move = booking_form_body("groups", &legacy_group_saved);
    legacy_group_move["ends_at"] = json!("2030-06-01T10:00:00.001Z");
    let (status, legacy_group_move) = patch(
        &pool,
        &actor,
        "groups",
        &legacy_group_saved,
        legacy_group_move,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{legacy_group_move}");
    let (status, program_into_legacy) = send(
        &pool,
        &actor,
        "POST",
        "/api/group_attendance",
        json!({"group_id": legacy_group, "client_id": five["id"], "episode_id": group_episode_two["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{program_into_legacy}");
    assert!(
        message(&program_into_legacy).contains("no program service"),
        "{program_into_legacy}"
    );

    sqlx::query(
        "UPDATE program_services SET active=false, updated_at=clock_timestamp() WHERE id=$1",
    )
    .bind(grouped)
    .execute(&pool)
    .await
    .unwrap();
    let (status, retired_enroll) = send(
        &pool,
        &actor,
        "POST",
        "/api/group_attendance",
        json!({"group_id": future_group["id"], "client_id": five["id"], "episode_id": group_episode_two["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{retired_enroll}");
    assert!(
        message(&retired_enroll).contains("inactive"),
        "{retired_enroll}"
    );
    let (status, retired_group_move) = patch(
        &pool,
        &actor,
        "groups",
        &future_group,
        json!({"starts_at": "2030-05-01T12:00:00Z", "ends_at": "2030-05-01T12:50:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{retired_group_move}");
    let mut retired_group_form = booking_form_body("groups", &future_group);
    retired_group_form["title"] = json!("Reviewed retired-service group");
    let (status, retired_group_saved) =
        patch(&pool, &actor, "groups", &future_group, retired_group_form).await;
    assert_eq!(status, StatusCode::OK, "{retired_group_saved}");
    assert_eq!(retired_group_saved["service_snapshot"], group_snapshot);
    let mut retired_group_move = booking_form_body("groups", &retired_group_saved);
    retired_group_move["ends_at"] = json!("2030-05-01T13:50:00.001Z");
    let (status, retired_group_move) = patch(
        &pool,
        &actor,
        "groups",
        &retired_group_saved,
        retired_group_move,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{retired_group_move}");
    let (status, attended) = patch(
        &pool,
        &actor,
        "group_attendance",
        &past_attendance,
        json!({"status": "attended"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{attended}");
    let (_, attended_snapshot) = session_snapshot(&pool, past_session).await;
    assert_eq!(attended_snapshot["name"], "Group skills");

    let malformed_id = Uuid::new_v4();
    sqlx::query("INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind,status,location,program_service_id,service_snapshot) VALUES($1,$2,$3,$4,$5,'2030-09-01T09:00:00Z','2030-09-01T09:40:00Z','individual','scheduled','office',$6,jsonb_build_object('id',$6::text))")
        .bind(malformed_id)
        .bind(org)
        .bind(uid(&toggle_client))
        .bind(uid(&toggle_episode))
        .bind(Uuid::parse_str(&actor).unwrap())
        .bind(toggle_service)
        .execute(&pool)
        .await
        .unwrap();
    let malformed: Value = sqlx::query_scalar("SELECT to_jsonb(s) FROM sessions s WHERE id=$1")
        .bind(malformed_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, malformed_move) = patch(
        &pool,
        &actor,
        "sessions",
        &malformed,
        json!({"starts_at": "2030-09-01T12:00:00Z", "ends_at": "2030-09-01T12:40:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{malformed_move}");
    assert!(
        message(&malformed_move).contains("snapshot"),
        "{malformed_move}"
    );
    let (status, malformed_status) = patch(
        &pool,
        &actor,
        "sessions",
        &malformed,
        json!({"status": "cancelled"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{malformed_status}");
    let (malformed_service, malformed_after) = session_snapshot(&pool, malformed_id).await;
    assert_eq!(malformed_service, Some(toggle_service));
    assert!(
        malformed_after.get("service_code").is_none(),
        "{malformed_after}"
    );
    let malformed_policy = create(
        &pool,
        &actor,
        "policies",
        json!({
            "client_id": toggle_client["id"],
            "payer_name": "Synthetic payer",
            "member_id": "M-MALFORMED",
            "group_number": "G-MALFORMED",
            "effective_date": "2020-01-01",
            "status": "active"
        }),
    )
    .await;
    let claims_before_malformed = count(&pool, "SELECT count(*) FROM claims").await;
    let (status, fallback_claim) = send(
        &pool,
        &actor,
        "POST",
        "/api/claims",
        json!({
            "client_id": toggle_client["id"],
            "policy_id": malformed_policy["id"],
            "session_id": malformed["id"],
            "service_code": "H2019T",
            "diagnosis_code": "F99",
            "amount_cents": 100,
            "units": 1
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{fallback_claim}");
    assert!(
        message(&fallback_claim).contains("snapshot")
            || message(&fallback_claim).contains("malformed"),
        "{fallback_claim}"
    );
    assert_eq!(
        claims_before_malformed,
        count(&pool, "SELECT count(*) FROM claims").await
    );
    let malformed_group = Uuid::new_v4();
    sqlx::query("INSERT INTO groups(id,organization_id,facility_id,clinician_id,title,kind,starts_at,ends_at,capacity,status,location,program_service_id,service_snapshot) VALUES($1,$2,$3,$4,'Malformed snapshot group','skills','2030-09-02T09:00:00Z','2030-09-02T09:10:00Z',4,'scheduled','group room',$5,jsonb_build_object('id',$5::text))")
        .bind(malformed_group)
        .bind(org)
        .bind(facility)
        .bind(Uuid::parse_str(&actor).unwrap())
        .bind(grouped)
        .execute(&pool)
        .await
        .unwrap();
    let attendance_malformed = count(&pool, "SELECT count(*) FROM group_attendance").await;
    let (status, malformed_enroll) = send(
        &pool,
        &actor,
        "POST",
        "/api/group_attendance",
        json!({"group_id": malformed_group, "client_id": five["id"], "episode_id": group_episode_two["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{malformed_enroll}");
    assert!(
        message(&malformed_enroll).contains("snapshot"),
        "{malformed_enroll}"
    );
    assert_eq!(
        attendance_malformed,
        count(&pool, "SELECT count(*) FROM group_attendance").await
    );
    sqlx::query("UPDATE facilities SET active=false, updated_at=clock_timestamp() WHERE id=$1")
        .bind(uid(&idle_facility))
        .execute(&pool)
        .await
        .unwrap();
    let (status, idle_booking) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        session_body(
            &seven,
            &idle_episode,
            &actor,
            idle_service,
            "2030-07-01T09:00:00Z",
            "2030-07-01T09:40:00Z",
            "individual",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{idle_booking}");
    assert!(
        message(&idle_booking).contains("inactive"),
        "{idle_booking}"
    );
    sqlx::query("UPDATE care_programs SET active=false, updated_at=clock_timestamp() WHERE id=$1")
        .bind(program_toggle)
        .execute(&pool)
        .await
        .unwrap();
    let inactive_before = count(&pool, "SELECT count(*) FROM sessions").await;
    let (status, inactive_program) = send(
        &pool,
        &actor,
        "POST",
        "/api/sessions",
        session_body(
            &toggle_client,
            &toggle_episode,
            &actor,
            toggle_service,
            "2030-07-02T09:00:00Z",
            "2030-07-02T09:40:00Z",
            "individual",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{inactive_program}");
    assert!(
        message(&inactive_program).contains("care program")
            && message(&inactive_program).contains("inactive"),
        "{inactive_program}"
    );
    assert_eq!(
        inactive_before,
        count(&pool, "SELECT count(*) FROM sessions").await
    );
    sqlx::query("UPDATE care_programs SET active=true, updated_at=clock_timestamp() WHERE id=$1")
        .bind(program_toggle)
        .execute(&pool)
        .await
        .unwrap();
    create(
        &pool,
        &actor,
        "sessions",
        session_body(
            &toggle_client,
            &toggle_episode,
            &actor,
            toggle_service,
            "2030-07-02T09:00:00Z",
            "2030-07-02T09:40:00Z",
            "individual",
        ),
    )
    .await;

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
