//! Patient handlers

use actix_web::{get, post, put, delete, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::error::Result;
use crate::handlers::{ApiResponse, PaginationParams, PaginatedResponse, PaginationMeta};
use crate::AppState;

/// Patient response DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct PatientResponse {
    pub id: String,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub active: bool,
}

/// Patient creation request
#[derive(Debug, Deserialize)]
pub struct CreatePatientRequest {
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
}

/// Get patient by ID
#[get("/patients/{id}")]
pub async fn get_patient(
    path: web::Path<String>,
    _req: HttpRequest,
    _data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let patient_id = path.into_inner();
    
    // TODO: Fetch from database
    let patient = PatientResponse {
        id: patient_id,
        name: "John Doe".to_string(),
        gender: Some("male".to_string()),
        birth_date: Some("1990-01-01".to_string()),
        active: true,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::new(patient)))
}

/// List patients with pagination
#[get("/patients")]
pub async fn list_patients(
    query: web::Query<PaginationParams>,
    _req: HttpRequest,
    _data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let (page, per_page) = query.normalize();
    
    // TODO: Fetch from database
    let patients = vec![
        PatientResponse {
            id: "1".to_string(),
            name: "John Doe".to_string(),
            gender: Some("male".to_string()),
            birth_date: Some("1990-01-01".to_string()),
            active: true,
        },
        PatientResponse {
            id: "2".to_string(),
            name: "Jane Smith".to_string(),
            gender: Some("female".to_string()),
            birth_date: Some("1985-05-15".to_string()),
            active: true,
        },
    ];
    
    let total = 2u64;
    let pagination = PaginationMeta::new(page, per_page, total);
    
    let response = PaginatedResponse {
        data: patients,
        pagination,
    };
    
    Ok(HttpResponse::Ok().json(response))
}

/// Create new patient
#[post("/patients")]
pub async fn create_patient(
    request: web::Json<CreatePatientRequest>,
    _req: HttpRequest,
    _data: web::Data<AppState>,
) -> Result<HttpResponse> {
    // TODO: Save to database
    let patient = PatientResponse {
        id: uuid::Uuid::new_v4().to_string(),
        name: request.name.clone(),
        gender: request.gender.clone(),
        birth_date: request.birth_date.clone(),
        active: true,
    };
    
    Ok(HttpResponse::Created().json(ApiResponse::new(patient)))
}

/// Update patient
#[put("/patients/{id}")]
pub async fn update_patient(
    path: web::Path<String>,
    request: web::Json<CreatePatientRequest>,
    _req: HttpRequest,
    _data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let patient_id = path.into_inner();
    
    // TODO: Update in database
    let patient = PatientResponse {
        id: patient_id,
        name: request.name.clone(),
        gender: request.gender.clone(),
        birth_date: request.birth_date.clone(),
        active: true,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse::new(patient)))
}

/// Delete patient
#[delete("/patients/{id}")]
pub async fn delete_patient(
    path: web::Path<String>,
    _req: HttpRequest,
    _data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let _patient_id = path.into_inner();
    
    // TODO: Delete from database
    
    Ok(HttpResponse::NoContent().finish())
} 