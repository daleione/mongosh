//! Candidate provider for completion suggestions
//!
//! This module provides the trait and implementation for fetching completion candidates
//! such as collection names, operation names, and commands.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::executor::ExecutionContext;
use crate::repl::SharedState;
use tokio::runtime::Handle;

/// Trait for providing completion candidates
pub trait CandidateProvider: Send + Sync {
    /// Get collection names matching the prefix
    fn collections(&self, prefix: &str) -> Vec<String>;

    /// Get operation names matching the prefix
    fn operations(&self, prefix: &str) -> Vec<String>;

    /// Get "show" subcommands matching the prefix
    fn show_subcommands(&self, prefix: &str) -> Vec<String>;

    /// Get database names matching the prefix
    fn databases(&self, prefix: &str) -> Vec<String>;

    /// Get top-level commands matching the prefix
    fn commands(&self, prefix: &str) -> Vec<String>;
}

/// Cache for collection names
struct CollectionCache {
    /// Cached collection names
    collections: Vec<String>,
    /// Database name these collections belong to
    database: String,
    /// When the cache was last updated
    last_fetch: Instant,
    /// Time-to-live for cache
    ttl: Duration,
}

impl CollectionCache {
    /// Create a new empty cache
    fn new(ttl: Duration) -> Self {
        Self {
            collections: Vec::new(),
            database: String::new(),
            last_fetch: Instant::now() - Duration::from_secs(3600), // Start expired
            ttl,
        }
    }

    /// Check if the cache is still valid
    fn is_valid(&self, current_db: &str) -> bool {
        self.database == current_db && self.last_fetch.elapsed() < self.ttl
    }

    /// Update the cache
    fn update(&mut self, database: String, collections: Vec<String>) {
        self.database = database;
        self.collections = collections;
        self.last_fetch = Instant::now();
    }
}

/// MongoDB candidate provider with caching
pub struct MongoCandidateProvider {
    /// Collection cache
    collection_cache: Arc<RwLock<CollectionCache>>,
    /// Shared state for accessing current database
    shared_state: SharedState,
    /// Execution context for querying database
    execution_context: Option<Arc<ExecutionContext>>,
}

impl MongoCandidateProvider {
    /// Create a new candidate provider
    ///
    /// # Arguments
    /// * `shared_state` - Shared state for accessing current database
    /// * `execution_context` - Optional execution context for querying database
    pub fn new(
        shared_state: SharedState,
        execution_context: Option<Arc<ExecutionContext>>,
    ) -> Self {
        Self {
            collection_cache: Arc::new(RwLock::new(CollectionCache::new(Duration::from_secs(30)))),
            shared_state,
            execution_context,
        }
    }

    /// Get cached collections or fetch from database
    fn get_cached_collections(&self) -> Vec<String> {
        let current_db = self.shared_state.get_database();

        // Check cache first
        {
            let cache = self.collection_cache.read().unwrap();
            if cache.is_valid(&current_db) {
                return cache.collections.clone();
            }
        }

        // Cache miss or expired - try to fetch
        if let Some(ctx) = &self.execution_context {
            // Try to fetch collections using tokio runtime
            // Use block_in_place to avoid nested runtime error
            let collections = if Handle::try_current().is_ok() {
                // We're in a tokio runtime context, use block_in_place
                let ctx_clone = ctx.clone();
                tokio::task::block_in_place(|| {
                    Handle::current().block_on(async move {
                        match ctx_clone.get_database().await {
                            Ok(db) => db.list_collection_names().await.unwrap_or_default(),
                            Err(_) => Vec::new(),
                        }
                    })
                })
            } else {
                // No tokio runtime available, return empty
                Vec::new()
            };

            // Update cache
            let mut cache = self.collection_cache.write().unwrap();
            cache.update(current_db, collections.clone());

            collections
        } else {
            // No execution context, return empty list
            Vec::new()
        }
    }

