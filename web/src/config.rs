//! Web application configuration

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

/// Web application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub server: ServerConfig,
    pub api: ApiConfig,
    pub auth: AuthConfig,
    pub features: FeatureConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub base_url: String,
    pub timeout: u64,
    pub retry_attempts: u32,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: String,
    pub session_timeout: u64,
}

/// Feature flags configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub patient_management: bool,
    pub fhir_integration: bool,
    pub analytics: bool,
    pub audit_logging: bool,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            api: ApiConfig::default(),
            auth: AuthConfig::default(),
            features: FeatureConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            workers: None,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            timeout: 30,
            retry_attempts: 3,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jwt_secret: "default-secret-change-in-production".to_string(),
            session_timeout: 3600,
        }
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            patient_management: true,
            fhir_integration: true,
            analytics: true,
            audit_logging: true,
        }
    }
}

impl WebConfig {
    /// Load configuration from environment variables and config files
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::builder();

        // Load from default config file if it exists
        if let Ok(config_path) = env::var("WEB_CONFIG_PATH") {
            config = config.add_source(File::with_name(&config_path).required(false));
        } else {
            config = config.add_source(File::with_name("web.toml").required(false));
        }

        // Load from environment variables
        config = config
            .add_source(
                Environment::with_prefix("WEB")
                    .separator("_")
                    .prefix_separator("_")
            );

        // Set defaults
        config = config
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 3000)?
            .set_default("api.base_url", "http://localhost:8080")?
            .set_default("api.timeout", 30)?
            .set_default("api.retry_attempts", 3)?
            .set_default("auth.enabled", true)?
            .set_default("auth.jwt_secret", "default-secret-change-in-production")?
            .set_default("auth.session_timeout", 3600)?
            .set_default("features.patient_management", true)?
            .set_default("features.fhir_integration", true)?
            .set_default("features.analytics", true)?
            .set_default("features.audit_logging", true)?;

        config.build()?.try_deserialize()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.server.host.is_empty() {
            return Err("Server host cannot be empty".to_string());
        }

        if self.server.port == 0 {
            return Err("Server port must be greater than 0".to_string());
        }

        if self.api.base_url.is_empty() {
            return Err("API base URL cannot be empty".to_string());
        }

        if self.api.timeout == 0 {
            return Err("API timeout must be greater than 0".to_string());
        }

        if self.auth.enabled && self.auth.jwt_secret.is_empty() {
            return Err("JWT secret cannot be empty when auth is enabled".to_string());
        }

        if self.auth.enabled && self.auth.jwt_secret == "default-secret-change-in-production" {
            return Err("JWT secret must be changed from default value in production".to_string());
        }

        if self.auth.session_timeout == 0 {
            return Err("Session timeout must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Get server bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    /// Check if a feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "patient_management" => self.features.patient_management,
            "fhir_integration" => self.features.fhir_integration,
            "analytics" => self.features.analytics,
            "audit_logging" => self.features.audit_logging,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = WebConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.api.base_url, "http://localhost:8080");
        assert!(config.auth.enabled);
        assert!(config.features.patient_management);
    }

    #[test]
    fn test_bind_address() {
        let config = WebConfig::default();
        assert_eq!(config.bind_address(), "127.0.0.1:3000");
    }

    #[test]
    fn test_feature_enabled() {
        let config = WebConfig::default();
        assert!(config.is_feature_enabled("patient_management"));
        assert!(config.is_feature_enabled("fhir_integration"));
        assert!(!config.is_feature_enabled("nonexistent_feature"));
    }

    #[test]
    fn test_config_validation() {
        let mut config = WebConfig::default();
        
        // Test empty host
        config.server.host = String::new();
        assert!(config.validate().is_err());
        
        // Test zero port
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 0;
        assert!(config.validate().is_err());
        
        // Test empty API URL
        config.server.port = 3000;
        config.api.base_url = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_load_with_env() {
        env::set_var("WEB_SERVER_HOST", "0.0.0.0");
        env::set_var("WEB_SERVER_PORT", "8080");
        env::set_var("WEB_API_BASE_URL", "https://api.example.com");
        
        let config = WebConfig::load().unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.api.base_url, "https://api.example.com");
        
        env::remove_var("WEB_SERVER_HOST");
        env::remove_var("WEB_SERVER_PORT");
        env::remove_var("WEB_API_BASE_URL");
    }
} 