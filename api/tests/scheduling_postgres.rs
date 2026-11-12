//! Scheduling invariants exercised through HTTP against an isolated PostgreSQL schema.
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
            .app_data(web::Data::new(AppState::local(pool.clone())))
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
fn session(client: &Value, episode: &Value, clinician: &Value, start: &str, end: &str) -> Value {
    json!({"program_service_id":episode["fixture_service_id"],"client_id":client["id"],"episode_id":episode["id"],"clinician_id":clinician["id"],"kind":"individual","starts_at":start,"ends_at":end})
}
async fn client(
    pool: &PgPool,
    actor: &str,
    facility: &Value,
    clinician: &Value,
    name: &str,
) -> (Value, Value) {
    let c=create(pool,actor,"clients",json!({"first_name":"Synthetic","last_name":name,"birth_date":"1990-01-01","facility_id":facility,"primary_clinician_id":clinician["id"]})).await;
    let e=support::legacy_episode(pool,actor,json!({"client_id":c["id"],"facility_id":facility,"level_of_care":"iop","start_date":"2026-01-01"})).await;
    let program: Value =
        sqlx::query_scalar("SELECT to_jsonb(p) FROM care_programs p WHERE facility_id=$1")
            .bind(Uuid::parse_str(facility.as_str().unwrap()).unwrap())
            .fetch_one(pool)
            .await
            .unwrap();
    let mut e = support::link_fixture_episode(pool, &e, &program).await;
    e["fixture_service_id"]=sqlx::query_scalar("SELECT to_jsonb(id) FROM program_services WHERE program_id=$1 AND session_kind='individual'").bind(Uuid::parse_str(program["id"].as_str().unwrap()).unwrap()).fetch_one(pool).await.unwrap();
    (c, e)
}

