//! Encounter domain entity

use crate::domain::traits::{Identifiable, Auditable, Validatable};
use crate::domain::values::*;
use crate::types::{Id, Timestamp, EntityMetadata};
use crate::{Result, Error};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Encounter entity representing healthcare encounters
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Encounter {
    #[serde(flatten)]
    pub metadata: EntityMetadata,
    
    /// Encounter identifiers
    pub identifiers: Vec<Identifier>,
    
    /// Status of the encounter
    pub status: EncounterStatus,
    
    /// Classification of the encounter
    pub class: EncounterClass,
    
    /// Type of encounter
    pub type_: Option<String>,
    
    /// Priority of the encounter
    pub priority: Option<String>,
    
    /// Subject of the encounter (patient)
    pub subject: Id,
    
    /// Practitioners involved in the encounter
    pub participants: Vec<EncounterParticipant>,
    
    /// Appointment that scheduled this encounter
    pub appointment: Vec<Id>,
    
    /// Period of the encounter
    pub period: Option<Period>,
    
    /// Length of the encounter
    pub length: Option<u32>,
    
    /// Reason for the encounter
    pub reason: Vec<String>,
    
    /// Diagnosis related to the encounter
    pub diagnosis: Vec<EncounterDiagnosis>,
    
    /// Location where encounter takes place
    pub location: Vec<EncounterLocation>,
    
    /// Organization responsible for the encounter
    pub service_provider: Option<Id>,
}

/// Encounter status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncounterStatus {
    Planned,
    Arrived,
    Triaged,
    InProgress,
    Onleave,
    Finished,
    Cancelled,
    EnteredInError,
    Unknown,
}

/// Encounter class
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncounterClass {
    Inpatient,
    Outpatient,
    Ambulatory,
    Emergency,
    Home,
    Field,
    Daytime,
    Virtual,
}

/// Encounter participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncounterParticipant {
    /// Type of participation
    pub type_: Option<String>,
    
    /// Period of participation
    pub period: Option<Period>,
    
    /// Person involved in the encounter
    pub individual: Option<Id>,
}

/// Encounter diagnosis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncounterDiagnosis {
    /// Diagnosis condition
    pub condition: Id,
    
    /// Role of diagnosis
    pub use_: Option<String>,
    
    /// Ranking of the diagnosis
    pub rank: Option<u32>,
}

/// Encounter location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncounterLocation {
    /// Location reference
    pub location: Id,
    
    /// Status of the location
    pub status: Option<EncounterLocationStatus>,
    
    /// Period at this location
    pub period: Option<Period>,
}

/// Encounter location status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncounterLocationStatus {
    Planned,
    Active,
    Reserved,
    Completed,
}

/// Period of time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
}

impl Encounter {
    /// Create a new encounter with required fields
    pub fn new(status: EncounterStatus, class: EncounterClass, subject: Id) -> Self {
        Self {
            metadata: EntityMetadata::new(),
            identifiers: Vec::new(),
            status,
            class,
            type_: None,
            priority: None,
            subject,
            participants: Vec::new(),
            appointment: Vec::new(),
            period: None,
            length: None,
            reason: Vec::new(),
            diagnosis: Vec::new(),
            location: Vec::new(),
            service_provider: None,
        }
    }
}

impl Identifiable for Encounter {
    fn id(&self) -> Id {
        self.metadata.id
    }
}

impl Auditable for Encounter {
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

impl Validatable for Encounter {
    fn validate(&self) -> Result<()> {
        use validator::Validate;
        self.validate().map_err(|e| {
            Error::validation_error(&format!("Encounter validation failed: {}", e))
        })?;

        Ok(())
    }
} 