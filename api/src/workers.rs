//! Supervision of the background workers.
//!
//! Each worker owns its queue through a PostgreSQL session advisory lock held on a
//! dedicated connection. A worker body returns when it cannot own the queue: another
//! instance holds the lock, no connection could be had, or its owning session died.
//! The supervisor then starts it again after a pause. A body is only ever started
//! after the previous one has returned and dropped its session, and every start must
//! win the advisory lock again, so there is never more than one owner.
use serde_json::{json, Map, Value};
use std::{
    future::Future,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Connecting and asking for the advisory lock.
    Starting,
    /// This process owns the queue.
    Running,
    /// Another session owns the queue. Normal when several instances run.
    WaitingForLock,
    /// The worker stopped and will be started again after a pause.
    Restarting,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Phase::Starting => "starting",
            Phase::Running => "running",
            Phase::WaitingForLock => "waiting_for_lock",
            Phase::Restarting => "restarting",
        }
    }
}

/// Why a worker body returned. The class is all that is ever reported: no error
/// text, no identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exit {
    /// Another session holds the advisory lock.
    LockHeld,
    /// No dedicated connection could be opened, or the lock could not be requested.
    ConnectionUnavailable,
    /// The session that owned the advisory lock stopped answering.
    OwnershipLost,
    /// The worker task panicked.
    Panicked,
}

