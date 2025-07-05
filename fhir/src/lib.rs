#![deny(unsafe_code)]

//! FHIR integration helpers for EMR platform
//! 
//! This crate provides utilities for working with FHIR R4 resources
//! and integrating with Kodjin FHIR server.

pub mod client;
pub mod converters;
pub mod validators;

pub use client::*;
pub use converters::*;
pub use validators::*;

use emr_core::{Result, Error};

/// FHIR resource types
#[derive(Debug, Clone)]
pub enum FhirResourceType {
    Patient,
    Practitioner,
    Organization,
    Encounter,
    Observation,
    Other(String),
}

impl From<&str> for FhirResourceType {
    fn from(s: &str) -> Self {
        match s {
            "Patient" => Self::Patient,
            "Practitioner" => Self::Practitioner,
            "Organization" => Self::Organization,
            "Encounter" => Self::Encounter,
            "Observation" => Self::Observation,
            other => Self::Other(other.to_string()),
        }
    }
}

impl std::fmt::Display for FhirResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Patient => write!(f, "Patient"),
            Self::Practitioner => write!(f, "Practitioner"),
            Self::Organization => write!(f, "Organization"),
            Self::Encounter => write!(f, "Encounter"),
            Self::Observation => write!(f, "Observation"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

/// FHIR operation outcomes
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct OperationOutcome {
    pub resource_type: String,
    pub issue: Vec<OperationOutcomeIssue>,
}

/// FHIR operation outcome issue
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct OperationOutcomeIssue {
    pub severity: String,
    pub code: String,
    pub details: Option<serde_json::Value>,
    pub diagnostics: Option<String>,
}

/// FHIR search parameters
#[derive(Debug, Default)]
pub struct SearchParameters {
    pub resource_type: String,
    pub parameters: Vec<(String, String)>,
    pub include: Vec<String>,
    pub rev_include: Vec<String>,
    pub count: Option<u32>,
    pub offset: Option<u32>,
}

impl SearchParameters {
    pub fn new(resource_type: &str) -> Self {
        Self {
            resource_type: resource_type.to_string(),
            ..Default::default()
        }
    }

    pub fn add_parameter(mut self, key: &str, value: &str) -> Self {
        self.parameters.push((key.to_string(), value.to_string()));
        self
    }

    pub fn with_count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    pub fn with_offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn to_query_string(&self) -> String {
        let mut params = self.parameters.clone();
        
        if let Some(count) = self.count {
            params.push(("_count".to_string(), count.to_string()));
        }
        
        if let Some(offset) = self.offset {
            params.push(("_offset".to_string(), offset.to_string()));
        }
        
        for include in &self.include {
            params.push(("_include".to_string(), include.clone()));
        }
        
        for rev_include in &self.rev_include {
            params.push(("_revinclude".to_string(), rev_include.clone()));
        }
        
        params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fhir_resource_type_from_str() {
        assert!(matches!(FhirResourceType::from("Patient"), FhirResourceType::Patient));
        assert!(matches!(FhirResourceType::from("CustomResource"), FhirResourceType::Other(_)));
    }

    #[test]
    fn test_fhir_resource_type_display() {
        assert_eq!(FhirResourceType::Patient.to_string(), "Patient");
        assert_eq!(FhirResourceType::Other("Custom".to_string()).to_string(), "Custom");
    }

    #[test]
    fn test_search_parameters() {
        let params = SearchParameters::new("Patient")
            .add_parameter("name", "John")
            .add_parameter("gender", "male")
            .with_count(10)
            .with_offset(20);

        let query = params.to_query_string();
        assert!(query.contains("name=John"));
        assert!(query.contains("gender=male"));
        assert!(query.contains("_count=10"));
        assert!(query.contains("_offset=20"));
    }

    #[test]
    fn test_operation_outcome_serialization() {
        let outcome = OperationOutcome {
            resource_type: "OperationOutcome".to_string(),
            issue: vec![
                OperationOutcomeIssue {
                    severity: "error".to_string(),
                    code: "invalid".to_string(),
                    details: None,
                    diagnostics: Some("Invalid patient data".to_string()),
                }
            ],
        };

        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("OperationOutcome"));
        assert!(json.contains("error"));
        assert!(json.contains("invalid"));
    }
} 