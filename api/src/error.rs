//! Error handling for the EMR API

use actix_web::{HttpResponse, ResponseError};
use emr_core::Error as CoreError;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Result type alias for API operations
pub type Result<T> = std::result::Result<T, ApiError>;

/// API-specific error types
#[derive(Error, Debug)]
pub enum ApiError {
    /// Core domain error
    #[error("Core error: {0}")]
    Core(#[from] CoreError),

    /// Configuration error
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Database error
    #[error("Database error: {message}")]
    Database { message: String },

    /// Authentication error
    #[error("Authentication error: {message}")]
    Authentication { message: String },

    /// Authorization error
    #[error("Authorization error: {message}")]
    Authorization { message: String },

    /// Validation error
    #[error("Validation error: {message}")]
    Validation { message: String },

    /// External service error
    #[error("External service error: {service} - {message}")]
    ExternalService { service: String, message: String },

    /// FHIR-specific error
    #[error("FHIR error: {message}")]
    Fhir { message: String },

    /// Internal server error
    #[error("Internal server error: {message}")]
    Internal { message: String },

    /// Bad request error
    #[error("Bad request: {message}")]
    BadRequest { message: String },

    /// Not found error
    #[error("Not found: {message}")]
    NotFound { message: String },

    /// Conflict error
    #[error("Conflict: {message}")]
    Conflict { message: String },

    /// Too many requests error
    #[error("Too many requests: {message}")]
    TooManyRequests { message: String },

    /// Service unavailable error
    #[error("Service unavailable: {message}")]
    ServiceUnavailable { message: String },
}

/// Error response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub path: Option<String>,
    pub request_id: Option<String>,
}

impl ApiError {
    /// Create a configuration error
    pub fn configuration_error(message: &str) -> Self {
        Self::Configuration {
            message: message.to_string(),
        }
    }

    /// Create a database error
    pub fn database_error(message: &str) -> Self {
        Self::Database {
            message: message.to_string(),
        }
    }

    /// Create an authentication error
    pub fn authentication_error(message: &str) -> Self {
        Self::Authentication {
            message: message.to_string(),
        }
    }

    /// Create an authorization error
    pub fn authorization_error(message: &str) -> Self {
        Self::Authorization {
            message: message.to_string(),
        }
    }

    /// Create a validation error
    pub fn validation_error(message: &str) -> Self {
        Self::Validation {
            message: message.to_string(),
        }
    }

    /// Create an external service error
    pub fn external_service_error(service: &str, message: &str) -> Self {
        Self::ExternalService {
            service: service.to_string(),
            message: message.to_string(),
        }
    }

