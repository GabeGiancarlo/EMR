//! Organization domain entity

use crate::domain::traits::{Identifiable, Auditable, Validatable};
use crate::domain::values::*;
use crate::types::{Id, Timestamp, EntityMetadata};
use crate::{Result, Error};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Organization entity representing healthcare organizations
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Organization {
    #[serde(flatten)]
    pub metadata: EntityMetadata,
    
    /// Organization identifiers (NPI, Tax ID, etc.)
    pub identifiers: Vec<Identifier>,
    
    /// Organization name
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    
    /// Organization alias names
    pub aliases: Vec<String>,
    
    /// Organization contact information
    pub telecom: Vec<ContactPoint>,
    
    /// Organization addresses
    pub addresses: Vec<Address>,
    
    /// Organization type (hospital, clinic, etc.)
    pub type_: Option<OrganizationType>,
    
    /// Part of another organization
    pub part_of: Option<Id>,
    
    /// Organization contacts (key personnel)
    pub contacts: Vec<OrganizationContact>,
    
    /// Organization endpoints (API endpoints, etc.)
    pub endpoints: Vec<Id>,
    
    /// Whether this organization record is active
    pub active: bool,
}

/// Organization type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrganizationType {
    /// Healthcare provider
    Prov,
    /// Hospital department
    Dept,
    /// Organizational team
    Team,
    /// Government
    Govt,
    /// Insurance company
    Ins,
    /// Educational institute
    Edu,
    /// Religious institution
    Reli,
    /// Clinical research sponsor
    Crs,
    /// Community group
    Cg,
    /// Non-healthcare business or corporation
    Bus,
    /// Other
    Other,
}

/// Organization contact person
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct OrganizationContact {
    /// Purpose of contact
    pub purpose: Option<ContactPurpose>,
    
    /// Contact person name
    pub name: Option<HumanName>,
    
    /// Contact information
    pub telecom: Vec<ContactPoint>,
    
    /// Contact address
    pub address: Option<Address>,
}

/// Contact purpose
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContactPurpose {
    /// Billing contact
    Billing,
    /// Administrative contact
    Admin,
    /// Human resources contact
    Hr,
    /// Payroll contact
    Payor,
    /// IT contact
    It,
    /// Press contact
    Press,
}

impl Organization {
    /// Create a new organization with required fields
    pub fn new(name: String) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(Error::validation_error("Organization name cannot be empty"));
        }

        Ok(Self {
            metadata: EntityMetadata::new(),
            identifiers: Vec::new(),
            name,
            aliases: Vec::new(),
            telecom: Vec::new(),
            addresses: Vec::new(),
            type_: None,
            part_of: None,
            contacts: Vec::new(),
            endpoints: Vec::new(),
            active: true,
        })
    }

    /// Add an identifier to the organization
    pub fn add_identifier(&mut self, identifier: Identifier) {
        self.identifiers.push(identifier);
        self.metadata.update();
    }

    /// Add an alias to the organization
    pub fn add_alias(&mut self, alias: String) {
        self.aliases.push(alias);
        self.metadata.update();
    }

    /// Add a contact point to the organization
    pub fn add_telecom(&mut self, telecom: ContactPoint) {
        self.telecom.push(telecom);
        self.metadata.update();
    }

    /// Add an address to the organization
    pub fn add_address(&mut self, address: Address) {
        self.addresses.push(address);
        self.metadata.update();
    }

    /// Set the organization type
    pub fn set_type(&mut self, type_: OrganizationType) {
        self.type_ = Some(type_);
        self.metadata.update();
    }

    /// Set the parent organization
    pub fn set_part_of(&mut self, part_of: Id) {
        self.part_of = Some(part_of);
        self.metadata.update();
    }

    /// Add a contact person to the organization
    pub fn add_contact(&mut self, contact: OrganizationContact) {
        self.contacts.push(contact);
        self.metadata.update();
    }

    /// Add an endpoint to the organization
    pub fn add_endpoint(&mut self, endpoint: Id) {
        self.endpoints.push(endpoint);
        self.metadata.update();
    }

    /// Get the organization's primary identifier
    pub fn primary_identifier(&self) -> Option<&Identifier> {
        self.identifiers.first()
    }

    /// Check if this organization is part of another organization
    pub fn is_part_of_organization(&self) -> bool {
        self.part_of.is_some()
    }

    /// Deactivate the organization record
    pub fn deactivate(&mut self) {
        self.active = false;
        self.metadata.update();
    }

    /// Activate the organization record
    pub fn activate(&mut self) {
        self.active = true;
        self.metadata.update();
    }
}

