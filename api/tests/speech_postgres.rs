//! HTTP consent/tenant checks without downloading a recognizer or storing audio.
//! Real recognizer acceptance is a separate, explicit local synthetic-audio check.
use actix_web::{http::StatusCode, test, web, App};
use emr_api::{configure, migrate, AppState};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

fn wav() -> Vec<u8> {
    let data_len = 320u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&16000u32.to_le_bytes());
    bytes.extend_from_slice(&32000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(44 + data_len as usize, 0);
    bytes
}

async fn send(
    pool: &PgPool,
    actor: Option<Uuid>,
    client: Uuid,
    consent: Uuid,
    bytes: Vec<u8>,
) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let mut req = test::TestRequest::post()
        .uri("/api/speech/transcribe")
        .insert_header(("content-type", "audio/wav"))
        .insert_header(("x-nexus-client", client.to_string()))
        .insert_header(("x-nexus-consent", consent.to_string()));
    if let Some(actor) = actor {
        req = req.insert_header(("x-nexus-actor", actor.to_string()));
    }
    let response = test::call_service(&app, req.set_payload(bytes).to_request()).await;
    let status = response.status();
    let body = test::read_body(response).await;
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn staff(pool: &PgPool, org: Uuid, role: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role) VALUES($1,$2,'Synthetic speech test operator',$3,$4)")
        .bind(id).bind(org).bind(format!("{id}@example.invalid")).bind(role).execute(pool).await.unwrap();
    id
}

