//! Application state management

use crate::config::WebConfig;
use leptos::*;
use std::sync::Arc;

/// Application state
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: WebConfig,
    pub api_client: Arc<ApiClient>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: WebConfig) -> Self {
        let api_client = Arc::new(ApiClient::new(config.api.base_url.clone()));
        
        Self {
            config,
            api_client,
        }
    }

    /// Get the API client
    pub fn api_client(&self) -> &ApiClient {
        &self.api_client
    }

    /// Check if a feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.config.is_feature_enabled(feature)
    }
}

/// API client for making HTTP requests
#[derive(Debug)]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url,
            client,
        }
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Make a GET request
    pub async fn get(&self, path: &str) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'));
        
        let response = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::HttpError(response.status().as_u16()));
        }

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))
    }

    /// Make a POST request
    pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'));
        
        let response = self.client
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::HttpError(response.status().as_u16()));
        }

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))
    }

    /// Make a PUT request
    pub async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'));
        
        let response = self.client
            .put(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::HttpError(response.status().as_u16()));
        }

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))
    }

    /// Make a DELETE request
    pub async fn delete(&self, path: &str) -> Result<(), ApiError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), path.trim_start_matches('/'));
        
        let response = self.client
            .delete(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::HttpError(response.status().as_u16()));
        }

        Ok(())
    }
}

/// API error types
#[derive(Debug, Clone)]
pub enum ApiError {
    NetworkError(String),
    HttpError(u16),
    ParseError(String),
    ValidationError(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ApiError::HttpError(code) => write!(f, "HTTP error: {}", code),
            ApiError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ApiError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

/// User session state
#[derive(Debug, Clone)]
pub struct UserSession {
    pub user_id: String,
    pub username: String,
    pub role: String,
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl UserSession {
    /// Check if the session is expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }

    /// Check if the user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.role == role
    }

    /// Get time until expiration
    pub fn time_until_expiration(&self) -> chrono::Duration {
        self.expires_at - chrono::Utc::now()
    }
}

/// Global application state signals
pub fn create_app_state(config: WebConfig) -> (ReadSignal<Option<AppState>>, WriteSignal<Option<AppState>>) {
    create_signal(None)
}

/// Global user session signals
pub fn create_user_session() -> (ReadSignal<Option<UserSession>>, WriteSignal<Option<UserSession>>) {
    create_signal(None)
}

/// Loading state management
pub fn create_loading_state() -> (ReadSignal<bool>, WriteSignal<bool>) {
    create_signal(false)
}

/// Error state management
pub fn create_error_state() -> (ReadSignal<Option<String>>, WriteSignal<Option<String>>) {
    create_signal(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_api_client_creation() {
        let client = ApiClient::new("http://localhost:8080".to_string());
        assert_eq!(client.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_user_session_expiration() {
        let session = UserSession {
            user_id: "user123".to_string(),
            username: "testuser".to_string(),
            role: "admin".to_string(),
            token: "token123".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
        };

        assert!(!session.is_expired());
        assert!(session.has_role("admin"));
        assert!(!session.has_role("user"));
    }

    #[test]
    fn test_user_session_expired() {
        let session = UserSession {
            user_id: "user123".to_string(),
            username: "testuser".to_string(),
            role: "admin".to_string(),
            token: "token123".to_string(),
            expires_at: Utc::now() - Duration::hours(1),
        };

        assert!(session.is_expired());
    }

    #[test]
    fn test_api_error_display() {
        let error = ApiError::NetworkError("Connection failed".to_string());
        assert_eq!(error.to_string(), "Network error: Connection failed");

        let error = ApiError::HttpError(404);
        assert_eq!(error.to_string(), "HTTP error: 404");
    }
} 