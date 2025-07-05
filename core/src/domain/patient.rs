//! Patient domain entity

use crate::domain::traits::{Identifiable, Auditable, Validatable, FhirConvertible};
use crate::domain::values::*;
use crate::types::{Id, Timestamp, EntityMetadata};
use crate::{Result, Error};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Patient entity representing a person receiving healthcare services
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Patient {
    #[serde(flatten)]
    pub metadata: EntityMetadata,
    
    /// Patient identifiers (MRN, SSN, etc.)
    pub identifiers: Vec<Identifier>,
    
    /// Patient name(s)
    #[validate(length(min = 1))]
    pub names: Vec<HumanName>,
    
    /// Patient contact information
    pub telecom: Vec<ContactPoint>,
    
    /// Administrative gender
    pub gender: Option<AdministrativeGender>,
    
    /// Date of birth
    pub birth_date: Option<NaiveDate>,
    
    /// Deceased information
    pub deceased: Option<DeceasedInfo>,
    
    /// Patient addresses
    pub addresses: Vec<Address>,
    
    /// Marital status
    pub marital_status: Option<MaritalStatus>,
    
    /// Is this patient record a multiple birth
    pub multiple_birth: Option<MultipleBirth>,
    
    /// Patient photos
    pub photos: Vec<Attachment>,
    
    /// Patient contacts (emergency contacts, etc.)
    pub contacts: Vec<PatientContact>,
    
    /// Patient communication preferences
    pub communications: Vec<PatientCommunication>,
    
    /// Managing organization
    pub managing_organization: Option<Id>,
    
    /// Links to other patient records
    pub links: Vec<PatientLink>,
    
    /// Whether this patient record is active
    pub active: bool,
}

/// Deceased information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeceasedInfo {
    Boolean(bool),
    DateTime(Timestamp),
}

/// Marital status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaritalStatus {
    Annulled,
    Divorced,
    Interlocutory,
    LegallySerarated,
    Married,
    Polygamous,
    NeverMarried,
    DomesticPartner,
    Unmarried,
    Widowed,
    Unknown,
}

/// Multiple birth information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultipleBirth {
    Boolean(bool),
    Integer(u32),
}

/// Attachment for photos, documents, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub content_type: String,
    pub language: Option<String>,
    pub data: Option<Vec<u8>>,
    pub url: Option<String>,
    pub size: Option<u64>,
    pub hash: Option<String>,
    pub title: Option<String>,
    pub creation: Option<Timestamp>,
}

/// Patient contact (emergency contact, guardian, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PatientContact {
    /// Relationship to patient
    pub relationships: Vec<ContactRelationship>,
    
    /// Contact name
    pub name: Option<HumanName>,
    
    /// Contact information
    pub telecom: Vec<ContactPoint>,
    
    /// Contact address
    pub address: Option<Address>,
    
    /// Administrative gender
    pub gender: Option<AdministrativeGender>,
    
    /// Organization this contact represents
    pub organization: Option<Id>,
    
    /// Period when this contact was/is valid
    pub period: Option<Period>,
}

/// Contact relationship types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContactRelationship {
    EmergencyContact,
    Guardian,
    Parent,
    Spouse,
    Child,
    Sibling,
    Other(String),
}

/// Patient communication preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientCommunication {
    /// Language preference
    pub language: String,
    
    /// Is this the preferred language
    pub preferred: bool,
}

/// Patient links to other records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientLink {
    /// Other patient record
    pub other: Id,
    
    /// Type of link
    pub type_: PatientLinkType,
}

/// Patient link types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatientLinkType {
    /// Patient records that are the same person
    ReplacedBy,
    /// Patient records that are the same person
    Replaces,
    /// Patient records that refer to the same person
    Refer,
    /// Patient records that are the same person
    Seealso,
}

/// Period of time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
}

impl Patient {
    /// Create a new patient with required fields
    pub fn new(names: Vec<HumanName>) -> Result<Self> {
        if names.is_empty() {
            return Err(Error::validation_error("Patient must have at least one name"));
        }

        Ok(Self {
            metadata: EntityMetadata::new(),
            identifiers: Vec::new(),
            names,
            telecom: Vec::new(),
            gender: None,
            birth_date: None,
            deceased: None,
            addresses: Vec::new(),
            marital_status: None,
            multiple_birth: None,
            photos: Vec::new(),
            contacts: Vec::new(),
            communications: Vec::new(),
            managing_organization: None,
            links: Vec::new(),
            active: true,
        })
    }

    /// Add an identifier to the patient
    pub fn add_identifier(&mut self, identifier: Identifier) {
        self.identifiers.push(identifier);
        self.metadata.update();
    }

    /// Add a contact point to the patient
    pub fn add_telecom(&mut self, telecom: ContactPoint) {
        self.telecom.push(telecom);
        self.metadata.update();
    }

