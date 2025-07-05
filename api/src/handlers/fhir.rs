//! FHIR proxy handlers

use actix_web::{get, web, HttpRequest, HttpResponse};
use crate::error::Result;
use crate::handlers::{ApiResponse, PaginationParams};
use crate::AppState;

/// Get FHIR patient by ID (proxy to Kodjin)
#[get("/fhir/Patient/{id}")]
pub async fn get_fhir_patient(
    path: web::Path<String>,
    _req: HttpRequest,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let patient_id = path.into_inner();
    
    // Proxy request to FHIR server
    let fhir_response = data.fhir_client.get_patient(&patient_id).await?;
    
    Ok(HttpResponse::Ok()
        .content_type("application/fhir+json")
        .json(fhir_response))
}

/// Search FHIR resources
#[get("/fhir/{resource_type}")]
pub async fn search_fhir_resources(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
    _req: HttpRequest,
    data: web::Data<AppState>,
) -> Result<HttpResponse> {
    let resource_type = path.into_inner();
    
    // Convert query parameters
    let params: Vec<(&str, &str)> = query
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    
    // Proxy search to FHIR server
    let fhir_response = data.fhir_client.search(&resource_type, &params).await?;
    
    Ok(HttpResponse::Ok()
        .content_type("application/fhir+json")
        .json(fhir_response))
} 