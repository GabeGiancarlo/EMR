//! Service-layer scaffolding for the Nexus API.
//!
//! During the architecture reset, this module documents where business rules
//! should live once handlers are split from domain logic and persistence.

use crate::error::Result;
use crate::models::PatientModel;
use emr_core::types::Id;

/// Patient service
pub struct PatientService;

impl PatientService {
    /// Create a new patient service.
    pub fn new() -> Self {
        Self
    }

    /// Resolve a patient by domain identifier.
    ///
    /// Current status: prototype stub, always returns `None`.
    pub async fn get_patient(&self, _id: Id) -> Result<Option<PatientModel>> {
        // TODO(nexus-phase1): Implement business rules and repository lookup.
        Ok(None)
    }

    /// Validate and create a patient.
    ///
    /// Current status: prototype stub, returns a not-implemented API error.
    pub async fn create_patient(&self, _patient: PatientModel) -> Result<PatientModel> {
        // TODO(nexus-phase1): Add validation and persistence orchestration.
        Err(crate::error::ApiError::internal_error("Not implemented"))
    }
} 