impl Exit {
    fn class(self) -> &'static str {
        match self {
            Exit::LockHeld => "lock_held",
            Exit::ConnectionUnavailable => "connection_unavailable",
            Exit::OwnershipLost => "ownership_lost",
            Exit::Panicked => "panicked",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Snapshot {
    phase: Phase,
    last_error: Option<&'static str>,
    restarts: u64,
}

/// One worker's state, shared between its supervisor and `/healthz`.
pub struct Status {
    name: &'static str,
    state: Mutex<Snapshot>,
}

impl Status {
    pub(crate) fn new(name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            name,
            state: Mutex::new(Snapshot {
                phase: Phase::Starting,
                last_error: None,
                restarts: 0,
            }),
        })
    }
    fn update(&self, change: impl FnOnce(&mut Snapshot)) {
        change(&mut self.state.lock().unwrap_or_else(PoisonError::into_inner));
    }
    fn snapshot(&self) -> Snapshot {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    /// Called by a worker body once it holds the advisory lock.
    pub(crate) fn running(&self) {
        self.update(|state| state.phase = Phase::Running);
    }
    pub fn phase(&self) -> Phase {
        self.snapshot().phase
    }
}

/// The workers started in this process. Empty when none were started, as in tests
/// that mount the routes alone.
#[derive(Clone, Default)]
pub struct Registry(Arc<Vec<Arc<Status>>>);

impl Registry {
    /// For `/healthz`: each worker's state, restart count and last error class.
    pub fn report(&self) -> Value {
        let mut report = Map::new();
        for worker in self.0.iter() {
            let state = worker.snapshot();
            report.insert(
                worker.name.into(),
                json!({
                    "state": state.phase.label(),
                    "restarts": state.restarts,
                    "last_error": state.last_error,
                }),
            );
        }
        Value::Object(report)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Policy {
    /// Pause before the first restart; doubles after each failure.
    pub first: Duration,
    /// Longest pause between restarts.
    pub longest: Duration,
    /// How often to ask again for a lock another session holds.
    pub lock_poll: Duration,
    /// A worker that ran this long before failing starts again from `first`.
    pub settled: Duration,
}

impl Policy {
    pub(crate) const STANDARD: Policy = Policy {
        first: Duration::from_secs(1),
        longest: Duration::from_secs(60),
        lock_poll: Duration::from_secs(30),
        settled: Duration::from_secs(120),
    };
}

/// Run `body` for ever, one run at a time, pausing between runs.
pub(crate) async fn supervise<F, Fut>(status: Arc<Status>, policy: Policy, body: F)
where
    F: Fn(Arc<Status>) -> Fut,
    Fut: Future<Output = Exit> + Send + 'static,
{
    let mut pause = policy.first;
    loop {
        status.update(|state| state.phase = Phase::Starting);
        let began = Instant::now();
        // Its own task, so a panic ends one run instead of the supervisor. The task
        // has finished, and dropped its database session, before this returns.
        let exit = tokio::spawn(body(status.clone()))
            .await
            .unwrap_or(Exit::Panicked);
        if exit == Exit::LockHeld {
            status.update(|state| state.phase = Phase::WaitingForLock);
            pause = policy.first;
            tokio::time::sleep(policy.lock_poll).await;
            continue;
        }
        if began.elapsed() >= policy.settled {
            pause = policy.first;
        }
        log::warn!(
            "Background worker {} stopped ({}); restarting in {} ms",
            status.name,
            exit.class(),
            pause.as_millis()
        );
        status.update(|state| {
            state.phase = Phase::Restarting;
            state.last_error = Some(exit.class());
            state.restarts += 1;
        });
        tokio::time::sleep(pause).await;
        pause = (pause * 2).min(policy.longest);
    }
}

/// Start a supervised worker and return its shared state.
pub(crate) fn spawn<F, Fut>(name: &'static str, policy: Policy, body: F) -> Arc<Status>
where
    F: Fn(Arc<Status>) -> Fut + Send + 'static,
    Fut: Future<Output = Exit> + Send + 'static,
{
    let status = Status::new(name);
    tokio::spawn(supervise(status.clone(), policy, body));
    status
}

pub(crate) fn registry(workers: Vec<Arc<Status>>) -> Registry {
    Registry(Arc::new(workers))
}

/// The PostgreSQL half of a worker's supervision contract, shared by both workers'
/// ignored database tests: a run yields to a held lock, a second run never becomes a
/// second owner, and a run ends (so the supervisor can restart it) when the session
/// that owns the lock is terminated.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use sqlx::PgPool;

    async fn until_running(status: &Status) -> Result<(), String> {
        for _ in 0..200 {
            if status.phase() == Phase::Running {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err("the worker never took ownership".into())
    }

    async fn owner_pid(control: &PgPool, key: i64) -> Result<Option<i32>, String> {
        // A bigint advisory key is stored as its high and low halves.
        sqlx::query_scalar("SELECT pid FROM pg_locks WHERE locktype='advisory' AND granted AND objsubid=1 AND classid::bigint=$1 AND objid::bigint=$2 AND database=(SELECT oid FROM pg_database WHERE datname=current_database())")
            .bind(key >> 32)
            .bind(key & 0xffff_ffff)
            .fetch_optional(control)
            .await
            .map_err(|error| error.to_string())
    }

    pub(crate) async fn ownership_case<F, Fut>(
        pool: &PgPool,
        control: &PgPool,
        key: i64,
        run: F,
    ) -> Result<(), String>
    where
        F: Fn(PgPool, Arc<Status>) -> Fut,
        Fut: Future<Output = Exit> + Send + 'static,
    {
        let text = |error: sqlx::Error| error.to_string();
        // Another session, standing in for another API instance, owns the queue.
        let mut other = pool.acquire().await.map_err(text)?.detach();
        let held: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(&mut other)
            .await
            .map_err(text)?;
        if !held {
            return Err("another nexus_test session owns this worker key; run serially".into());
        }
        let status = Status::new("synthetic");
        let yielded =
            tokio::time::timeout(Duration::from_secs(10), run(pool.clone(), status.clone()))
                .await
                .map_err(|_| "a run blocked on a held lock instead of yielding".to_string())?;
        if yielded != Exit::LockHeld || status.phase() == Phase::Running {
            return Err(format!("a run did not yield to the held lock: {yielded:?}"));
        }
        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(key)
            .execute(&mut other)
            .await
            .map_err(text)?;
        drop(other);

        // Now a run takes ownership and keeps working.
        let first = tokio::spawn(run(pool.clone(), status.clone()));
        until_running(&status).await?;
        // While it does, a second run is refused: never two owners.
        let second = Status::new("synthetic-second");
        let refused =
            tokio::time::timeout(Duration::from_secs(10), run(pool.clone(), second.clone()))
                .await
                .map_err(|_| "a second run blocked instead of yielding".to_string())?;
        if refused != Exit::LockHeld || second.phase() == Phase::Running {
            return Err(format!("a second run became an owner: {refused:?}"));
        }

        // The owning session dies, as it does when the database restarts or the
        // network drops. The run notices and returns, which is what lets the
        // supervisor start a new one.
        let pid = owner_pid(control, key)
            .await?
            .ok_or("no session holds the worker lock")?;
        let own: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(control)
            .await
            .map_err(text)?;
        if pid == own {
            return Err("refusing to terminate the control session".into());
        }
        sqlx::query("SELECT pg_terminate_backend($1)")
            .bind(pid)
            .execute(control)
            .await
            .map_err(text)?;
        let ended = tokio::time::timeout(Duration::from_secs(20), first)
            .await
            .map_err(|_| "the run kept going after its owning session was terminated".to_string())?
            .map_err(|error| error.to_string())?;
        if ended != Exit::OwnershipLost {
            return Err(format!("expected ownership_lost, got {ended:?}"));
        }

        // A new run, as the supervisor would start, owns the queue again on a new
        // session.
        let restarted = Status::new("synthetic-restarted");
        let again = tokio::spawn(run(pool.clone(), restarted.clone()));
        until_running(&restarted).await?;
        let new_pid = owner_pid(control, key).await?;
        if new_pid.is_none() || new_pid == Some(pid) {
            return Err(format!(
                "the restarted run does not own the lock: {new_pid:?}"
            ));
        }
        // Stopping the task drops its session, which releases the lock.
        again.abort();
        let _ = again.await;
        for _ in 0..100 {
            if owner_pid(control, key).await?.is_none() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err("the lock was still held after the run was stopped".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const QUICK: Policy = Policy {
        first: Duration::from_millis(5),
        longest: Duration::from_millis(20),
        lock_poll: Duration::from_millis(5),
        settled: Duration::from_secs(60),
    };

    async fn until(status: &Status, wanted: impl Fn(Snapshot) -> bool) -> Snapshot {
        for _ in 0..400 {
            let now = status.snapshot();
            if wanted(now) {
                return now;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!(
            "worker never reached the expected state: {:?}",
            status.snapshot()
        );
    }

    /// A body that returns each scripted exit in turn, then runs until dropped.
    fn scripted(
        exits: &'static [Exit],
        runs: Arc<AtomicUsize>,
        live: Arc<AtomicUsize>,
    ) -> impl Fn(Arc<Status>) -> std::pin::Pin<Box<dyn Future<Output = Exit> + Send>> + Send + 'static
    {
        move |status| {
            let (runs, live) = (runs.clone(), live.clone());
            Box::pin(async move {
                let run = runs.fetch_add(1, Ordering::SeqCst);
                // More than one body alive at once would be two owners.
                assert_eq!(live.fetch_add(1, Ordering::SeqCst), 0);
                let exit = match exits.get(run) {
                    Some(exit) => {
                        tokio::time::sleep(Duration::from_millis(2)).await;
                        *exit
                    }
                    None => {
                        status.running();
                        live.fetch_sub(1, Ordering::SeqCst);
                        std::future::pending::<Exit>().await
                    }
                };
                live.fetch_sub(1, Ordering::SeqCst);
                exit
            })
        }
    }

    #[tokio::test]
    async fn a_worker_that_loses_its_session_is_restarted_and_reports_why() {
        let (runs, live) = (Arc::default(), Arc::default());
        let status = spawn(
            "synthetic",
            QUICK,
            scripted(
                &[Exit::ConnectionUnavailable, Exit::OwnershipLost],
                Arc::clone(&runs),
                live,
            ),
        );
        let settled = until(&status, |state| state.phase == Phase::Running).await;
        assert_eq!(settled.restarts, 2);
        assert_eq!(settled.last_error, Some("ownership_lost"));
        assert_eq!(runs.load(Ordering::SeqCst), 3);
        let report = registry(vec![status]).report();
        assert_eq!(
            report,
            json!({"synthetic": {"state": "running", "restarts": 2, "last_error": "ownership_lost"}})
        );
    }

    #[tokio::test]
    async fn a_held_lock_is_waited_for_and_is_not_an_error() {
        let (runs, live) = (Arc::default(), Arc::default());
        static HELD: [Exit; 3] = [Exit::LockHeld; 3];
        let status = spawn("synthetic", QUICK, scripted(&HELD, Arc::clone(&runs), live));
        let waiting = until(&status, |state| state.phase == Phase::WaitingForLock).await;
        assert_eq!((waiting.restarts, waiting.last_error), (0, None));
        // Once the other owner lets go, this one takes over without a restart count.
        let running = until(&status, |state| state.phase == Phase::Running).await;
        assert_eq!((running.restarts, running.last_error), (0, None));
        assert_eq!(runs.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn a_panicking_worker_is_restarted() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        let status = spawn("synthetic", QUICK, move |status: Arc<Status>| {
            let run = counter.fetch_add(1, Ordering::SeqCst);
            async move {
                if run == 0 {
                    panic!("synthetic worker failure");
                }
                status.running();
                std::future::pending::<Exit>().await
            }
        });
        let running = until(&status, |state| state.phase == Phase::Running).await;
        assert_eq!(running.restarts, 1);
        assert_eq!(running.last_error, Some("panicked"));
    }

    #[tokio::test]
    async fn restart_pauses_grow_to_a_ceiling() {
        static FAILING: [Exit; 6] = [Exit::ConnectionUnavailable; 6];
        let (runs, live) = (Arc::default(), Arc::default());
        let began = Instant::now();
        let status = spawn(
            "synthetic",
            QUICK,
            scripted(&FAILING, Arc::clone(&runs), live),
        );
        until(&status, |state| state.phase == Phase::Running).await;
        // Pauses of 5, 10, 20, 20, 20, 20 ms: doubling, then held at the ceiling.
        assert!(began.elapsed() >= Duration::from_millis(95));
        assert_eq!(status.snapshot().restarts, 6);
    }

    #[test]
    fn an_empty_registry_reports_no_workers() {
        assert_eq!(Registry::default().report(), json!({}));
    }
}
