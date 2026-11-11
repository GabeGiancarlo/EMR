//! Form applicability contract tests against real PostgreSQL.
//! Synthetic fixtures exist only in a uniquely named schema inside nexus_test.
mod support;
use actix_web::{http::StatusCode, test, web, App};
use emr_api::{configure, migrate, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

async fn call(
    pool: &PgPool,
    method: &str,
    path: &str,
    actor: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let mut req = match method {
        "POST" => test::TestRequest::post(),
        "PATCH" => test::TestRequest::patch(),
        _ => test::TestRequest::get(),
    }
    .uri(path);
    if let Some(actor) = actor {
        req = req.insert_header(("x-nexus-actor", actor));
    }
    if let Some(body) = body {
        req = req.set_json(body);
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    let parsed = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"raw": String::from_utf8_lossy(&bytes).into_owned()}))
    };
    (status, parsed)
}

fn assert_status(actual: StatusCode, expected: StatusCode, body: &Value) {
    assert_eq!(actual, expected, "{body}");
}

fn template_body(name: &str, extra: Value) -> Value {
    let mut body = json!({
        "name": name,
        "kind": "assessment",
        "description": "Organization-authored applicability test",
        "fields": [{"key": "marker", "label": "Marker", "type": "text", "required": false}],
        "active": true
    });
    if let (Some(object), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            object.insert(key.clone(), value.clone());
        }
    }
    body
}

fn requirement<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["template_name"] == name)
        .unwrap_or_else(|| panic!("missing {name} in {body}"))
}

