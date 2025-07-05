//! Observation domain entity

use crate::domain::traits::{Identifiable, Auditable, Validatable};
use crate::domain::values::*;
use crate::types::{Id, Timestamp, EntityMetadata};
use crate::{Result, Error};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Observation entity representing clinical observations
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Observation {
    #[serde(flatten)]
    pub metadata: EntityMetadata,
    
    /// Observation identifiers
    pub identifiers: Vec<Identifier>,
    
    /// Status of the observation
    pub status: ObservationStatus,
    
    /// Category of the observation
    pub category: Vec<String>,
    
    /// Type of observation
    pub code: String,
    
    /// Subject of the observation (patient)
    pub subject: Id,
    
    /// Encounter related to this observation
    pub encounter: Option<Id>,
    
    /// Effective time of the observation
    pub effective: Option<Timestamp>,
    
    /// When observation was issued
    pub issued: Option<Timestamp>,
    
    /// Who performed the observation
    pub performer: Vec<Id>,
    
    /// Observed value
    pub value: Option<ObservationValue>,
    
    /// Interpretation of the observation
    pub interpretation: Vec<String>,
    
    /// Comments about the observation
    pub note: Vec<String>,
    
    /// Method used for the observation
    pub method: Option<String>,
    
    /// Specimen used for the observation
    pub specimen: Option<Id>,
    
    /// Device used for the observation
    pub device: Option<Id>,
    
    /// Reference range for the observation
    pub reference_range: Vec<ObservationReferenceRange>,
    
    /// Related observations
    pub has_member: Vec<Id>,
    
    /// Derived from observations
    pub derived_from: Vec<Id>,
}

/// Observation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationStatus {
    Registered,
    Preliminary,
    Final,
    Amended,
    Corrected,
    Cancelled,
    EnteredInError,
    Unknown,
}

/// Observation value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationValue {
    Quantity {
        value: f64,
        unit: String,
        system: Option<String>,
        code: Option<String>,
    },
    String(String),
    Boolean(bool),
    Integer(i64),
    Range {
        low: Option<f64>,
        high: Option<f64>,
        unit: String,
    },
    Ratio {
        numerator: f64,
        denominator: f64,
        unit: String,
    },
    SampledData {
        origin: f64,
        period: f64,
        factor: Option<f64>,
        lower_limit: Option<f64>,
        upper_limit: Option<f64>,
        dimensions: u32,
        data: String,
    },
    Time(Timestamp),
    DateTime(Timestamp),
    Period {
        start: Option<Timestamp>,
        end: Option<Timestamp>,
    },
}

/// Observation reference range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationReferenceRange {
    /// Low limit
    pub low: Option<f64>,
    
    /// High limit
    pub high: Option<f64>,
    
    /// Type of reference range
    pub type_: Option<String>,
    
    /// Applicable population
    pub applies_to: Vec<String>,
    
    /// Age range
    pub age: Option<String>,
    
    /// Text description
    pub text: Option<String>,
}

impl Observation {
    /// Create a new observation with required fields
    pub fn new(status: ObservationStatus, code: String, subject: Id) -> Self {
        Self {
            metadata: EntityMetadata::new(),
            identifiers: Vec::new(),
            status,
            category: Vec::new(),
            code,
            subject,
            encounter: None,
            effective: None,
            issued: None,
            performer: Vec::new(),
            value: None,
            interpretation: Vec::new(),
            note: Vec::new(),
            method: None,
            specimen: None,
            device: None,
            reference_range: Vec::new(),
            has_member: Vec::new(),
            derived_from: Vec::new(),
        }
    }
}

impl Identifiable for Observation {
    fn id(&self) -> Id {
        self.metadata.id
    }
}

impl Auditable for Observation {
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

impl Validatable for Observation {
    fn validate(&self) -> Result<()> {
        use validator::Validate;
        self.validate().map_err(|e| {
            Error::validation_error(&format!("Observation validation failed: {}", e))
        })?;

        if self.code.trim().is_empty() {
            return Err(Error::validation_error("Observation code cannot be empty"));
        }

        Ok(())
    }
} 