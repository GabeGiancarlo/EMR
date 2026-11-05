//! Real SQLx upgrade/rollback and database identity regressions. Synthetic fixtures only.
use emr_api::migrate;
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{borrow::Cow, time::Duration};
use uuid::Uuid;

async fn database(prefix: Option<i64>) -> (PgPool, PgPool, String) {
    let url = std::env::var("TEST_DATABASE_URL").expect("requires disposable nexus_test");
    let control = PgPool::connect(&url).await.unwrap();
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&control)
        .await
        .unwrap();
    assert_eq!(name, "nexus_test");
    let schema = format!("nexus_identity_{}", Uuid::new_v4().simple());
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
    if let Some(version) = prefix {
        let mut old = sqlx::migrate!("../migrations");
        old.migrations = Cow::Owned(
            old.iter()
                .filter(|m| m.version <= version)
                .cloned()
                .collect(),
        );
        old.run(&pool).await.unwrap();
    } else {
        migrate(&pool).await.unwrap();
    }
    (control, pool, schema)
}
async fn cleanup(control: PgPool, pool: PgPool, schema: String) {
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
async fn fixture(pool: &PgPool) {
    sqlx::raw_sql(r#"
      INSERT INTO organizations(id,name) VALUES('00000000-0000-0000-0000-000000000001','Synthetic identity');
      INSERT INTO facilities(id,organization_id,name) VALUES('00000000-0000-0000-0000-000000000002','00000000-0000-0000-0000-000000000001','Synthetic');
      INSERT INTO staff(id,organization_id,name,email,role) VALUES('00000000-0000-0000-0000-000000000003','00000000-0000-0000-0000-000000000001','Synthetic','synthetic@example.invalid','admin');
      INSERT INTO clients(id,organization_id,facility_id,first_name,last_name,birth_date) VALUES('00000000-0000-0000-0000-000000000004','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','Synthetic','Identity','1990-01-01');
      INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date) VALUES('00000000-0000-0000-0000-000000000005','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000004','00000000-0000-0000-0000-000000000002','outpatient','2026-01-01');
      INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind) VALUES('00000000-0000-0000-0000-000000000006','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000004','00000000-0000-0000-0000-000000000005','00000000-0000-0000-0000-000000000003','2026-01-02T10:00Z','2026-01-02T11:00Z','individual');
      INSERT INTO audit_events(id,organization_id,actor_id,action,resource,record_id) VALUES('00000000-0000-0000-0000-000000000008','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000003','create','sessions','00000000-0000-0000-0000-000000000006');
      INSERT INTO policies(id,organization_id,client_id,payer_name,member_id,effective_date) VALUES('00000000-0000-0000-0000-000000000007','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000004','Synthetic payer','Synthetic member','2026-01-01');
    "#).execute(pool).await.unwrap();
}
async fn catalog_snapshot(pool: &PgPool) {
    sqlx::raw_sql(r#"
      INSERT INTO care_programs(id,organization_id,facility_id,code,name,level_of_care,min_age,capacity) VALUES('00000000-0000-0000-0000-000000000010','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','SYNTHETIC','Synthetic program','outpatient',18,10);
      INSERT INTO program_services(id,organization_id,program_id,code_system,service_code,name,session_kind,min_minutes,default_minutes,max_minutes) VALUES('00000000-0000-0000-0000-000000000011','00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000010','hcpcs','H2019','Synthetic service','individual',30,60,90);
      INSERT INTO program_service_revisions(id,organization_id,service_id,version,snapshot,reason,actor_id) SELECT '00000000-0000-0000-0000-000000000012',organization_id,id,1,to_jsonb(s),'Synthetic initial revision','00000000-0000-0000-0000-000000000003' FROM program_services s;
      UPDATE sessions SET program_service_id='00000000-0000-0000-0000-000000000011',service_snapshot=(SELECT to_jsonb(s) FROM program_services s LIMIT 1);
    "#).execute(pool).await.unwrap();
}
async fn claim(pool: &PgPool, code: &str) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar("INSERT INTO claims(id,organization_id,client_id,policy_id,session_id,service_code,diagnosis_code,amount_cents) VALUES($1,'00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000004','00000000-0000-0000-0000-000000000007','00000000-0000-0000-0000-000000000006',$2,'F41.1',1500) RETURNING id")
      .bind(Uuid::new_v4()).bind(code).fetch_one(pool).await
}
async fn evidence(pool: &PgPool) -> Value {
    sqlx::query_scalar(r#"SELECT jsonb_build_object(
      'claims',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM claims c),
      'sessions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM sessions s),
      'catalog',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM program_services s),
      'revisions',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM program_service_revisions s),
      'audit',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM audit_events a),
      'migrations',(SELECT jsonb_agg(to_jsonb(m) ORDER BY version) FROM _sqlx_migrations m),
      'indexes',(SELECT jsonb_agg(to_jsonb(i) ORDER BY indexname) FROM pg_indexes i WHERE schemaname=current_schema()),
      'constraints',(SELECT jsonb_agg(pg_get_constraintdef(oid) ORDER BY conname) FROM pg_constraint WHERE connamespace=current_schema()::regnamespace))"#)
      .fetch_one(pool).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable nexus_test PostgreSQL"]
