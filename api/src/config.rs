//! Configuration management for the EMR API server

use config::{Config as ConfigBuilder, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File as StdFile;
use std::io::BufReader;
use actix_web::web::Data;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub fhir: FhirConfig,
    pub nats: NatsConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub tls_cert_path: String,
    pub tls_key_path: String,
    pub max_connections: usize,
    pub keep_alive: u64,
    pub client_timeout: u64,
    pub client_shutdown: u64,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: u64,
    pub idle_timeout: u64,
    pub max_lifetime: u64,
}

/// FHIR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirConfig {
    pub base_url: String,
    pub timeout: u64,
    pub max_retries: u32,
    pub retry_delay: u64,
}

/// NATS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    pub url: String,
    pub cluster_id: String,
    pub client_id: String,
    pub connection_timeout: u64,
    pub max_reconnects: u32,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration: u64,
    pub oauth2_client_id: String,
    pub oauth2_client_secret: String,
    pub oauth2_redirect_uri: String,
    pub oauth2_auth_url: String,
    pub oauth2_token_url: String,
    pub password_hash_cost: u32,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub file_path: Option<String>,
    pub max_file_size: u64,
    pub max_files: u32,
}

impl Config {
    /// Load configuration from environment variables and files
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = ConfigBuilder::new();

        // Add default configuration file
        config.merge(File::with_name("config/default").required(false))?;

        // Add environment-specific configuration
        let env = env::var("EMR_ENV").unwrap_or_else(|_| "development".into());
        config.merge(File::with_name(&format!("config/{}", env)).required(false))?;

        // Add local configuration (gitignored)
        config.merge(File::with_name("config/local").required(false))?;

        // Add environment variables with prefix
        config.merge(Environment::with_prefix("EMR"))?;

        // Try to deserialize into our Config struct
        let mut cfg: Config = config.try_into()?;

        // Set defaults if not provided
        cfg.set_defaults();

        Ok(cfg)
    }

    /// Set default values for configuration
    fn set_defaults(&mut self) {
        // Server defaults
        if self.server.host.is_empty() {
            self.server.host = "127.0.0.1".to_string();
        }
        if self.server.port == 0 {
            self.server.port = 8443;
        }
        if self.server.tls_cert_path.is_empty() {
            self.server.tls_cert_path = "certs/cert.pem".to_string();
        }
        if self.server.tls_key_path.is_empty() {
            self.server.tls_key_path = "certs/key.pem".to_string();
        }
        if self.server.max_connections == 0 {
            self.server.max_connections = 25000;
        }
        if self.server.keep_alive == 0 {
            self.server.keep_alive = 5;
        }
        if self.server.client_timeout == 0 {
            self.server.client_timeout = 5000;
        }
        if self.server.client_shutdown == 0 {
            self.server.client_shutdown = 5000;
        }

        // Database defaults
        if self.database.url.is_empty() {
            self.database.url = "postgresql://emr:emr@localhost/emr".to_string();
        }
        if self.database.max_connections == 0 {
            self.database.max_connections = 32;
        }
        if self.database.min_connections == 0 {
            self.database.min_connections = 1;
        }
        if self.database.connection_timeout == 0 {
            self.database.connection_timeout = 30;
        }
        if self.database.idle_timeout == 0 {
            self.database.idle_timeout = 600;
        }
        if self.database.max_lifetime == 0 {
            self.database.max_lifetime = 1800;
        }

        // FHIR defaults
        if self.fhir.base_url.is_empty() {
            self.fhir.base_url = "http://localhost:8080/fhir".to_string();
        }
        if self.fhir.timeout == 0 {
            self.fhir.timeout = 30;
        }
        if self.fhir.max_retries == 0 {
            self.fhir.max_retries = 3;
        }
        if self.fhir.retry_delay == 0 {
            self.fhir.retry_delay = 1000;
        }

        // NATS defaults
        if self.nats.url.is_empty() {
            self.nats.url = "nats://localhost:4222".to_string();
        }
        if self.nats.cluster_id.is_empty() {
            self.nats.cluster_id = "emr-cluster".to_string();
        }
        if self.nats.client_id.is_empty() {
            self.nats.client_id = "emr-api".to_string();
        }
        if self.nats.connection_timeout == 0 {
            self.nats.connection_timeout = 5;
        }
        if self.nats.max_reconnects == 0 {
            self.nats.max_reconnects = 10;
        }

        // Auth defaults
        if self.auth.jwt_secret.is_empty() {
            self.auth.jwt_secret = "your-secret-key-here".to_string();
        }
        if self.auth.jwt_expiration == 0 {
            self.auth.jwt_expiration = 3600;
        }
        if self.auth.password_hash_cost == 0 {
            self.auth.password_hash_cost = 12;
        }

        // Logging defaults
        if self.logging.level.is_empty() {
            self.logging.level = "info".to_string();
        }
        if self.logging.format.is_empty() {
            self.logging.format = "json".to_string();
        }
        if self.logging.max_file_size == 0 {
            self.logging.max_file_size = 10 * 1024 * 1024; // 10MB
        }
        if self.logging.max_files == 0 {
            self.logging.max_files = 5;
        }
    }
}

