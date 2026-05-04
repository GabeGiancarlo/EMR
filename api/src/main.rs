//! Prototype Actix API entrypoint for Nexus.
//!
//! This binary intentionally keeps a small in-memory surface while the
//! repository is in an architecture reset. API routes return mock data in a
//! predictable format so backend wiring, error handling, and route structure can
//! evolve before database-backed workflows are introduced.

use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer, Result};
use serde::Serialize;
use std::env;
use std::sync::OnceLock;
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    timestamp: chrono::DateTime<chrono::Utc>,
    version: &'static str,
    uptime: u64,
    rust_version: &'static str,
}

#[derive(Serialize)]
struct PatientResponse {
    id: String,
    name: String,
    email: String,
    phone: String,
    birth_date: String,
    status: String,
}

#[derive(Serialize)]
struct PatientsListResponse {
    patients: Vec<PatientResponse>,
    total: usize,
    page: usize,
    per_page: usize,
}

async fn health_check() -> Result<HttpResponse> {
    let uptime = START_TIME
        .get()
        .map(|start| start.elapsed().as_secs())
        .unwrap_or_default();

    let response = HealthResponse {
        status: "healthy",
        timestamp: chrono::Utc::now(),
        version: env!("CARGO_PKG_VERSION"),
        uptime,
        rust_version: match option_env!("CARGO_PKG_RUST_VERSION") {
            Some(v) => v,
            None => "unknown",
        },
    };

    Ok(HttpResponse::Ok().json(response))
}

fn mock_patients() -> Vec<PatientResponse> {
    vec![
        PatientResponse {
            id: "patient-001".to_string(),
            name: "John Doe".to_string(),
            email: "john.doe@example.com".to_string(),
            phone: "+1-555-0123".to_string(),
            birth_date: "1985-06-15".to_string(),
            status: "active".to_string(),
        },
        PatientResponse {
            id: "patient-002".to_string(),
            name: "Jane Smith".to_string(),
            email: "jane.smith@example.com".to_string(),
            phone: "+1-555-0456".to_string(),
            birth_date: "1990-03-22".to_string(),
            status: "active".to_string(),
        },
        PatientResponse {
            id: "patient-003".to_string(),
            name: "Bob Johnson".to_string(),
            email: "bob.johnson@example.com".to_string(),
            phone: "+1-555-0789".to_string(),
            birth_date: "1978-11-03".to_string(),
            status: "active".to_string(),
        },
    ]
}

async fn get_patients() -> Result<HttpResponse> {
    // Prototype-only data while persistent storage is being wired.
    let patients = mock_patients();

    let response = PatientsListResponse {
        total: patients.len(),
        patients,
        page: 1,
        per_page: 10,
    };

    Ok(HttpResponse::Ok().json(response))
}

async fn get_patient(path: web::Path<String>) -> Result<HttpResponse> {
    let patient_id = path.into_inner();

    // Prototype-only data while persistent storage is being wired.
    let patient = PatientResponse {
        id: patient_id,
        name: "John Doe".to_string(),
        email: "john.doe@example.com".to_string(),
        phone: "+1-555-0123".to_string(),
        birth_date: "1985-06-15".to_string(),
        status: "active".to_string(),
    };

    Ok(HttpResponse::Ok().json(patient))
}

#[derive(Serialize)]
struct RootResponse {
    service: &'static str,
    status_endpoint: &'static str,
    patients_endpoint: &'static str,
    note: &'static str,
}

async fn root() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(RootResponse {
        service: "Nexus API prototype backend",
        status_endpoint: "/healthz",
        patients_endpoint: "/api/patients",
        note: "Responses are scaffold-level and include mock data.",
    }))
}

fn configure_cors() -> actix_cors::Cors {
    // Wide-open CORS is temporary for local prototyping and must be narrowed
    // with origin allowlists before production exposure.
    actix_cors::Cors::default()
        .allow_any_origin()
        .allow_any_method()
        .allow_any_header()
        .max_age(3600)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let _ = START_TIME.set(Instant::now());

    let port = env::var("PORT").unwrap_or_else(|_| "8090".to_string());
    let bind_address = format!("127.0.0.1:{}", port);

    println!("Nexus API prototype starting on http://{}", bind_address);
    println!("📋 Health Check: http://{}/healthz", bind_address);
    println!("👥 Patients API: http://{}/api/patients", bind_address);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(configure_cors())
            .service(
                web::scope("/api")
                    .route("/patients", web::get().to(get_patients))
                    .route("/patients/{id}", web::get().to(get_patient)),
            )
            .route("/healthz", web::get().to(health_check))
            .route("/health", web::get().to(health_check))
            .route("/", web::get().to(root))
    })
    .bind(&bind_address)?
    .run()
    .await
}
