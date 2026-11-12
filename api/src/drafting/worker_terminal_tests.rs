// Included from drafting::worker::tests. Not a standalone module.
// Synthetic state fixtures only. Does not start the worker loop or a local model.
use super::{owned_interrupted, persist_terminal, TerminalPersist, ADVISORY_LOCK_KEY};
use actix_web::http::Method;
use actix_web::{test, web, App};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

type TestResult<T = ()> = std::result::Result<T, String>;

macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {{
        if !$cond {
            return Err(format!($($arg)*));
        }
    }};
}

struct Harness {
    pool: PgPool,
    control: PgPool,
    schema: String,
}

impl Harness {
    async fn create(prefix: &str) -> TestResult<Self> {
        let url = std::env::var("TEST_DATABASE_URL").map_err(|_| "TEST_DATABASE_URL is not set".to_string())?;
        if !url.ends_with("/nexus_test") {
            return Err("refusing to run outside a nexus_test database".into());
        }
        let schema = format!("{prefix}_{}", Uuid::new_v4().simple());
        if !(prefix.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            && schema.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && schema.len() <= 63)
        {
            return Err(format!("unsafe schema name {schema}"));
        }
        let control = PgPool::connect(&url).await.map_err(|e| e.to_string())?;
        if let Err(error) = sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
            .execute(&control)
            .await
        {
            control.close().await;
            return Err(format!("create schema: {error}"));
        }
        let search = format!("SET search_path TO \"{schema}\"");
        let pool = match PgPoolOptions::new()
            .max_connections(8)
            .after_connect({
                let search = search.clone();
                move |conn, _| {
                    let search = search.clone();
                    Box::pin(async move {
                        sqlx::query(&search).execute(conn).await?;
                        Ok(())
                    })
                }
            })
            .connect(&url)
            .await
        {
            Ok(pool) => pool,
            Err(error) => {
                let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
                    .execute(&control)
                    .await;
                control.close().await;
                return Err(format!("connect: {error}"));
            }
        };
        if let Err(error) = crate::migrate(&pool).await {
            pool.close().await;
            let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
                .execute(&control)
                .await;
            control.close().await;
            return Err(format!("migrate: {error}"));
        }
        let current: String = match sqlx::query_scalar("SELECT current_schema()").fetch_one(&pool).await {
            Ok(current) => current,
            Err(error) => {
                pool.close().await;
                let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
                    .execute(&control)
                    .await;
                control.close().await;
                return Err(format!("current_schema: {error}"));
            }
        };
        if current != schema {
            pool.close().await;
            let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
                .execute(&control)
                .await;
            control.close().await;
            return Err(format!("search_path is {current}, expected {schema}"));
        }
        Ok(Self { pool, control, schema })
    }

    async fn cleanup(self) -> TestResult {
        self.pool.close().await;
        let _ = sqlx::query("SET lock_timeout = '5s'").execute(&self.control).await;
        let dropped = sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{}\" CASCADE", self.schema))
            .execute(&self.control)
            .await;
        self.control.close().await;
        dropped.map(|_| ()).map_err(|e| format!("drop schema: {e}"))
    }
}

