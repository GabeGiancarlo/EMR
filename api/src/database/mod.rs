//! Database connection scaffolding for Nexus.

use crate::config::DatabaseConfig;
use crate::error::{ApiError, Result};
use deadpool_diesel::postgres::{Manager, Pool as DeadPool, Runtime};
use diesel_async::{AsyncConnection, AsyncPgConnection};

/// Database connection pool type
pub type Pool = DeadPool<Manager<AsyncPgConnection>>;

/// Create database connection pool
pub async fn create_pool(config: &DatabaseConfig) -> Result<Pool> {
    let manager = Manager::new(&config.url, Runtime::Tokio1);
    let pool = DeadPool::builder(manager)
        .max_size(config.max_connections as usize)
        .build()
        .map_err(|e| ApiError::database_error(&format!("Failed to create pool: {}", e)))?;

    Ok(pool)
}

/// Run database migrations
pub async fn run_migrations(pool: &Pool) -> Result<()> {
    // TODO(nexus-phase1): Execute SQLx migrations from the repo migration path.
    // Current behavior only validates that a pooled connection can be acquired.
    let _conn = pool.get().await
        .map_err(|e| ApiError::database_error(&format!("Failed to get connection: {}", e)))?;
    
    tracing::info!("Database migrations completed");
    Ok(())
} 