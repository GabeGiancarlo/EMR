#![deny(unsafe_code)]

//! Background job processing for the EMR platform
//!
//! This crate provides background job processing capabilities using Apalis,
//! supporting various job types like FHIR synchronization, data validation,
//! and audit logging.

use std::collections::HashMap;

use anyhow::Result;
use apalis::prelude::*;
use chrono::{DateTime, Utc};
use core::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use uuid::Uuid;

pub mod config;
pub mod handlers;
pub mod types;
pub mod worker;

pub use config::JobsConfig;
pub use handlers::*;
pub use types::*;
pub use worker::JobsWorker;

/// Re-export commonly used types
pub mod prelude {
    pub use super::{
        config::JobsConfig,
        handlers::*,
        types::*,
        worker::JobsWorker,
        JobContext,
        JobError,
        JobResult,
    };
}

/// Job execution context
#[derive(Debug, Clone)]
pub struct JobContext {
    pub job_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl JobContext {
    /// Create a new job context
    pub fn new(job_id: Uuid) -> Self {
        Self {
            job_id,
            started_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the job context
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Job execution result
pub type JobResult<T> = Result<T, JobError>;

/// Job execution errors
#[derive(thiserror::Error, Debug)]
pub enum JobError {
    #[error("Job validation failed: {0}")]
    ValidationError(String),

    #[error("Job processing failed: {0}")]
    ProcessingError(String),

    #[error("External service error: {0}")]
    ExternalServiceError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl JobError {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            JobError::ValidationError(_) => false,
            JobError::ProcessingError(_) => false,
            JobError::ExternalServiceError(_) => true,
            JobError::DatabaseError(_) => true,
            JobError::NetworkError(_) => true,
            JobError::TimeoutError(_) => true,
            JobError::SerializationError(_) => false,
            JobError::ConfigurationError(_) => false,
            JobError::UnknownError(_) => false,
        }
    }

    /// Get retry delay in seconds
    pub fn retry_delay(&self) -> u64 {
        match self {
            JobError::ExternalServiceError(_) => 60,
            JobError::DatabaseError(_) => 30,
            JobError::NetworkError(_) => 30,
            JobError::TimeoutError(_) => 120,
            _ => 0,
        }
    }
}

/// Job execution statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct JobStats {
    pub total_jobs: u64,
    pub successful_jobs: u64,
    pub failed_jobs: u64,
    pub retried_jobs: u64,
    pub average_duration_ms: f64,
    pub last_updated: DateTime<Utc>,
}

impl Default for JobStats {
    fn default() -> Self {
        Self {
            total_jobs: 0,
            successful_jobs: 0,
            failed_jobs: 0,
            retried_jobs: 0,
            average_duration_ms: 0.0,
            last_updated: Utc::now(),
        }
    }
}

/// Job monitoring and metrics
pub struct JobMonitor {
    stats: JobStats,
}

impl JobMonitor {
    /// Create a new job monitor
    pub fn new() -> Self {
        Self {
            stats: JobStats::default(),
        }
    }

    /// Record job execution
    pub fn record_job(&mut self, duration_ms: u64, success: bool) {
        self.stats.total_jobs += 1;
        
        if success {
            self.stats.successful_jobs += 1;
        } else {
            self.stats.failed_jobs += 1;
        }

        // Update average duration
        let total_duration = self.stats.average_duration_ms * (self.stats.total_jobs - 1) as f64;
        self.stats.average_duration_ms = (total_duration + duration_ms as f64) / self.stats.total_jobs as f64;
        
        self.stats.last_updated = Utc::now();
    }

    /// Get current statistics
    pub fn get_stats(&self) -> &JobStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = JobStats::default();
    }
}

impl Default for JobMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_context_creation() {
        let job_id = Uuid::new_v4();
        let context = JobContext::new(job_id);
        
        assert_eq!(context.job_id, job_id);
        assert!(context.metadata.is_empty());
    }

    #[test]
    fn test_job_context_with_metadata() {
        let job_id = Uuid::new_v4();
        let context = JobContext::new(job_id)
            .with_metadata("key1".to_string(), "value1".to_string())
            .with_metadata("key2".to_string(), "value2".to_string());
        
        assert_eq!(context.get_metadata("key1"), Some(&"value1".to_string()));
        assert_eq!(context.get_metadata("key2"), Some(&"value2".to_string()));
        assert_eq!(context.get_metadata("key3"), None);
    }

    #[test]
    fn test_job_error_retryable() {
        assert!(!JobError::ValidationError("test".to_string()).is_retryable());
        assert!(JobError::NetworkError("test".to_string()).is_retryable());
        assert!(JobError::DatabaseError("test".to_string()).is_retryable());
    }

    #[test]
    fn test_job_monitor() {
        let mut monitor = JobMonitor::new();
        
        // Record some jobs
        monitor.record_job(100, true);
        monitor.record_job(200, false);
        monitor.record_job(150, true);
        
        let stats = monitor.get_stats();
        assert_eq!(stats.total_jobs, 3);
        assert_eq!(stats.successful_jobs, 2);
        assert_eq!(stats.failed_jobs, 1);
        assert_eq!(stats.average_duration_ms, 150.0);
    }
} 