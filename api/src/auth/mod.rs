//! Authentication and authorization module

pub mod oauth2;
pub mod jwt;

use crate::error::{ApiError, Result};

/// JWT claims
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub scope: Option<String>,
}

/// Validate JWT token
pub fn validate_token(token: &str) -> Result<Claims> {
    // TODO: Implement JWT validation
    Err(ApiError::authentication_error("JWT validation not implemented"))
} 