async fn upgrade_collision_rolls_back_and_releases_migration_lock() {
    let (control, pool, schema) = database(Some(202611020020)).await;
    fixture(&pool).await;
    let first = claim(&pool, "H2019").await.unwrap();
    let second = claim(&pool, "h2019").await.unwrap();
    let before = evidence(&pool).await;
    for _ in 0..2 {
        let error = tokio::time::timeout(Duration::from_secs(10), migrate(&pool))
            .await
            .expect("migration lock released")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&first.to_string()) && error.contains(&second.to_string()),
            "{error}"
        );
        assert_eq!(evidence(&pool).await, before);
        let absent: bool =
            sqlx::query_scalar("SELECT to_regprocedure('nexus_service_code_v1(text)') IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(absent);
    }
    cleanup(control, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable nexus_test PostgreSQL"]
async fn populated_upgrade_preserves_rows_and_repeat_is_noop() {
    let (control, pool, schema) = database(Some(202611020020)).await;
    fixture(&pool).await;
    catalog_snapshot(&pool).await;
    claim(&pool, " h2019 ").await.unwrap();
    let before = evidence(&pool).await;
    tokio::time::timeout(Duration::from_secs(10), migrate(&pool))
        .await
        .unwrap()
        .unwrap();
    let after = evidence(&pool).await;
    for key in ["claims", "sessions", "audit", "catalog", "revisions"] {
        assert_eq!(before[key], after[key]);
    }
    assert_eq!(after["migrations"].as_array().unwrap().len(), 21);
    tokio::time::timeout(Duration::from_secs(10), migrate(&pool))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(evidence(&pool).await, after);
    cleanup(control, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable nexus_test PostgreSQL"]
async fn unicode_identity_and_concurrent_unique_guard() {
    let (control, pool, schema) = database(None).await;
    for point in [
        9, 10, 11, 12, 13, 32, 0x85, 0xA0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005,
        0x2006, 0x2007, 0x2008, 0x2009, 0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000,
    ] {
        let ch = char::from_u32(point).unwrap();
        let result: Option<String> = sqlx::query_scalar("SELECT nexus_service_code_v1($1)")
            .bind(format!("{ch}h2019{ch}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(result.as_deref(), Some("H2019"));
    }
    for (input, expected) in [
        ("straße", "STRASSE"),
        ("ﬁ", "FI"),
        ("ıiİ", "IIİ"),
        ("σς", "ΣΣ"),
        ("é", "É"),
        ("e\u{301}", "E\u{301}"),
        ("a b-c", "A B-C"),
        ("\u{1c}a\u{1f}", "\u{1c}A\u{1f}"),
    ] {
        let result: Option<String> = sqlx::query_scalar("SELECT nexus_service_code_v1($1)")
            .bind(input)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(result.as_deref(), Some(expected));
    }
    for input in [None, Some(""), Some(" \t\u{a0}")] {
        let result: Option<String> = sqlx::query_scalar("SELECT nexus_service_code_v1($1)")
            .bind(input)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(result, None);
    }
    fixture(&pool).await;
    let (left, right) = tokio::join!(claim(&pool, " Straße "), claim(&pool, "STRASSE"));
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let error = left.err().or_else(|| right.err()).unwrap();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM claims")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    cleanup(control, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires disposable nexus_test PostgreSQL"]
async fn populated_pre_program_schema_upgrades_without_new_columns() {
    let (control, pool, schema) = database(Some(202611020019)).await;
    fixture(&pool).await;
    claim(&pool, "h2019").await.unwrap();
    let before: Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM claims c")
            .fetch_one(&pool)
            .await
            .unwrap();
    tokio::time::timeout(Duration::from_secs(10), migrate(&pool))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(evidence(&pool).await["claims"], before);
    assert_eq!(
        evidence(&pool).await["migrations"]
            .as_array()
            .unwrap()
            .len(),
        21
    );
    cleanup(control, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable nexus_test PostgreSQL"]
async fn catalog_boundary_whitespace_collision_blocks_upgrade() {
    let (control, pool, schema) = database(Some(202611020020)).await;
    fixture(&pool).await;
    catalog_snapshot(&pool).await;
    sqlx::query("INSERT INTO program_services(id,organization_id,program_id,code_system,service_code,name,session_kind,min_minutes,default_minutes,max_minutes) SELECT $1,organization_id,program_id,code_system,$2,name,session_kind,min_minutes,default_minutes,max_minutes FROM program_services LIMIT 1")
      .bind(Uuid::new_v4()).bind("H2019\u{a0}").execute(&pool).await.unwrap();
    let before = evidence(&pool).await;
    let error = migrate(&pool).await.unwrap_err().to_string();
    assert!(error.contains("Canonical catalog collisions"), "{error}");
    assert_eq!(evidence(&pool).await, before);
    cleanup(control, pool, schema).await;
}
