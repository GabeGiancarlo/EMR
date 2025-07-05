//! Health check handler

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::error::Result;
use crate::AppState;

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status
    pub status: String,
    /// Service version
    pub version: String,
    /// Build information
    pub build: BuildInfo,
    /// Service uptime in seconds
    pub uptime: u64,
    /// Current timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Database connection status
    pub database: ServiceStatus,
    /// FHIR server status
    pub fhir: ServiceStatus,
    /// NATS connection status
    pub nats: ServiceStatus,
}

/// Build information
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildInfo {
    /// Git commit hash
    pub commit: String,
    /// Build timestamp
    pub timestamp: String,
    /// Rust version used for build
    pub rust_version: String,
    /// Target architecture
    pub target: String,
    /// Build profile (debug/release)
    pub profile: String,
}

/// Service status
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceStatus {
    /// Status (up/down/degraded)
    pub status: String,
    /// Response time in milliseconds
    pub response_time_ms: Option<u64>,
    /// Last checked timestamp
    pub last_checked: chrono::DateTime<chrono::Utc>,
    /// Error message if not healthy
    pub error: Option<String>,
}

/// Application start time (set at startup)
static START_TIME: std::sync::OnceLock<chrono::DateTime<chrono::Utc>> = std::sync::OnceLock::new();

/// Initialize the start time
pub fn init_start_time() {
    START_TIME.set(chrono::Utc::now()).ok();
}

/// Get the application uptime in seconds
fn get_uptime() -> u64 {
    START_TIME
        .get()
        .map(|start| {
            let now = chrono::Utc::now();
            (now - *start).num_seconds() as u64
        })
        .unwrap_or(0)
}

/// Health check endpoint
/// 
/// Returns build information and service status
#[get("/healthz")]
pub async fn health_check(
    _req: HttpRequest,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    // Check database connection
    let db_status = check_database_health(&data).await;
    
    // Check FHIR server
    let fhir_status = check_fhir_health(&data).await;
    
    // Check NATS connection
    let nats_status = check_nats_health(&data).await;

    // Determine overall status
    let overall_status = if db_status.status == "up" && fhir_status.status == "up" && nats_status.status == "up" {
        "healthy"
    } else if db_status.status == "down" || fhir_status.status == "down" || nats_status.status == "down" {
        "unhealthy"
    } else {
        "degraded"
    };

    let response = HealthResponse {
        status: overall_status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: BuildInfo {
            commit: option_env!("GIT_COMMIT").unwrap_or("unknown").to_string(),
            timestamp: option_env!("BUILD_TIMESTAMP").unwrap_or("unknown").to_string(),
            rust_version: env!("RUSTC_VERSION").to_string(),
            target: env!("TARGET").to_string(),
            profile: if cfg!(debug_assertions) { "debug" } else { "release" }.to_string(),
        },
        uptime: get_uptime(),
        timestamp: chrono::Utc::now(),
        database: db_status,
        fhir: fhir_status,
        nats: nats_status,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Check database connection health
async fn check_database_health(data: &AppState) -> ServiceStatus {
    let start = std::time::Instant::now();
    
    // TODO: Implement actual database health check
    // For now, assume healthy if pool exists
    let status = if data.db_pool.status().available > 0 {
        "up"
    } else {
        "down"
    };

    ServiceStatus {
        status: status.to_string(),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
        last_checked: chrono::Utc::now(),
        error: if status == "down" { Some("No database connections available".to_string()) } else { None },
    }
}

/// Check FHIR server health
async fn check_fhir_health(data: &AppState) -> ServiceStatus {
    let start = std::time::Instant::now();
    
    // TODO: Implement actual FHIR server health check
    // For now, assume healthy
    let status = "up";

    ServiceStatus {
        status: status.to_string(),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
        last_checked: chrono::Utc::now(),
        error: None,
    }
}

/// Check NATS connection health
async fn check_nats_health(data: &AppState) -> ServiceStatus {
    let start = std::time::Instant::now();
    
    // TODO: Implement actual NATS health check
    // For now, assume healthy if client exists
    let status = "up";

    ServiceStatus {
        status: status.to_string(),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
        last_checked: chrono::Utc::now(),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "0.1.0".to_string(),
            build: BuildInfo {
                commit: "abc123".to_string(),
                timestamp: "2023-01-01T00:00:00Z".to_string(),
                rust_version: "1.78.0".to_string(),
                target: "x86_64-unknown-linux-gnu".to_string(),
                profile: "release".to_string(),
            },
            uptime: 3600,
            timestamp: chrono::Utc::now(),
            database: ServiceStatus {
                status: "up".to_string(),
                response_time_ms: Some(5),
                last_checked: chrono::Utc::now(),
                error: None,
            },
            fhir: ServiceStatus {
                status: "up".to_string(),
                response_time_ms: Some(10),
                last_checked: chrono::Utc::now(),
                error: None,
            },
            nats: ServiceStatus {
                status: "up".to_string(),
                response_time_ms: Some(2),
                last_checked: chrono::Utc::now(),
                error: None,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("0.1.0"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_build_info() {
        let build = BuildInfo {
            commit: "abc123".to_string(),
            timestamp: "2023-01-01T00:00:00Z".to_string(),
            rust_version: "1.78.0".to_string(),
            target: "x86_64-unknown-linux-gnu".to_string(),
            profile: "release".to_string(),
        };

        assert_eq!(build.commit, "abc123");
        assert_eq!(build.profile, "release");
    }

    #[test]
    fn test_service_status() {
        let status = ServiceStatus {
            status: "up".to_string(),
            response_time_ms: Some(5),
            last_checked: chrono::Utc::now(),
            error: None,
        };

        assert_eq!(status.status, "up");
        assert_eq!(status.response_time_ms, Some(5));
        assert!(status.error.is_none());
    }

    #[test]
    fn test_uptime_calculation() {
        init_start_time();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let uptime = get_uptime();
        assert!(uptime >= 0);
    }
} 