#[actix_web::test]
#[ignore = "Requires disposable nexus_test; creates only a unique test schema"]
async fn bookings_groups_availability_and_transfers_are_atomic() {
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
    let schema = format!("nexus_schedule_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let path = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(8)
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
    let (status,setup)=send(&pool,"","POST","/api/setup",json!({"organization_name":"Synthetic scheduling","facility_name":"Synthetic facility","admin_name":"Synthetic admin","admin_email":"schedule@example.invalid"})).await;
    assert_eq!(status, StatusCode::CREATED, "{setup}");
    let actor = setup["actor"]["id"].as_str().unwrap();
    let org = Uuid::parse_str(setup["actor"]["organization_id"].as_str().unwrap()).unwrap();
    let facility = &setup["facility"]["id"];
    let (_program, _individual, group_service) =
        support::scheduling_catalog(&pool, actor, facility).await;
    let a=create(&pool,actor,"staff",json!({"name":"Clinician A","email":"a@example.invalid","role":"clinician","facility_id":facility})).await;
    let b=create(&pool,actor,"staff",json!({"name":"Clinician B","email":"b@example.invalid","role":"clinician","facility_id":facility})).await;
    let (one, e1) = client(&pool, actor, facility, &a, "One").await;
    let (two, e2) = client(&pool, actor, facility, &a, "Two").await;
    let (three, e3) = client(&pool, actor, facility, &b, "Three").await;

    let booked = create(
        &pool,
        actor,
        "sessions",
        session(
            &one,
            &e1,
            &a,
            "2030-01-07T09:00:00Z",
            "2030-01-07T10:00:00Z",
        ),
    )
    .await;
    for candidate in [
        session(
            &two,
            &e2,
            &a,
            "2030-01-07T09:30:00Z",
            "2030-01-07T10:30:00Z",
        ),
        session(
            &one,
            &e1,
            &b,
            "2030-01-07T09:30:00Z",
            "2030-01-07T10:30:00Z",
        ),
    ] {
        let (status, error) = send(&pool, actor, "POST", "/api/sessions", candidate).await;
        assert_eq!(status, StatusCode::CONFLICT, "{error}");
        assert!(
            !error.to_string().contains("Synthetic"),
            "Conflicts must not expose other records"
        );
    }
    create(
        &pool,
        actor,
        "sessions",
        session(
            &one,
            &e1,
            &a,
            "2030-01-07T10:00:00Z",
            "2030-01-07T11:00:00Z",
        ),
    )
    .await;
    let (status, _) = patch(
        &pool,
        actor,
        "sessions",
        &booked,
        json!({"status":"cancelled"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    create(
        &pool,
        actor,
        "sessions",
        session(
            &two,
            &e2,
            &a,
            "2030-01-07T09:00:00Z",
            "2030-01-07T10:00:00Z",
        ),
    )
    .await;
    // Concurrent HTTP writers must not both observe a free slot.
    let (left, right) = tokio::join!(
        send(
            &pool,
            actor,
            "POST",
            "/api/sessions",
            session(
                &one,
                &e1,
                &a,
                "2030-01-08T09:00:00Z",
                "2030-01-08T10:00:00Z"
            )
        ),
        send(
            &pool,
            actor,
            "POST",
            "/api/sessions",
            session(
                &two,
                &e2,
                &a,
                "2030-01-08T09:00:00Z",
                "2030-01-08T10:00:00Z"
            )
        )
    );
    assert!(
        (left.0 == StatusCode::CREATED && right.0 == StatusCode::CONFLICT)
            || (right.0 == StatusCode::CREATED && left.0 == StatusCode::CONFLICT),
        "{left:?} {right:?}"
    );

    let group=create(&pool,actor,"groups",json!({"facility_id":facility,"clinician_id":a["id"],"title":"Synthetic group","program_service_id":group_service["id"],"kind":"skills","starts_at":"2030-01-09T09:00:00Z","ends_at":"2030-01-09T10:00:00Z","capacity":4})).await;
    let (status, _) = send(
        &pool,
        actor,
        "POST",
        "/api/sessions",
        session(
            &three,
            &e3,
            &a,
            "2030-01-09T09:00:00Z",
            "2030-01-09T10:00:00Z",
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "An empty group reserves its facilitator"
    );
    let g1 = create(
        &pool,
        actor,
        "group_attendance",
        json!({"group_id":group["id"],"client_id":one["id"],"episode_id":e1["id"]}),
    )
    .await;
    create(
        &pool,
        actor,
        "group_attendance",
        json!({"group_id":group["id"],"client_id":two["id"],"episode_id":e2["id"]}),
    )
    .await;
    let other = create(
        &pool,
        actor,
        "sessions",
        session(
            &three,
            &e3,
            &b,
            "2030-01-09T09:00:00Z",
            "2030-01-09T10:00:00Z",
        ),
    )
    .await;
    let (status, _) = send(
        &pool,
        actor,
        "POST",
        "/api/group_attendance",
        json!({"group_id":group["id"],"client_id":three["id"],"episode_id":e3["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE group_id=$1")
        .bind(Uuid::parse_str(group["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "Failed enrollment must not leave a child session");
    create(
        &pool,
        actor,
        "sessions",
        session(
            &two,
            &e2,
            &b,
            "2030-01-09T11:00:00Z",
            "2030-01-09T12:00:00Z",
        ),
    )
    .await;
    let (status, _) = patch(
        &pool,
        actor,
        "groups",
        &group,
        json!({"starts_at":"2030-01-09T11:00:00Z","ends_at":"2030-01-09T12:00:00Z"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "Rescheduling must check every participant"
    );
    let unchanged:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM sessions WHERE group_id=$1 AND starts_at<>'2030-01-09T09:00:00Z'::timestamptz) AND EXISTS(SELECT 1 FROM groups WHERE id=$1 AND starts_at='2030-01-09T09:00:00Z'::timestamptz)").bind(Uuid::parse_str(group["id"].as_str().unwrap()).unwrap()).fetch_one(&pool).await.unwrap();
    assert!(unchanged, "Parent and all children must stay unchanged");
    let (status, excused) = patch(
        &pool,
        actor,
        "group_attendance",
        &g1,
        json!({"status":"excused"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{excused}");
    let (status, _) = patch(
        &pool,
        actor,
        "sessions",
        &other,
        json!({"status":"cancelled"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    create(
        &pool,
        actor,
        "sessions",
        session(
            &one,
            &e1,
            &b,
            "2030-01-09T09:00:00Z",
            "2030-01-09T10:00:00Z",
        ),
    )
    .await;
    let (status, _) = patch(
        &pool,
        actor,
        "group_attendance",
        &excused,
        json!({"status":"scheduled"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "Restoring attendance must check conflicts"
    );

    let a_id = a["id"].as_str().unwrap();
    let block_body = json!({"staff_id":a["id"],"starts_at":"2030-01-10T09:00:00Z","ends_at":"2030-01-10T10:00:00Z","reason":"Synthetic training"});
    let block = create(&pool, a_id, "scheduling/unavailability", block_body.clone()).await;
    let (status, _) = send(
        &pool,
        actor,
        "POST",
        "/api/sessions",
        session(
            &one,
            &e1,
            &a,
            "2030-01-10T09:00:00Z",
            "2030-01-10T10:00:00Z",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status,_)=send(&pool,a_id,"POST","/api/scheduling/unavailability",json!({"staff_id":b["id"],"starts_at":"2030-01-10T09:00:00Z","ends_at":"2030-01-10T10:00:00Z","reason":"Not authorized"})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, visible) = send(
        &pool,
        b["id"].as_str().unwrap(),
        "GET",
        "/api/scheduling/unavailability",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(visible[0]["reason"], "Unavailable");
    let (status, cancelled) = patch(
        &pool,
        actor,
        "scheduling/unavailability",
        &block,
        json!({"active":false}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    let (status, _) = patch(
        &pool,
        actor,
        "scheduling/unavailability",
        &block,
        json!({"reason":"Stale edit"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, reactivated) = patch(
        &pool,
        actor,
        "scheduling/unavailability",
        &cancelled,
        json!({"active":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reactivated}");
    let (status, _) = patch(
        &pool,
        actor,
        "scheduling/unavailability",
        &reactivated,
        json!({"ends_at":"2030-01-10T08:00:00Z"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status,_)=send(&pool,actor,"POST","/api/scheduling/unavailability",json!({"staff_id":a["id"],"starts_at":"2030-01-07T10:00:00Z","ends_at":"2030-01-07T11:00:00Z","reason":"Over care"})).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Offboarding cannot create conflicts by bulk reassignment; all earlier writes roll back.
    let departing=create(&pool,actor,"staff",json!({"name":"Departing clinician","email":"departing@example.invalid","role":"clinician","facility_id":facility})).await;
    let (four, e4) = client(&pool, actor, facility, &departing, "Four").await;
    let transfer_session = create(
        &pool,
        actor,
        "sessions",
        session(
            &four,
            &e4,
            &departing,
            "2030-01-10T09:00:00Z",
            "2030-01-10T10:00:00Z",
        ),
    )
    .await;
    let transfer_path = format!("/api/staff/{}/offboard", departing["id"].as_str().unwrap());
    let (status, _) = send(
        &pool,
        actor,
        "POST",
        &transfer_path,
        json!({"transfer_to":a["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let still_active: bool = sqlx::query_scalar("SELECT active FROM staff WHERE id=$1")
        .bind(Uuid::parse_str(departing["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(still_active);
    let assigned: Uuid = sqlx::query_scalar("SELECT clinician_id FROM sessions WHERE id=$1")
        .bind(Uuid::parse_str(transfer_session["id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(assigned.to_string(), departing["id"].as_str().unwrap());
    let (status, _) = send(
        &pool,
        actor,
        "POST",
        &transfer_path,
        json!({"transfer_to":b["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Exceptions are checked against the actual time after waiting, not transaction start.
    let executive = create(
        &pool,
        actor,
        "staff",
        json!({"name":"Synthetic executive","email":"exec@example.invalid","role":"executive"}),
    )
    .await;
    let executive_id = executive["id"].as_str().unwrap();
    let exception = Uuid::new_v4();
    sqlx::query("INSERT INTO exceptions(id,organization_id,staff_id,resource,reason,expires_at,access_mode) VALUES($1,$2,$3,'sessions','Synthetic scheduling exception',clock_timestamp()+interval '1 hour','write')").bind(exception).bind(org).bind(Uuid::parse_str(executive_id).unwrap()).execute(&pool).await.unwrap();
    let mut expiry_lock = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,9182410))")
        .bind(org.to_string())
        .execute(&mut *expiry_lock)
        .await
        .unwrap();
    let waiting = send(
        &pool,
        executive_id,
        "POST",
        "/api/sessions",
        session(
            &three,
            &e3,
            &b,
            "2030-01-20T09:00:00Z",
            "2030-01-20T10:00:00Z",
        ),
    );
    tokio::pin!(waiting);
    tokio::select! { result=&mut waiting=>panic!("Mutation escaped organization lock: {result:?}"), _=tokio::time::sleep(std::time::Duration::from_millis(100))=>{} }
    sqlx::query("UPDATE exceptions SET expires_at=clock_timestamp() WHERE id=$1")
        .bind(exception)
        .execute(&mut *expiry_lock)
        .await
        .unwrap();
    expiry_lock.commit().await.unwrap();
    assert_eq!(
        waiting.await.0,
        StatusCode::FORBIDDEN,
        "Expired exceptions must not survive a lock wait"
    );

    let (status, _) = send(
        &pool,
        actor,
        "PATCH",
        &format!(
            "/api/scheduling/unavailability/{}",
            block["id"].as_str().unwrap()
        ),
        json!({"active":false}),
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    let (booking, time_away) = tokio::join!(
        send(
            &pool,
            actor,
            "POST",
            "/api/sessions",
            session(
                &three,
                &e3,
                &b,
                "2030-01-21T09:00:00Z",
                "2030-01-21T10:00:00Z"
            )
        ),
        send(
            &pool,
            actor,
            "POST",
            "/api/scheduling/unavailability",
            json!({"staff_id":b["id"],"starts_at":"2030-01-21T09:00:00Z","ends_at":"2030-01-21T10:00:00Z","reason":"Concurrent time away"})
        )
    );
    assert!(
        (booking.0 == StatusCode::CREATED && time_away.0 == StatusCode::CONFLICT)
            || (time_away.0 == StatusCode::CREATED && booking.0 == StatusCode::CONFLICT),
        "{booking:?} {time_away:?}"
    );
    let mut missed = session(
        &three,
        &e3,
        &b,
        "2026-01-15T09:00:00Z",
        "2026-01-15T10:00:00Z",
    );
    missed["status"] = json!("no_show");
    create(&pool, actor, "sessions", missed).await;
    create(
        &pool,
        actor,
        "sessions",
        session(
            &two,
            &e2,
            &b,
            "2026-01-15T09:00:00Z",
            "2026-01-15T10:00:00Z",
        ),
    )
    .await;

    // An operator who is deactivated while waiting for the organization lock loses write access.
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,9182410))")
        .bind(org.to_string())
        .execute(&mut *held)
        .await
        .unwrap();
    let pending = send(
        &pool,
        a_id,
        "POST",
        "/api/scheduling/unavailability",
        json!({"staff_id":a["id"],"starts_at":"2030-01-11T09:00:00Z","ends_at":"2030-01-11T10:00:00Z","reason":"Waiting operator"}),
    );
    tokio::pin!(pending);
    tokio::select! { result=&mut pending=>panic!("Mutation escaped organization lock: {result:?}"), _=tokio::time::sleep(std::time::Duration::from_millis(100))=>{} }
    sqlx::query("UPDATE staff SET active=false WHERE id=$1")
        .bind(Uuid::parse_str(a_id).unwrap())
        .execute(&mut *held)
        .await
        .unwrap();
    held.commit().await.unwrap();
    assert_eq!(pending.await.0, StatusCode::FORBIDDEN);
    let (status, _) = patch(
        &pool,
        actor,
        "scheduling/unavailability",
        &reactivated,
        json!({"active":false}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "An administrator can cancel an inactive staff member's block"
    );

    // Another organization's operator cannot discover or modify these blocks.
    let foreign_org = Uuid::new_v4();
    let foreign_actor = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,'Other synthetic organization')")
        .bind(foreign_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff(id,organization_id,name,email,role) VALUES($1,$2,'Foreign admin','foreign@example.invalid','admin')").bind(foreign_actor).bind(foreign_org).execute(&pool).await.unwrap();
    let (status, rows) = send(
        &pool,
        &foreign_actor.to_string(),
        "GET",
        "/api/scheduling/unavailability",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows, json!([]));
    let (status, _) = patch(
        &pool,
        &foreign_actor.to_string(),
        "scheduling/unavailability",
        &reactivated,
        json!({"active":false}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
