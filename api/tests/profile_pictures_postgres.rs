//! Profile pictures against a disposable isolated schema: who may set and see them, what
//! is stored, what is refused, and that the client record itself never changes.
use actix_web::{
    http::{Method, StatusCode},
    test, web, App,
};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

/// One request with any body. Returns the status, the raw body and the content type.
async fn call(
    pool: &PgPool,
    actor: &str,
    method: Method,
    path: &str,
    content_type: &str,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>, String) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let mut request = test::TestRequest::default()
        .method(method)
        .uri(path)
        .insert_header(("x-nexus-actor", actor));
    if !content_type.is_empty() {
        request = request.insert_header(("content-type", content_type));
    }
    let response = test::call_service(&app, request.set_payload(body).to_request()).await;
    let status = response.status();
    let kind = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    (status, test::read_body(response).await.to_vec(), kind)
}
async fn json_call(
    pool: &PgPool,
    actor: &str,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let (status, bytes, _) = call(
        pool,
        actor,
        method,
        path,
        "application/json",
        serde_json::to_vec(&body).unwrap(),
    )
    .await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn create(pool: &PgPool, actor: &str, path: &str, body: Value) -> Value {
    let (status, value) = json_call(pool, actor, Method::POST, path, body).await;
    assert!(status.is_success(), "{path} {status}: {value}");
    value
}
async fn avatars(pool: &PgPool, actor: &str) -> Value {
    let (status, bytes, _) = call(pool, actor, Method::GET, "/api/bootstrap", "", vec![]).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_slice::<Value>(&bytes).unwrap()["avatars"].clone()
}
/// The start of a PNG: enough for the API, which reads the header and stores the bytes.
fn png(width: u32, height: u32, salt: u8) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, salt]);
    bytes
}

