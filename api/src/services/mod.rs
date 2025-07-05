//! Service implementations

use crate::error::Result;
use crate::models::PatientModel;
use emr_core::types::Id;

/// Patient service
pub struct PatientService;

impl PatientService {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_patient(&self, _id: Id) -> Result<Option<PatientModel>> {
        // TODO: Implement business logic
        Ok(None)
    }

    pub async fn create_patient(&self, _patient: PatientModel) -> Result<PatientModel> {
        // TODO: Implement business logic with validation
        Err(crate::error::ApiError::internal_error("Not implemented"))
    }
} 