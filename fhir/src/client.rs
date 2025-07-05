//! FHIR client for Kodjin server integration

use crate::{SearchParameters, OperationOutcome};
use emr_core::{Result, Error};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

/// FHIR client for interacting with Kodjin FHIR server
#[derive(Debug, Clone)]
pub struct KodjinClient {
    base_url: String,
    client: Client,
    timeout: Duration,
}

impl KodjinClient {
    /// Create a new Kodjin FHIR client
    pub fn new(base_url: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::external_service_error("HTTP", &e.to_string()))?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            timeout: Duration::from_secs(30),
        })
    }

    /// Set custom timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Get capability statement
    pub async fn get_capability_statement(&self) -> Result<Value> {
        let url = format!("{}/metadata", self.base_url);
        self.get_json(&url).await
    }

    /// Read a resource by type and ID
    pub async fn read(&self, resource_type: &str, id: &str) -> Result<Value> {
        let url = format!("{}/{}/{}", self.base_url, resource_type, id);
        self.get_json(&url).await
    }

    /// Search resources
    pub async fn search(&self, params: &SearchParameters) -> Result<Value> {
        let mut url = format!("{}/{}", self.base_url, params.resource_type);
        
        let query_string = params.to_query_string();
        if !query_string.is_empty() {
            url.push('?');
            url.push_str(&query_string);
        }

        self.get_json(&url).await
    }

    /// Create a new resource
    pub async fn create(&self, resource_type: &str, resource: &Value) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, resource_type);
        self.post_json(&url, resource).await
    }

    /// Update a resource
    pub async fn update(&self, resource_type: &str, id: &str, resource: &Value) -> Result<Value> {
        let url = format!("{}/{}/{}", self.base_url, resource_type, id);
        self.put_json(&url, resource).await
    }

    /// Delete a resource
    pub async fn delete(&self, resource_type: &str, id: &str) -> Result<()> {
        let url = format!("{}/{}/{}", self.base_url, resource_type, id);
        
        let response = self.client
            .delete(&url)
            .header("Accept", "application/fhir+json")
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| Error::external_service_error("FHIR", &e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(Error::fhir_error(&format!("Delete failed: {} - {}", status, error_text), None))
        }
    }

    /// Validate a resource
    pub async fn validate(&self, resource_type: &str, resource: &Value) -> Result<OperationOutcome> {
        let url = format!("{}/$validate", self.base_url);
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/fhir+json")
            .header("Accept", "application/fhir+json")
            .query(&[("profile", resource_type)])
            .json(resource)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| Error::external_service_error("FHIR", &e.to_string()))?;

        if response.status().is_success() {
            let outcome: OperationOutcome = response.json().await
                .map_err(|e| Error::fhir_error(&format!("Failed to parse validation response: {}", e), None))?;
            Ok(outcome)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(Error::fhir_error(&format!("Validation failed: {} - {}", status, error_text), None))
        }
    }

    /// Perform a GET request and parse JSON response
    async fn get_json(&self, url: &str) -> Result<Value> {
        let response = self.client
            .get(url)
            .header("Accept", "application/fhir+json")
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| Error::external_service_error("FHIR", &e.to_string()))?;

        if response.status().is_success() {
            let json: Value = response.json().await
                .map_err(|e| Error::fhir_error(&format!("Failed to parse JSON response: {}", e), None))?;
            Ok(json)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(Error::fhir_error(&format!("Request failed: {} - {}", status, error_text), None))
        }
    }

    /// Perform a POST request with JSON body
    async fn post_json(&self, url: &str, body: &Value) -> Result<Value> {
        let response = self.client
            .post(url)
            .header("Content-Type", "application/fhir+json")
            .header("Accept", "application/fhir+json")
            .json(body)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| Error::external_service_error("FHIR", &e.to_string()))?;

        if response.status().is_success() {
            let json: Value = response.json().await
                .map_err(|e| Error::fhir_error(&format!("Failed to parse JSON response: {}", e), None))?;
            Ok(json)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(Error::fhir_error(&format!("POST request failed: {} - {}", status, error_text), None))
        }
    }

    /// Perform a PUT request with JSON body
    async fn put_json(&self, url: &str, body: &Value) -> Result<Value> {
        let response = self.client
            .put(url)
            .header("Content-Type", "application/fhir+json")
            .header("Accept", "application/fhir+json")
            .json(body)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| Error::external_service_error("FHIR", &e.to_string()))?;

        if response.status().is_success() {
            let json: Value = response.json().await
                .map_err(|e| Error::fhir_error(&format!("Failed to parse JSON response: {}", e), None))?;
            Ok(json)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(Error::fhir_error(&format!("PUT request failed: {} - {}", status, error_text), None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kodjin_client_creation() {
        let client = KodjinClient::new("http://localhost:8080/fhir").unwrap();
        assert_eq!(client.base_url, "http://localhost:8080/fhir");
    }

    #[test]
    fn test_kodjin_client_with_timeout() {
        let client = KodjinClient::new("http://localhost:8080/fhir")
            .unwrap()
            .with_timeout(Duration::from_secs(60));
        assert_eq!(client.timeout, Duration::from_secs(60));
    }
} 