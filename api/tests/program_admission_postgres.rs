//! Program admission, immutable decision evidence, capacity races and discharge.
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
    actor: &str,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
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
    if !actor.is_empty() {
        req = req.insert_header(("x-nexus-actor", actor));
    }
    let res = test::call_service(&app, req.to_request()).await;
    let status = res.status();
    let bytes = test::read_body(res).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn post(pool: &PgPool, actor: &str, path: &str, body: Value) -> Value {
    let (status, row) = send(pool, actor, Method::POST, path, body).await;
    assert!(status.is_success(), "{path}: {status} {row}");
    row
}
async fn patch(pool: &PgPool, actor: &str, path: &str, row: &Value, mut body: Value) -> Value {
    body["_expected_updated_at"] = row["updated_at"].clone();
    let (status, row) = send(pool, actor, Method::PATCH, path, body).await;
    assert!(status.is_success(), "{path}: {status} {row}");
    row
}
async fn transition(pool: &PgPool, actor: &str, case: &Value, to: &str) -> Value {
    post(pool,actor,&format!("/api/intake-cases/{}/transition",case["id"].as_str().unwrap()),json!({"to_status":to,"reason":"Synthetic reviewed program decision","attested":true,"_expected_updated_at":case["updated_at"]})).await
}
async fn signed_form(
    pool: &PgPool,
    actor: &str,
    client: &Value,
    episode: &Value,
    template: &Value,
) -> Value {
    let row=post(pool,actor,"/api/form_responses",json!({"client_id":client["id"],"episode_id":episode,"template_id":template["id"],"answers":{"review":"Synthetic reviewed evidence"},"source_type":"manual","source_excerpt":"","field_provenance":{}})).await;
    for action in ["review", "sign"] {
        post(
            pool,
            actor,
            &format!(
                "/api/form_responses/{}/{action}",
                row["id"].as_str().unwrap()
            ),
            json!({}),
        )
        .await;
    }
    row
}
async fn intake(pool: &PgPool, actor: &str, client: &Value, program: &Value, today: &str) -> Value {
    post(pool,actor,&format!("/api/clients/{}/intake",client["id"].as_str().unwrap()),json!({"program_id":program["id"],"starts_on":today,"received_on":today,"owner_id":actor,"referral_type":"self","referrer_name":"","referrer_contact":"","referral_reason":"Synthetic program admission review"})).await
}
async fn prepare(
    pool: &PgPool,
    actor: &str,
    client: &Value,
    program: &Value,
    template: &Value,
    today: &str,
) -> Value {
    let case = intake(pool, actor, client, program, today).await;
    post(pool,actor,&format!("/api/clients/{}/identity-checks",client["id"].as_str().unwrap()),json!({"method":"patient_report","outcome":"confirmed","reason":"Synthetic patient identity reviewed","attested":true})).await;
    post(pool,actor,"/api/consents",json!({"client_id":client["id"],"kind":"treatment","granted_on":today,"scope":"Synthetic program care"})).await;
    signed_form(pool, actor, client, &case["episode_id"], template).await;
    let screening = transition(pool, actor, &case, "screening").await;
    transition(pool, actor, &screening, "ready").await
}
#[actix_web::test]
#[ignore = "Requires isolated nexus_test schema"]
async fn program_decisions_capacity_and_exit_forms_are_transactional() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let schema = format!("nexus_program_admission_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let search = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(8)
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
    let setup=post(&pool,"","/api/setup",json!({"organization_name":"Synthetic program decisions","facility_name":"Synthetic East","admin_name":"Synthetic admin","admin_email":"program-admin@example.invalid"})).await;
    let actor = setup["actor"]["id"].as_str().unwrap();
    let facility = &setup["facility"]["id"];
    let today: String = sqlx::query_scalar(
        "SELECT ((CURRENT_TIMESTAMP AT TIME ZONE 'America/New_York')::date)::text",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let second_facility = post(
        &pool,
        actor,
        "/api/facilities",
        json!({"name":"Synthetic West","timezone":"America/Los_Angeles","active":true}),
    )
    .await;
    let template=post(&pool,actor,"/api/templates",json!({"name":"Synthetic program intake","kind":"assessment","fields":[{"key":"review","label":"Review","type":"textarea","required":true}],"applies_to_stage":"active","applies_to_care_level":"php","required_for_stage":false})).await;
    let exit_template=post(&pool,actor,"/api/templates",json!({"name":"Synthetic program exit","kind":"discharge","fields":[{"key":"review","label":"Review","type":"textarea","required":true}],"required_for_stage":false})).await;
    let mut program=post(&pool,actor,"/api/care-programs",json!({"facility_id":facility,"code":"IOP-21","name":"Synthetic IOP","level_of_care":"iop","min_age":21,"max_age":65,"capacity":1,"active":true,"required_intake_template_ids":[template["id"]],"required_discharge_template_ids":[exit_template["id"]],"change_reason":"Configure the synthetic program"})).await;
    let second_program=post(&pool,actor,"/api/care-programs",json!({"facility_id":second_facility["id"],"code":"WEST","name":"Synthetic West IOP","level_of_care":"iop","min_age":18,"max_age":null,"capacity":5,"active":true,"required_intake_template_ids":[],"required_discharge_template_ids":[],"change_reason":"Configure the second facility program"})).await;
    let mut clients = Vec::new();
    for (name, years) in [
        ("Below configured range", 19),
        ("Above configured range", 70),
    ] {
        let birth: String = sqlx::query_scalar(
            "SELECT (($1::text::date - make_interval(years => $2))::date)::text",
        )
        .bind(&today)
        .bind(years)
        .fetch_one(&pool)
        .await
        .unwrap();
        let patient=post(&pool,actor,"/api/clients",json!({"facility_id":facility,"first_name":"Synthetic age","last_name":name,"birth_date":birth,"primary_clinician_id":actor})).await;
        let rejected=send(&pool,actor,Method::POST,&format!("/api/clients/{}/intake",patient["id"].as_str().unwrap()),json!({"program_id":program["id"],"starts_on":today,"received_on":today,"owner_id":actor,"referral_type":"self","referrer_name":"","referrer_contact":"","referral_reason":"Configured program range applies"})).await;
        assert_eq!(rejected.0, StatusCode::BAD_REQUEST, "{rejected:?}");
    }
    for name in ["One", "Two", "Historical", "Adopted"] {
        clients.push(post(&pool,actor,"/api/clients",json!({"facility_id":facility,"first_name":"Synthetic","last_name":name,"birth_date":"1990-06-15","primary_clinician_id":actor})).await);
    }
    assert_eq!(send(&pool,actor,Method::POST,"/api/episodes",json!({"client_id":clients[0]["id"],"facility_id":facility,"level_of_care":"iop","start_date":today})).await.0,StatusCode::CONFLICT);
    let wrong_context=send(&pool,actor,Method::POST,&format!("/api/clients/{}/intake",clients[0]["id"].as_str().unwrap()),json!({"program_id":program["id"],"facility_id":second_facility["id"],"starts_on":today,"received_on":today,"owner_id":actor,"referral_type":"self","referrer_name":"","referrer_contact":"","referral_reason":"Wrong facility must not silently route"})).await;
    assert_eq!(wrong_context.0, StatusCode::BAD_REQUEST);
    let ready_one = prepare(&pool, actor, &clients[0], &program, &template, &today).await;
    let ready_two = prepare(&pool, actor, &clients[1], &program, &template, &today).await;
    assert_eq!(ready_one["decision_evidence"][0]["program_version"], 1);
    assert_eq!(
        ready_one["decision_evidence"][0]["program_snapshot"]["capacity"],
        1
    );
    let stored = ready_one["decision_evidence"].clone();
    program=patch(&pool,actor,&format!("/api/care-programs/{}",program["id"].as_str().unwrap()),&program,json!({"name":"Synthetic IOP revised","change_reason":"Rename while preserving prior decisions"})).await;
    let left_path = format!(
        "/api/intake-cases/{}/transition",
        ready_one["id"].as_str().unwrap()
    );
    let right_path = format!(
        "/api/intake-cases/{}/transition",
        ready_two["id"].as_str().unwrap()
    );
    let (left, right) = tokio::join!(
        send(
            &pool,
            actor,
            Method::POST,
            &left_path,
            json!({"to_status":"admitted","reason":"First concurrent capacity decision","attested":true,"_expected_updated_at":ready_one["updated_at"]})
        ),
        send(
            &pool,
            actor,
            Method::POST,
            &right_path,
            json!({"to_status":"admitted","reason":"Second concurrent capacity decision","attested":true,"_expected_updated_at":ready_two["updated_at"]})
        )
    );
    assert_eq!(
        [left.0, right.0].iter().filter(|s| s.is_success()).count(),
        1,
        "{left:?} {right:?}"
    );
    assert_eq!(
        [left.0, right.0]
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1
    );
    let (winner, loser, winner_client, loser_client) = if left.0.is_success() {
        (left.1, ready_two.clone(), &clients[0], &clients[1])
    } else {
        (right.1, ready_one.clone(), &clients[1], &clients[0])
    };
    assert_eq!(winner["decision_evidence"][1]["program_version"], 2);
    let (_, history) = send(
        &pool,
        actor,
        Method::GET,
        &format!("/api/clients/{}/intake", clients[0]["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(history["cases"][0]["decision_evidence"][0], stored[0]);
    assert!(
        sqlx::query("UPDATE intake_decision_evidence SET checklist='[]'")
            .execute(&pool)
            .await
            .is_err()
    );
    let episode = winner["episode_id"].as_str().unwrap();
    let discharge_path = format!("/api/episodes/{episode}/discharge");
    let discharge =
        json!({"end_date":today,"discharge_reason":"Synthetic continuing care handoff reviewed"});
    assert_eq!(
        send(
            &pool,
            actor,
            Method::POST,
            &discharge_path,
            discharge.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, rules) = send(
        &pool,
        actor,
        Method::GET,
        &format!(
            "/api/clients/{}/form-requirements?episode_id={episode}&stage=discharge",
            winner_client["id"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert!(rules["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["template_id"] == exit_template["id"] && r["completion"] == "needed"));
    let inactive = patch(
        &pool,
        actor,
        &format!("/api/templates/{}", exit_template["id"].as_str().unwrap()),
        &exit_template,
        json!({"active":false}),
    )
    .await;
    let (_, rules) = send(
        &pool,
        actor,
        Method::GET,
        &format!(
            "/api/clients/{}/form-requirements?episode_id={episode}&stage=discharge",
            winner_client["id"].as_str().unwrap()
        ),
        Value::Null,
    )
    .await;
    assert!(rules["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["template_id"] == exit_template["id"]
            && r["completion"] == "configuration_review"));
    patch(
        &pool,
        actor,
        &format!("/api/templates/{}", exit_template["id"].as_str().unwrap()),
        &inactive,
        json!({"active":true}),
    )
    .await;
    program = patch(
        &pool,
        actor,
        &format!("/api/care-programs/{}", program["id"].as_str().unwrap()),
        &program,
        json!({"active":false,"change_reason":"Pause new admissions during review"}),
    )
    .await;
    signed_form(
        &pool,
        actor,
        winner_client,
        &winner["episode_id"],
        &exit_template,
    )
    .await;
    post(&pool, actor, &discharge_path, discharge).await;
    let blocked=send(&pool,actor,Method::POST,&format!("/api/intake-cases/{}/transition",loser["id"].as_str().unwrap()),json!({"to_status":"admitted","reason":"Retired programs block new admission","attested":true,"_expected_updated_at":loser["updated_at"]})).await;
    assert_eq!(blocked.0, StatusCode::CONFLICT);
    patch(
        &pool,
        actor,
        &format!("/api/care-programs/{}", program["id"].as_str().unwrap()),
        &program,
        json!({"active":true,"change_reason":"Resume the configured admission program"}),
    )
    .await;
    let admitted = transition(&pool, actor, &loser, "admitted").await;
    assert_eq!(admitted["client_id"], loser_client["id"]);
    let old=support::legacy_episode(&pool,actor,json!({"client_id":clients[2]["id"],"facility_id":second_facility["id"],"level_of_care":"iop","start_date":"2025-01-01","status":"active"})).await;
    let assigned=post(&pool,actor,&format!("/api/episodes/{}/assign-program",old["id"].as_str().unwrap()),json!({"program_id":second_program["id"],"reason":"Reviewed existing active course of care","attested":true,"_expected_updated_at":old["updated_at"]})).await;
    for key in ["id", "start_date", "status", "client_id"] {
        assert_eq!(assigned[key], old[key]);
    }
    assert_eq!(assigned["program_id"], second_program["id"]);
    let planned=support::legacy_episode(&pool,actor,json!({"client_id":clients[3]["id"],"facility_id":second_facility["id"],"level_of_care":"iop","start_date":"2025-01-01","status":"planned"})).await;
    let adopted=post(&pool,actor,&format!("/api/clients/{}/intake",clients[3]["id"].as_str().unwrap()),json!({"program_id":second_program["id"],"legacy_episode_id":planned["id"],"starts_on":"2025-01-01","received_on":today,"owner_id":actor,"referral_type":"self","referrer_name":"","referrer_contact":"","referral_reason":"x"})).await;
    assert_eq!(adopted["episode_id"], planned["id"]);
    let declined = transition(&pool, actor, &adopted, "declined").await;
    assert_eq!(declined["status"], "declined");
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM episode_program_assignments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(used, 2);
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