async fn call(pool: &PgPool, actor: Option<&str>, method: Method, path: &str, body: Value) -> (actix_web::http::StatusCode, Value) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(crate::AppState::local(pool.clone())))
            .configure(crate::configure),
    )
    .await;
    let mut req = test::TestRequest::default().method(method).uri(path).set_json(body);
    if let Some(actor) = actor {
        req = req.insert_header(("x-nexus-actor", actor));
    }
    let response = test::call_service(&app, req.to_request()).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn parents(pool: &PgPool) -> TestResult<(Uuid, Uuid, Uuid, Uuid)> {
    let email = format!("synthetic-draft-{}@example.invalid", Uuid::new_v4().simple());
    let (status, setup) = call(
        pool,
        None,
        Method::POST,
        "/api/setup",
        json!({"organization_name":"Synthetic terminal QA","facility_name":"Synthetic facility","admin_name":"Synthetic reviewer","admin_email":email}),
    )
    .await;
    if !status.is_success() {
        return Err(format!("setup {status}: {setup}"));
    }
    let actor = setup["actor"]["id"].as_str().ok_or("setup missing actor")?.to_owned();
    let actor_id = Uuid::parse_str(&actor).map_err(|e| e.to_string())?;
    let org = Uuid::parse_str(setup["actor"]["organization_id"].as_str().ok_or("setup missing organization")?).map_err(|e| e.to_string())?;
    let facility = setup["facility"]["id"].as_str().ok_or("setup missing facility")?.to_owned();
    let (status, client) = call(
        pool,
        Some(&actor),
        Method::POST,
        "/api/clients",
        json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Terminal","birth_date":"1990-01-01","primary_clinician_id":actor}),
    )
    .await;
    if !status.is_success() {
        return Err(format!("client {status}: {client}"));
    }
    let client_id = Uuid::parse_str(client["id"].as_str().ok_or("client missing id")?).map_err(|e| e.to_string())?;
    let (status, template) = call(
        pool,
        Some(&actor),
        Method::POST,
        "/api/templates",
        json!({"name":"Synthetic clinical form","kind":"progress","fields":[{"key":"observation","label":"Observation","type":"textarea","required":true}]}),
    )
    .await;
    if !status.is_success() {
        return Err(format!("template {status}: {template}"));
    }
    let template_id = Uuid::parse_str(template["id"].as_str().ok_or("template missing id")?).map_err(|e| e.to_string())?;
    Ok((org, actor_id, client_id, template_id))
}

fn sources(marker: &str) -> Value {
    json!([{
        "id": Uuid::new_v4(),
        "title": format!("Synthetic source {marker}"),
        "text": format!("Synthetic observation only ({marker}). Do not rewrite."),
        "source_hash": format!("synthetic-source-{marker}"),
        "source_version": 1
    }])
}

fn snapshot() -> Value {
    json!({"name":"Synthetic clinical form","kind":"progress","fields":[{"key":"observation","label":"Observation","type":"textarea","required":true}]})
}

fn ready_output() -> Value {
    json!({
        "model": "synthetic-engine-fixture",
        "model_sha256": "a".repeat(64),
        "prompt_version": "synthetic-fixture",
        "sections": [{"key":"observation","label":"Observation","statements":[{"text":"Synthetic appointment on Monday.","citations":[]}]}],
        "unresolved": ["Synthetic fixture does not establish care."]
    })
}

