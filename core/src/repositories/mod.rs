//! Repository traits for data access

use crate::domain::*;
use crate::types::Id;
use crate::Result;
use async_trait::async_trait;

/// Generic repository trait for CRUD operations
#[async_trait]
pub trait Repository<T> {
    /// Create a new entity
    async fn create(&self, entity: &T) -> Result<T>;
    
    /// Find an entity by ID
    async fn find_by_id(&self, id: Id) -> Result<Option<T>>;
    
    /// Update an existing entity
    async fn update(&self, entity: &T) -> Result<T>;
    
    /// Delete an entity by ID
    async fn delete(&self, id: Id) -> Result<()>;
    
    /// List all entities with pagination
    async fn list(&self, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<T>>;
    
    /// Count total entities
    async fn count(&self) -> Result<usize>;
}

/// Patient repository trait
#[async_trait]
pub trait PatientRepository: Repository<Patient> {
    /// Find patients by name
    async fn find_by_name(&self, name: &str) -> Result<Vec<Patient>>;
    
    /// Find patients by identifier
    async fn find_by_identifier(&self, system: &str, value: &str) -> Result<Vec<Patient>>;
    
    /// Find active patients
    async fn find_active(&self) -> Result<Vec<Patient>>;
    
    /// Search patients by text
    async fn search(&self, query: &str) -> Result<Vec<Patient>>;
}

/// Organization repository trait
#[async_trait]
pub trait OrganizationRepository: Repository<Organization> {
    /// Find organizations by name
    async fn find_by_name(&self, name: &str) -> Result<Vec<Organization>>;
    
    /// Find organizations by type
    async fn find_by_type(&self, type_: &str) -> Result<Vec<Organization>>;
    
    /// Find child organizations
    async fn find_children(&self, parent_id: Id) -> Result<Vec<Organization>>;
    
    /// Find active organizations
    async fn find_active(&self) -> Result<Vec<Organization>>;
}

/// Practitioner repository trait
#[async_trait]
pub trait PractitionerRepository: Repository<Practitioner> {
    /// Find practitioners by name
    async fn find_by_name(&self, name: &str) -> Result<Vec<Practitioner>>;
    
    /// Find practitioners by qualification
    async fn find_by_qualification(&self, qualification: &str) -> Result<Vec<Practitioner>>;
    
    /// Find active practitioners
    async fn find_active(&self) -> Result<Vec<Practitioner>>;
}

/// Encounter repository trait
#[async_trait]
pub trait EncounterRepository: Repository<Encounter> {
    /// Find encounters by patient
    async fn find_by_patient(&self, patient_id: Id) -> Result<Vec<Encounter>>;
    
    /// Find encounters by practitioner
    async fn find_by_practitioner(&self, practitioner_id: Id) -> Result<Vec<Encounter>>;
    
    /// Find encounters by status
    async fn find_by_status(&self, status: &str) -> Result<Vec<Encounter>>;
    
    /// Find encounters by date range
    async fn find_by_date_range(&self, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Result<Vec<Encounter>>;
}

/// Observation repository trait
#[async_trait]
pub trait ObservationRepository: Repository<Observation> {
    /// Find observations by patient
    async fn find_by_patient(&self, patient_id: Id) -> Result<Vec<Observation>>;
    
    /// Find observations by encounter
    async fn find_by_encounter(&self, encounter_id: Id) -> Result<Vec<Observation>>;
    
    /// Find observations by code
    async fn find_by_code(&self, code: &str) -> Result<Vec<Observation>>;
    
    /// Find observations by category
    async fn find_by_category(&self, category: &str) -> Result<Vec<Observation>>;
    
    /// Find observations by date range
    async fn find_by_date_range(&self, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Result<Vec<Observation>>;
} 