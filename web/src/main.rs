#![deny(unsafe_code)]

//! EMR Web Application Server
//!
//! This is the main server for the EMR web application using Leptos SSR with Axum.
//! It provides server-side rendering and handles client-side hydration.

use axum::{
    extract::{Path, State},
    response::Response as AxumResponse,
    routing::{get, post},
    Router,
};
use leptos::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tracing::{info, warn};

use emr_web::{
    app::App,
    config::WebConfig,
    error_template::{AppError, ErrorTemplate},
    state::AppState,
};

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "emr_web=info,warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting EMR Web Server");

    // Load configuration
    let config = match WebConfig::load() {
        Ok(config) => {
            info!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            panic!("Failed to load configuration: {}", e);
        }
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        panic!("Configuration validation failed: {}", e);
    }

    // Create application state
    let app_state = AppState::new(config.clone()).await;

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // Build the application
    let app = Router::new()
        .leptos_routes(&leptos_options, routes, App)
        .route("/api/*fn_name", post(leptos_axum::handle_server_fns))
        .route("/api/health", get(health_check))
        .route("/api/patients", get(get_patients))
        .route("/api/patients/:id", get(get_patient))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(leptos_options)
        .layer(
            ServiceBuilder::new()
                .layer(tower_http::cors::CorsLayer::permissive())
                .layer(tower_http::trace::TraceLayer::new_for_http())
        );

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("EMR Web Server listening on http://{}", addr);

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(feature = "ssr")]
async fn health_check() -> &'static str {
    "OK"
}

#[cfg(feature = "ssr")]
async fn get_patients() -> axum::Json<serde_json::Value> {
    // TODO: Implement actual patient retrieval
    // This is a stub implementation
    axum::Json(serde_json::json!({
        "patients": [
            {
                "id": "patient-1",
                "name": "John Doe",
                "birthDate": "1980-01-01",
                "gender": "male"
            },
            {
                "id": "patient-2", 
                "name": "Jane Smith",
                "birthDate": "1985-05-15",
                "gender": "female"
            }
        ]
    }))
}

#[cfg(feature = "ssr")]
async fn get_patient(Path(id): Path<String>) -> axum::Json<serde_json::Value> {
    // TODO: Implement actual patient retrieval by ID
    // This is a stub implementation
    axum::Json(serde_json::json!({
        "id": id,
        "name": "John Doe",
        "birthDate": "1980-01-01",
        "gender": "male",
        "phone": "+1-555-123-4567",
        "email": "john.doe@example.com"
    }))
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // No-op for when we are compiling for the frontend
} 