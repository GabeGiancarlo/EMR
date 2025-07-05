//! Job worker implementation using Apalis

use crate::{
    config::JobsConfig,
    handlers::*,
    types::*,
    JobContext,
    JobError,
    JobMonitor,
    JobResult,
};
use anyhow::Result;
use apalis::prelude::*;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Jobs worker that manages background job processing
pub struct JobsWorker {
    config: JobsConfig,
    monitor: Arc<RwLock<JobMonitor>>,
    data_validation_handler: DataValidationHandler,
    notification_handler: NotificationHandler,
}

impl JobsWorker {
    /// Create a new jobs worker
    pub fn new(config: JobsConfig) -> Self {
        Self {
            config,
            monitor: Arc::new(RwLock::new(JobMonitor::new())),
            data_validation_handler: DataValidationHandler,
            notification_handler: NotificationHandler,
        }
    }

    /// Start the worker
    pub async fn start(self) -> Result<()> {
        info!("Starting jobs worker");

        // TODO: Set up Apalis workers here
        // This is a stub implementation
        
        let worker_config = &self.config.worker;
        info!(
            max_workers = worker_config.max_workers,
            max_retries = worker_config.max_retries,
            "Jobs worker configuration loaded"
        );

        // Simulate worker running
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(worker_config.poll_interval)).await;
            
            // Check for pending jobs
            self.process_pending_jobs().await?;
        }
    }

    /// Process pending jobs
    async fn process_pending_jobs(&self) -> Result<()> {
        // TODO: Implement actual job processing from database/queue
        // This is a stub implementation
        
        info!("Checking for pending jobs");
        
        // Simulate processing some jobs
        if rand::random::<f64>() < 0.3 {
            self.process_sample_job().await?;
        }

        Ok(())
    }

    /// Process a sample job for demonstration
    async fn process_sample_job(&self) -> Result<()> {
        let job_id = Uuid::new_v4();
        let context = JobContext::new(job_id);
        
        info!(job_id = ?job_id, "Processing sample job");
        
        let start_time = std::time::Instant::now();
        
        // Create a sample validation job
        let validation_job = DataValidationJob {
            patient_id: Some(Uuid::new_v4()),
            validation_type: ValidationType::Schema,
            rules: vec![
                ValidationRule {
                    name: "sample_rule".to_string(),
                    description: "Sample validation rule".to_string(),
                    rule_type: "required".to_string(),
                    expression: "field != null".to_string(),
                    severity: ValidationSeverity::Warning,
                },
            ],
            auto_fix: false,
        };

        // Execute the job
        let result = self.data_validation_handler.execute(validation_job, context).await;
        
        let duration = start_time.elapsed().as_millis() as u64;
        let success = result.is_ok();
        
        // Update monitoring statistics
        {
            let mut monitor = self.monitor.write().await;
            monitor.record_job(duration, success);
        }

        match result {
            Ok(job_result) => {
                info!(
                    job_id = ?job_id,
                    duration_ms = duration,
                    message = %job_result.message,
                    "Job completed successfully"
                );
            }
            Err(error) => {
                error!(
                    job_id = ?job_id,
                    duration_ms = duration,
                    error = %error,
                    "Job failed"
                );
            }
        }

        Ok(())
    }

    /// Get worker statistics
    pub async fn get_stats(&self) -> crate::JobStats {
        let monitor = self.monitor.read().await;
        monitor.get_stats().clone()
    }

    /// Reset worker statistics
    pub async fn reset_stats(&self) {
        let mut monitor = self.monitor.write().await;
        monitor.reset_stats();
    }

    /// Check worker health
    pub async fn health_check(&self) -> Result<WorkerHealth> {
        let stats = self.get_stats().await;
        let uptime = Utc::now() - stats.last_updated;
        
        Ok(WorkerHealth {
            status: WorkerStatus::Running,
            uptime_seconds: uptime.num_seconds() as u64,
            jobs_processed: stats.total_jobs,
            success_rate: if stats.total_jobs > 0 {
                (stats.successful_jobs as f64 / stats.total_jobs as f64) * 100.0
            } else {
                0.0
            },
            average_duration_ms: stats.average_duration_ms,
            last_activity: stats.last_updated,
        })
    }

    /// Shutdown worker gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down jobs worker");
        
        // TODO: Implement graceful shutdown
        // - Stop accepting new jobs
        // - Wait for current jobs to complete
        // - Close database connections
        // - Clean up resources
        
        Ok(())
    }
}

/// Worker health status
#[derive(Debug, Clone)]
pub struct WorkerHealth {
    pub status: WorkerStatus,
    pub uptime_seconds: u64,
    pub jobs_processed: u64,
    pub success_rate: f64,
    pub average_duration_ms: f64,
    pub last_activity: chrono::DateTime<Utc>,
}

/// Worker status
#[derive(Debug, Clone)]
pub enum WorkerStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

/// Job execution function for Apalis
pub async fn execute_job(job: JobType, context: JobContext) -> JobResult<JobExecutionResult> {
    match job {
        JobType::DataValidation(validation_job) => {
            let handler = DataValidationHandler;
            handler.execute(validation_job, context).await
        }
        JobType::Notification(notification_job) => {
            let handler = NotificationHandler;
            handler.execute(notification_job, context).await
        }
        _ => {
            // TODO: Implement other job types
            Err(JobError::ProcessingError(
                "Job type not yet implemented".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::JobsConfig;

    #[tokio::test]
    async fn test_worker_creation() {
        let config = JobsConfig::default();
        let worker = JobsWorker::new(config);
        
        let stats = worker.get_stats().await;
        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.successful_jobs, 0);
        assert_eq!(stats.failed_jobs, 0);
    }

    #[tokio::test]
    async fn test_worker_health_check() {
        let config = JobsConfig::default();
        let worker = JobsWorker::new(config);
        
        let health = worker.health_check().await.unwrap();
        assert!(matches!(health.status, WorkerStatus::Running));
        assert_eq!(health.jobs_processed, 0);
        assert_eq!(health.success_rate, 0.0);
    }

    #[tokio::test]
    async fn test_execute_job() {
        let job = JobType::DataValidation(DataValidationJob {
            patient_id: Some(Uuid::new_v4()),
            validation_type: ValidationType::Schema,
            rules: vec![],
            auto_fix: false,
        });
        
        let context = JobContext::new(Uuid::new_v4());
        let result = execute_job(job, context).await;
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_stats_recording() {
        let config = JobsConfig::default();
        let worker = JobsWorker::new(config);
        
        // Record some job executions
        {
            let mut monitor = worker.monitor.write().await;
            monitor.record_job(100, true);
            monitor.record_job(200, false);
            monitor.record_job(150, true);
        }
        
        let stats = worker.get_stats().await;
        assert_eq!(stats.total_jobs, 3);
        assert_eq!(stats.successful_jobs, 2);
        assert_eq!(stats.failed_jobs, 1);
        assert_eq!(stats.average_duration_ms, 150.0);
    }
} 