//! Explicit pre-program database fixtures for historical workflow regression tests.
//! These are not HTTP helpers: new admissions/bookings use the real program APIs in
//! program_*_postgres. Never use these functions to assert a create endpoint works.
#![allow(dead_code)]
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn org(pool: &PgPool, actor: &str) -> Uuid {
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        database, "nexus_test",
        "Legacy fixtures are disposable-test-only"
    );
    sqlx::query_scalar("SELECT organization_id FROM staff WHERE id=$1")
        .bind(Uuid::parse_str(actor).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}
pub async fn legacy_episode(pool: &PgPool, actor: &str, body: Value) -> Value {
    let org = org(pool, actor).await;
    let row:Value=sqlx::query_scalar("INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date,status,end_date,discharge_reason) VALUES($1,$2,($3->>'client_id')::uuid,($3->>'facility_id')::uuid,$3->>'level_of_care',($3->>'start_date')::date,COALESCE($3->>'status','active'),($3->>'end_date')::date,COALESCE($3->>'discharge_reason','')) RETURNING to_jsonb(episodes)").bind(Uuid::new_v4()).bind(org).bind(body).fetch_one(pool).await.unwrap();
    if row["status"] == "active" {
        sqlx::query("UPDATE clients SET status='active',updated_at=clock_timestamp() WHERE id=$1 AND organization_id=$2").bind(Uuid::parse_str(row["client_id"].as_str().unwrap()).unwrap()).bind(org).execute(pool).await.unwrap();
    }
    row
}
pub async fn legacy_session(pool: &PgPool, actor: &str, body: Value) -> Value {
    let org = org(pool, actor).await;
    sqlx::query_scalar("INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind,status,location) VALUES($1,$2,($3->>'client_id')::uuid,($3->>'episode_id')::uuid,($3->>'clinician_id')::uuid,($3->>'starts_at')::timestamptz,($3->>'ends_at')::timestamptz,$3->>'kind',COALESCE($3->>'status','scheduled'),COALESCE($3->>'location','')) RETURNING to_jsonb(sessions)").bind(Uuid::new_v4()).bind(org).bind(body).fetch_one(pool).await.unwrap()
}
pub async fn legacy_group(pool: &PgPool, actor: &str, body: Value) -> Value {
    let org = org(pool, actor).await;
    sqlx::query_scalar("INSERT INTO groups(id,organization_id,facility_id,clinician_id,title,kind,starts_at,ends_at,capacity,location) VALUES($1,$2,($3->>'facility_id')::uuid,($3->>'clinician_id')::uuid,$3->>'title',$3->>'kind',($3->>'starts_at')::timestamptz,($3->>'ends_at')::timestamptz,($3->>'capacity')::integer,COALESCE($3->>'location','')) RETURNING to_jsonb(groups)").bind(Uuid::new_v4()).bind(org).bind(body).fetch_one(pool).await.unwrap()
}
/// Minimal configured catalog fixture for tests of the scheduling conflict engine.
/// It does not book anything or replace an HTTP request.
pub async fn scheduling_catalog(
    pool: &PgPool,
    actor: &str,
    facility: &Value,
) -> (Value, Value, Value) {
    let org = org(pool, actor).await;
    let id = Uuid::new_v4();
    let program:Value=sqlx::query_scalar("INSERT INTO care_programs(id,organization_id,facility_id,code,name,level_of_care,min_age,capacity) VALUES($1,$2,$3,$4,'Synthetic scheduling program','iop',18,1000) RETURNING to_jsonb(care_programs)").bind(id).bind(org).bind(Uuid::parse_str(facility.as_str().unwrap()).unwrap()).bind(format!("QA-{id}")).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO program_revisions(id,organization_id,program_id,version,snapshot,reason,actor_id) VALUES($1,$2,$3,1,$4,'Synthetic scheduling fixture',$5)").bind(Uuid::new_v4()).bind(org).bind(id).bind(&program).bind(Uuid::parse_str(actor).unwrap()).execute(pool).await.unwrap();
    let mut services = Vec::new();
    for kind in ["individual", "group"] {
        let row:Value=sqlx::query_scalar("INSERT INTO program_services(id,organization_id,program_id,code_system,service_code,name,session_kind,min_minutes,default_minutes,max_minutes) VALUES($1,$2,$3,'internal',$4,'Synthetic schedule service',$4,1,60,480) RETURNING to_jsonb(program_services)").bind(Uuid::new_v4()).bind(org).bind(id).bind(kind).fetch_one(pool).await.unwrap();
        services.push(row);
    }
    (program, services.remove(0), services.remove(0))
}
pub async fn link_fixture_episode(pool: &PgPool, episode: &Value, program: &Value) -> Value {
    sqlx::query_scalar("UPDATE episodes SET program_id=$1 WHERE id=$2 RETURNING to_jsonb(episodes)")
        .bind(Uuid::parse_str(program["id"].as_str().unwrap()).unwrap())
        .bind(Uuid::parse_str(episode["id"].as_str().unwrap()).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}
pub fn with_service(mut body: Value, service: &Value) -> Value {
    body["program_service_id"] = json!(service["id"]);
    body
}
