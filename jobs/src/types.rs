//! Job type definitions and payloads

use chrono::{DateTime, Utc};
use core::entities::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// All job types supported by the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JobType {
    /// Synchronize patient data with external FHIR server
    FhirSync(FhirSyncJob),
    
    /// Validate patient data integrity
    DataValidation(DataValidationJob),
    
    /// Generate audit reports
    AuditReport(AuditReportJob),
    
    /// Send notifications to users
    Notification(NotificationJob),
    
    /// Export patient data
    DataExport(DataExportJob),
    
    /// Import patient data
    DataImport(DataImportJob),
    
    /// Cleanup old records
    DataCleanup(DataCleanupJob),
    
    /// Generate analytics reports
    Analytics(AnalyticsJob),
}

/// FHIR synchronization job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSyncJob {
    pub patient_id: Uuid,
    pub resource_type: String,
    pub source_url: String,
    pub target_url: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub sync_direction: SyncDirection,
}

/// Data validation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataValidationJob {
    pub patient_id: Option<Uuid>,
    pub validation_type: ValidationType,
    pub rules: Vec<ValidationRule>,
    pub auto_fix: bool,
}

/// Audit report generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReportJob {
    pub report_type: AuditReportType,
    pub date_range: DateRange,
    pub patient_ids: Option<Vec<Uuid>>,
    pub practitioner_ids: Option<Vec<Uuid>>,
    pub output_format: OutputFormat,
}

/// Notification job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationJob {
    pub recipient_id: Uuid,
    pub notification_type: NotificationType,
    pub message: String,
    pub channel: NotificationChannel,
    pub priority: Priority,
    pub scheduled_for: Option<DateTime<Utc>>,
}

/// Data export job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataExportJob {
    pub patient_ids: Vec<Uuid>,
    pub export_format: ExportFormat,
    pub include_resources: Vec<String>,
    pub output_location: String,
    pub encryption_key: Option<String>,
}

/// Data import job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataImportJob {
    pub source_location: String,
    pub import_format: ImportFormat,
    pub mapping_config: Option<String>,
    pub validation_rules: Vec<ValidationRule>,
    pub auto_merge: bool,
}

/// Data cleanup job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCleanupJob {
    pub cleanup_type: CleanupType,
    pub older_than: DateTime<Utc>,
    pub dry_run: bool,
    pub preserve_audit: bool,
}

/// Analytics job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsJob {
    pub analytics_type: AnalyticsType,
    pub date_range: DateRange,
    pub dimensions: Vec<String>,
    pub metrics: Vec<String>,
    pub output_location: String,
}

/// Synchronization direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncDirection {
    Pull,
    Push,
    Bidirectional,
}

/// Validation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationType {
    Schema,
    BusinessRules,
    Completeness,
    Consistency,
    Accuracy,
}

/// Validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub name: String,
    pub description: String,
    pub rule_type: String,
    pub expression: String,
    pub severity: ValidationSeverity,
}

/// Validation severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Audit report types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditReportType {
    AccessLog,
    DataChanges,
    UserActivity,
    SecurityEvents,
    ComplianceReport,
}

/// Date range for reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Output formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Json,
    Xml,
    Csv,
    Pdf,
    Html,
}

/// Notification types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    Alert,
    Reminder,
    Update,
    Warning,
    Error,
}

/// Notification channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    Email,
    Sms,
    Push,
    InApp,
}

/// Priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

/// Export formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Fhir,
    Hl7,
    Csv,
    Json,
    Xml,
}

/// Import formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportFormat {
    Fhir,
    Hl7,
    Csv,
    Json,
    Xml,
}

/// Cleanup types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupType {
    Logs,
    TempFiles,
    OldRecords,
    Duplicates,
    Orphaned,
}

/// Analytics types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalyticsType {
    Usage,
    Performance,
    Quality,
    Trends,
    Predictions,
}

/// Job execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    pub id: Uuid,
    pub job_type: String,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub max_attempts: u32,
    pub last_error: Option<String>,
    pub progress: f64,
    pub metadata: serde_json::Value,
}

/// Job status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

impl Default for JobStatus {
    fn default() -> Self {
        JobStatus::Pending
    }
}

impl JobMetadata {
    /// Create new job metadata
    pub fn new(job_type: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_type,
            status: JobStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            attempts: 0,
            max_attempts: 3,
            last_error: None,
            progress: 0.0,
            metadata: serde_json::Value::Null,
        }
    }

    /// Mark job as started
    pub fn start(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Some(Utc::now());
        self.attempts += 1;
    }

    /// Mark job as completed
    pub fn complete(&mut self) {
        self.status = JobStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.progress = 100.0;
    }

    /// Mark job as failed
    pub fn fail(&mut self, error: String) {
        self.status = JobStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.last_error = Some(error);
    }

    /// Check if job can be retried
    pub fn can_retry(&self) -> bool {
        matches!(self.status, JobStatus::Failed) && self.attempts < self.max_attempts
    }

    /// Get execution duration
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_metadata_creation() {
        let metadata = JobMetadata::new("test_job".to_string());
        assert_eq!(metadata.job_type, "test_job");
        assert!(matches!(metadata.status, JobStatus::Pending));
        assert_eq!(metadata.attempts, 0);
        assert_eq!(metadata.progress, 0.0);
    }

    #[test]
    fn test_job_lifecycle() {
        let mut metadata = JobMetadata::new("test_job".to_string());
        
        // Start job
        metadata.start();
        assert!(matches!(metadata.status, JobStatus::Running));
        assert!(metadata.started_at.is_some());
        assert_eq!(metadata.attempts, 1);
        
        // Complete job
        metadata.complete();
        assert!(matches!(metadata.status, JobStatus::Completed));
        assert!(metadata.completed_at.is_some());
        assert_eq!(metadata.progress, 100.0);
    }

    #[test]
    fn test_job_retry() {
        let mut metadata = JobMetadata::new("test_job".to_string());
        metadata.start();
        
        // Fail job
        metadata.fail("Test error".to_string());
        assert!(matches!(metadata.status, JobStatus::Failed));
        assert_eq!(metadata.last_error, Some("Test error".to_string()));
        assert!(metadata.can_retry());
        
        // Exhaust retries
        metadata.attempts = metadata.max_attempts;
        assert!(!metadata.can_retry());
    }

    #[test]
    fn test_job_duration() {
        let mut metadata = JobMetadata::new("test_job".to_string());
        
        // No duration when not started
        assert!(metadata.duration().is_none());
        
        // Start job
        metadata.start();
        assert!(metadata.duration().is_none());
        
        // Complete job
        metadata.complete();
        assert!(metadata.duration().is_some());
    }
} 