    /// Create a FHIR error
    pub fn fhir_error(message: &str) -> Self {
        Self::Fhir {
            message: message.to_string(),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: &str) -> Self {
        Self::Internal {
            message: message.to_string(),
        }
    }

    /// Create a bad request error
    pub fn bad_request(message: &str) -> Self {
        Self::BadRequest {
            message: message.to_string(),
        }
    }

    /// Create a not found error
    pub fn not_found(message: &str) -> Self {
        Self::NotFound {
            message: message.to_string(),
        }
    }

    /// Create a conflict error
    pub fn conflict(message: &str) -> Self {
        Self::Conflict {
            message: message.to_string(),
        }
    }

    /// Create a too many requests error
    pub fn too_many_requests(message: &str) -> Self {
        Self::TooManyRequests {
            message: message.to_string(),
        }
    }

    /// Create a service unavailable error
    pub fn service_unavailable(message: &str) -> Self {
        Self::ServiceUnavailable {
            message: message.to_string(),
        }
    }

    /// Get the error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            ApiError::Core(_) => "core",
            ApiError::Configuration { .. } => "configuration",
            ApiError::Database { .. } => "database",
            ApiError::Authentication { .. } => "authentication",
            ApiError::Authorization { .. } => "authorization",
            ApiError::Validation { .. } => "validation",
            ApiError::ExternalService { .. } => "external_service",
            ApiError::Fhir { .. } => "fhir",
            ApiError::Internal { .. } => "internal",
            ApiError::BadRequest { .. } => "bad_request",
            ApiError::NotFound { .. } => "not_found",
            ApiError::Conflict { .. } => "conflict",
            ApiError::TooManyRequests { .. } => "too_many_requests",
            ApiError::ServiceUnavailable { .. } => "service_unavailable",
        }
    }

    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ApiError::ExternalService { .. }
                | ApiError::Internal { .. }
                | ApiError::ServiceUnavailable { .. }
        )
    }

    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> actix_web::http::StatusCode {
        use actix_web::http::StatusCode;

        match self {
            ApiError::Core(core_error) => match core_error {
                CoreError::EntityNotFound { .. } => StatusCode::NOT_FOUND,
                CoreError::ValidationError { .. } => StatusCode::BAD_REQUEST,
                CoreError::BusinessRuleViolation { .. } => StatusCode::BAD_REQUEST,
                CoreError::AuthorizationError { .. } => StatusCode::FORBIDDEN,
                CoreError::FhirError { .. } => StatusCode::BAD_REQUEST,
                CoreError::DataIntegrityError { .. } => StatusCode::CONFLICT,
                CoreError::ExternalServiceError { .. } => StatusCode::BAD_GATEWAY,
                CoreError::ConfigurationError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                CoreError::InternalError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ApiError::Configuration { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Database { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Authentication { .. } => StatusCode::UNAUTHORIZED,
            ApiError::Authorization { .. } => StatusCode::FORBIDDEN,
            ApiError::Validation { .. } => StatusCode::BAD_REQUEST,
            ApiError::ExternalService { .. } => StatusCode::BAD_GATEWAY,
            ApiError::Fhir { .. } => StatusCode::BAD_REQUEST,
            ApiError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::BadRequest { .. } => StatusCode::BAD_REQUEST,
            ApiError::NotFound { .. } => StatusCode::NOT_FOUND,
            ApiError::Conflict { .. } => StatusCode::CONFLICT,
            ApiError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Create an error response
    pub fn error_response(&self, path: Option<String>, request_id: Option<String>) -> ErrorResponse {
        ErrorResponse {
            error: self.category().to_string(),
            message: self.to_string(),
            details: None,
            timestamp: chrono::Utc::now(),
            path,
            request_id,
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let error_response = self.error_response(None, None);
        
        HttpResponse::build(self.status_code())
            .insert_header(("Content-Type", "application/json"))
            .json(error_response)
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        self.status_code()
    }
}

// Convert from various error types
impl From<config::ConfigError> for ApiError {
    fn from(err: config::ConfigError) -> Self {
        ApiError::configuration_error(&err.to_string())
    }
}

impl From<diesel::result::Error> for ApiError {
    fn from(err: diesel::result::Error) -> Self {
        ApiError::database_error(&err.to_string())
    }
}

impl From<deadpool_diesel::PoolError> for ApiError {
    fn from(err: deadpool_diesel::PoolError) -> Self {
        ApiError::database_error(&err.to_string())
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        ApiError::external_service_error("HTTP", &err.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::validation_error(&format!("JSON serialization error: {}", err))
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::internal_error(&err.to_string())
    }
}

impl From<actix_web::error::BlockingError> for ApiError {
    fn from(err: actix_web::error::BlockingError) -> Self {
        ApiError::internal_error(&err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = ApiError::bad_request("Invalid input");
        assert_eq!(error.category(), "bad_request");
        assert_eq!(error.status_code(), actix_web::http::StatusCode::BAD_REQUEST);
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_error_response() {
        let error = ApiError::not_found("Resource not found");
        let response = error.error_response(
            Some("/api/v1/patients/123".to_string()),
            Some("req-123".to_string()),
        );

        assert_eq!(response.error, "not_found");
        assert_eq!(response.message, "Not found: Resource not found");
        assert_eq!(response.path, Some("/api/v1/patients/123".to_string()));
        assert_eq!(response.request_id, Some("req-123".to_string()));
    }

    #[test]
    fn test_retryable_errors() {
        let external_error = ApiError::external_service_error("TestService", "Connection failed");
        assert!(external_error.is_retryable());

        let validation_error = ApiError::validation_error("Invalid input");
        assert!(!validation_error.is_retryable());

        let internal_error = ApiError::internal_error("Database connection failed");
        assert!(internal_error.is_retryable());
    }

    #[test]
    fn test_core_error_conversion() {
        let core_error = CoreError::entity_not_found("Patient", uuid::Uuid::new_v4());
        let api_error = ApiError::from(core_error);

        assert_eq!(api_error.category(), "core");
        assert_eq!(api_error.status_code(), actix_web::http::StatusCode::NOT_FOUND);
    }
} 