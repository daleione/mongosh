//! Security manager implementation for MCP operations

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Security configuration for MCP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Allow read operations (find, aggregate, count, etc.)
    pub allow_read: bool,

    /// Allow write operations (insert, update)
    pub allow_write: bool,

    /// Allow delete operations
    pub allow_delete: bool,

    /// Maximum number of documents that can be returned in a single query
    pub max_documents_per_query: usize,

    /// Maximum number of pipeline stages allowed in aggregation
    pub max_pipeline_stages: usize,

    /// Query timeout in seconds
    pub query_timeout_seconds: u64,

    /// List of allowed databases (empty means all allowed)
    pub allowed_databases: Vec<String>,

    /// List of forbidden collection patterns
    pub forbidden_collections: Vec<String>,

    /// Enable audit logging
    pub audit_enabled: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            allow_read: true,
            allow_write: false,
            allow_delete: false,
            max_documents_per_query: 1000,
            max_pipeline_stages: 10,
            query_timeout_seconds: 30,
            allowed_databases: vec![],
            forbidden_collections: vec!["system.*".to_string(), "admin.*".to_string()],
            audit_enabled: true,
        }
    }
}

/// Security manager for enforcing access control policies
pub struct SecurityManager {
    config: SecurityConfig,
    forbidden_patterns: HashSet<String>,
}

impl SecurityManager {
    /// Create a new security manager with the given configuration
    pub fn new(config: SecurityConfig) -> Self {
        let forbidden_patterns = config
            .forbidden_collections
            .iter()
            .cloned()
            .collect::<HashSet<_>>();

        Self {
            config,
            forbidden_patterns,
        }
    }

    /// Check if read operations are allowed
    pub fn check_read_permission(&self) -> Result<(), String> {
        if !self.config.allow_read {
            return Err("Read operations are not permitted".to_string());
        }
        Ok(())
    }

    /// Check if write operations are allowed
    pub fn check_write_permission(&self) -> Result<(), String> {
        if !self.config.allow_write {
            return Err("Write operations are not permitted".to_string());
        }
        Ok(())
    }

    /// Check if delete operations are allowed
    pub fn check_delete_permission(&self) -> Result<(), String> {
        if !self.config.allow_delete {
            return Err("Delete operations are not permitted".to_string());
        }
        Ok(())
    }

    /// Check if access to a specific database is allowed
    pub fn check_database_access(&self, database: &str) -> Result<(), String> {
        if !self.config.allowed_databases.is_empty()
            && !self.config.allowed_databases.contains(&database.to_string())
        {
            return Err(format!("Access to database '{}' is not permitted", database));
        }
        Ok(())
    }

    /// Check if access to a specific collection is allowed
    pub fn check_collection_access(&self, database: &str, collection: &str) -> Result<(), String> {
        let full_name = format!("{}.{}", database, collection);

        // Check forbidden patterns against both full name and collection name alone
        for pattern in &self.forbidden_patterns {
            if self.matches_pattern(&full_name, pattern) || self.matches_pattern(collection, pattern) {
                return Err(format!("Access to collection '{}' is forbidden", full_name));
            }
        }

        Ok(())
    }

    /// Validate query limit against maximum allowed
    pub fn validate_limit(&self, limit: Option<i64>) -> Result<(), String> {
        if let Some(limit) = limit {
            if limit > self.config.max_documents_per_query as i64 {
                return Err(format!(
                    "Limit {} exceeds maximum allowed {}",
                    limit, self.config.max_documents_per_query
                ));
            }
        }
        Ok(())
    }

    /// Validate aggregation pipeline stages count
    pub fn validate_pipeline_stages(&self, pipeline: &[bson::Document]) -> Result<(), String> {
        if pipeline.len() > self.config.max_pipeline_stages {
            return Err(format!(
                "Pipeline has {} stages, maximum allowed is {}",
                pipeline.len(),
                self.config.max_pipeline_stages
            ));
        }
        Ok(())
    }

    /// Log audit information for an operation
    pub async fn audit_log(&self, operation: &str, database: &str, collection: &str, details: &str) {
        if self.config.audit_enabled {
            tracing::info!(
                operation = operation,
                database = database,
                collection = collection,
                details = details,
                "MCP operation audit log"
            );
        }
    }

    /// Check if a collection name matches a forbidden pattern
    fn matches_pattern(&self, name: &str, pattern: &str) -> bool {
        // Simple pattern matching: support * as wildcard
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                // Handle empty prefix or suffix
                if prefix.is_empty() {
                    return name.ends_with(suffix);
                }
                if suffix.is_empty() {
                    return name.starts_with(prefix);
                }
                return name.starts_with(prefix) && name.ends_with(suffix);
            }
        }
        name == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SecurityConfig::default();
        assert!(config.allow_read);
        assert!(!config.allow_write);
        assert!(!config.allow_delete);
        assert_eq!(config.max_documents_per_query, 1000);
    }

    #[test]
    fn test_permission_checks() {
        let config = SecurityConfig {
            allow_read: true,
            allow_write: false,
            allow_delete: false,
            ..Default::default()
        };
        let manager = SecurityManager::new(config);

        assert!(manager.check_read_permission().is_ok());
        assert!(manager.check_write_permission().is_err());
        assert!(manager.check_delete_permission().is_err());
    }

    #[test]
    fn test_database_access() {
        let config = SecurityConfig {
            allowed_databases: vec!["test".to_string(), "prod".to_string()],
            ..Default::default()
        };
        let manager = SecurityManager::new(config);

        assert!(manager.check_database_access("test").is_ok());
        assert!(manager.check_database_access("prod").is_ok());
        assert!(manager.check_database_access("unauthorized").is_err());
    }

    #[test]
    fn test_collection_pattern_matching() {
        let config = SecurityConfig {
            forbidden_collections: vec!["system.*".to_string(), "*.internal".to_string()],
            ..Default::default()
        };
        let manager = SecurityManager::new(config);

        assert!(manager.check_collection_access("test", "users").is_ok());
        assert!(manager.check_collection_access("test", "system.indexes").is_err());
        assert!(manager.check_collection_access("test", "data.internal").is_err());
    }

    #[test]
    fn test_validate_limit() {
        let config = SecurityConfig {
            max_documents_per_query: 100,
            ..Default::default()
        };
        let manager = SecurityManager::new(config);

        assert!(manager.validate_limit(Some(50)).is_ok());
        assert!(manager.validate_limit(Some(100)).is_ok());
        assert!(manager.validate_limit(Some(101)).is_err());
        assert!(manager.validate_limit(None).is_ok());
    }

    #[test]
    fn test_validate_pipeline_stages() {
        let config = SecurityConfig {
            max_pipeline_stages: 3,
            ..Default::default()
        };
        let manager = SecurityManager::new(config);

        let pipeline = vec![
            bson::doc! { "$match": { "status": "active" } },
            bson::doc! { "$group": { "_id": "$category" } },
        ];
        assert!(manager.validate_pipeline_stages(&pipeline).is_ok());

        let long_pipeline = vec![
            bson::doc! { "$match": {} },
            bson::doc! { "$group": {} },
            bson::doc! { "$sort": {} },
            bson::doc! { "$limit": {} },
        ];
        assert!(manager.validate_pipeline_stages(&long_pipeline).is_err());
    }
}
