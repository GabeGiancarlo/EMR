// Included from telehealth::recording::transcripts::worker::tests. Not a standalone module.
// Synthetic state fixtures only. Does not start the worker loop, decoder, or recognizer.
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
            .app_data(web::Data::new(crate::AppState { pool: pool.clone() }))
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

struct Chain {
    org: Uuid,
    actor: Uuid,
    room: Uuid,
    recording: Uuid,
    track: Uuid,
}

async fn chain(pool: &PgPool) -> TestResult<Chain> {
    let email = format!("synthetic-transcript-{}@example.invalid", Uuid::new_v4().simple());
    let (status, setup) = call(
        pool,
        None,
        Method::POST,
        "/api/setup",
        json!({"organization_name":"Synthetic transcript terminal QA","facility_name":"Synthetic facility","admin_name":"Synthetic reviewer","admin_email":email}),
    )
    .await;
    if !status.is_success() {
        return Err(format!("setup {status}: {setup}"));
    }
    let actor_text = setup["actor"]["id"].as_str().ok_or("setup missing actor")?.to_owned();
    let actor = Uuid::parse_str(&actor_text).map_err(|e| e.to_string())?;
    let org = Uuid::parse_str(setup["actor"]["organization_id"].as_str().ok_or("setup missing organization")?).map_err(|e| e.to_string())?;
    let facility = Uuid::parse_str(setup["facility"]["id"].as_str().ok_or("setup missing facility")?).map_err(|e| e.to_string())?;
    let (status, client) = call(
        pool,
        Some(&actor_text),
        Method::POST,
        "/api/clients",
        json!({"facility_id":facility,"first_name":"Synthetic","last_name":"Transcript","birth_date":"1990-01-01","primary_clinician_id":actor_text}),
    )
    .await;
    if !status.is_success() {
        return Err(format!("client {status}: {client}"));
    }
    let client_id = Uuid::parse_str(client["id"].as_str().ok_or("client missing id")?).map_err(|e| e.to_string())?;
    let episode = Uuid::new_v4();
    sqlx::query("INSERT INTO episodes(id,organization_id,client_id,facility_id,level_of_care,start_date) VALUES($1,$2,$3,$4,'iop',DATE '2026-01-01')")
        .bind(episode)
        .bind(org)
        .bind(client_id)
        .bind(facility)
        .execute(pool)
        .await
        .map_err(|e| format!("episode insert used foundation columns only: {e}"))?;
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO sessions(id,organization_id,client_id,episode_id,clinician_id,starts_at,ends_at,kind) VALUES($1,$2,$3,$4,$5,'2030-01-01T09:00:00Z','2030-01-01T10:00:00Z','individual')")
        .bind(session)
        .bind(org)
        .bind(client_id)
        .bind(episode)
        .bind(actor)
        .execute(pool)
        .await
        .map_err(|e| format!("session insert used foundation columns only: {e}"))?;
    let (status, room) = call(pool, Some(&actor_text), Method::POST, "/api/telehealth/rooms", json!({"session_id":session})).await;
    if !status.is_success() {
        return Err(format!("room {status}: {room}"));
    }
    let room_id = Uuid::parse_str(room["id"].as_str().ok_or("room missing id")?).map_err(|e| e.to_string())?;
    let (status, guest) = call(
        pool,
        Some(&actor_text),
        Method::POST,
        &format!("/api/telehealth/rooms/{room_id}/invite"),
        json!({"client_id":client_id}),
    )
    .await;
    if !status.is_success() {
        return Err(format!("invite {status}: {guest}"));
    }
    let person = Uuid::parse_str(guest["participant"]["id"].as_str().ok_or("invite missing participant")?).map_err(|e| e.to_string())?;
    let (status, policy) = call(
        pool,
        Some(&actor_text),
        Method::POST,
        "/api/recording-policies",
        json!({"title":"Synthetic capture","purpose":"Synthetic transcript terminal fixture only","retention_days":7,"allow_video":false}),
    )
    .await;
    if !status.is_success() {
        return Err(format!("recording policy {status}: {policy}"));
    }
    let policy_id = Uuid::parse_str(policy["id"].as_str().ok_or("policy missing id")?).map_err(|e| e.to_string())?;
    let recording = Uuid::new_v4();
    let track = Uuid::new_v4();
    sqlx::query("INSERT INTO recordings(id,organization_id,room_id,policy_id,policy_snapshot,mode,state,created_by,retain_until) VALUES($1,$2,$3,$4,$5,'audio','ended',$6,now()+interval '7 days')")
        .bind(recording)
        .bind(org)
        .bind(room_id)
        .bind(policy_id)
        .bind(&policy)
        .bind(actor)
        .execute(pool)
        .await
        .map_err(|e| format!("recording insert followed transcripts_postgres.rs: {e}"))?;
    sqlx::query("INSERT INTO recording_participants(organization_id,recording_id,room_id,participant_id,decision) VALUES($1,$2,$3,$4,'accepted')")
        .bind(org)
        .bind(recording)
        .bind(room_id)
        .bind(person)
        .execute(pool)
        .await
        .map_err(|e| format!("recording participant insert followed transcripts_postgres.rs: {e}"))?;
    sqlx::query("INSERT INTO recording_tracks(id,organization_id,recording_id,participant_id,connection_id,mime_type,token_hash,state,expected_chunks,finished_at) VALUES($1,$2,$3,$4,$5,'audio/webm',$6,'complete',1,now())")
        .bind(track)
        .bind(org)
        .bind(recording)
        .bind(person)
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4().to_string())
        .execute(pool)
        .await
        .map_err(|e| format!("track insert followed transcripts_postgres.rs: {e}"))?;
    Ok(Chain { org, actor, room: room_id, recording, track })
}

