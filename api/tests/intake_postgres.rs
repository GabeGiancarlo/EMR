//! Adult intake rule against a unique schema in nexus_test. No live-database writes.
use actix_web::{http::StatusCode, test, web, App};
use chrono::{Datelike, NaiveDate};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

async fn send(
    pool: &PgPool,
    actor: Uuid,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
            .configure(configure),
    )
    .await;
    let mut req = match method {
        "POST" => test::TestRequest::post().uri(uri),
        "PATCH" => test::TestRequest::patch().uri(uri),
        _ => test::TestRequest::get().uri(uri),
    }
    .insert_header(("x-nexus-actor", actor.to_string()));
    if let Some(body) = body {
        req = req
            .insert_header(("content-type", "application/json"))
            .set_json(body);
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let raw = test::read_body(response).await;
    (status, serde_json::from_slice(&raw).unwrap_or(Value::Null))
}

fn message(body: &Value) -> String {
    body["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

async fn staff(pool: &PgPool, org: Uuid, role: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role) VALUES($1,$2,'Synthetic intake operator',$3,$4)")
        .bind(id)
        .bind(org)
        .bind(format!("{id}@example.invalid"))
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn facility(pool: &PgPool, org: Uuid, timezone: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO facilities(id,organization_id,name,timezone) VALUES($1,$2,'Synthetic adult facility',$3)")
        .bind(id)
        .bind(org)
        .bind(timezone)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn facility_today(pool: &PgPool, facility: Uuid) -> NaiveDate {
    sqlx::query_scalar(
        "SELECT (CURRENT_TIMESTAMP AT TIME ZONE timezone)::date FROM facilities WHERE id=$1",
    )
    .bind(facility)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn years_before(date: NaiveDate, years: i32) -> NaiveDate {
    date.with_year(date.year() - years)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(date.year() - years, 3, 1).unwrap())
}

async fn insert_client(
    pool: &PgPool,
    org: Uuid,
    facility: Uuid,
    birth: NaiveDate,
    clinician: Uuid,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date,primary_clinician_id,status) VALUES($1,$2,$3,'Legacy','Minor',$4,$5,'inquiry')")
        .bind(id)
        .bind(org)
        .bind(facility)
        .bind(birth)
        .bind(clinician)
        .execute(pool)
        .await
        .unwrap();
    id
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn adult_program_blocks_minor_intake_and_preserves_legacy_rows() {
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
    let schema = format!("nexus_intake_test_{}", Uuid::new_v4().simple());
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

    let org = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Synthetic adult intake')")
        .bind(org)
        .execute(&pool)
        .await
        .unwrap();
    let admin = staff(&pool, org, "admin").await;
    let ahead = facility(&pool, org, "Pacific/Kiritimati").await;
    let behind = facility(&pool, org, "Etc/GMT+12").await;
    let ahead_today = facility_today(&pool, ahead).await;
    let behind_today = facility_today(&pool, behind).await;
    assert!(
        ahead_today >= behind_today,
        "facility calendars should be ordered, {ahead_today} vs {behind_today}"
    );

    let adult_birth = behind_today
        .with_year(behind_today.year() - 18)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(behind_today.year() - 18, 2, 28).unwrap());
    let minor_birth = years_before(ahead_today, 17);

    let (status, body) = send(
        &pool,
        admin,
        "POST",
        "/api/clients",
        Some(json!({
            "first_name": "New",
            "last_name": "Minor",
            "birth_date": minor_birth,
            "facility_id": ahead,
            "status": "inquiry"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let text = message(&body);
    assert!(text.contains("birth_date"), "{text}");
    assert!(text.to_lowercase().contains("18"), "{text}");

    let (status, created) = send(
        &pool,
        admin,
        "POST",
        "/api/clients",
        Some(json!({
            "first_name": "New",
            "last_name": "Adult",
            "birth_date": adult_birth,
            "facility_id": behind,
            "status": "inquiry"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let adult_id = created["id"].as_str().unwrap();

    // Turning 18 on the facility-local today is eligible; tomorrow is not.
    let turns_today = ahead_today
        .with_year(ahead_today.year() - 18)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(ahead_today.year() - 18, 2, 28).unwrap());
    let (status, body) = send(
        &pool,
        admin,
        "POST",
        "/api/clients",
        Some(json!({
            "first_name": "Turns",
            "last_name": "Today",
            "birth_date": turns_today,
            "facility_id": ahead,
            "status": "intake"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let tomorrow_birth = turns_today.succ_opt().unwrap();
    let (status, body) = send(
        &pool,
        admin,
        "POST",
        "/api/clients",
        Some(json!({
            "first_name": "Turns",
            "last_name": "Tomorrow",
            "birth_date": tomorrow_birth,
            "facility_id": ahead,
            "status": "intake"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("birth_date"));

    let legacy_birth = years_before(ahead_today, 10);
    let legacy = insert_client(&pool, org, ahead, legacy_birth, admin).await;
    let stored: NaiveDate = sqlx::query_scalar("SELECT birth_date FROM clients WHERE id=$1")
        .bind(legacy)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, legacy_birth);

    let updated_at: String = sqlx::query_scalar("SELECT to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') FROM clients WHERE id=$1")
        .bind(legacy)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, corrected) = send(
        &pool,
        admin,
        "PATCH",
        &format!("/api/clients/{legacy}"),
        Some(json!({
            "phone": "555-0100",
            "preferred_name": "Kept",
            "birth_date": legacy_birth,
            "_expected_updated_at": updated_at
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{corrected}");
    assert_eq!(
        corrected["birth_date"],
        json!(legacy_birth.format("%Y-%m-%d").to_string())
    );
    assert_eq!(corrected["phone"], "555-0100");

    let updated_at = corrected["updated_at"].as_str().unwrap();
    let (status, rejected_admission) = send(
        &pool,
        admin,
        "PATCH",
        &format!("/api/clients/{legacy}"),
        Some(json!({"status":"intake", "_expected_updated_at": updated_at})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected_admission}");
    assert!(message(&rejected_admission).contains("18"));
    let (status, rejected) = send(
        &pool,
        admin,
        "PATCH",
        &format!("/api/clients/{legacy}"),
        Some(json!({
            "birth_date": minor_birth,
            "_expected_updated_at": updated_at
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
    assert!(message(&rejected).contains("birth_date"));
    let still: NaiveDate = sqlx::query_scalar("SELECT birth_date FROM clients WHERE id=$1")
        .bind(legacy)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        still, legacy_birth,
        "rejected birth-date change must not rewrite the stored date"
    );

    let (status, episode) = send(
        &pool,
        admin,
        "POST",
        "/api/episodes",
        Some(json!({
            "client_id": legacy,
            "facility_id": ahead,
            "level_of_care": "iop",
            "start_date": ahead_today,
            "status": "planned"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{episode}");
    let episode_message = message(&episode);
    assert!(
        episode_message.contains("birth_date") || episode_message.to_lowercase().contains("18"),
        "{episode_message}"
    );

    let (status, episode) = send(
        &pool,
        admin,
        "POST",
        "/api/episodes",
        Some(json!({
            "client_id": adult_id,
            "facility_id": behind,
            "level_of_care": "outpatient",
            "start_date": behind_today,
            "status": "planned"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{episode}");
    let episode_id = episode["id"].as_str().unwrap();
    let episode_updated = episode["updated_at"].as_str().unwrap();
    let (status, activated) = send(
        &pool,
        admin,
        "PATCH",
        &format!("/api/episodes/{episode_id}"),
        Some(json!({
            "status": "active",
            "_expected_updated_at": episode_updated
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activated}");

    // Legacy planned episode remains, but activation is rejected and the row is unchanged.
    let planned = Uuid::new_v4();
    sqlx::query("INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date,status) VALUES($1,$2,$3,$4,'php',$5,'planned')")
        .bind(planned)
        .bind(org)
        .bind(legacy)
        .bind(ahead)
        .bind(ahead_today)
        .execute(&pool)
        .await
        .unwrap();
    // Use the API-visible timestamp so the concurrency token matches.
    let visible: Value = sqlx::query_scalar("SELECT to_jsonb(episodes) FROM episodes WHERE id=$1")
        .bind(planned)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, blocked) = send(
        &pool,
        admin,
        "PATCH",
        &format!("/api/episodes/{planned}"),
        Some(json!({
            "status": "active",
            "_expected_updated_at": visible["updated_at"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{blocked}");
    let status_now: String = sqlx::query_scalar("SELECT status FROM episodes WHERE id=$1")
        .bind(planned)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_now, "planned");

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM clients WHERE organization_id=$1 AND birth_date=$2",
    )
    .bind(org)
    .bind(legacy_birth)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "legacy minor row must be preserved");

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
