//! Configuration for the background job processing system

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

/// Jobs configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobsConfig {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub worker: WorkerConfig,
    pub monitoring: MonitoringConfig,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: u64,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: u64,
}

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub max_workers: u32,
    pub max_retries: u32,
    pub retry_delay: u64,
    pub job_timeout: u64,
    pub poll_interval: u64,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub metrics_port: u16,
    pub health_check_interval: u64,
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            worker: WorkerConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/emr_jobs".to_string(),
            max_connections: 10,
            min_connections: 1,
            connection_timeout: 30,
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            connection_timeout: 30,
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            max_retries: 3,
            retry_delay: 30,
            job_timeout: 300,
            poll_interval: 5,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics_port: 9090,
            health_check_interval: 30,
        }
    }
}

impl JobsConfig {
    /// Load configuration from environment variables and config files
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::builder();

        // Load from default config file if it exists
        if let Ok(config_path) = env::var("JOBS_CONFIG_PATH") {
            config = config.add_source(File::with_name(&config_path).required(false));
        } else {
            config = config.add_source(File::with_name("jobs.toml").required(false));
        }

        // Load from environment variables
        config = config.add_source(
            Environment::with_prefix("JOBS")
                .separator("_")
                .prefix_separator("_")
        );

        // Set defaults
        config = config
            .set_default("database.url", "postgresql://localhost/emr_jobs")?
            .set_default("database.max_connections", 10)?
            .set_default("database.min_connections", 1)?
            .set_default("database.connection_timeout", 30)?
            .set_default("redis.url", "redis://localhost:6379")?
            .set_default("redis.max_connections", 10)?
            .set_default("redis.connection_timeout", 30)?
            .set_default("worker.max_workers", 4)?
            .set_default("worker.max_retries", 3)?
            .set_default("worker.retry_delay", 30)?
            .set_default("worker.job_timeout", 300)?
            .set_default("worker.poll_interval", 5)?
            .set_default("monitoring.enabled", true)?
            .set_default("monitoring.metrics_port", 9090)?
            .set_default("monitoring.health_check_interval", 30)?;

        config.build()?.try_deserialize()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.database.url.is_empty() {
            return Err("Database URL cannot be empty".to_string());
        }

        if self.redis.url.is_empty() {
            return Err("Redis URL cannot be empty".to_string());
        }

        if self.worker.max_workers == 0 {
            return Err("Max workers must be greater than 0".to_string());
        }

        if self.worker.job_timeout == 0 {
            return Err("Job timeout must be greater than 0".to_string());
        }

        if self.monitoring.enabled && self.monitoring.metrics_port == 0 {
            return Err("Metrics port must be greater than 0 when monitoring is enabled".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = JobsConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = JobsConfig::default();
        
        // Test empty database URL
        config.database.url = String::new();
        assert!(config.validate().is_err());
        
        // Test zero max workers
        config.database.url = "postgresql://localhost/test".to_string();
        config.worker.max_workers = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_load_with_env() {
        env::set_var("JOBS_DATABASE_URL", "postgresql://test:5432/test");
        env::set_var("JOBS_WORKER_MAX_WORKERS", "8");
        
        let config = JobsConfig::load().unwrap();
        assert_eq!(config.database.url, "postgresql://test:5432/test");
        assert_eq!(config.worker.max_workers, 8);
        
        env::remove_var("JOBS_DATABASE_URL");
        env::remove_var("JOBS_WORKER_MAX_WORKERS");
    }
} 