//! HTTP handlers for the EMR API

pub mod health;
pub mod patients;
pub mod fhir;
pub mod auth;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::error::{ApiError, Result};

/// Common pagination parameters
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl PaginationParams {
    pub fn normalize(&self) -> (u32, u32) {
        let page = self.page.unwrap_or(1).max(1);
        let per_page = self.per_page.unwrap_or(20).clamp(1, 100);
        (page, per_page)
    }

    pub fn offset(&self) -> u32 {
        let (page, per_page) = self.normalize();
        (page - 1) * per_page
    }

    pub fn limit(&self) -> u32 {
        let (_, per_page) = self.normalize();
        per_page
    }
}

/// Common response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub meta: Option<serde_json::Value>,
}

impl<T> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data, meta: None }
    }

    pub fn with_meta(data: T, meta: serde_json::Value) -> Self {
        Self {
            data,
            meta: Some(meta),
        }
    }
}

/// Paginated response
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationMeta,
}

/// Pagination metadata
#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
    pub has_next: bool,
    pub has_prev: bool,
}

impl PaginationMeta {
    pub fn new(page: u32, per_page: u32, total: u64) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;
        Self {
            page,
            per_page,
            total,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        }
    }
}

/// Extract request ID from headers
pub fn extract_request_id(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("X-Request-ID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Extract user ID from request (after authentication)
pub fn extract_user_id(req: &HttpRequest) -> Option<uuid::Uuid> {
    req.extensions()
        .get::<uuid::Uuid>()
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_params_normalize() {
        let params = PaginationParams {
            page: Some(2),
            per_page: Some(10),
        };
        assert_eq!(params.normalize(), (2, 10));
        assert_eq!(params.offset(), 10);
        assert_eq!(params.limit(), 10);
    }

    #[test]
    fn test_pagination_params_defaults() {
        let params = PaginationParams {
            page: None,
            per_page: None,
        };
        assert_eq!(params.normalize(), (1, 20));
        assert_eq!(params.offset(), 0);
        assert_eq!(params.limit(), 20);
    }

    #[test]
    fn test_pagination_params_limits() {
        let params = PaginationParams {
            page: Some(0),
            per_page: Some(200),
        };
        assert_eq!(params.normalize(), (1, 100));
    }

    #[test]
    fn test_pagination_meta() {
        let meta = PaginationMeta::new(2, 10, 25);
        assert_eq!(meta.page, 2);
        assert_eq!(meta.per_page, 10);
        assert_eq!(meta.total, 25);
        assert_eq!(meta.total_pages, 3);
        assert!(meta.has_next);
        assert!(meta.has_prev);
    }

    #[test]
    fn test_api_response() {
        let response = ApiResponse::new("test data");
        assert_eq!(response.data, "test data");
        assert!(response.meta.is_none());

        let response_with_meta = ApiResponse::with_meta(
            "test data",
            serde_json::json!({"key": "value"}),
        );
        assert_eq!(response_with_meta.data, "test data");
        assert!(response_with_meta.meta.is_some());
    }
} 