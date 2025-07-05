#![deny(unsafe_code)]
#![warn(missing_docs)]

//! Core domain logic for EMR platform
//! 
//! This crate contains the pure domain logic without any external dependencies
//! on web frameworks, databases, or other infrastructure concerns.

pub mod domain;
pub mod error;
pub mod services;
pub mod repositories;

pub use error::{Result, Error};

/// Common types used throughout the application
pub mod types {
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;
    use chrono::{DateTime, Utc};

    /// Unique identifier for entities
    pub type Id = Uuid;

    /// Timestamp type
    pub type Timestamp = DateTime<Utc>;

    /// Common metadata for all entities
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EntityMetadata {
        pub id: Id,
        pub created_at: Timestamp,
        pub updated_at: Timestamp,
        pub version: u64,
    }

    impl EntityMetadata {
        /// Create new metadata
        pub fn new() -> Self {
            let now = Utc::now();
            Self {
                id: Uuid::new_v4(),
                created_at: now,
                updated_at: now,
                version: 1,
            }
        }

        /// Update metadata with new timestamp and version
        pub fn update(&mut self) {
            self.updated_at = Utc::now();
            self.version += 1;
        }
    }

    impl Default for EntityMetadata {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_metadata_creation() {
        let metadata = types::EntityMetadata::new();
        assert_eq!(metadata.version, 1);
        assert!(metadata.created_at <= metadata.updated_at);
    }

    #[test]
    fn test_entity_metadata_update() {
        let mut metadata = types::EntityMetadata::new();
        let original_version = metadata.version;
        let original_updated_at = metadata.updated_at;
        
        // Sleep to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));
        
        metadata.update();
        
        assert_eq!(metadata.version, original_version + 1);
        assert!(metadata.updated_at > original_updated_at);
    }
} 