async fn insert_queued(pool: &PgPool, chain: &Chain, id: Uuid, purpose: &str, consent: &Value) -> TestResult {
    sqlx::query("INSERT INTO transcription_jobs(id,organization_id,room_id,recording_id,track_id,requested_by,purpose,consent_snapshot) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(id)
        .bind(chain.org)
        .bind(chain.room)
        .bind(chain.recording)
        .bind(chain.track)
        .bind(chain.actor)
        .bind(purpose)
        .bind(consent)
        .execute(pool)
        .await
        .map_err(|e| format!("insert transcription job: {e}"))?;
    Ok(())
}

async fn transition(pool: &PgPool, sql: &str, id: Uuid) -> TestResult {
    let changed = sqlx::query(sql).bind(id).execute(pool).await.map_err(|e| format!("fixture transition: {e}"))?.rows_affected();
    if changed != 1 {
        return Err(format!("fixture transition changed {changed} rows"));
    }
    Ok(())
}

fn segments() -> Value {
    json!([{
        "id": Uuid::new_v4(),
        "start_ms": 0,
        "end_ms": 1000,
        "text": "Synthetic appointment on Monday.",
        "speaker_participant_id": null,
        "speaker_verified": false,
        "patient_id": null,
        "context_verified": false,
        "uncertain": true,
        "excluded": false,
        "review_note": ""
    }])
}

async fn insert_transcript(pool: &PgPool, chain: &Chain, job: Uuid, segments: &Value) -> TestResult<Uuid> {
    let doc = Uuid::new_v4();
    let version = Uuid::new_v4();
    let hash = "a".repeat(64);
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO transcripts(id,organization_id,room_id,recording_id,track_id,job_id,duration_ms,engine,model_label,model_sha256,source_sha256,recognized_segments,current_version_id) VALUES($1,$2,$3,$4,$5,$6,2000,'synthetic-test-fixture','not-a-real-recognizer',$7,$7,$8,$9)")
        .bind(doc)
        .bind(chain.org)
        .bind(chain.room)
        .bind(chain.recording)
        .bind(chain.track)
        .bind(job)
        .bind(&hash)
        .bind(segments)
        .bind(version)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("transcript insert: {e}"))?;
    sqlx::query("INSERT INTO transcript_versions(id,organization_id,transcript_id,number,segments,reason,created_by) VALUES($1,$2,$3,1,$4,'Explicit synthetic review fixture',$5)")
        .bind(version)
        .bind(chain.org)
        .bind(doc)
        .bind(segments)
        .bind(chain.actor)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("transcript version insert: {e}"))?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(doc)
}

struct Jobs {
    chain: Chain,
    running: Uuid,
    cancelled: Uuid,
    ready: Uuid,
    transcript: Uuid,
    segments: Value,
}

