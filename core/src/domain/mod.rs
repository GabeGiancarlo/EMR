//! Domain entities for the EMR platform

pub mod patient;
pub mod organization;
pub mod practitioner;
pub mod encounter;
pub mod observation;

pub use patient::*;
pub use organization::*;
pub use practitioner::*;
pub use encounter::*;
pub use observation::*;

/// Common domain traits
pub mod traits {
    use crate::types::{Id, Timestamp};
    use crate::Result;

    /// Trait for entities that can be identified
    pub trait Identifiable {
        fn id(&self) -> Id;
    }

    /// Trait for entities that have audit information
    pub trait Auditable {
        fn created_at(&self) -> Timestamp;
        fn updated_at(&self) -> Timestamp;
        fn version(&self) -> u64;
    }

    /// Trait for entities that can be validated
    pub trait Validatable {
        fn validate(&self) -> Result<()>;
    }

    /// Trait for entities that can be converted to FHIR resources
    pub trait FhirConvertible<T> {
        fn to_fhir(&self) -> Result<T>;
        fn from_fhir(resource: T) -> Result<Self>
        where
            Self: Sized;
    }
}

/// Value objects used across the domain
pub mod values {
    use serde::{Deserialize, Serialize};
    use validator::Validate;

    /// Human name representation
    #[derive(Debug, Clone, Serialize, Deserialize, Validate)]
    pub struct HumanName {
        #[validate(length(min = 1, max = 100))]
        pub given: Vec<String>,
        #[validate(length(min = 1, max = 100))]
        pub family: String,
        pub prefix: Option<String>,
        pub suffix: Option<String>,
        pub use_: Option<NameUse>,
    }

    /// Name use types
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum NameUse {
        Usual,
        Official,
        Temp,
        Nickname,
        Anonymous,
        Old,
        Maiden,
    }

    /// Contact information
    #[derive(Debug, Clone, Serialize, Deserialize, Validate)]
    pub struct ContactPoint {
        pub system: ContactSystem,
        #[validate(length(min = 1, max = 100))]
        pub value: String,
        pub use_: Option<ContactUse>,
        pub rank: Option<u32>,
    }

    /// Contact system types
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum ContactSystem {
        Phone,
        Fax,
        Email,
        Pager,
        Url,
        Sms,
        Other,
    }

    /// Contact use types
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum ContactUse {
        Home,
        Work,
        Temp,
        Old,
        Mobile,
    }

    /// Address representation
    #[derive(Debug, Clone, Serialize, Deserialize, Validate)]
    pub struct Address {
        pub use_: Option<AddressUse>,
        pub type_: Option<AddressType>,
        #[validate(length(min = 1, max = 200))]
        pub text: Option<String>,
        pub line: Vec<String>,
        #[validate(length(min = 1, max = 100))]
        pub city: Option<String>,
        #[validate(length(min = 1, max = 100))]
        pub district: Option<String>,
        #[validate(length(min = 1, max = 100))]
        pub state: Option<String>,
        #[validate(length(min = 1, max = 20))]
        pub postal_code: Option<String>,
        #[validate(length(min = 1, max = 100))]
        pub country: Option<String>,
    }

    /// Address use types
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum AddressUse {
        Home,
        Work,
        Temp,
        Old,
        Billing,
    }

    /// Address type
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum AddressType {
        Postal,
        Physical,
        Both,
    }

    /// Identifier for external systems
    #[derive(Debug, Clone, Serialize, Deserialize, Validate)]
    pub struct Identifier {
        pub use_: Option<IdentifierUse>,
        pub system: Option<String>,
        #[validate(length(min = 1, max = 100))]
        pub value: String,
    }

    /// Identifier use types
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum IdentifierUse {
        Usual,
        Official,
        Temp,
        Secondary,
        Old,
    }

    /// Gender representation
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Gender {
        Male,
        Female,
        Other,
        Unknown,
    }

    /// Administrative gender as per FHIR
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum AdministrativeGender {
        Male,
        Female,
        Other,
        Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_name_creation() {
        let name = values::HumanName {
            given: vec!["John".to_string()],
            family: "Doe".to_string(),
            prefix: None,
            suffix: None,
            use_: Some(values::NameUse::Official),
        };

        assert_eq!(name.given.len(), 1);
        assert_eq!(name.family, "Doe");
    }

    #[test]
    fn test_contact_point_creation() {
        let contact = values::ContactPoint {
            system: values::ContactSystem::Email,
            value: "john.doe@example.com".to_string(),
            use_: Some(values::ContactUse::Work),
            rank: None,
        };

        assert_eq!(contact.value, "john.doe@example.com");
        assert!(matches!(contact.system, values::ContactSystem::Email));
    }
} 