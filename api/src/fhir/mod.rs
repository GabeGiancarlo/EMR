//! FHIR client module

use crate::error::{ApiError, Result};
use reqwest::Client;
use serde_json::Value;

/// FHIR client for interacting with Kodjin FHIR server
#[derive(Clone)]
pub struct FhirClient {
    client: Client,
    base_url: String,
}

impl FhirClient {
    /// Create a new FHIR client
    pub fn new(base_url: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ApiError::fhir_error(&format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            base_url: base_url.to_string(),
        })
    }

    /// Get a patient by ID
    pub async fn get_patient(&self, id: &str) -> Result<Value> {
        let url = format!("{}/Patient/{}", self.base_url, id);
        
        let response = self.client
            .get(&url)
            .header("Accept", "application/fhir+json")
            .send()
            .await
            .map_err(|e| ApiError::external_service_error("FHIR", &e.to_string()))?;

        if response.status().is_success() {
            let json: Value = response.json().await
                .map_err(|e| ApiError::fhir_error(&format!("Failed to parse FHIR response: {}", e)))?;
            Ok(json)
        } else {
            Err(ApiError::fhir_error(&format!("FHIR request failed with status: {}", response.status())))
        }
    }

    /// Search for FHIR resources
    pub async fn search(&self, resource_type: &str, params: &[(&str, &str)]) -> Result<Value> {
        let mut url = format!("{}/{}", self.base_url, resource_type);
        
        if !params.is_empty() {
            url.push('?');
            for (i, (key, value)) in params.iter().enumerate() {
                if i > 0 {
                    url.push('&');
                }
                url.push_str(&format!("{}={}", key, value));
            }
        }

        let response = self.client
            .get(&url)
            .header("Accept", "application/fhir+json")
            .send()
            .await
            .map_err(|e| ApiError::external_service_error("FHIR", &e.to_string()))?;

        if response.status().is_success() {
            let json: Value = response.json().await
                .map_err(|e| ApiError::fhir_error(&format!("Failed to parse FHIR response: {}", e)))?;
            Ok(json)
        } else {
            Err(ApiError::fhir_error(&format!("FHIR search failed with status: {}", response.status())))
        }
    }
} 