async fn jobs(pool: &PgPool) -> TestResult<Jobs> {
    let chain = chain(pool).await?;
    let cancelled = Uuid::new_v4();
    let ready = Uuid::new_v4();
    let running = Uuid::new_v4();
    let recognized = segments();
    insert_queued(pool, &chain, cancelled, "Synthetic cancelled processing fixture purpose.", &json!([{"id":"synthetic-cancelled","kind":"transcription"}])).await?;
    transition(pool, "UPDATE transcription_jobs SET state='cancelled', error_code='cancelled_by_operator', completed_at=clock_timestamp() WHERE id=$1 AND state='queued'", cancelled).await?;
    insert_queued(pool, &chain, ready, "Synthetic ready processing fixture purpose.", &json!([{"id":"synthetic-ready","kind":"transcription"}])).await?;
    transition(pool, "UPDATE transcription_jobs SET state='running', started_at=clock_timestamp() WHERE id=$1 AND state='queued'", ready).await?;
    transition(pool, "UPDATE transcription_jobs SET state='ready', completed_at=clock_timestamp() WHERE id=$1 AND state='running'", ready).await?;
    let transcript = insert_transcript(pool, &chain, ready, &recognized).await?;
    insert_queued(pool, &chain, running, "Synthetic running processing fixture purpose.", &json!([{"id":"synthetic-running","kind":"transcription","scope":"synthetic terminal failure fixture"}])).await?;
    transition(pool, "UPDATE transcription_jobs SET state='running', started_at=clock_timestamp() WHERE id=$1 AND state='queued'", running).await?;
    Ok(Jobs { chain, running, cancelled, ready, transcript, segments: recognized })
}

#[derive(Debug, PartialEq)]
struct Snap {
    raw: Value,
    state: String,
    error_code: String,
    completed_is_null: bool,
    consent_snapshot: Value,
    purpose: String,
    started_text: Option<String>,
}

async fn snap(pool: &PgPool, id: Uuid) -> TestResult<Snap> {
    let row = sqlx::query("SELECT to_jsonb(transcription_jobs) AS raw, state AS job_state, error_code, (completed_at IS NULL) AS completed_is_null, consent_snapshot, purpose, started_at::text AS started_text FROM transcription_jobs WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("snap {id}: {e}"))?;
    Ok(Snap {
        raw: row.try_get("raw").map_err(|e| e.to_string())?,
        state: row.try_get("job_state").map_err(|e| e.to_string())?,
        error_code: row.try_get("error_code").map_err(|e| e.to_string())?,
        completed_is_null: row.try_get("completed_is_null").map_err(|e| e.to_string())?,
        consent_snapshot: row.try_get("consent_snapshot").map_err(|e| e.to_string())?,
        purpose: row.try_get("purpose").map_err(|e| e.to_string())?,
        started_text: row.try_get("started_text").map_err(|e| e.to_string())?,
    })
}

async fn audit_count(pool: &PgPool, id: Uuid) -> TestResult<i64> {
    sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE record_id=$1 AND action='processing_failed' AND resource='transcription_jobs'")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
}

async fn evidence_unchanged(pool: &PgPool, jobs: &Jobs) -> TestResult {
    let row = sqlx::query("SELECT engine, model_label, model_sha256, source_sha256, recognized_segments FROM transcripts WHERE id=$1")
        .bind(jobs.transcript)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("transcript evidence: {e}"))?;
    let engine: String = row.try_get("engine").map_err(|e| e.to_string())?;
    let model_label: String = row.try_get("model_label").map_err(|e| e.to_string())?;
    let model_sha256: String = row.try_get("model_sha256").map_err(|e| e.to_string())?;
    let source_sha256: String = row.try_get("source_sha256").map_err(|e| e.to_string())?;
    let recognized: Value = row.try_get("recognized_segments").map_err(|e| e.to_string())?;
    ensure!(engine == "synthetic-test-fixture", "engine changed: {engine}");
    ensure!(model_label == "not-a-real-recognizer", "model label changed: {model_label}");
    ensure!(model_sha256 == "a".repeat(64), "model hash changed");
    ensure!(source_sha256 == "a".repeat(64), "source hash changed");
    ensure!(recognized == jobs.segments, "recognized segments changed: {recognized}");
    ensure!(recognized[0]["text"] == "Synthetic appointment on Monday.", "segment text changed");
    let version_segments: Value = sqlx::query_scalar("SELECT segments FROM transcript_versions WHERE transcript_id=$1")
        .bind(jobs.transcript)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    ensure!(version_segments == jobs.segments, "version segments changed");
    let running_docs: i64 = sqlx::query_scalar("SELECT count(*) FROM transcripts WHERE job_id=$1")
        .bind(jobs.running)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    ensure!(running_docs == 0, "terminal failure created a transcript for the running job");
    Ok(())
}