    /// Filter a list of strings by prefix and sort intelligently
    fn filter_by_prefix(&self, items: &[String], prefix: &str) -> Vec<String> {
        let mut filtered: Vec<String> = if prefix.is_empty() {
            items.to_vec()
        } else {
            items
                .iter()
                .filter(|item| item.starts_with(prefix))
                .cloned()
                .collect()
        };

        // Sort candidates intelligently:
        // 1. Exact matches first (only when prefix is not empty)
        // 2. Shorter names before longer (more specific matches)
        // 3. Alphabetically for same length
        filtered.sort_by(|a, b| {
            // Exact match has highest priority (only check if prefix is not empty)
            if !prefix.is_empty() {
                let a_exact = a == prefix;
                let b_exact = b == prefix;
                if a_exact && !b_exact {
                    return std::cmp::Ordering::Less;
                }
                if !a_exact && b_exact {
                    return std::cmp::Ordering::Greater;
                }
            }

            // Then sort by length (shorter first - more likely what user wants)
            let len_cmp = a.len().cmp(&b.len());
            if len_cmp != std::cmp::Ordering::Equal {
                return len_cmp;
            }

            // Finally alphabetically
            a.cmp(b)
        });

        filtered
    }
}

impl CandidateProvider for MongoCandidateProvider {
    fn collections(&self, prefix: &str) -> Vec<String> {
        let cached = self.get_cached_collections();
        self.filter_by_prefix(&cached, prefix)
    }

    fn operations(&self, prefix: &str) -> Vec<String> {
        let ops = vec![
            "find".to_string(),
            "findOne".to_string(),
            "insertOne".to_string(),
            "insertMany".to_string(),
            "updateOne".to_string(),
            "updateMany".to_string(),
            "deleteOne".to_string(),
            "deleteMany".to_string(),
            "replaceOne".to_string(),
            "countDocuments".to_string(),
            "estimatedDocumentCount".to_string(),
            "distinct".to_string(),
            "aggregate".to_string(),
            "createIndex".to_string(),
            "dropIndex".to_string(),
            "drop".to_string(),
            "rename".to_string(),
        ];
        self.filter_by_prefix(&ops, prefix)
    }

    fn show_subcommands(&self, prefix: &str) -> Vec<String> {
        let cmds = vec![
            "dbs".to_string(),
            "databases".to_string(),
            "collections".to_string(),
            "tables".to_string(),
            "users".to_string(),
            "roles".to_string(),
        ];
        self.filter_by_prefix(&cmds, prefix)
    }

    fn databases(&self, prefix: &str) -> Vec<String> {
        // For now, just return the current database
        // TODO: Implement actual database listing from ExecutionContext
        let current_db = self.shared_state.get_database();
        self.filter_by_prefix(&[current_db], prefix)
    }

