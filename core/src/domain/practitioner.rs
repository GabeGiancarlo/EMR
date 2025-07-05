//! Practitioner domain entity

use crate::domain::traits::{Identifiable, Auditable, Validatable};
use crate::domain::values::*;
use crate::types::{Id, Timestamp, EntityMetadata};
use crate::{Result, Error};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Practitioner entity representing healthcare practitioners
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Practitioner {
    #[serde(flatten)]
    pub metadata: EntityMetadata,
    
    /// Practitioner identifiers (NPI, license numbers, etc.)
    pub identifiers: Vec<Identifier>,
    
    /// Practitioner name(s)
    #[validate(length(min = 1))]
    pub names: Vec<HumanName>,
    
    /// Practitioner contact information
    pub telecom: Vec<ContactPoint>,
    
    /// Practitioner addresses
    pub addresses: Vec<Address>,
    
    /// Administrative gender
    pub gender: Option<AdministrativeGender>,
    
    /// Practitioner qualifications
    pub qualifications: Vec<PractitionerQualification>,
    
    /// Communications (languages spoken)
    pub communications: Vec<String>,
    
    /// Whether this practitioner record is active
    pub active: bool,
}

/// Practitioner qualification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PractitionerQualification {
    /// Identifiers for this qualification
    pub identifiers: Vec<Identifier>,
    
    /// Qualification code
    pub code: String,
    
    /// Period when qualification is valid
    pub period: Option<Period>,
    
    /// Issuing organization
    pub issuer: Option<Id>,
}

/// Period of time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
}

impl Practitioner {
    /// Create a new practitioner with required fields
    pub fn new(names: Vec<HumanName>) -> Result<Self> {
        if names.is_empty() {
            return Err(Error::validation_error("Practitioner must have at least one name"));
        }

        Ok(Self {
            metadata: EntityMetadata::new(),
            identifiers: Vec::new(),
            names,
            telecom: Vec::new(),
            addresses: Vec::new(),
            gender: None,
            qualifications: Vec::new(),
            communications: Vec::new(),
            active: true,
        })
    }

    /// Get the practitioner's primary name
    pub fn primary_name(&self) -> Option<&HumanName> {
        self.names.first()
    }
}

impl Identifiable for Practitioner {
    fn id(&self) -> Id {
        self.metadata.id
    }
}

impl Auditable for Practitioner {
    fn created_at(&self) -> Timestamp {
        self.metadata.created_at
    }

    fn updated_at(&self) -> Timestamp {
        self.metadata.updated_at
    }

    fn version(&self) -> u64 {
        self.metadata.version
    }
}

impl Validatable for Practitioner {
    fn validate(&self) -> Result<()> {
        use validator::Validate;
        self.validate().map_err(|e| {
            Error::validation_error(&format!("Practitioner validation failed: {}", e))
        })?;

        if self.names.is_empty() {
            return Err(Error::validation_error("Practitioner must have at least one name"));
        }

        Ok(())
    }
} 