//! Database models

use serde::{Deserialize, Serialize};

/// Patient database model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientModel {
    pub id: uuid::Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<chrono::NaiveDate>,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
} 