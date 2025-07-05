//! Error types for the EMR core domain

use thiserror::Error;
use uuid::Uuid;

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;

/// Core domain errors
#[derive(Error, Debug)]
pub enum Error {
    /// Entity not found
    #[error("Entity not found: {entity_type} with id {id}")]
    EntityNotFound {
        entity_type: String,
        id: Uuid,
    },

    /// Validation error
    #[error("Validation error: {message}")]
    ValidationError {
        message: String,
        field: Option<String>,
    },

    /// Business rule violation
    #[error("Business rule violation: {rule}")]
    BusinessRuleViolation {
        rule: String,
        context: String,
    },

    /// Authorization error
    #[error("Authorization error: {message}")]
    AuthorizationError {
        message: String,
        required_scope: Option<String>,
    },

    /// FHIR-specific errors
    #[error("FHIR error: {message}")]
    FhirError {
        message: String,
        resource_type: Option<String>,
    },

    /// Data integrity error
    #[error("Data integrity error: {message}")]
    DataIntegrityError {
        message: String,
        constraint: Option<String>,
    },

    /// External service error
    #[error("External service error: {service} - {message}")]
    ExternalServiceError {
        service: String,
        message: String,
    },

    /// Configuration error
    #[error("Configuration error: {message}")]
    ConfigurationError {
        message: String,
    },

    /// Generic internal error
    #[error("Internal error: {message}")]
    InternalError {
        message: String,
    },
}

impl Error {
    /// Create a new entity not found error
    pub fn entity_not_found(entity_type: &str, id: Uuid) -> Self {
        Self::EntityNotFound {
            entity_type: entity_type.to_string(),
            id,
        }
    }

    /// Create a new validation error
    pub fn validation_error(message: &str) -> Self {
        Self::ValidationError {
            message: message.to_string(),
            field: None,
        }
    }

    /// Create a new validation error with field information
    pub fn validation_error_with_field(message: &str, field: &str) -> Self {
        Self::ValidationError {
            message: message.to_string(),
            field: Some(field.to_string()),
        }
    }

    /// Create a new business rule violation error
    pub fn business_rule_violation(rule: &str, context: &str) -> Self {
        Self::BusinessRuleViolation {
            rule: rule.to_string(),
            context: context.to_string(),
        }
    }

    /// Create a new authorization error
    pub fn authorization_error(message: &str) -> Self {
        Self::AuthorizationError {
            message: message.to_string(),
            required_scope: None,
        }
    }

    /// Create a new FHIR error
    pub fn fhir_error(message: &str, resource_type: Option<&str>) -> Self {
        Self::FhirError {
            message: message.to_string(),
            resource_type: resource_type.map(|s| s.to_string()),
        }
    }

    /// Create a new data integrity error
    pub fn data_integrity_error(message: &str) -> Self {
        Self::DataIntegrityError {
            message: message.to_string(),
            constraint: None,
        }
    }

    /// Create a new external service error
    pub fn external_service_error(service: &str, message: &str) -> Self {
        Self::ExternalServiceError {
            service: service.to_string(),
            message: message.to_string(),
        }
    }

    /// Create a new configuration error
    pub fn configuration_error(message: &str) -> Self {
        Self::ConfigurationError {
            message: message.to_string(),
        }
    }

    /// Create a new internal error
    pub fn internal_error(message: &str) -> Self {
        Self::InternalError {
            message: message.to_string(),
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self, 
            Error::ExternalServiceError { .. } | 
            Error::InternalError { .. }
        )
    }

    /// Get the error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            Error::EntityNotFound { .. } => "not_found",
            Error::ValidationError { .. } => "validation",
            Error::BusinessRuleViolation { .. } => "business_rule",
            Error::AuthorizationError { .. } => "authorization",
            Error::FhirError { .. } => "fhir",
            Error::DataIntegrityError { .. } => "data_integrity",
            Error::ExternalServiceError { .. } => "external_service",
            Error::ConfigurationError { .. } => "configuration",
            Error::InternalError { .. } => "internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let id = Uuid::new_v4();
        let error = Error::entity_not_found("Patient", id);
        
        match error {
            Error::EntityNotFound { entity_type, id: err_id } => {
                assert_eq!(entity_type, "Patient");
                assert_eq!(err_id, id);
            }
            _ => panic!("Expected EntityNotFound error"),
        }
    }

    #[test]
    fn test_error_category() {
        let error = Error::validation_error("Test validation error");
        assert_eq!(error.category(), "validation");
    }

    #[test]
    fn test_error_retryable() {
        let external_error = Error::external_service_error("TestService", "Connection failed");
        assert!(external_error.is_retryable());

        let validation_error = Error::validation_error("Invalid input");
        assert!(!validation_error.is_retryable());
    }
} 