fn has_rule(body: &Value, name: &str) -> bool {
    body["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["template_name"] == name)
}

#[actix_web::test]
#[ignore = "Requires TEST_DATABASE_URL ending in /nexus_test; creates and removes only a unique test schema"]
async fn form_requirements_follow_authored_rules_scope_and_preserve_snapshots() {
    let url = std::env::var("TEST_DATABASE_URL").expect("Set TEST_DATABASE_URL");
    assert!(
        url.ends_with("/nexus_test"),
        "Refusing fixtures outside nexus_test"
    );
    let control = PgPool::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(database, "nexus_test");
    let schema = format!("nexus_form_rule_test_{}", Uuid::new_v4().simple());
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

    let (status, setup) = call(
        &pool,
        "POST",
        "/api/setup",
        None,
        Some(json!({
            "organization_name": "Applicability test organization",
            "facility_name": "Applicability test facility",
            "admin_name": "Applicability admin",
            "admin_email": "admin@example.invalid"
        })),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &setup);
    let admin = setup["actor"]["id"].as_str().unwrap().to_string();
    let facility = setup["facility"]["id"].as_str().unwrap().to_string();
    let (status, facility_row) = call(
        &pool,
        "PATCH",
        &format!("/api/facilities/{facility}"),
        Some(&admin),
        Some(json!({"timezone": "America/New_York", "_expected_updated_at": setup["facility"]["updated_at"]})),
    )
    .await;
    assert_status(status, StatusCode::OK, &facility_row);

    async fn staff(pool: &PgPool, admin: &str, facility: &str, role: &str, name: &str) -> String {
        let (status, row) = call(
            pool,
            "POST",
            "/api/staff",
            Some(admin),
            Some(json!({
                "name": name,
                "email": format!("{name}@example.invalid").replace(' ', "-"),
                "role": role,
                "facility_id": facility
            })),
        )
        .await;
        assert_status(status, StatusCode::CREATED, &row);
        row["id"].as_str().unwrap().to_string()
    }
    let clinician = staff(&pool, &admin, &facility, "clinician", "Assigned clinician").await;
    let other = staff(&pool, &admin, &facility, "clinician", "Other clinician").await;
    let lead = staff(&pool, &admin, &facility, "clinical_lead", "Clinical lead").await;
    let billing = staff(&pool, &admin, &facility, "billing", "Billing actor").await;

    for body in [
        template_body("Bad stage", json!({"applies_to_stage": "inquiry"})),
        template_body("Bad care", json!({"applies_to_care_level": "residential"})),
        template_body("Bad flag", json!({"required_for_stage": "true"})),
    ] {
        let (status, row) = call(&pool, "POST", "/api/templates", Some(&admin), Some(body)).await;
        assert_status(status, StatusCode::BAD_REQUEST, &row);
    }

    let (status, alpha) = call(
        &pool,
        "POST",
        "/api/templates",
        Some(&admin),
        Some(template_body("Alpha suggested", json!({}))),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &alpha);
    assert_eq!(alpha["applies_to_stage"].as_str(), Some("any"));
    assert_eq!(alpha["applies_to_care_level"].as_str(), Some("any"));
    assert_eq!(alpha["required_for_stage"].as_bool(), Some(false));

    async fn make_template(pool: &PgPool, admin: &str, name: &str, extra: Value) -> Value {
        let (status, row) = call(
            pool,
            "POST",
            "/api/templates",
            Some(admin),
            Some(template_body(name, extra)),
        )
        .await;
        assert_status(status, StatusCode::CREATED, &row);
        row
    }
    let zebra = make_template(
        &pool,
        &admin,
        "Zebra intake",
        json!({"applies_to_stage": "intake", "applies_to_care_level": "any", "required_for_stage": true}),
    )
    .await;
    let any_packet = make_template(
        &pool,
        &admin,
        "Any packet",
        json!({"applies_to_stage": "any", "applies_to_care_level": "any", "required_for_stage": true}),
    )
    .await;
    let _iop = make_template(
        &pool,
        &admin,
        "IOP rule",
        json!({"applies_to_stage": "any", "applies_to_care_level": "iop", "required_for_stage": false}),
    )
    .await;
    let _php = make_template(
        &pool,
        &admin,
        "PHP rule",
        json!({"applies_to_stage": "any", "applies_to_care_level": "php", "required_for_stage": true}),
    )
    .await;
    let _inactive = make_template(
        &pool,
        &admin,
        "Inactive rule",
        json!({"active": false, "applies_to_stage": "intake", "applies_to_care_level": "any", "required_for_stage": true}),
    )
    .await;
    let _discharge = make_template(
        &pool,
        &admin,
        "Discharge rule",
        json!({"applies_to_stage": "discharged", "applies_to_care_level": "any", "required_for_stage": true}),
    )
    .await;

    let (status, client) = call(
        &pool,
        "POST",
        "/api/clients",
        Some(&admin),
        Some(json!({
            "first_name": "Rule",
            "last_name": "Subject",
            "birth_date": "1990-01-01",
            "facility_id": facility,
            "status": "intake",
            "primary_clinician_id": clinician
        })),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &client);
    let client_id = client["id"].as_str().unwrap().to_string();
    let (status, other_client) = call(
        &pool,
        "POST",
        "/api/clients",
        Some(&admin),
        Some(json!({
            "first_name": "Other",
            "last_name": "Subject",
            "birth_date": "1991-01-01",
            "facility_id": facility,
            "status": "intake",
            "primary_clinician_id": other
        })),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &other_client);
    let other_client_id = other_client["id"].as_str().unwrap();

    let path = format!("/api/clients/{client_id}/form-requirements");
    for actor in [None, Some(billing.as_str()), Some(other.as_str())] {
        let (status, body) = call(&pool, "GET", &path, actor, None).await;
        assert_status(status, StatusCode::FORBIDDEN, &body);
    }
    let (status, missing) = call(
        &pool,
        "GET",
        &format!("/api/clients/{}/form-requirements", Uuid::new_v4()),
        Some(&admin),
        None,
    )
    .await;
    assert_status(status, StatusCode::NOT_FOUND, &missing);
    let (status, denied_other) = call(
        &pool,
        "GET",
        &format!("/api/clients/{other_client_id}/form-requirements"),
        Some(&clinician),
        None,
    )
    .await;
    assert_status(status, StatusCode::FORBIDDEN, &denied_other);

    let (status, rules) = call(&pool, "GET", &path, Some(&clinician), None).await;
    assert_status(status, StatusCode::OK, &rules);
    let (lead_status, lead_rules) = call(&pool, "GET", &path, Some(&lead), None).await;
    assert_status(lead_status, StatusCode::OK, &lead_rules);
    assert_eq!(rules["relevant_episode_id"], Value::Null);
    assert_eq!(rules["relevant_stage"], "intake");
    assert_eq!(rules["relevant_care_level"], Value::Null);
    assert!(rules["disclaimer"]
        .as_str()
        .unwrap()
        .contains("does not infer legal or clinical requirements"));
    assert_eq!(
        rules["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["template_name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Any packet", "Zebra intake", "Alpha suggested"]
    );
    assert_eq!(requirement(&rules, "Any packet")["disposition"], "required");
    assert_eq!(requirement(&rules, "Any packet")["completion"], "needed");
    assert_eq!(
        requirement(&rules, "Alpha suggested")["disposition"],
        "suggested"
    );
    assert!(!has_rule(&rules, "IOP rule"));
    assert!(!has_rule(&rules, "PHP rule"));
    assert!(!has_rule(&rules, "Inactive rule"));
    assert!(!has_rule(&rules, "Discharge rule"));
    assert!(requirement(&rules, "Zebra intake")["reason"]
        .as_str()
        .unwrap()
        .ends_with("Based on your organization's configuration."));

    let (status, response) = call(
        &pool,
        "POST",
        "/api/form_responses",
        Some(&admin),
        Some(json!({
            "client_id": client_id,
            "template_id": zebra["id"],
            "answers": {"marker": "recorded"},
            "source_type": "manual",
            "source_excerpt": "",
            "field_provenance": {}
        })),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &response);
    let response_id = response["id"].as_str().unwrap().to_string();
    let snapshot_before = response["template_snapshot"].clone();
    let (status, patched) = call(
        &pool,
        "PATCH",
        &format!("/api/templates/{}", zebra["id"].as_str().unwrap()),
        Some(&admin),
        Some(json!({
            "_expected_updated_at": zebra["updated_at"],
            "applies_to_stage": "discharged",
            "applies_to_care_level": "php",
            "required_for_stage": true
        })),
    )
    .await;
    assert_status(status, StatusCode::OK, &patched);
    assert_eq!(patched["applies_to_stage"], "discharged");
    let (status, listed) = call(&pool, "GET", "/api/form_responses", Some(&admin), None).await;
    assert_status(status, StatusCode::OK, &listed);
    let stored = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == response_id)
        .unwrap();
    assert_eq!(stored["template_snapshot"], snapshot_before);
    let (status, after_patch) = call(&pool, "GET", &path, Some(&admin), None).await;
    assert_status(status, StatusCode::OK, &after_patch);
    assert!(!has_rule(&after_patch, "Zebra intake"));

    let (status, signed_response) = call(
        &pool,
        "POST",
        "/api/form_responses",
        Some(&admin),
        Some(json!({
            "client_id": client_id,
            "template_id": any_packet["id"],
            "answers": {"marker": "recorded"},
            "source_type": "manual",
            "source_excerpt": "",
            "field_provenance": {}
        })),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &signed_response);
    let any_id = signed_response["id"].as_str().unwrap();
    for action in ["review", "sign"] {
        let (status, row) = call(
            &pool,
            "POST",
            &format!("/api/form_responses/{any_id}/{action}"),
            Some(&admin),
            Some(json!({})),
        )
        .await;
        assert_status(status, StatusCode::OK, &row);
    }
    let (status, signed_rules) = call(&pool, "GET", &path, Some(&clinician), None).await;
    assert_status(status, StatusCode::OK, &signed_rules);
    assert_eq!(
        requirement(&signed_rules, "Any packet")["completion"],
        "signed"
    );

    let iop=support::legacy_episode(&pool,&admin,json!({"client_id":client_id,"facility_id":facility,"level_of_care":"iop","start_date":"2020-01-01","status":"active"})).await;
    let iop_id = iop["id"].as_str().unwrap().to_string();
    let (status, with_episode) = call(&pool, "GET", &path, Some(&admin), None).await;
    assert_status(status, StatusCode::OK, &with_episode);
    assert_eq!(with_episode["relevant_episode_id"], iop_id);
    assert_eq!(with_episode["relevant_stage"], "active");
    assert_eq!(with_episode["relevant_care_level"], "iop");
    assert_eq!(
        requirement(&with_episode, "Any packet")["completion"],
        "needed"
    );
    assert_eq!(
        requirement(&with_episode, "Any packet")["episode_id"],
        iop_id
    );
    assert!(has_rule(&with_episode, "IOP rule"));
    assert!(!has_rule(&with_episode, "PHP rule"));
    assert_eq!(
        requirement(&with_episode, "IOP rule")["disposition"],
        "suggested"
    );

    let (status, iop_draft) = call(
        &pool,
        "POST",
        "/api/form_responses",
        Some(&admin),
        Some(json!({
            "client_id": client_id,
            "episode_id": iop_id,
            "template_id": any_packet["id"],
            "answers": {"marker": "episode"},
            "source_type": "manual",
            "source_excerpt": "",
            "field_provenance": {}
        })),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &iop_draft);
    let (status, drafted) = call(&pool, "GET", &path, Some(&admin), None).await;
    assert_status(status, StatusCode::OK, &drafted);
    assert_eq!(requirement(&drafted, "Any packet")["completion"], "draft");

    let (status, discharged_iop) = call(
        &pool,
        "POST",
        &format!("/api/episodes/{iop_id}/discharge"),
        Some(&admin),
        Some(json!({"end_date": "2020-02-01", "discharge_reason": "Synthetic applicability discharge"})),
    )
    .await;
    assert_status(status, StatusCode::OK, &discharged_iop);
    let php=support::legacy_episode(&pool,&admin,json!({"client_id":client_id,"facility_id":facility,"level_of_care":"php","start_date":"2020-03-01","status":"active"})).await;
    let php_id = php["id"].as_str().unwrap().to_string();
    let (status, php_rules) = call(&pool, "GET", &path, Some(&admin), None).await;
    assert_status(status, StatusCode::OK, &php_rules);
    assert_eq!(php_rules["relevant_episode_id"], php_id);
    assert_eq!(php_rules["relevant_care_level"], "php");
    assert!(has_rule(&php_rules, "PHP rule"));
    assert!(!has_rule(&php_rules, "IOP rule"));
    assert_eq!(
        requirement(&php_rules, "Any packet")["completion"],
        "needed"
    );

    let (status, php_response) = call(
        &pool,
        "POST",
        "/api/form_responses",
        Some(&admin),
        Some(json!({
            "client_id": client_id,
            "episode_id": php_id,
            "template_id": any_packet["id"],
            "answers": {"marker": "php"},
            "source_type": "manual",
            "source_excerpt": "",
            "field_provenance": {}
        })),
    )
    .await;
    assert_status(status, StatusCode::CREATED, &php_response);
    let php_response_id = php_response["id"].as_str().unwrap();
    for action in ["review", "sign"] {
        let (status, row) = call(
            &pool,
            "POST",
            &format!("/api/form_responses/{php_response_id}/{action}"),
            Some(&admin),
            Some(json!({})),
        )
        .await;
        assert_status(status, StatusCode::OK, &row);
    }
    let (status, php_signed) = call(&pool, "GET", &path, Some(&admin), None).await;
    assert_status(status, StatusCode::OK, &php_signed);
    assert_eq!(
        requirement(&php_signed, "Any packet")["completion"],
        "signed"
    );

    let (status, discharged_php) = call(
        &pool,
        "POST",
        &format!("/api/episodes/{php_id}/discharge"),
        Some(&admin),
        Some(json!({"end_date": "2020-04-01", "discharge_reason": "Synthetic later discharge"})),
    )
    .await;
    assert_status(status, StatusCode::OK, &discharged_php);
    let (status, final_rules) = call(&pool, "GET", &path, Some(&clinician), None).await;
    assert_status(status, StatusCode::OK, &final_rules);
    assert_eq!(final_rules["relevant_stage"], "discharged");
    assert_eq!(final_rules["relevant_episode_id"], php_id);
    assert_eq!(final_rules["relevant_care_level"], "php");
    assert_eq!(
        requirement(&final_rules, "Any packet")["completion"],
        "signed"
    );
    assert_eq!(
        requirement(&final_rules, "PHP rule")["disposition"],
        "required"
    );
    assert!(has_rule(&final_rules, "Discharge rule"));
    assert_eq!(
        requirement(&final_rules, "Discharge rule")["completion"],
        "needed"
    );
    assert!(!has_rule(&final_rules, "IOP rule"));
    assert!(!has_rule(&final_rules, "Inactive rule"));
    // Intake explicitly targets its planned episode, even when older care exists.
    let planned=support::legacy_episode(&pool,&admin,json!({"client_id":client_id,"facility_id":facility,"level_of_care":"iop","start_date":"2026-11-02","status":"planned"})).await;
    let (status, planned_rules) = call(
        &pool,
        "GET",
        &format!("{path}?episode_id={}", planned["id"].as_str().unwrap()),
        Some(&clinician),
        None,
    )
    .await;
    assert_status(status, StatusCode::OK, &planned_rules);
    assert_eq!(planned_rules["client_status"], "discharged");
    assert_eq!(planned_rules["relevant_stage"], "intake");
    assert_eq!(planned_rules["relevant_episode_id"], planned["id"]);
    assert_eq!(
        requirement(&planned_rules, "Any packet")["completion"],
        "needed"
    );
    assert_eq!(
        call(
            &pool,
            "GET",
            &format!("{path}?episode_id={}", Uuid::new_v4()),
            Some(&clinician),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE resource='form_requirements' AND action='read' AND record_id=$1",
    )
    .bind(Uuid::parse_str(&client_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audits >= 1);

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
