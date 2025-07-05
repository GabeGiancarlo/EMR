#![deny(unsafe_code)]

//! Background jobs worker binary
//!
//! This binary runs the background job processing service for the EMR platform.
//! It handles various job types including FHIR synchronization, data validation,
//! audit reporting, and notifications.

use anyhow::Result;
use dotenvy::dotenv;
use emr_jobs::{config::JobsConfig, worker::JobsWorker};
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "emr_jobs=info,warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting EMR Jobs Worker");

    // Load configuration
    let config = match JobsConfig::load() {
        Ok(config) => {
            info!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    // Create and start the worker
    let worker = JobsWorker::new(config);
    
    // Set up graceful shutdown
    let shutdown_signal = setup_shutdown_signal();
    
    // Start the worker
    tokio::select! {
        result = worker.start() => {
            match result {
                Ok(_) => {
                    info!("Jobs worker completed successfully");
                }
                Err(e) => {
                    error!("Jobs worker failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ = shutdown_signal => {
            info!("Shutdown signal received, stopping worker");
            if let Err(e) = worker.shutdown().await {
                error!("Error during shutdown: {}", e);
                std::process::exit(1);
            }
        }
    }

    info!("EMR Jobs Worker stopped");
    Ok(())
}

/// Set up graceful shutdown signal handling
async fn setup_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }
} 