//! The request connection pool, and the connections that deliberately sit outside it.
//!
//! Request connections fail fast: a bounded wait for a free connection, and session
//! `statement_timeout` and `lock_timeout` so one slow statement or one held lock cannot
//! pin a connection for ever. Migrations and the background workers' owner sessions use
//! [`dedicated`] connections, which are detached from the pool and carry no timeouts.
//! The workers' pooled transactions lift the timeouts for themselves
//! ([`begin_background`]), because a queued job should wait, not fail.
use sqlx::{
    postgres::{PgConnection, PgPoolOptions},
    PgPool, Postgres, Transaction,
};
use std::time::{Duration, Instant};

/// How long a request waits for a free connection before it is refused.
pub const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a background worker keeps asking for a pooled connection.
const BACKGROUND_ACQUIRE_PATIENCE: Duration = Duration::from_secs(30);
/// An unused connection is closed after this long.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Every pooled connection is replaced after this long, so a host that recycles
/// database connections never finds the pool holding a stale one.
pub const MAX_LIFETIME: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolSettings {
    pub max_connections: u32,
    /// `None` leaves the server's own setting in place.
    pub statement_timeout: Option<Duration>,
    /// `None` leaves the server's own setting in place.
    pub lock_timeout: Option<Duration>,
}

impl Default for PoolSettings {
    fn default() -> Self {
        Self {
            max_connections: 10,
            statement_timeout: Some(Duration::from_secs(30)),
            lock_timeout: Some(Duration::from_secs(15)),
        }
    }
}

/// Pool options for request connections. Every new connection gets the session
/// timeouts before it is handed out.
pub fn pool_options(settings: &PoolSettings) -> PgPoolOptions {
    let statement = settings.statement_timeout.map(|limit| limit.as_millis());
    let lock = settings.lock_timeout.map(|limit| limit.as_millis());
    PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .idle_timeout(Some(IDLE_TIMEOUT))
        .max_lifetime(Some(MAX_LIFETIME))
        .after_connect(move |conn, _meta| {
            Box::pin(async move {
                for (name, limit) in [("statement_timeout", statement), ("lock_timeout", lock)] {
                    if let Some(limit) = limit {
                        sqlx::query("SELECT set_config($1, $2, false)")
                            .bind(name)
                            .bind(limit.to_string())
                            .execute(&mut *conn)
                            .await?;
                    }
                }
                Ok(())
            })
        })
}

/// A connection of its own for work that outlives a request: migrations, and the
/// session that owns a background worker's advisory lock. It is detached, so the pool
/// neither counts it nor closes it for age or idleness, and the request timeouts are
/// reset so a long migration or a patient lock wait is not cancelled.
pub async fn dedicated(pool: &PgPool) -> Result<PgConnection, sqlx::Error> {
    let mut conn = pool.acquire().await?.detach();
    for name in ["statement_timeout", "lock_timeout"] {
        // RESET returns to the server or role default, as if the pool had set nothing.
        sqlx::query(&format!("RESET {name}"))
            .execute(&mut conn)
            .await?;
    }
    Ok(conn)
}

/// Begin a background worker's pooled transaction. A request should give up quickly;
/// a queued job should not be marked failed because the pool or the organization was
/// busy for a moment. So this waits for a connection as long as requests used to
/// (thirty seconds), and the transaction waits for locks instead of timing out. The
/// timeouts are lifted for this transaction only and return when it ends.
pub(crate) async fn begin_background(
    pool: &PgPool,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let started = Instant::now();
    let mut tx = loop {
        match pool.begin().await {
            Err(sqlx::Error::PoolTimedOut) if started.elapsed() < BACKGROUND_ACQUIRE_PATIENCE => {}
            other => break other?,
        }
    };
    sqlx::query(
        "SELECT set_config('statement_timeout', '0', true), set_config('lock_timeout', '0', true)",
    )
    .execute(&mut *tx)
    .await?;
    Ok(tx)
}