impl ServerConfig {
    /// Create TLS configuration for the server
    pub fn tls_config(&self) -> Result<rustls::ServerConfig, Box<dyn std::error::Error>> {
        // Load certificate chain
        let cert_file = &mut BufReader::new(StdFile::open(&self.tls_cert_path)?);
        let cert_chain = certs(cert_file)?
            .into_iter()
            .map(Certificate)
            .collect();

        // Load private key
        let key_file = &mut BufReader::new(StdFile::open(&self.tls_key_path)?);
        let mut keys = pkcs8_private_keys(key_file)?;
        
        if keys.is_empty() {
            return Err("No private key found".into());
        }

        let config = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, PrivateKey(keys.remove(0)))?;

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8443,
                tls_cert_path: "certs/cert.pem".to_string(),
                tls_key_path: "certs/key.pem".to_string(),
                max_connections: 25000,
                keep_alive: 5,
                client_timeout: 5000,
                client_shutdown: 5000,
            },
            database: DatabaseConfig {
                url: "postgresql://emr:emr@localhost/emr".to_string(),
                max_connections: 32,
                min_connections: 1,
                connection_timeout: 30,
                idle_timeout: 600,
                max_lifetime: 1800,
            },
            fhir: FhirConfig {
                base_url: "http://localhost:8080/fhir".to_string(),
                timeout: 30,
                max_retries: 3,
                retry_delay: 1000,
            },
            nats: NatsConfig {
                url: "nats://localhost:4222".to_string(),
                cluster_id: "emr-cluster".to_string(),
                client_id: "emr-api".to_string(),
                connection_timeout: 5,
                max_reconnects: 10,
            },
            auth: AuthConfig {
                jwt_secret: "your-secret-key-here".to_string(),
                jwt_expiration: 3600,
                oauth2_client_id: "emr-client".to_string(),
                oauth2_client_secret: "emr-client-secret".to_string(),
                oauth2_redirect_uri: "https://localhost:8443/auth/callback".to_string(),
                oauth2_auth_url: "https://auth.example.com/oauth2/authorize".to_string(),
                oauth2_token_url: "https://auth.example.com/oauth2/token".to_string(),
                password_hash_cost: 12,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
                file_path: None,
                max_file_size: 10 * 1024 * 1024,
                max_files: 5,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8443);
        assert_eq!(config.database.max_connections, 32);
        assert_eq!(config.fhir.timeout, 30);
        assert_eq!(config.nats.max_reconnects, 10);
    }

    #[test]
    fn test_config_set_defaults() {
        let mut config = Config {
            server: ServerConfig {
                host: "".to_string(),
                port: 0,
                tls_cert_path: "".to_string(),
                tls_key_path: "".to_string(),
                max_connections: 0,
                keep_alive: 0,
                client_timeout: 0,
                client_shutdown: 0,
            },
            database: DatabaseConfig {
                url: "".to_string(),
                max_connections: 0,
                min_connections: 0,
                connection_timeout: 0,
                idle_timeout: 0,
                max_lifetime: 0,
            },
            fhir: FhirConfig {
                base_url: "".to_string(),
                timeout: 0,
                max_retries: 0,
                retry_delay: 0,
            },
            nats: NatsConfig {
                url: "".to_string(),
                cluster_id: "".to_string(),
                client_id: "".to_string(),
                connection_timeout: 0,
                max_reconnects: 0,
            },
            auth: AuthConfig {
                jwt_secret: "".to_string(),
                jwt_expiration: 0,
                oauth2_client_id: "".to_string(),
                oauth2_client_secret: "".to_string(),
                oauth2_redirect_uri: "".to_string(),
                oauth2_auth_url: "".to_string(),
                oauth2_token_url: "".to_string(),
                password_hash_cost: 0,
            },
            logging: LoggingConfig {
                level: "".to_string(),
                format: "".to_string(),
                file_path: None,
                max_file_size: 0,
                max_files: 0,
            },
        };

        config.set_defaults();

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8443);
        assert_eq!(config.database.max_connections, 32);
        assert_eq!(config.fhir.timeout, 30);
        assert_eq!(config.nats.max_reconnects, 10);
    }
} 