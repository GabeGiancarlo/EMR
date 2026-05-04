//! Repository-layer scaffolding for the Nexus API.
//!
//! Database access should be centralized in this layer so handlers and services
//! remain testable. Current implementations are placeholders.

use crate::error::Result;
use crate::models::PatientModel;
use emr_core::types::Id;

/// Patient repository
pub struct PatientRepository;

impl PatientRepository {
    /// Create a new repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Find a patient by ID.
    ///
    /// Current status: prototype stub, always returns `None`.
    pub async fn find_by_id(&self, _id: Id) -> Result<Option<PatientModel>> {
        // TODO(nexus-phase1): Implement SQLx-backed query.
        Ok(None)
    }

    /// Persist a new patient.
    ///
    /// Current status: prototype stub, returns a not-implemented API error.
    pub async fn create(&self, _patient: &PatientModel) -> Result<PatientModel> {
        // TODO(nexus-phase1): Implement SQLx-backed insert.
        Err(crate::error::ApiError::internal_error("Not implemented"))
    }
} 