#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn pictures_follow_the_person_they_belong_to() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(actual, "nexus_test");
    let schema = format!("nexus_pictures_test_{}", Uuid::new_v4().simple());
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

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let setup = test::call_service(&app, test::TestRequest::post().uri("/api/setup").set_json(json!({"organization_name":"Synthetic picture QA","facility_name":"Synthetic adult facility","admin_name":"Synthetic administrator","admin_email":"pictures-admin@example.invalid"})).to_request()).await;
    assert_eq!(setup.status(), StatusCode::CREATED);
    let setup: Value = test::read_body_json(setup).await;
    let admin = setup["actor"]["id"].as_str().unwrap().to_owned();
    let facility = setup["facility"]["id"].clone();
    let mut staff = std::collections::HashMap::new();
    for (name, role) in [
        ("assigned", "clinician"),
        ("other", "clinician"),
        ("billing", "billing"),
        ("support", "support"),
    ] {
        let row = create(&pool, &admin, "/api/staff", json!({"name":format!("Synthetic {name}"),"email":format!("pictures-{name}@example.invalid"),"role":role,"facility_id":facility})).await;
        staff.insert(name, row["id"].as_str().unwrap().to_owned());
    }
    let client = create(&pool, &admin, "/api/clients", json!({"first_name":"Robin","last_name":"Sample","birth_date":"1988-04-12","facility_id":facility,"primary_clinician_id":staff["assigned"]})).await;
    let client_id = client["id"].as_str().unwrap().to_owned();
    let picture = format!("/api/profile-pictures/clients/{client_id}");

    // Nothing stored: the automatic default has no file and no entry.
    assert_eq!(
        call(&pool, &admin, Method::GET, &picture, "", vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(avatars(&pool, &admin).await, json!({}));

    // A photo is stored exactly as sent and served as what it is, never cached.
    let first = png(256, 256, 1);
    let (status, body, _) = call(
        &pool,
        &support_of(&staff),
        Method::PUT,
        &picture,
        "image/png",
        first.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let stored: Value = serde_json::from_slice(&body).unwrap();
    let tag = stored["avatar"].as_str().unwrap().to_owned();
    assert!(tag.starts_with("photo:") && tag.len() == 22, "{tag}");
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let served = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&picture)
            .insert_header(("x-nexus-actor", admin.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(served.status(), StatusCode::OK);
    assert_eq!(served.headers().get("content-type").unwrap(), "image/png");
    assert_eq!(
        served.headers().get("cache-control").unwrap(),
        "private, no-store"
    );
    assert_eq!(
        served.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(test::read_body(served).await.to_vec(), first);
    assert_eq!(avatars(&pool, &admin).await[&client_id], json!(tag));

    // A new photo has a new tag, so a browser holding the old one knows to fetch again.
    let (status, body, _) = call(
        &pool,
        &admin,
        Method::PUT,
        &picture,
        "image/png",
        png(256, 256, 2),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let second: Value = serde_json::from_slice(&body).unwrap();
    assert_ne!(second["avatar"], json!(tag));

    // Clinician assignment scope applies to the picture as it does to the chart.
    assert_eq!(
        call(&pool, &staff["assigned"], Method::GET, &picture, "", vec![])
            .await
            .0,
        StatusCode::OK
    );
    assert!(avatars(&pool, &staff["assigned"])
        .await
        .get(&client_id)
        .is_some());
    assert_eq!(
        call(&pool, &staff["other"], Method::GET, &picture, "", vec![])
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &pool,
            &staff["other"],
            Method::PUT,
            &picture,
            "image/png",
            png(64, 64, 3)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert!(avatars(&pool, &staff["other"])
        .await
        .get(&client_id)
        .is_none());

    // Billing may see clients, and so their pictures, but may not change either.
    assert_eq!(
        call(&pool, &staff["billing"], Method::GET, &picture, "", vec![])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &pool,
            &staff["billing"],
            Method::PUT,
            &picture,
            "image/png",
            png(64, 64, 4)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        json_call(
            &pool,
            &staff["billing"],
            Method::PUT,
            &picture,
            json!({"motif":"fern"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // Choosing a Nexus default replaces the photo; the automatic default removes the row.
    let (status, chosen) = json_call(
        &pool,
        &staff["assigned"],
        Method::PUT,
        &picture,
        json!({"motif":"fern"}),
    )
    .await;
    assert_eq!(
        (status, chosen["avatar"].as_str()),
        (StatusCode::OK, Some("fern"))
    );
    assert_eq!(
        call(&pool, &admin, Method::GET, &picture, "", vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(avatars(&pool, &admin).await[&client_id], json!("fern"));
    let (status, cleared) =
        json_call(&pool, &admin, Method::PUT, &picture, json!({"motif":null})).await;
    assert_eq!(
        (status, cleared["avatar"].as_str()),
        (StatusCode::OK, Some(""))
    );
    assert_eq!(avatars(&pool, &admin).await, json!({}));
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM profile_pictures")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);

    // What is refused: unknown defaults, script-capable or mislabelled files, huge bitmaps, big files.
    for (kind, body) in [
        ("application/json", br#"{"motif":"dragon"}"#.to_vec()),
        ("application/json", br#"{"style":"fern"}"#.to_vec()),
        ("application/json", b"not json".to_vec()),
        (
            "image/svg+xml",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>".to_vec(),
        ),
        ("image/jpeg", png(256, 256, 5)),
        ("image/gif", b"GIF89a\x01\0\x01\0\0\0\0".to_vec()),
        ("", png(256, 256, 6)),
        ("image/png", png(20_000, 20_000, 7)),
    ] {
        let (status, reply, _) = call(&pool, &admin, Method::PUT, &picture, kind, body).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{kind}: {}",
            String::from_utf8_lossy(&reply)
        );
    }
    let mut heavy = png(256, 256, 8);
    heavy.resize(512 * 1024 + 1, 0);
    assert_eq!(
        call(&pool, &admin, Method::PUT, &picture, "image/png", heavy)
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(avatars(&pool, &admin).await, json!({}));

    // Staff: your own picture is yours to set; someone else's needs an administrator.
    let own = format!("/api/profile-pictures/staff/{}", staff["billing"]);
    assert_eq!(
        call(
            &pool,
            &staff["billing"],
            Method::PUT,
            &own,
            "image/png",
            png(128, 128, 9)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        json_call(
            &pool,
            &staff["billing"],
            Method::PUT,
            &format!("/api/profile-pictures/staff/{admin}"),
            json!({"motif":"pine"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        json_call(&pool, &admin, Method::PUT, &own, json!({"motif":"river"}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        avatars(&pool, &staff["support"]).await[&staff["billing"]],
        json!("river")
    );

    // Another organization's people do not exist here, for reading or for writing.
    let (elsewhere, outsider) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Synthetic other organization')")
        .bind(elsewhere)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role) VALUES($1,$2,'Synthetic outsider','pictures-outsider@example.invalid','admin')")
        .bind(outsider)
        .bind(elsewhere)
        .execute(&pool)
        .await
        .unwrap();
    let foreign = format!("/api/profile-pictures/staff/{outsider}");
    assert_eq!(
        json_call(&pool, &admin, Method::PUT, &foreign, json!({"motif":"sun"}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&pool, &admin, Method::GET, &foreign, "", vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_call(
            &pool,
            &outsider.to_string(),
            Method::PUT,
            &picture,
            json!({"motif":"sun"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert!(avatars(&pool, &outsider.to_string())
        .await
        .get(&staff["billing"])
        .is_none());

    // Addresses that are not a picture.
    assert_eq!(
        call(
            &pool,
            &admin,
            Method::GET,
            &format!(
                "/api/profile-pictures/facilities/{}",
                facility.as_str().unwrap()
            ),
            "",
            vec![]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &pool,
            &admin,
            Method::GET,
            &format!("/api/profile-pictures/clients/{}", Uuid::new_v4()),
            "",
            vec![]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_call(
            &pool,
            &admin,
            Method::POST,
            &picture,
            json!({"motif":"fern"})
        )
        .await
        .0,
        StatusCode::METHOD_NOT_ALLOWED
    );

    // Every change is in the audit trail, and none of them touched the client record.
    let audited: Vec<(String, i64)> = sqlx::query_as("SELECT action, count(*) FROM audit_events WHERE action IN ('set_profile_photo','choose_profile_default','clear_profile_picture') GROUP BY action ORDER BY action")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(
        audited,
        vec![
            ("choose_profile_default".to_owned(), 2),
            ("clear_profile_picture".to_owned(), 1),
            ("set_profile_photo".to_owned(), 3)
        ]
    );
    let unchanged: String =
        sqlx::query_scalar("SELECT to_jsonb(c)->>'updated_at' FROM clients c WHERE id=$1")
            .bind(Uuid::parse_str(&client_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    let before = emr_api_timestamp(client["updated_at"].as_str().unwrap());
    assert_eq!(emr_api_timestamp(&unchanged), before);

    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
}

fn support_of(staff: &std::collections::HashMap<&str, String>) -> String {
    staff["support"].clone()
}

/// Timestamps print with different precision and offsets; compare the instant.
fn emr_api_timestamp(text: &str) -> i128 {
    time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .or_else(|_| {
            time::OffsetDateTime::parse(
                &text.replace(' ', "T"),
                &time::format_description::well_known::Rfc3339,
            )
        })
        .unwrap_or_else(|_| panic!("unreadable timestamp {text}"))
        .unix_timestamp_nanos()
}
