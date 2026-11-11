//! Program catalog role, tenant, census, history, and constraint coverage.
//! Requires root to declare `mod programs` and configure it before generic /api routes.
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
    actor: Option<&str>,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value, Option<String>) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
            .configure(configure),
    )
    .await;
    let mut req = test::TestRequest::default()
        .method(method)
        .uri(path)
        .set_json(body);
    if let Some(actor) = actor {
        req = req.insert_header(("x-nexus-actor", actor));
    }
    let res = test::call_service(&app, req.to_request()).await;
    let status = res.status();
    let cache = res
        .headers()
        .get("cache-control")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = test::read_body(res).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cache,
    )
}
async fn expect(
    pool: &PgPool,
    actor: &str,
    method: Method,
    path: &str,
    body: Value,
    status: StatusCode,
) -> Value {
    let (actual, value, cache) = send(pool, Some(actor), method.clone(), path, body).await;
    assert_eq!(actual, status, "{method} {path}: {value}");
    if actual.is_success()
        && (path.starts_with("/api/care-programs")
            || path.starts_with("/api/program-services")
            || path == "/api/program-catalog")
    {
        assert_eq!(cache.as_deref(), Some("no-store"), "{path}");
    }
    value
}
fn id_of(value: &Value) -> Uuid {
    Uuid::parse_str(value["id"].as_str().unwrap()).unwrap()
}
fn find<'a>(items: &'a [Value], key: &str, expected: &str) -> &'a Value {
    items
        .iter()
        .find(|item| item[key].as_str() == Some(expected))
        .unwrap_or_else(|| panic!("missing {key}={expected}"))
}

