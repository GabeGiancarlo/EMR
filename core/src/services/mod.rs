//! Domain services

use crate::domain::*;
use crate::types::Id;
use crate::Result;
use async_trait::async_trait;

/// Patient service for business logic
#[async_trait]
pub trait PatientService {
    /// Create a new patient
    async fn create_patient(&self, patient: Patient) -> Result<Patient>;
    
    /// Get patient by ID
    async fn get_patient(&self, id: Id) -> Result<Option<Patient>>;
    
    /// Update patient
    async fn update_patient(&self, patient: Patient) -> Result<Patient>;
    
    /// Delete patient
    async fn delete_patient(&self, id: Id) -> Result<()>;
    
    /// Search patients
    async fn search_patients(&self, query: &str) -> Result<Vec<Patient>>;
    
    /// Get patient demographics
    async fn get_patient_demographics(&self, id: Id) -> Result<Option<PatientDemographics>>;
}

/// Organization service for business logic
#[async_trait]
pub trait OrganizationService {
    /// Create a new organization
    async fn create_organization(&self, organization: Organization) -> Result<Organization>;
    
    /// Get organization by ID
    async fn get_organization(&self, id: Id) -> Result<Option<Organization>>;
    
    /// Update organization
    async fn update_organization(&self, organization: Organization) -> Result<Organization>;
    
    /// Delete organization
    async fn delete_organization(&self, id: Id) -> Result<()>;
    
    /// Get organization hierarchy
    async fn get_organization_hierarchy(&self, id: Id) -> Result<Vec<Organization>>;
}

/// Encounter service for business logic
#[async_trait]
pub trait EncounterService {
    /// Create a new encounter
    async fn create_encounter(&self, encounter: Encounter) -> Result<Encounter>;
    
    /// Get encounter by ID
    async fn get_encounter(&self, id: Id) -> Result<Option<Encounter>>;
    
    /// Update encounter
    async fn update_encounter(&self, encounter: Encounter) -> Result<Encounter>;
    
    /// Get patient encounters
    async fn get_patient_encounters(&self, patient_id: Id) -> Result<Vec<Encounter>>;
    
    /// Start encounter
    async fn start_encounter(&self, id: Id) -> Result<()>;
    
    /// End encounter
    async fn end_encounter(&self, id: Id) -> Result<()>;
}

/// Observation service for business logic
#[async_trait]
pub trait ObservationService {
    /// Create a new observation
    async fn create_observation(&self, observation: Observation) -> Result<Observation>;
    
    /// Get observation by ID
    async fn get_observation(&self, id: Id) -> Result<Option<Observation>>;
    
    /// Update observation
    async fn update_observation(&self, observation: Observation) -> Result<Observation>;
    
    /// Get patient observations
    async fn get_patient_observations(&self, patient_id: Id) -> Result<Vec<Observation>>;
    
    /// Get encounter observations
    async fn get_encounter_observations(&self, encounter_id: Id) -> Result<Vec<Observation>>;
}

/// FHIR service for FHIR operations
#[async_trait]
pub trait FhirService {
    /// Convert domain patient to FHIR patient
    async fn patient_to_fhir(&self, patient: &Patient) -> Result<String>;
    
    /// Convert FHIR patient to domain patient
    async fn patient_from_fhir(&self, fhir_json: &str) -> Result<Patient>;
    
    /// Validate FHIR resource
    async fn validate_fhir_resource(&self, resource_type: &str, resource_json: &str) -> Result<bool>;
    
    /// Search FHIR resources
    async fn search_fhir(&self, resource_type: &str, parameters: &[(&str, &str)]) -> Result<String>;
}

/// Patient demographics summary
#[derive(Debug, Clone)]
pub struct PatientDemographics {
    pub id: Id,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<chrono::NaiveDate>,
    pub age: Option<u32>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub mrn: Option<String>,
    pub active: bool,
}

/// Audit service for tracking changes
#[async_trait]
pub trait AuditService {
    /// Record entity creation
    async fn record_create(&self, entity_type: &str, entity_id: Id, user_id: Id) -> Result<()>;
    
    /// Record entity update
    async fn record_update(&self, entity_type: &str, entity_id: Id, user_id: Id, changes: &str) -> Result<()>;
    
    /// Record entity deletion
    async fn record_delete(&self, entity_type: &str, entity_id: Id, user_id: Id) -> Result<()>;
    
    /// Record data access
    async fn record_access(&self, entity_type: &str, entity_id: Id, user_id: Id, access_type: &str) -> Result<()>;
    
    /// Get audit trail for entity
    async fn get_audit_trail(&self, entity_type: &str, entity_id: Id) -> Result<Vec<AuditEvent>>;
}

/// Audit event
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub id: Id,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: Id,
    pub user_id: Id,
    pub changes: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Security service for authorization
#[async_trait]
pub trait SecurityService {
    /// Check if user has permission to access resource
    async fn check_permission(&self, user_id: Id, resource_type: &str, resource_id: Id, action: &str) -> Result<bool>;
    
    /// Check if user has role
    async fn check_role(&self, user_id: Id, role: &str) -> Result<bool>;
    
    /// Get user permissions
    async fn get_user_permissions(&self, user_id: Id) -> Result<Vec<Permission>>;
    
    /// Validate SMART on FHIR scope
    async fn validate_smart_scope(&self, scope: &str, resource_type: &str, resource_id: Id) -> Result<bool>;
}

/// Permission definition
#[derive(Debug, Clone)]
pub struct Permission {
    pub resource_type: String,
    pub resource_id: Option<Id>,
    pub action: String,
    pub scope: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::values::*;

    #[test]
    fn test_patient_demographics_creation() {
        let demographics = PatientDemographics {
            id: uuid::Uuid::new_v4(),
            name: "John Doe".to_string(),
            gender: Some("male".to_string()),
            birth_date: Some(chrono::NaiveDate::from_ymd_opt(1990, 1, 1).unwrap()),
            age: Some(33),
            address: Some("123 Main St".to_string()),
            phone: Some("555-1234".to_string()),
            email: Some("john.doe@example.com".to_string()),
            mrn: Some("MRN123456".to_string()),
            active: true,
        };

        assert_eq!(demographics.name, "John Doe");
        assert!(demographics.active);
    }

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            event_type: "CREATE".to_string(),
            entity_type: "Patient".to_string(),
            entity_id: uuid::Uuid::new_v4(),
            user_id: uuid::Uuid::new_v4(),
            changes: Some("Created new patient".to_string()),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
        };

        assert_eq!(event.event_type, "CREATE");
        assert_eq!(event.entity_type, "Patient");
    }

    #[test]
    fn test_permission_creation() {
        let permission = Permission {
            resource_type: "Patient".to_string(),
            resource_id: Some(uuid::Uuid::new_v4()),
            action: "read".to_string(),
            scope: Some("patient/*.read".to_string()),
        };

        assert_eq!(permission.resource_type, "Patient");
        assert_eq!(permission.action, "read");
    }
} 