async fn insert_queued(pool: &PgPool, id: Uuid, parents: (Uuid, Uuid, Uuid, Uuid), title: &str, purpose: &str, sources: &Value) -> TestResult {
    let (org, client, actor, template) = parents;
    sqlx::query("INSERT INTO clinical_draft_jobs(id,organization_id,client_id,requested_by,template_id,title,purpose,template_snapshot,sources) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(id)
        .bind(org)
        .bind(client)
        .bind(actor)
        .bind(template)
        .bind(title)
        .bind(purpose)
        .bind(snapshot())
        .bind(sources)
        .execute(pool)
        .await
        .map_err(|e| format!("insert draft {title}: {e}"))?;
    Ok(())
}

async fn transition(pool: &PgPool, sql: &str, id: Uuid, output: Option<Value>) -> TestResult {
    let mut query = sqlx::query(sql).bind(id);
    if let Some(output) = output {
        query = query.bind(output);
    }
    let changed = query.execute(pool).await.map_err(|e| format!("fixture transition: {e}"))?.rows_affected();
    if changed != 1 {
        return Err(format!("fixture transition changed {changed} rows"));
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Snap {
    raw: Value,
    state: String,
    error_code: String,
    completed_is_null: bool,
    sources: Value,
    output: Option<Value>,
    purpose: String,
    title: String,
    template_snapshot: Value,
    started_text: Option<String>,
}

async fn snap(pool: &PgPool, id: Uuid) -> TestResult<Snap> {
    let row = sqlx::query("SELECT to_jsonb(clinical_draft_jobs) AS raw, state AS job_state, error_code, (completed_at IS NULL) AS completed_is_null, sources, output AS job_output, purpose, title, template_snapshot, started_at::text AS started_text FROM clinical_draft_jobs WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("snap {id}: {e}"))?;
    Ok(Snap {
        raw: row.try_get("raw").map_err(|e| e.to_string())?,
        state: row.try_get("job_state").map_err(|e| e.to_string())?,
        error_code: row.try_get("error_code").map_err(|e| e.to_string())?,
        completed_is_null: row.try_get("completed_is_null").map_err(|e| e.to_string())?,
        sources: row.try_get("sources").map_err(|e| e.to_string())?,
        output: row.try_get("job_output").map_err(|e| e.to_string())?,
        purpose: row.try_get("purpose").map_err(|e| e.to_string())?,
        title: row.try_get("title").map_err(|e| e.to_string())?,
        template_snapshot: row.try_get("template_snapshot").map_err(|e| e.to_string())?,
        started_text: row.try_get("started_text").map_err(|e| e.to_string())?,
    })
}

async fn audit_count(pool: &PgPool, id: Uuid) -> TestResult<i64> {
    sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE record_id=$1 AND action='generation_failed' AND resource='clinical_draft_jobs'")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
}

async fn install_audit_failure(pool: &PgPool) -> TestResult {
    sqlx::query("CREATE SEQUENCE draft_terminal_audit_fail START WITH 1")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        r#"CREATE FUNCTION draft_terminal_audit_fail() RETURNS trigger LANGUAGE plpgsql AS $fn$
        BEGIN
          IF NEW.action = 'generation_failed' AND nextval('draft_terminal_audit_fail') = 1 THEN
            RAISE EXCEPTION 'injected terminal audit failure' USING ERRCODE = '40001';
          END IF;
          RETURN NEW;
        END;
        $fn$"#,
    )
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("CREATE TRIGGER draft_terminal_audit_fail BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION draft_terminal_audit_fail()")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

struct Jobs {
    org: Uuid,
    actor: Uuid,
    running: Uuid,
    cancelled: Uuid,
    ready: Uuid,
}

async fn jobs(pool: &PgPool) -> TestResult<Jobs> {
    let (org, actor, client, template) = parents(pool).await?;
    let cancelled = Uuid::new_v4();
    let ready = Uuid::new_v4();
    let running = Uuid::new_v4();
    insert_queued(pool, cancelled, (org, client, actor, template), "Synthetic cancelled draft", "Synthetic cancelled terminal fixture purpose.", &sources("cancelled")).await?;
    transition(pool, "UPDATE clinical_draft_jobs SET state='cancelled', error_code='cancelled_by_operator', completed_at=clock_timestamp() WHERE id=$1 AND state='queued'", cancelled, None).await?;
    insert_queued(pool, ready, (org, client, actor, template), "Synthetic ready draft", "Synthetic ready terminal fixture purpose.", &sources("ready")).await?;
    transition(pool, "UPDATE clinical_draft_jobs SET state='running', started_at=clock_timestamp() WHERE id=$1 AND state='queued'", ready, None).await?;
    transition(pool, "UPDATE clinical_draft_jobs SET state='ready', output=$2, completed_at=clock_timestamp() WHERE id=$1 AND state='running'", ready, Some(ready_output())).await?;
    insert_queued(pool, running, (org, client, actor, template), "Synthetic running draft", "Synthetic running terminal fixture purpose.", &sources("running")).await?;
    transition(pool, "UPDATE clinical_draft_jobs SET state='running', started_at=clock_timestamp() WHERE id=$1 AND state='queued'", running, None).await?;
    Ok(Jobs { org, actor, running, cancelled, ready })
}

fn contains(ids: &std::collections::VecDeque<Uuid>, id: Uuid) -> bool {
    ids.iter().any(|found| *found == id)
}

async fn audit_case(pool: &PgPool) -> TestResult {
    let jobs = jobs(pool).await?;
    install_audit_failure(pool).await?;
    let mut owner = pool.acquire().await.map_err(|e| e.to_string())?.detach();
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(ADVISORY_LOCK_KEY).fetch_one(&mut owner).await.map_err(|e| e.to_string())?;
    ensure!(locked, "another nexus_test session owns this worker key; run serially");

    let running_before = snap(pool, jobs.running).await?;
    let cancelled_before = snap(pool, jobs.cancelled).await?;
    let ready_before = snap(pool, jobs.ready).await?;
    ensure!(running_before.state == "running" && running_before.error_code.is_empty(), "running fixture {running_before:?}");
    ensure!(cancelled_before.state == "cancelled" && cancelled_before.error_code == "cancelled_by_operator", "cancelled fixture {cancelled_before:?}");
    ensure!(ready_before.state == "ready" && ready_before.output.as_ref().is_some_and(|v| v["model"] == "synthetic-engine-fixture"), "ready fixture {ready_before:?}");
    let found = owned_interrupted(&mut owner).await.map_err(|e| e.to_string())?;
    ensure!(contains(&found, jobs.running), "recovery query missed running id: {found:?}");
    ensure!(!contains(&found, jobs.cancelled) && !contains(&found, jobs.ready), "recovery query included terminal ids: {found:?}");
    let first = persist_terminal(&mut owner, jobs.running, "database").await;
    ensure!(first == TerminalPersist::Retry, "audit failure was not retained as retry: {first:?}");
    let running_mid = snap(pool, jobs.running).await?;
    ensure!(running_mid == running_before, "rolled-back terminal write changed the running row\nbefore {running_before:?}\nafter {running_mid:?}");
    ensure!(audit_count(pool, jobs.running).await? == 0, "audit row survived rollback");
    let still = owned_interrupted(&mut owner).await.map_err(|e| e.to_string())?;
    ensure!(contains(&still, jobs.running), "running id dropped while unsaved");
    let cancelled_hold = persist_terminal(&mut owner, jobs.cancelled, "database").await;
    ensure!(cancelled_hold == TerminalPersist::Saved, "cancelled no-op was not saved: {cancelled_hold:?}");
    ensure!(snap(pool, jobs.cancelled).await? == cancelled_before, "cancelled history changed");
    ensure!(audit_count(pool, jobs.cancelled).await? == 0, "cancelled job gained generation_failed audit");
    let ready_hold = persist_terminal(&mut owner, jobs.ready, "database").await;
    ensure!(ready_hold == TerminalPersist::Saved, "ready no-op was not saved: {ready_hold:?}");
    ensure!(snap(pool, jobs.ready).await? == ready_before, "ready output or sources changed");
    ensure!(audit_count(pool, jobs.ready).await? == 0, "ready job gained generation_failed audit");
    let second = persist_terminal(&mut owner, jobs.running, "database").await;
    ensure!(second == TerminalPersist::Saved, "second terminal write did not commit: {second:?}");
    let saved = snap(pool, jobs.running).await?;
    ensure!(saved.state == "failed", "state {}", saved.state);
    ensure!(saved.error_code == "database", "error code rewritten: {}", saved.error_code);
    ensure!(!saved.completed_is_null, "completed_at was not set");
    ensure!(saved.sources == running_before.sources, "sources rewritten");
    ensure!(saved.output == running_before.output, "output rewritten");
    ensure!(saved.template_snapshot == running_before.template_snapshot, "template snapshot rewritten");
    ensure!(saved.purpose == running_before.purpose && saved.title == running_before.title, "provenance text rewritten");
    ensure!(saved.started_text == running_before.started_text, "started_at rewritten");
    ensure!(audit_count(pool, jobs.running).await? == 1, "expected exactly one generation_failed audit");
    let audit = sqlx::query("SELECT action, actor_id, organization_id, resource FROM audit_events WHERE record_id=$1 AND action='generation_failed'")
        .bind(jobs.running)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    let action: String = audit.try_get("action").map_err(|e| e.to_string())?;
    let audit_actor: Uuid = audit.try_get("actor_id").map_err(|e| e.to_string())?;
    let audit_org: Uuid = audit.try_get("organization_id").map_err(|e| e.to_string())?;
    let resource: String = audit.try_get("resource").map_err(|e| e.to_string())?;
    ensure!(action == "generation_failed" && resource == "clinical_draft_jobs" && audit_actor == jobs.actor && audit_org == jobs.org, "audit payload mismatch");
    let after = owned_interrupted(&mut owner).await.map_err(|e| e.to_string())?;
    ensure!(!contains(&after, jobs.running), "failed id still recovered as running");
    let rewrite = persist_terminal(&mut owner, jobs.running, "conflict").await;
    ensure!(rewrite == TerminalPersist::Saved, "failed-history no-op: {rewrite:?}");
    let kept = snap(pool, jobs.running).await?;
    ensure!(kept.error_code == "database" && kept.state == "failed", "failed history rewritten: {kept:?}");
    ensure!(audit_count(pool, jobs.running).await? == 1, "failed history gained another audit");
    ensure!(snap(pool, jobs.cancelled).await? == cancelled_before, "cancelled row changed after success");
    ensure!(snap(pool, jobs.ready).await? == ready_before, "ready row changed after success");
    let pending_cancel = Uuid::new_v4();
    let (client, template): (Uuid, Uuid) = sqlx::query_as("SELECT client_id,template_id FROM clinical_draft_jobs WHERE id=$1")
        .bind(jobs.running).fetch_one(pool).await.map_err(|e| e.to_string())?;
    insert_queued(pool, pending_cancel, (jobs.org, client, jobs.actor, template), "Synthetic cancellation race", "Synthetic pending failure cancellation.", &sources("pending-cancel")).await?;
    transition(pool, "UPDATE clinical_draft_jobs SET state='running', started_at=clock_timestamp() WHERE id=$1 AND state='queued'", pending_cancel, None).await?;
    sqlx::query("SELECT setval('draft_terminal_audit_fail',1,false)").execute(pool).await.map_err(|e| e.to_string())?;
    ensure!(persist_terminal(&mut owner, pending_cancel, "database").await == TerminalPersist::Retry, "expected unsaved failure before cancellation");
    let (status, _) = call(pool, Some(&jobs.actor.to_string()), Method::POST, &format!("/api/clinical-drafts/{pending_cancel}/cancel"), json!({})).await;
    ensure!(status.is_success(), "cancel request failed: {status}");
    let cancelled_between = snap(pool, pending_cancel).await?;
    ensure!(cancelled_between.state == "cancelled", "cancellation did not persist");
    ensure!(persist_terminal(&mut owner, pending_cancel, "database").await == TerminalPersist::Saved, "cancelled pending failure should resolve without overwrite");
    ensure!(snap(pool, pending_cancel).await? == cancelled_between, "pending failure overwrote cancellation history");
    ensure!(audit_count(pool, pending_cancel).await? == 0, "cancelled pending job gained failure audit");
    drop(owner);
    Ok(())
}

async fn ownership_case(harness: &Harness) -> TestResult {
    let jobs = jobs(&harness.pool).await?;
    let before = snap(&harness.pool, jobs.running).await?;
    let app_name = "nexus_draft_terminal_owner";
    let mut old = harness.pool.acquire().await.map_err(|e| e.to_string())?.detach();
    sqlx::query("SELECT set_config('application_name', $1, false)")
        .bind(app_name)
        .execute(&mut old)
        .await
        .map_err(|e| e.to_string())?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .fetch_one(&mut old)
        .await
        .map_err(|e| e.to_string())?;
    ensure!(locked, "could not acquire drafting advisory lock; another nexus_test session holds it");
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&mut old).await.map_err(|e| e.to_string())?;
    let mut challenger = harness.pool.acquire().await.map_err(|e| e.to_string())?.detach();
    let challenger_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&mut challenger).await.map_err(|e| e.to_string())?;
    let control_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&harness.control).await.map_err(|e| e.to_string())?;
    ensure!(pid != challenger_pid && pid != control_pid, "refusing to terminate a non-owner backend");
    let blocked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .fetch_one(&mut challenger)
        .await
        .map_err(|e| e.to_string())?;
    ensure!(!blocked, "challenger acquired the lock while the old owner was alive");
    let terminated: Option<bool> = sqlx::query_scalar("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE pid=$1 AND application_name=$2")
        .bind(pid)
        .bind(app_name)
        .fetch_optional(&harness.control)
        .await
        .map_err(|e| e.to_string())?;
    ensure!(terminated == Some(true), "targeted backend was not terminated: {terminated:?}");
    let mut new_locked = false;
    for _ in 0..20 {
        new_locked = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(ADVISORY_LOCK_KEY)
            .fetch_one(&mut challenger)
            .await
            .map_err(|e| e.to_string())?;
        if new_locked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    ensure!(new_locked, "new owner did not acquire after the dedicated backend was terminated");
    let mut outsider = harness.pool.acquire().await.map_err(|e| e.to_string())?.detach();
    let outsider_locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(ADVISORY_LOCK_KEY)
        .fetch_one(&mut outsider)
        .await
        .map_err(|e| e.to_string())?;
    if outsider_locked {
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)").bind(ADVISORY_LOCK_KEY).execute(&mut outsider).await;
        drop(outsider);
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)").bind(ADVISORY_LOCK_KEY).execute(&mut challenger).await;
        return Err("new owner does not exclusively hold the advisory lock".into());
    }
    drop(outsider);
    let outcome = persist_terminal(&mut old, jobs.running, "worker_interrupted").await;
    ensure!(outcome == TerminalPersist::OwnershipLost, "old session persisted after the new owner acquired: {outcome:?}");
    let after = snap(&harness.pool, jobs.running).await?;
    ensure!(after == before, "old session changed the row after ownership moved\nbefore {before:?}\nafter {after:?}");
    ensure!(audit_count(&harness.pool, jobs.running).await? == 0, "ownership-loss path wrote an audit");
    let visible: String = sqlx::query_scalar("SELECT state FROM clinical_draft_jobs WHERE id=$1")
        .bind(jobs.running)
        .fetch_one(&mut challenger)
        .await
        .map_err(|e| e.to_string())?;
    ensure!(visible == "running", "new owner does not see the preserved running row: {visible}");
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)").bind(ADVISORY_LOCK_KEY).execute(&mut challenger).await;
    drop(challenger);
    drop(old);
    Ok(())
}

#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn draft_terminal_write_rolls_back_failed_audit_then_saves_one() {
    let harness = Harness::create("nexus_draft_terminal").await.unwrap_or_else(|e| panic!("{e}"));
    let result = audit_case(&harness.pool).await;
    let cleanup = harness.cleanup().await;
    result.unwrap();
    cleanup.unwrap();
}

#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn draft_lost_owner_cannot_terminal_write_after_new_owner_acquires() {
    let harness = Harness::create("nexus_draft_owner").await.unwrap_or_else(|e| panic!("{e}"));
    let result = ownership_case(&harness).await;
    let cleanup = harness.cleanup().await;
    result.unwrap();
    cleanup.unwrap();
}