#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn program_catalog_enforces_roles_history_forms_and_census() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(actual, "nexus_test");
    let schema = format!("nexus_program_catalog_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&control)
        .await
        .unwrap();
    let search = format!("SET search_path TO \"{schema}\"");
    let pool = PgPoolOptions::new()
        .max_connections(5)
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
    let (status, setup, _) = send(
        &pool,
        None,
        Method::POST,
        "/api/setup",
        json!({
            "organization_name": "Synthetic program QA",
            "facility_name": "Synthetic west facility",
            "admin_name": "Synthetic registrar",
            "admin_email": "programs@example.invalid"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{setup}");
    let admin = setup["actor"]["id"].as_str().unwrap().to_string();
    let admin_id = Uuid::parse_str(&admin).unwrap();
    let org: Uuid = sqlx::query_scalar("SELECT organization_id FROM staff WHERE id=$1")
        .bind(admin_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let facility: Uuid =
        sqlx::query_scalar("SELECT id FROM facilities WHERE organization_id=$1 LIMIT 1")
            .bind(org)
            .fetch_one(&pool)
            .await
            .unwrap();
    let admin_name: String = sqlx::query_scalar("SELECT name FROM staff WHERE id=$1")
        .bind(admin_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let east = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/facilities",
        json!({"name": "Synthetic east facility", "timezone": "America/Chicago", "active": true}),
        StatusCode::CREATED,
    )
    .await;
    let east_id = id_of(&east);
    let intake = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/templates",
        json!({
            "name": "Synthetic intake packet",
            "kind": "assessment",
            "description": "Synthetic operational packet",
            "fields": [{"key": "a", "label": "A", "type": "text"}],
            "active": true,
            "applies_to_stage": "intake",
            "applies_to_care_level": "iop",
            "required_for_stage": false
        }),
        StatusCode::CREATED,
    )
    .await;
    let discharge = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/templates",
        json!({
            "name": "Synthetic discharge packet",
            "kind": "assessment",
            "description": "Synthetic operational packet",
            "fields": [{"key": "b", "label": "B", "type": "text"}],
            "active": true,
            "applies_to_stage": "discharged",
            "applies_to_care_level": "any",
            "required_for_stage": false
        }),
        StatusCode::CREATED,
    )
    .await;
    let inactive_template = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/templates",
        json!({
            "name": "Synthetic retired packet",
            "kind": "assessment",
            "description": "Synthetic",
            "fields": [{"key": "c", "label": "C", "type": "text"}],
            "active": false,
            "applies_to_stage": "any",
            "applies_to_care_level": "any",
            "required_for_stage": false
        }),
        StatusCode::CREATED,
    )
    .await;
    let intake_id = id_of(&intake);
    let discharge_id = id_of(&discharge);
    let inactive_template_id = id_of(&inactive_template);
    let mut staff = Vec::new();
    for role in [
        "clinical_lead",
        "clinician",
        "billing",
        "executive",
        "support",
    ] {
        let row = expect(
            &pool,
            &admin,
            Method::POST,
            "/api/staff",
            json!({"name": format!("Synthetic {role}"), "email": format!("{role}@example.invalid"), "role": role}),
            StatusCode::CREATED,
        )
        .await;
        staff.push((role, row["id"].as_str().unwrap().to_string()));
    }
    let clinician = staff
        .iter()
        .find(|(role, _)| *role == "clinician")
        .unwrap()
        .1
        .clone();
    sqlx::query("INSERT INTO exceptions(id, organization_id, staff_id, resource, reason, expires_at, revoked, access_mode) VALUES($1,$2,$3,'templates','Synthetic local grant', clock_timestamp() + interval '2 days', false, 'write')")
        .bind(Uuid::new_v4())
        .bind(org)
        .bind(Uuid::parse_str(&clinician).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let other_org = Uuid::new_v4();
    let other_admin = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id, name) VALUES($1, 'Synthetic other tenant')")
        .bind(other_org)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff(id, organization_id, name, email, role) VALUES($1,$2,'Synthetic other admin','other-admin@example.invalid','admin')")
        .bind(other_admin)
        .bind(other_org)
        .execute(&pool)
        .await
        .unwrap();
    let empty = expect(
        &pool,
        &clinician,
        Method::GET,
        "/api/program-catalog",
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(empty["programs"], json!([]));
    assert_eq!(empty["services"], json!([]));
    assert_eq!(empty["can_manage"], false);
    assert_eq!(
        send(&pool, None, Method::GET, "/api/program-catalog", json!({}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    for (_, actor) in &staff {
        assert_eq!(
            send(
                &pool,
                Some(actor),
                Method::POST,
                "/api/care-programs",
                json!({"change_reason": "Should not be stored anywhere."})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(
                &pool,
                Some(actor),
                Method::GET,
                &format!("/api/care-programs/{}/history", Uuid::new_v4()),
                json!({})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::GET,
            &format!("/api/care-programs/{}/history", Uuid::new_v4()),
            json!({})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let reason = "Opened synthetic adult program for local QA.";
    assert_eq!(
        expect(
            &pool,
            &admin,
            Method::POST,
            "/api/care-programs",
            json!({
                "facility_id": facility,
                "code": "ADULT",
                "name": "Synthetic adult",
                "level_of_care": "IOP",
                "min_age": 18,
                "max_age": 65,
                "capacity": 2,
                "active": true,
                "required_intake_template_ids": [],
                "required_discharge_template_ids": [],
                "change_reason": reason
            }),
            StatusCode::BAD_REQUEST,
        )
        .await["error"]["code"],
        "validation"
    );
    assert!(expect(
        &pool,
        &admin,
        Method::POST,
        "/api/care-programs",
        json!({
            "facility_id": facility,
            "code": "ADULT",
            "name": "Synthetic adult",
            "level_of_care": "iop",
            "min_age": 17,
            "capacity": 2,
            "active": true,
            "required_intake_template_ids": [],
            "required_discharge_template_ids": [],
            "change_reason": reason
        }),
        StatusCode::BAD_REQUEST,
    )
    .await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("min_age"));
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::POST,
            "/api/care-programs",
            json!({
                "facility_id": facility,
                "code": "BAD",
                "name": "Synthetic",
                "level_of_care": "iop",
                "min_age": 18,
                "capacity": 0,
                "active": true,
                "required_intake_template_ids": [],
                "required_discharge_template_ids": [],
                "change_reason": reason,
                "organization_id": org
            })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::POST,
            "/api/care-programs",
            json!({
                "facility_id": facility,
                "code": "DUPFORM",
                "name": "Synthetic",
                "level_of_care": "iop",
                "min_age": 18,
                "capacity": 2,
                "active": true,
                "required_intake_template_ids": [intake_id, intake_id],
                "required_discharge_template_ids": [],
                "change_reason": reason
            })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::POST,
            "/api/care-programs",
            json!({
                "facility_id": facility,
                "code": "INACTIVE",
                "name": "Synthetic",
                "level_of_care": "iop",
                "min_age": 18,
                "capacity": 2,
                "active": true,
                "required_intake_template_ids": [inactive_template_id],
                "required_discharge_template_ids": [],
                "change_reason": reason
            })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let created = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/care-programs",
        json!({
            "facility_id": facility,
            "code": "  adult ",
            "name": "  Synthetic adult track  ",
            "level_of_care": " iop ",
            "min_age": 18,
            "max_age": 65,
            "capacity": 2,
            "active": true,
            "required_intake_template_ids": [intake_id, discharge_id],
            "required_discharge_template_ids": [discharge_id],
            "change_reason": reason
        }),
        StatusCode::CREATED,
    )
    .await;
    let program_id = id_of(&created);
    assert_eq!(created["code"], "ADULT");
    assert_eq!(created["name"], "Synthetic adult track");
    assert_eq!(created["level_of_care"], "iop");
    assert_eq!(created["version"], 1);
    assert_eq!(created["active_census"], 0);
    assert_eq!(created["available_places"], 2);
    assert_eq!(
        created["required_discharge_template_ids"][0],
        discharge["id"]
    );
    assert_eq!(created["configuration_blockers"], json!([]));
    assert!(created.get("change_reason").is_none());
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::POST,
            "/api/care-programs",
            json!({
                "facility_id": east_id,
                "code": "adult",
                "name": "Duplicate case",
                "level_of_care": "php",
                "min_age": 18,
                "capacity": 4,
                "active": true,
                "required_intake_template_ids": [],
                "required_discharge_template_ids": [],
                "change_reason": reason
            })
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let east_program = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/care-programs",
        json!({
            "facility_id": east_id,
            "code": "php-east",
            "name": "Synthetic east PHP",
            "level_of_care": "php",
            "min_age": 21,
            "max_age": null,
            "capacity": 8,
            "active": true,
            "required_intake_template_ids": [],
            "required_discharge_template_ids": [],
            "change_reason": "Opened a second facility program for routing checks."
        }),
        StatusCode::CREATED,
    )
    .await;
    let east_program_id = id_of(&east_program);
    assert!(east_program["max_age"].is_null());
    let service = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/program-services",
        json!({
            "program_id": program_id,
            "code_system": "cpt",
            "service_code": "  h0015 ",
            "name": "  Synthetic group hour  ",
            "session_kind": "group",
            "min_minutes": 30,
            "default_minutes": 60,
            "max_minutes": 90,
            "default_charge_cents": 15000,
            "note_template_id": intake_id,
            "active": true,
            "change_reason": "Added an organization-authored group service code."
        }),
        StatusCode::CREATED,
    )
    .await;
    let service_id = id_of(&service);
    assert_eq!(service["service_code"], "H0015");
    assert_eq!(service["name"], "Synthetic group hour");
    assert_eq!(service["version"], 1);
    assert_eq!(service["note_template_id"], intake["id"]);
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::POST,
            "/api/program-services",
            json!({
                "program_id": program_id,
                "code_system": "cpt",
                "service_code": "h0015",
                "name": "Duplicate",
                "session_kind": "group",
                "min_minutes": 30,
                "default_minutes": 60,
                "max_minutes": 90,
                "default_charge_cents": null,
                "note_template_id": null,
                "active": true,
                "change_reason": "Duplicate service code should be rejected."
            })
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let other_system = expect(
        &pool,
        &admin,
        Method::POST,
        "/api/program-services",
        json!({
            "program_id": program_id,
            "code_system": "internal",
            "service_code": "h0015",
            "name": "Synthetic internal alias",
            "session_kind": "individual",
            "min_minutes": 1,
            "default_minutes": 45,
            "max_minutes": 480,
            "default_charge_cents": 0,
            "note_template_id": null,
            "active": true,
            "change_reason": "Same letters are allowed under another code system."
        }),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(other_system["service_code"], "H0015");
    assert_eq!(other_system["code_system"], "internal");
    expect(
        &pool,
        &admin,
        Method::POST,
        "/api/program-services",
        json!({
            "program_id": east_program_id,
            "code_system": "hcpcs",
            "service_code": "S9475",
            "name": "Synthetic east service",
            "session_kind": "assessment",
            "min_minutes": 480,
            "default_minutes": 480,
            "max_minutes": 480,
            "default_charge_cents": 100000000,
            "note_template_id": null,
            "active": false,
            "change_reason": "Recorded an inactive east assessment service."
        }),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::POST,
            "/api/program-services",
            json!({
                "program_id": program_id,
                "code_system": "CPT",
                "service_code": "99213",
                "name": "Bad system",
                "session_kind": "individual",
                "min_minutes": 1,
                "default_minutes": 1,
                "max_minutes": 1,
                "active": true,
                "change_reason": "Code system case must be exact."
            })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::POST,
            "/api/program-services",
            json!({
                "program_id": program_id,
                "code_system": "cpt",
                "service_code": "99214",
                "name": "Bad minutes",
                "session_kind": "individual",
                "min_minutes": 40,
                "default_minutes": 30,
                "max_minutes": 50,
                "default_charge_cents": 10.5,
                "active": true,
                "change_reason": "Fractional charge and inverted minutes fail."
            })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let catalog = expect(
        &pool,
        &clinician,
        Method::GET,
        "/api/program-catalog",
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(catalog["can_manage"], false);
    assert_eq!(catalog["programs"].as_array().unwrap().len(), 2);
    assert!(catalog["services"].as_array().unwrap().len() >= 3);
    let listed = find(catalog["programs"].as_array().unwrap(), "code", "ADULT");
    assert_eq!(listed["id"], created["id"]);
    assert!(!catalog.to_string().contains("Quillon"));
    let admin_catalog = expect(
        &pool,
        &admin,
        Method::GET,
        "/api/program-catalog",
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(admin_catalog["can_manage"], true);
    let other_catalog = expect(
        &pool,
        &other_admin.to_string(),
        Method::GET,
        "/api/program-catalog",
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(other_catalog["programs"], json!([]));
    assert_eq!(
        send(
            &pool,
            Some(&other_admin.to_string()),
            Method::PATCH,
            &format!("/api/care-programs/{program_id}"),
            json!({"_expected_updated_at": created["updated_at"], "change_reason": "Cross tenant edit must not apply.", "name": "Hidden"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let history = expect(
        &pool,
        &admin,
        Method::GET,
        &format!("/api/care-programs/{program_id}/history"),
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(history["revisions"].as_array().unwrap().len(), 1);
    assert_eq!(history["revisions"][0]["version"], 1);
    assert_eq!(history["revisions"][0]["actor_id"], admin);
    assert_eq!(history["revisions"][0]["actor_name"], admin_name);
    assert_eq!(history["revisions"][0]["reason"], reason);
    assert!(history["revisions"][0]["snapshot"]
        .get("active_census")
        .is_none());
    assert!(history["revisions"][0]["snapshot"]
        .get("configuration_blockers")
        .is_none());
    assert_eq!(
        history["revisions"][0]["snapshot"]["required_intake_template_ids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let original_snapshot = history["revisions"][0]["snapshot"].clone();
    assert_eq!(
        send(
            &pool,
            Some(&clinician),
            Method::GET,
            &format!("/api/care-programs/{program_id}/history"),
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        expect(
            &pool,
            &admin,
            Method::PATCH,
            &format!("/api/care-programs/{program_id}"),
            json!({"change_reason": "Missing stamp must not save."}),
            StatusCode::PRECONDITION_REQUIRED,
        )
        .await["error"]["code"],
        "precondition_required"
    );
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::PATCH,
            &format!("/api/care-programs/{program_id}"),
            json!({"_expected_updated_at": created["updated_at"], "change_reason": "too short", "name": "Nope"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::PATCH,
            &format!("/api/care-programs/{program_id}"),
            json!({"_expected_updated_at": created["updated_at"], "change_reason": "Attempt to rewrite immutable context.", "code": "OTHER", "level_of_care": "php", "facility_id": east_id})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::PATCH,
            &format!("/api/care-programs/{program_id}"),
            json!({"_expected_updated_at": created["updated_at"], "change_reason": "No fields changed in this request."})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let renamed = expect(
        &pool,
        &admin,
        Method::PATCH,
        &format!("/api/care-programs/{program_id}"),
        json!({
            "_expected_updated_at": created["updated_at"],
            "change_reason": "Renamed the adult track without touching forms.",
            "name": "Synthetic adult track renamed"
        }),
        StatusCode::OK,
    )
    .await;
    assert_eq!(renamed["version"], 2);
    assert_eq!(
        renamed["required_discharge_template_ids"][0],
        discharge["id"]
    );
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::PATCH,
            &format!("/api/care-programs/{program_id}"),
            json!({"_expected_updated_at": created["updated_at"], "change_reason": "Stale edit must be rejected.", "name": "Stale name"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let missing_template = Uuid::new_v4();
    assert_eq!(
        send(
            &pool,
            Some(&admin),
            Method::PATCH,
            &format!("/api/care-programs/{program_id}"),
            json!({
                "_expected_updated_at": renamed["updated_at"],
                "change_reason": "Unknown template must not replace forms.",
                "required_intake_template_ids": [missing_template],
                "required_discharge_template_ids": [missing_template]
            })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let still = expect(
        &pool,
        &admin,
        Method::GET,
        "/api/program-catalog",
        json!({}),
        StatusCode::OK,
    )
    .await;
    let still_program = find(still["programs"].as_array().unwrap(), "code", "ADULT");
    assert_eq!(still_program["version"], 2);
    assert_eq!(
        still_program["required_intake_template_ids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let forms_replaced = expect(
        &pool,
        &admin,
        Method::PATCH,
        &format!("/api/care-programs/{program_id}"),
        json!({
            "_expected_updated_at": renamed["updated_at"],
            "change_reason": "Replaced intake forms and cleared the optional max age.",
            "max_age": null,
            "required_intake_template_ids": [intake_id]
        }),
        StatusCode::OK,
    )
    .await;
    assert!(forms_replaced["max_age"].is_null());
    assert_eq!(
        forms_replaced["required_intake_template_ids"],
        json!([intake["id"]])
    );
    assert_eq!(
        forms_replaced["required_discharge_template_ids"],
        json!([discharge["id"]])
    );
    let history = expect(
        &pool,
        &admin,
        Method::GET,
        &format!("/api/care-programs/{program_id}/history"),
        json!({}),
        StatusCode::OK,
    )
    .await;
    let revisions = history["revisions"].as_array().unwrap();
    assert_eq!(
        revisions
            .iter()
            .map(|row| row["version"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(revisions[2]["snapshot"], original_snapshot);
    assert!(sqlx::query(
        "UPDATE program_revisions SET reason='rewritten reason text' WHERE program_id=$1"
    )
    .bind(program_id)
    .execute(&pool)
    .await
    .is_err());
    assert!(
        sqlx::query("DELETE FROM program_revisions WHERE program_id=$1")
            .bind(program_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE care_programs SET code='OTHER' WHERE id=$1")
            .bind(program_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE care_programs SET level_of_care='php' WHERE id=$1")
            .bind(program_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE care_programs SET facility_id=$2 WHERE id=$1")
            .bind(program_id)
            .bind(east_id)
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::query("UPDATE templates SET active=false, updated_at=clock_timestamp() WHERE id=$1")
        .bind(intake_id)
        .execute(&pool)
        .await
        .unwrap();
    let blocked = expect(
        &pool,
        &admin,
        Method::GET,
        "/api/program-catalog",
        json!({}),
        StatusCode::OK,
    )
    .await;
    let blocked_program = find(blocked["programs"].as_array().unwrap(), "code", "ADULT");
    assert_eq!(
        blocked_program["required_intake_template_ids"],
        json!([intake["id"]])
    );
    assert_eq!(
        blocked_program["configuration_blockers"][0]["template_id"],
        intake["id"]
    );
    assert_eq!(
        blocked_program["configuration_blockers"][0]["code"],
        "inactive_template"
    );
    let kept = expect(
        &pool,
        &admin,
        Method::PATCH,
        &format!("/api/care-programs/{program_id}"),
        json!({
            "_expected_updated_at": forms_replaced["updated_at"],
            "change_reason": "Name edit keeps an already referenced inactive template.",
            "name": "Synthetic adult track kept forms"
        }),
        StatusCode::OK,
    )
    .await;
    assert_eq!(kept["required_intake_template_ids"], json!([intake["id"]]));
    let path = format!("/api/program-services/{service_id}");
    expect(
        &pool,
        &admin,
        Method::PATCH,
        &path,
        json!({"name":"Without timestamp","change_reason":"Missing concurrency token test"}),
        StatusCode::PRECONDITION_REQUIRED,
    )
    .await;
    let edited=expect(&pool,&admin,Method::PATCH,&path,json!({"_expected_updated_at":service["updated_at"],"name":"Reviewed group service","default_charge_cents":0,"note_template_id":null,"change_reason":"Clear the inactive template and update fee"}),StatusCode::OK).await;
    assert_eq!(edited["version"], 2);
    assert_eq!(edited["default_charge_cents"], 0);
    assert!(edited["note_template_id"].is_null());
    expect(&pool,&admin,Method::PATCH,&path,json!({"_expected_updated_at":service["updated_at"],"name":"Stale update","change_reason":"Stale updates must preserve the latest version"}),StatusCode::CONFLICT).await;
    expect(&pool,&admin,Method::PATCH,&path,json!({"_expected_updated_at":edited["updated_at"],"service_code":"OTHER","change_reason":"Immutable service context cannot change"}),StatusCode::BAD_REQUEST).await;
    expect(&pool,&admin,Method::PATCH,&path,json!({"_expected_updated_at":edited["updated_at"],"max_minutes":15,"change_reason":"Duration ordering must remain valid"}),StatusCode::BAD_REQUEST).await;
    let history = expect(
        &pool,
        &admin,
        Method::GET,
        &format!("{path}/history"),
        json!({}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(history["revisions"].as_array().unwrap().len(), 2);
    assert_eq!(
        history["revisions"][1]["snapshot"]["default_charge_cents"],
        15000
    );
    assert!(
        sqlx::query("DELETE FROM program_service_revisions WHERE service_id=$1")
            .bind(service_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE program_services SET session_kind='individual' WHERE id=$1")
            .bind(service_id)
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