async fn install_audit_failure(pool: &PgPool) -> TestResult {
    sqlx::query("CREATE SEQUENCE transcript_terminal_audit_fail START WITH 1")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        r#"CREATE FUNCTION transcript_terminal_audit_fail() RETURNS trigger LANGUAGE plpgsql AS $fn$
        BEGIN
          IF NEW.action = 'processing_failed' AND nextval('transcript_terminal_audit_fail') = 1 THEN
            RAISE EXCEPTION 'injected terminal audit failure' USING ERRCODE = '40001';
          END IF;
          RETURN NEW;
        END;
        $fn$"#,
    )
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("CREATE TRIGGER transcript_terminal_audit_fail BEFORE INSERT ON audit_events FOR EACH ROW EXECUTE FUNCTION transcript_terminal_audit_fail()")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
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
    evidence_unchanged(pool, &jobs).await?;
    ensure!(running_before.state == "running" && running_before.error_code.is_empty(), "running fixture {running_before:?}");
    ensure!(cancelled_before.state == "cancelled" && cancelled_before.error_code == "cancelled_by_operator", "cancelled fixture {cancelled_before:?}");
    ensure!(ready_before.state == "ready", "ready fixture {ready_before:?}");
    let found = owned_interrupted(&mut owner).await.map_err(|e| e.to_string())?;
    ensure!(contains(&found, jobs.running), "recovery query missed running id: {found:?}");
    ensure!(!contains(&found, jobs.cancelled) && !contains(&found, jobs.ready), "recovery query included terminal ids: {found:?}");
    let first = persist_terminal(&mut owner, jobs.running, "database").await;
    ensure!(first == TerminalPersist::Retry, "audit failure was not retained as retry: {first:?}");
    ensure!(snap(pool, jobs.running).await? == running_before, "rolled-back terminal write changed the running row");
    ensure!(audit_count(pool, jobs.running).await? == 0, "audit row survived rollback");
    evidence_unchanged(pool, &jobs).await?;
    let still = owned_interrupted(&mut owner).await.map_err(|e| e.to_string())?;
    ensure!(contains(&still, jobs.running), "running id dropped while unsaved");
    ensure!(persist_terminal(&mut owner, jobs.cancelled, "database").await == TerminalPersist::Saved, "cancelled no-op failed");
    ensure!(snap(pool, jobs.cancelled).await? == cancelled_before, "cancelled history changed");
    ensure!(audit_count(pool, jobs.cancelled).await? == 0, "cancelled job gained processing_failed audit");
    ensure!(persist_terminal(&mut owner, jobs.ready, "database").await == TerminalPersist::Saved, "ready no-op failed");
    ensure!(snap(pool, jobs.ready).await? == ready_before, "ready job changed");
    ensure!(audit_count(pool, jobs.ready).await? == 0, "ready job gained processing_failed audit");
    evidence_unchanged(pool, &jobs).await?;
    let second = persist_terminal(&mut owner, jobs.running, "database").await;
    ensure!(second == TerminalPersist::Saved, "second terminal write did not commit: {second:?}");
    let saved = snap(pool, jobs.running).await?;
    ensure!(saved.state == "failed", "state {}", saved.state);
    ensure!(saved.error_code == "database", "error code rewritten: {}", saved.error_code);
    ensure!(!saved.completed_is_null, "completed_at was not set");
    ensure!(saved.consent_snapshot == running_before.consent_snapshot, "consent snapshot rewritten");
    ensure!(saved.purpose == running_before.purpose, "purpose rewritten");
    ensure!(saved.started_text == running_before.started_text, "started_at rewritten");
    ensure!(audit_count(pool, jobs.running).await? == 1, "expected exactly one processing_failed audit");
    let audit = sqlx::query("SELECT action, actor_id, organization_id, resource FROM audit_events WHERE record_id=$1 AND action='processing_failed'")
        .bind(jobs.running)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    let action: String = audit.try_get("action").map_err(|e| e.to_string())?;
    let audit_actor: Uuid = audit.try_get("actor_id").map_err(|e| e.to_string())?;
    let audit_org: Uuid = audit.try_get("organization_id").map_err(|e| e.to_string())?;
    let resource: String = audit.try_get("resource").map_err(|e| e.to_string())?;
    ensure!(action == "processing_failed" && resource == "transcription_jobs" && audit_actor == jobs.chain.actor && audit_org == jobs.chain.org, "audit payload mismatch");
    let after = owned_interrupted(&mut owner).await.map_err(|e| e.to_string())?;
    ensure!(!contains(&after, jobs.running), "failed id still recovered as running");
    ensure!(persist_terminal(&mut owner, jobs.running, "conflict").await == TerminalPersist::Saved, "failed-history no-op failed");
    let kept = snap(pool, jobs.running).await?;
    ensure!(kept.state == "failed" && kept.error_code == "database", "failed history rewritten: {kept:?}");
    ensure!(audit_count(pool, jobs.running).await? == 1, "failed history gained another audit");
    ensure!(snap(pool, jobs.cancelled).await? == cancelled_before, "cancelled row changed after success");
    ensure!(snap(pool, jobs.ready).await? == ready_before, "ready row changed after success");
    evidence_unchanged(pool, &jobs).await?;
    let pending_cancel = Uuid::new_v4();
    insert_queued(pool, &jobs.chain, pending_cancel, "Synthetic pending failure cancellation.", &json!([{"scope":"synthetic state fixture"}])).await?;
    transition(pool, "UPDATE transcription_jobs SET state='running', started_at=clock_timestamp() WHERE id=$1 AND state='queued'", pending_cancel).await?;
    sqlx::query("SELECT setval('transcript_terminal_audit_fail',1,false)").execute(pool).await.map_err(|e| e.to_string())?;
    ensure!(persist_terminal(&mut owner, pending_cancel, "database").await == TerminalPersist::Retry, "expected unsaved failure before cancellation");
    let (status, _) = call(pool, Some(&jobs.chain.actor.to_string()), Method::POST, &format!("/api/transcription-jobs/{pending_cancel}/cancel"), json!({})).await;
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
    let chain = chain(&harness.pool).await?;
    let running = Uuid::new_v4();
    insert_queued(&harness.pool, &chain, running, "Synthetic ownership-loss processing fixture purpose.", &json!([{"id":"synthetic-owner","kind":"transcription"}])).await?;
    transition(&harness.pool, "UPDATE transcription_jobs SET state='running', started_at=clock_timestamp() WHERE id=$1 AND state='queued'", running).await?;
    let before = snap(&harness.pool, running).await?;
    let app_name = "nexus_transcript_terminal_owner";
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
    ensure!(locked, "could not acquire transcription advisory lock; another nexus_test session holds it");
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
    let outcome = persist_terminal(&mut old, running, "worker_interrupted").await;
    ensure!(outcome == TerminalPersist::OwnershipLost, "old session persisted after the new owner acquired: {outcome:?}");
    let after = snap(&harness.pool, running).await?;
    ensure!(after == before, "old session changed the row after ownership moved\nbefore {before:?}\nafter {after:?}");
    ensure!(audit_count(&harness.pool, running).await? == 0, "ownership-loss path wrote an audit");
    let docs: i64 = sqlx::query_scalar("SELECT count(*) FROM transcripts WHERE job_id=$1")
        .bind(running)
        .fetch_one(&harness.pool)
        .await
        .map_err(|e| e.to_string())?;
    ensure!(docs == 0, "ownership-loss path created transcript evidence");
    let visible: String = sqlx::query_scalar("SELECT state FROM transcription_jobs WHERE id=$1")
        .bind(running)
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
async fn transcript_terminal_write_rolls_back_failed_audit_then_saves_one() {
    let harness = Harness::create("nexus_transcript_terminal").await.unwrap_or_else(|e| panic!("{e}"));
    let result = audit_case(&harness.pool).await;
    let cleanup = harness.cleanup().await;
    result.unwrap();
    cleanup.unwrap();
}

#[actix_web::test]
#[ignore = "Requires isolated nexus_test schemas"]
async fn transcript_lost_owner_cannot_terminal_write_after_new_owner_acquires() {
    let harness = Harness::create("nexus_transcript_owner").await.unwrap_or_else(|e| panic!("{e}"));
    let result = ownership_case(&harness).await;
    let cleanup = harness.cleanup().await;
    result.unwrap();
    cleanup.unwrap();
}
