//! Superbills against a disposable isolated schema: who may open one, which claims
//! qualify, that the totals come from the ledger, and that opening one is audited.
use actix_web::{http::StatusCode, test, web, App};
use emr_api::{configure, migrate, AppState};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

async fn superbill(pool: &PgPool, actor: Uuid, claim: Uuid) -> (StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState::local(pool.clone())))
            .configure(configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/claims/{claim}/superbill"))
            .insert_header(("x-nexus-actor", actor.to_string()))
            .to_request(),
    )
    .await;
    let status = response.status();
    let cache = response
        .headers()
        .get("cache-control")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body: Value =
        serde_json::from_slice(&test::read_body(response).await).unwrap_or(Value::Null);
    if status.is_success() {
        assert_eq!(cache, "no-store, private");
    }
    (status, body)
}

struct Tenant {
    org: Uuid,
    facility: Uuid,
}
async fn tenant(pool: &PgPool, name: &str, zone: &str) -> Tenant {
    let (org, facility) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO organizations(id,name) VALUES($1,$2)")
        .bind(org)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO facilities(id,organization_id,name,timezone) VALUES($1,$2,'Synthetic superbill clinic',$3)")
        .bind(facility)
        .bind(org)
        .bind(zone)
        .execute(pool)
        .await
        .unwrap();
    Tenant { org, facility }
}
async fn staff(pool: &PgPool, tenant: &Tenant, role: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO staff(id,organization_id,facility_id,name,email,role) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(id)
        .bind(tenant.org)
        .bind(tenant.facility)
        .bind(format!("Synthetic {role}"))
        .bind(format!("superbill-{role}-{id}@example.invalid"))
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
    id
}
/// A client, an episode, one session and one claim on it. Returns the claim.
async fn claim(
    pool: &PgPool,
    tenant: &Tenant,
    clinician: Uuid,
    session_status: &str,
    claim_status: &str,
) -> Uuid {
    let (client, episode, session, policy, claim) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    sqlx::query("INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date,primary_clinician_id,address_line1,city,region,postal_code) VALUES($1,$2,$3,'Robin','Sample','1988-04-12',$4,'12 Example Way','Albany','NY','12207')")
        .bind(client).bind(tenant.org).bind(tenant.facility).bind(clinician).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date) VALUES($1,$2,$3,$4,'outpatient','2026-01-01')")
        .bind(episode).bind(tenant.org).bind(client).bind(tenant.facility).execute(pool).await.unwrap();
    // 18:30 UTC on June 1 is 2:30 PM in New York.
    sqlx::query("INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind,status,location) VALUES($1,$2,$3,$4,$5,'2026-06-01T18:30:00Z','2026-06-01T19:23:00Z','individual',$6,'Room 2')")
        .bind(session).bind(tenant.org).bind(client).bind(episode).bind(clinician).bind(session_status).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO policies(id,organization_id,client_id,payer_name,member_id,group_number,effective_date) VALUES($1,$2,$3,'Synthetic payer','MEMBER-1','GROUP-9','2026-01-01')")
        .bind(policy).bind(tenant.org).bind(client).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO claims(id,organization_id,client_id,policy_id,session_id,service_code,diagnosis_code,units,amount_cents,status) VALUES($1,$2,$3,$4,$5,'90837','F10.20',1,18000,$6)")
        .bind(claim).bind(tenant.org).bind(client).bind(policy).bind(session).bind(claim_status).execute(pool).await.unwrap();
    claim
}
async fn pay(
    pool: &PgPool,
    tenant: &Tenant,
    claim: Uuid,
    source: &str,
    cents: i64,
    reference: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO payments(id,organization_id,claim_id,received_on,source,amount_cents,reference) VALUES($1,$2,$3,'2026-06-20',$4,$5,$6)")
        .bind(id).bind(tenant.org).bind(claim).bind(source).bind(cents).bind(reference).execute(pool).await.unwrap();
    id
}

