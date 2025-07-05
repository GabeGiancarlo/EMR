//! Repository implementations

use crate::error::Result;
use crate::models::PatientModel;
use emr_core::types::Id;

/// Patient repository
pub struct PatientRepository;

impl PatientRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn find_by_id(&self, _id: Id) -> Result<Option<PatientModel>> {
        // TODO: Implement database query
        Ok(None)
    }

    pub async fn create(&self, _patient: &PatientModel) -> Result<PatientModel> {
        // TODO: Implement database insert
        Err(crate::error::ApiError::internal_error("Not implemented"))
    }
} 