    /// Add an address to the patient
    pub fn add_address(&mut self, address: Address) {
        self.addresses.push(address);
        self.metadata.update();
    }

    /// Set the patient as deceased
    pub fn set_deceased(&mut self, deceased: DeceasedInfo) {
        self.deceased = Some(deceased);
        self.metadata.update();
    }

    /// Get the patient's primary name
    pub fn primary_name(&self) -> Option<&HumanName> {
        self.names.first()
    }

    /// Get the patient's primary identifier
    pub fn primary_identifier(&self) -> Option<&Identifier> {
        self.identifiers.first()
    }

    /// Check if the patient is deceased
    pub fn is_deceased(&self) -> bool {
        self.deceased.is_some()
    }

    /// Get patient age in years (if birth date is available)
    pub fn age_in_years(&self) -> Option<u32> {
        self.birth_date.map(|birth_date| {
            let today = chrono::Utc::now().date_naive();
            let age = today.years_since(birth_date).unwrap_or(0);
            age as u32
        })
    }

    /// Deactivate the patient record
    pub fn deactivate(&mut self) {
        self.active = false;
        self.metadata.update();
    }

    /// Activate the patient record
    pub fn activate(&mut self) {
        self.active = true;
        self.metadata.update();
    }
}

impl Identifiable for Patient {
    fn id(&self) -> Id {
        self.metadata.id
    }
}

impl Auditable for Patient {
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

impl Validatable for Patient {
    fn validate(&self) -> Result<()> {
        // Use validator crate for basic validation
        use validator::Validate;
        self.validate().map_err(|e| {
            Error::validation_error(&format!("Patient validation failed: {}", e))
        })?;

        // Additional business rule validation
        if self.names.is_empty() {
            return Err(Error::validation_error("Patient must have at least one name"));
        }

        // Validate that deceased patients have either boolean or datetime
        if let Some(DeceasedInfo::Boolean(false)) = self.deceased {
            return Err(Error::validation_error("If deceased is false, it should be None instead"));
        }

        // Validate birth date is not in the future
        if let Some(birth_date) = self.birth_date {
            let today = chrono::Utc::now().date_naive();
            if birth_date > today {
                return Err(Error::validation_error("Birth date cannot be in the future"));
            }
        }

        // Validate multiple birth
        if let Some(MultipleBirth::Integer(n)) = self.multiple_birth {
            if n == 0 {
                return Err(Error::validation_error("Multiple birth integer must be greater than 0"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_name() -> HumanName {
        HumanName {
            given: vec!["John".to_string()],
            family: "Doe".to_string(),
            prefix: None,
            suffix: None,
            use_: Some(NameUse::Official),
        }
    }

    #[test]
    fn test_patient_creation() {
        let names = vec![create_test_name()];
        let patient = Patient::new(names).unwrap();
        
        assert_eq!(patient.names.len(), 1);
        assert!(patient.active);
        assert_eq!(patient.metadata.version, 1);
    }

    #[test]
    fn test_patient_creation_without_names() {
        let result = Patient::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_patient_add_identifier() {
        let names = vec![create_test_name()];
        let mut patient = Patient::new(names).unwrap();
        
        let identifier = Identifier {
            use_: Some(IdentifierUse::Official),
            system: Some("MRN".to_string()),
            value: "123456".to_string(),
        };
        
        patient.add_identifier(identifier);
        
        assert_eq!(patient.identifiers.len(), 1);
        assert_eq!(patient.metadata.version, 2);
    }

    #[test]
    fn test_patient_age_calculation() {
        let names = vec![create_test_name()];
        let mut patient = Patient::new(names).unwrap();
        
        // Set birth date to 30 years ago
        let birth_date = chrono::Utc::now().date_naive() - chrono::Duration::days(30 * 365);
        patient.birth_date = Some(birth_date);
        
        let age = patient.age_in_years().unwrap();
        assert!(age >= 29 && age <= 31); // Allow for some variance
    }

    #[test]
    fn test_patient_validation() {
        let names = vec![create_test_name()];
        let patient = Patient::new(names).unwrap();
        
        assert!(patient.validate().is_ok());
    }

    #[test]
    fn test_patient_deceased_validation() {
        let names = vec![create_test_name()];
        let mut patient = Patient::new(names).unwrap();
        
        // Setting deceased to false should cause validation error
        patient.deceased = Some(DeceasedInfo::Boolean(false));
        assert!(patient.validate().is_err());
    }

    #[test]
    fn test_patient_future_birth_date_validation() {
        let names = vec![create_test_name()];
        let mut patient = Patient::new(names).unwrap();
        
        // Set birth date to tomorrow
        let tomorrow = chrono::Utc::now().date_naive() + chrono::Duration::days(1);
        patient.birth_date = Some(tomorrow);
        
        assert!(patient.validate().is_err());
    }
} 