impl Identifiable for Organization {
    fn id(&self) -> Id {
        self.metadata.id
    }
}

impl Auditable for Organization {
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

impl Validatable for Organization {
    fn validate(&self) -> Result<()> {
        // Use validator crate for basic validation
        use validator::Validate;
        self.validate().map_err(|e| {
            Error::validation_error(&format!("Organization validation failed: {}", e))
        })?;

        // Additional business rule validation
        if self.name.trim().is_empty() {
            return Err(Error::validation_error("Organization name cannot be empty"));
        }

        // Validate that organization doesn't reference itself as parent
        if let Some(part_of) = self.part_of {
            if part_of == self.metadata.id {
                return Err(Error::validation_error("Organization cannot be part of itself"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_organization_creation() {
        let org = Organization::new("Test Hospital".to_string()).unwrap();
        
        assert_eq!(org.name, "Test Hospital");
        assert!(org.active);
        assert_eq!(org.metadata.version, 1);
    }

    #[test]
    fn test_organization_creation_with_empty_name() {
        let result = Organization::new("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_organization_creation_with_whitespace_name() {
        let result = Organization::new("   ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_organization_add_identifier() {
        let mut org = Organization::new("Test Hospital".to_string()).unwrap();
        
        let identifier = Identifier {
            use_: Some(IdentifierUse::Official),
            system: Some("NPI".to_string()),
            value: "1234567890".to_string(),
        };
        
        org.add_identifier(identifier);
        
        assert_eq!(org.identifiers.len(), 1);
        assert_eq!(org.metadata.version, 2);
    }

    #[test]
    fn test_organization_add_alias() {
        let mut org = Organization::new("Test Hospital".to_string()).unwrap();
        
        org.add_alias("TH".to_string());
        
        assert_eq!(org.aliases.len(), 1);
        assert_eq!(org.aliases[0], "TH");
        assert_eq!(org.metadata.version, 2);
    }

    #[test]
    fn test_organization_set_type() {
        let mut org = Organization::new("Test Hospital".to_string()).unwrap();
        
        org.set_type(OrganizationType::Prov);
        
        assert!(matches!(org.type_, Some(OrganizationType::Prov)));
        assert_eq!(org.metadata.version, 2);
    }

    #[test]
    fn test_organization_validation() {
        let org = Organization::new("Test Hospital".to_string()).unwrap();
        assert!(org.validate().is_ok());
    }

    #[test]
    fn test_organization_self_reference_validation() {
        let mut org = Organization::new("Test Hospital".to_string()).unwrap();
        org.part_of = Some(org.metadata.id);
        
        assert!(org.validate().is_err());
    }

    #[test]
    fn test_organization_deactivate() {
        let mut org = Organization::new("Test Hospital".to_string()).unwrap();
        
        org.deactivate();
        
        assert!(!org.active);
        assert_eq!(org.metadata.version, 2);
    }

    #[test]
    fn test_organization_primary_identifier() {
        let mut org = Organization::new("Test Hospital".to_string()).unwrap();
        
        let identifier = Identifier {
            use_: Some(IdentifierUse::Official),
            system: Some("NPI".to_string()),
            value: "1234567890".to_string(),
        };
        
        org.add_identifier(identifier);
        
        let primary = org.primary_identifier().unwrap();
        assert_eq!(primary.value, "1234567890");
    }
} 