#[actix_web::test]
#[ignore = "Requires TEST_DATABASE_URL ending in /nexus_test; creates and removes only a unique test schema"]
async fn a_superbill_is_built_from_the_claim_for_the_roles_that_prepare_claims() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    assert!(url.ends_with("/nexus_test"));
    let control = PgPool::connect(&url).await.unwrap();
    let schema = format!("nexus_superbill_test_{}", Uuid::new_v4().simple());
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

    let here = tenant(
        &pool,
        "Synthetic superbill organization",
        "America/New_York",
    )
    .await;
    let clinician = staff(&pool, &here, "clinician").await;
    let billing = staff(&pool, &here, "billing").await;
    let admin = staff(&pool, &here, "admin").await;
    let submitted = claim(&pool, &here, clinician, "completed", "submitted").await;

    // The statement states the visit in the facility's own time and the charge in cents.
    let (status, bill) = superbill(&pool, billing, submitted).await;
    assert_eq!(status, StatusCode::OK, "{bill}");
    assert_eq!(
        bill["number"],
        format!("SB-{}", submitted.simple().to_string()[..8].to_uppercase())
    );
    assert_eq!(bill["organization"], "Synthetic superbill organization");
    assert_eq!(bill["facility"], "Synthetic superbill clinic");
    assert_eq!(bill["client"]["name"], "Robin Sample");
    assert_eq!(bill["client"]["birth_date"], "1988-04-12");
    assert_eq!(
        bill["client"]["address"],
        serde_json::json!(["12 Example Way", "Albany, NY, 12207"])
    );
    assert_eq!(bill["provider"]["name"], "Synthetic clinician");
    assert_eq!(bill["session"]["service_date"], "2026-06-01");
    assert_eq!(bill["session"]["start_time"], "14:30");
    assert_eq!(bill["session"]["end_time"], "15:23");
    assert_eq!(bill["session"]["minutes"], 53);
    assert_eq!(bill["service"]["code"], "90837");
    assert_eq!(bill["service"]["charge_cents"], 18000);
    assert_eq!(bill["diagnosis_code"], "F10.20");
    assert_eq!(bill["coverage"]["member_id"], "MEMBER-1");
    assert_eq!(bill["coverage"]["authorization_number"], Value::Null);
    assert_eq!(bill["totals"]["balance_cents"], 18000);
    assert!(bill["not_on_file"].as_array().unwrap().len() >= 4);

    // The balance follows the ledger: a payment, a write-off, and a reversed payment that no longer counts.
    pay(&pool, &here, submitted, "payer", 9000, "EFT-1").await;
    pay(&pool, &here, submitted, "adjustment", 2500, "ADJ-1").await;
    let mistaken = pay(&pool, &here, submitted, "client", 4000, "CASH-1").await;
    sqlx::query("INSERT INTO payment_reversals(id,organization_id,payment_id,claim_id,amount_cents,reversed_on,reason,actor_id) VALUES($1,$2,$3,$4,4000,'2026-06-21','Recorded against the wrong claim',$5)")
        .bind(Uuid::new_v4()).bind(here.org).bind(mistaken).bind(submitted).bind(billing).execute(&pool).await.unwrap();
    let (_, bill) = superbill(&pool, admin, submitted).await;
    assert_eq!(bill["totals"]["received_cents"], 9000);
    assert_eq!(bill["totals"]["adjustment_cents"], 2500);
    assert_eq!(bill["totals"]["balance_cents"], 6500);
    let listed = bill["payments"].as_array().unwrap();
    assert_eq!(listed.len(), 3);
    assert_eq!(
        listed
            .iter()
            .filter(|entry| entry["reversed"] == true)
            .count(),
        1
    );

    // Only the roles that prepare claims. Reading claims is not enough.
    for role in ["clinician", "clinical_lead", "executive", "support"] {
        let person = if role == "clinician" {
            clinician
        } else {
            staff(&pool, &here, role).await
        };
        assert_eq!(
            superbill(&pool, person, submitted).await.0,
            StatusCode::FORBIDDEN,
            "{role}"
        );
    }

    // Only care that was delivered, and only charges past draft and not voided.
    let draft = claim(&pool, &here, clinician, "completed", "draft").await;
    let (status, refusal) = superbill(&pool, billing, draft).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(refusal["error"]["message"]
        .as_str()
        .unwrap()
        .contains("ready first"));
    let voided = claim(&pool, &here, clinician, "completed", "voided").await;
    assert_eq!(
        superbill(&pool, billing, voided).await.0,
        StatusCode::CONFLICT
    );
    let missed = claim(&pool, &here, clinician, "no_show", "ready").await;
    let (status, refusal) = superbill(&pool, billing, missed).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(refusal["error"]["message"]
        .as_str()
        .unwrap()
        .contains("delivered"));

    // Another organization's claim does not exist here.
    let elsewhere = tenant(&pool, "Synthetic other organization", "America/Chicago").await;
    let their_clinician = staff(&pool, &elsewhere, "clinician").await;
    let their_billing = staff(&pool, &elsewhere, "billing").await;
    let theirs = claim(&pool, &elsewhere, their_clinician, "completed", "ready").await;
    assert_eq!(
        superbill(&pool, billing, theirs).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        superbill(&pool, their_billing, submitted).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        superbill(&pool, their_billing, theirs).await.0,
        StatusCode::OK
    );
    assert_eq!(
        superbill(&pool, billing, Uuid::new_v4()).await.0,
        StatusCode::NOT_FOUND
    );

    // Each statement that was produced is in the audit trail; refusals are not.
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE action='superbill' AND resource='claims' AND organization_id=$1")
        .bind(here.org)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audited, 2);

    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
}