    fn commands(&self, prefix: &str) -> Vec<String> {
        let cmds = vec![
            "show".to_string(),
            "use".to_string(),
            "db".to_string(),
            "exit".to_string(),
            "quit".to_string(),
            "help".to_string(),
        ];
        self.filter_by_prefix(&cmds, prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_provider() -> MongoCandidateProvider {
        let shared_state = SharedState::new("test".to_string());
        MongoCandidateProvider::new(shared_state, None)
    }

    #[test]
    fn test_operations_all() {
        let provider = create_test_provider();
        let ops = provider.operations("");

        assert!(ops.contains(&"find".to_string()));
        assert!(ops.contains(&"findOne".to_string()));
        assert!(ops.contains(&"insertOne".to_string()));
        assert!(ops.contains(&"aggregate".to_string()));
    }

    #[test]
    fn test_operations_filter() {
        let provider = create_test_provider();
        let ops = provider.operations("fi");

        assert!(ops.contains(&"find".to_string()));
        assert!(ops.contains(&"findOne".to_string()));
        assert!(!ops.contains(&"insertOne".to_string()));
    }

    #[test]
    fn test_show_subcommands() {
        let provider = create_test_provider();
        let cmds = provider.show_subcommands("");

        assert!(cmds.contains(&"dbs".to_string()));
        assert!(cmds.contains(&"databases".to_string()));
        assert!(cmds.contains(&"collections".to_string()));
    }

    #[test]
    fn test_show_subcommands_filter() {
        let provider = create_test_provider();
        let cmds = provider.show_subcommands("c");

        assert!(cmds.contains(&"collections".to_string()));
        assert!(!cmds.contains(&"dbs".to_string()));
    }

    #[test]
    fn test_top_level_commands() {
        let provider = create_test_provider();
        let cmds = provider.commands("");

        assert!(cmds.contains(&"show".to_string()));
        assert!(cmds.contains(&"use".to_string()));
        assert!(cmds.contains(&"db".to_string()));
        assert!(cmds.contains(&"help".to_string()));
    }

    #[test]
    fn test_commands_filter() {
        let provider = create_test_provider();
        let cmds = provider.commands("sh");

        assert!(cmds.contains(&"show".to_string()));
        assert!(!cmds.contains(&"use".to_string()));
    }

    #[test]
    fn test_databases_returns_current() {
        let provider = create_test_provider();
        let dbs = provider.databases("");

        assert!(dbs.contains(&"test".to_string()));
    }

    #[test]
    fn test_filter_empty_prefix() {
        let provider = create_test_provider();
        let items = vec!["alpha".to_string(), "beta".to_string()];
        let filtered = provider.filter_by_prefix(&items, "");

        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_matching_prefix() {
        let provider = create_test_provider();
        let items = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        let filtered = provider.filter_by_prefix(&items, "a");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "alpha");
    }

    #[test]
    fn test_filter_no_match() {
        let provider = create_test_provider();
        let items = vec!["alpha".to_string(), "beta".to_string()];
        let filtered = provider.filter_by_prefix(&items, "z");

        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_sort_shorter_names_first() {
        let provider = create_test_provider();
        let items = vec![
            "tag_spare_shadow".to_string(),
            "tag_spare".to_string(),
            "tag_spare_archive".to_string(),
        ];
        let filtered = provider.filter_by_prefix(&items, "tag_sp");

        // Should be sorted by length: tag_spare, tag_spare_shadow, tag_spare_archive
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0], "tag_spare");
        assert_eq!(filtered[1], "tag_spare_shadow");
        assert_eq!(filtered[2], "tag_spare_archive");
    }

    #[test]
    fn test_exact_match_first() {
        let provider = create_test_provider();
        let items = vec![
            "users_archive".to_string(),
            "users".to_string(),
            "users_backup".to_string(),
        ];
        let filtered = provider.filter_by_prefix(&items, "users");

        // Exact match "users" should come first
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0], "users");
        assert_eq!(filtered[1], "users_backup");
        assert_eq!(filtered[2], "users_archive");
    }

    #[test]
    fn test_alphabetical_for_same_length() {
        let provider = create_test_provider();
        let items = vec![
            "users".to_string(),
            "tasks".to_string(),
            "notes".to_string(),
        ];
        let filtered = provider.filter_by_prefix(&items, "");

        // Same length, should be alphabetically sorted
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0], "notes");
        assert_eq!(filtered[1], "tasks");
        assert_eq!(filtered[2], "users");
    }

    #[test]
    fn test_complex_sorting_scenario() {
        let provider = create_test_provider();
        let items = vec![
            "collection_long_name".to_string(),
            "coll".to_string(),
            "collection".to_string(),
            "collections".to_string(),
            "col".to_string(),
        ];
        let filtered = provider.filter_by_prefix(&items, "col");

        // Should be sorted: exact match first, then by length, then alphabetically
        assert_eq!(filtered.len(), 5);
        assert_eq!(filtered[0], "col"); // Exact match
        assert_eq!(filtered[1], "coll"); // Length 4
        assert_eq!(filtered[2], "collection"); // Length 10
        assert_eq!(filtered[3], "collections"); // Length 11
        assert_eq!(filtered[4], "collection_long_name"); // Length 20
    }
}