async fn consent(pool: &PgPool, org: Uuid, client: Uuid, kind: &str, window: &str) -> Uuid {
    let id = Uuid::new_v4();
    let (start, end, revoked) = match window {
        "expired" => (-10, -1, None),
        "future" => (1, 10, None),
        "revoked" => (-10, 10, Some(0)),
        _ => (-1, 1, None),
    };
    // Consent windows are facility-calendar dates, including after UTC midnight.
    sqlx::query("INSERT INTO consents(id,organization_id,client_id,kind,scope,granted_on,expires_on,revoked_on) SELECT $1,$2,$3,$4,'Synthetic local speech verification only',today + $5::int,today + $6::int,CASE WHEN $7::int IS NULL THEN NULL ELSE today + $7::int END FROM (SELECT (CURRENT_TIMESTAMP AT TIME ZONE f.timezone)::date AS today FROM clients c JOIN facilities f ON f.id=c.facility_id AND f.organization_id=c.organization_id WHERE c.id=$3 AND c.organization_id=$2) calendar")
        .bind(id).bind(org).bind(client).bind(kind).bind(start).bind(end).bind(revoked).execute(pool).await.unwrap();
    id
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn transcription_rejects_unconsented_and_unscoped_requests() {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(database, "nexus_test");
    let schema = format!("nexus_speech_test_{}", Uuid::new_v4().simple());
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
    let other_org = Uuid::new_v4();
    for id in [org, other_org] {
        sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Synthetic speech test')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let admin = staff(&pool, org, "admin").await;
    let clinician = staff(&pool, org, "clinician").await;
    let unassigned = staff(&pool, org, "clinician").await;
    let billing = staff(&pool, org, "billing").await;
    let foreign_admin = staff(&pool, other_org, "admin").await;
    let facility = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO facilities(id,organization_id,name) VALUES($1,$2,'Synthetic speech facility')",
    )
    .bind(facility)
    .bind(org)
    .execute(&pool)
    .await
    .unwrap();
    let client = Uuid::new_v4();
    let another_client = Uuid::new_v4();
    for id in [client, another_client] {
        sqlx::query("INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date,primary_clinician_id) VALUES($1,$2,$3,'Synthetic','Speech','1990-01-01',$4)")
            .bind(id).bind(org).bind(facility).bind(clinician).execute(&pool).await.unwrap();
    }
    let valid = consent(&pool, org, client, "recording", "valid").await;
    for actor in [None, Some(billing), Some(unassigned), Some(foreign_admin)] {
        let (status, body) = send(&pool, actor, client, valid, wav()).await;
        assert!(
            matches!(status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
            "{status}: {body}"
        );
    }
    for (kind, window) in [
        ("recording", "expired"),
        ("recording", "future"),
        ("recording", "revoked"),
        ("release", "valid"),
    ] {
        let invalid = consent(&pool, org, client, kind, window).await;
        let (status, body) = send(&pool, Some(admin), client, invalid, wav()).await;
        assert!(
            matches!(status, StatusCode::FORBIDDEN | StatusCode::BAD_REQUEST),
            "{kind}/{window}: {status}: {body}"
        );
    }
    let mismatched = consent(&pool, org, another_client, "recording", "valid").await;
    let (status, body) = send(&pool, Some(admin), client, mismatched, wav()).await;
    assert!(
        matches!(
            status,
            StatusCode::FORBIDDEN | StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
        ),
        "{status}: {body}"
    );
    let (status, body) = send(&pool, Some(clinician), client, valid, b"not a WAV".to_vec()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // Consent dates follow the facility calendar, including dates across UTC midnight.
    // Invalid audio is intentional: reaching WAV validation proves authorization passed.
    for timezone in ["Pacific/Kiritimati", "Etc/GMT+12"] {
        sqlx::query("UPDATE facilities SET timezone=$1 WHERE id=$2")
            .bind(timezone)
            .bind(facility)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE consents SET granted_on=(CURRENT_TIMESTAMP AT TIME ZONE $1)::date, expires_on=(CURRENT_TIMESTAMP AT TIME ZONE $1)::date WHERE id=$2")
            .bind(timezone).bind(valid).execute(&pool).await.unwrap();
        let (status, body) =
            send(&pool, Some(clinician), client, valid, b"not a WAV".to_vec()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{timezone}: {body}");
    }
    sqlx::query(
        "UPDATE consents SET granted_on=CURRENT_DATE-1, expires_on=CURRENT_DATE+1 WHERE id=$1",
    )
    .bind(valid)
    .execute(&pool)
    .await
    .unwrap();

    // Concurrent revocation must be observed after its lock is released, rather
    // than using the previously committed consent snapshot.
    let mut revocation = pool.begin().await.unwrap();
    sqlx::query("UPDATE consents SET revoked_on=CURRENT_DATE WHERE id=$1")
        .bind(valid)
        .execute(&mut *revocation)
        .await
        .unwrap();
    let finished = std::sync::atomic::AtomicBool::new(false);
    let (result, ()) = tokio::join!(
        async {
            let result = send(&pool, Some(clinician), client, valid, wav()).await;
            finished.store(true, std::sync::atomic::Ordering::SeqCst);
            result
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            assert!(
                !finished.load(std::sync::atomic::Ordering::SeqCst),
                "Authorization must wait for consent revocation"
            );
            revocation.commit().await.unwrap();
        }
    );
    assert_eq!(result.0, StatusCode::FORBIDDEN, "{}", result.1);
    sqlx::query("UPDATE consents SET revoked_on=NULL WHERE id=$1")
        .bind(valid)
        .execute(&pool)
        .await
        .unwrap();

    let mut offboarding = pool.begin().await.unwrap();
    sqlx::query("UPDATE staff SET active=false WHERE id=$1")
        .bind(clinician)
        .execute(&mut *offboarding)
        .await
        .unwrap();
    finished.store(false, std::sync::atomic::Ordering::SeqCst);
    let (result, ()) = tokio::join!(
        async {
            let result = send(&pool, Some(clinician), client, valid, wav()).await;
            finished.store(true, std::sync::atomic::Ordering::SeqCst);
            result
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            assert!(
                !finished.load(std::sync::atomic::Ordering::SeqCst),
                "Authorization must wait for operator deactivation"
            );
            offboarding.commit().await.unwrap();
        }
    );
    assert_eq!(result.0, StatusCode::FORBIDDEN, "{}", result.1);
    sqlx::query("UPDATE staff SET active=true WHERE id=$1")
        .bind(clinician)
        .execute(&pool)
        .await
        .unwrap();
    // A configured recognizer is exercised separately with actual speech. The ordinary
    // automated test checks that an absent adapter is explicit rather than fabricated.
    if std::env::var_os("NEXUS_WHISPER_CLI").is_none()
        && std::env::var_os("NEXUS_WHISPER_MODEL").is_none()
    {
        let (status, body) = send(&pool, Some(clinician), client, valid, wav()).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    }
    let completed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE action='transcribe'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        completed, 0,
        "Rejected requests must not record completed transcription"
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
