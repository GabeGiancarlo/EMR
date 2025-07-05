//! JWT token handling

use crate::error::{ApiError, Result};
use crate::auth::Claims;

/// Create JWT token
pub fn create_token(_claims: &Claims) -> Result<String> {
    // TODO: Implement JWT creation
    Ok("dummy.jwt.token".to_string())
}

/// Validate JWT token
pub fn validate_token(_token: &str) -> Result<Claims> {
    // TODO: Implement JWT validation
    Ok(Claims {
        sub: "test_user".to_string(),
        exp: (chrono::Utc::now().timestamp() + 3600) as usize,
        iat: chrono::Utc::now().timestamp() as usize,
        scope: Some("patient/*.read".to_string()),
    })
} 