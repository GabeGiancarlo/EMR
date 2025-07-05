#![deny(unsafe_code)]

//! EMR API Server
//! 
//! HIPAA-grade, FHIR-native Electronic Medical Record platform API

use actix_web::{
    middleware::{Logger, NormalizePath},
    web, App, HttpServer,
};
use actix_cors::Cors;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;

mod config;
mod handlers;
mod middleware;
mod database;
mod auth;
mod fhir;
mod error;
mod models;
mod repositories;
mod services;

use config::Config;
use error::Result;

#[actix_web::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "emr_api=info,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env()?;
    info!("Starting EMR API server on {}:{}", config.server.host, config.server.port);

    // Initialize database
    let db_pool = database::create_pool(&config.database).await?;
    
    // Run database migrations
    database::run_migrations(&db_pool).await?;

    // Initialize NATS connection
    let nats_client = async_nats::connect(&config.nats.url).await
        .map_err(|e| error::ApiError::external_service_error("NATS", &e.to_string()))?;

    // Initialize FHIR client
    let fhir_client = fhir::FhirClient::new(&config.fhir.base_url)?;

    // Create application state
    let app_state = web::Data::new(AppState {
        db_pool,
        nats_client,
        fhir_client,
        config: config.clone(),
    });

    // Create HTTP server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(Logger::default())
            .wrap(NormalizePath::trim())
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .supports_credentials(),
            )
            .wrap(middleware::security::SecurityHeaders)
            .wrap(middleware::auth::AuthMiddleware)
            .service(
                web::scope("/api/v1")
                    .service(handlers::health::health_check)
                    .service(handlers::patients::get_patient)
                    .service(handlers::patients::list_patients)
                    .service(handlers::patients::create_patient)
                    .service(handlers::patients::update_patient)
                    .service(handlers::patients::delete_patient)
                    .service(handlers::fhir::get_fhir_patient)
                    .service(handlers::fhir::search_fhir_resources)
                    .service(handlers::auth::authorize)
                    .service(handlers::auth::token)
            )
    })
    .bind_rustls_021(
        (config.server.host.clone(), config.server.port),
        config.server.tls_config()?,
    )?
    .run();

    info!("EMR API server started successfully");
    server.await.map_err(Into::into)
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub db_pool: database::Pool,
    pub nats_client: async_nats::Client,
    pub fhir_client: fhir::FhirClient,
    pub config: Config,
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};

    #[actix_web::test]
    async fn test_health_check() {
        let app = test::init_service(
            App::new()
                .service(handlers::health::health_check)
        ).await;

        let req = test::TestRequest::get()
            .uri("/healthz")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
} 