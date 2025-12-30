//! Admin executor for administrative commands
//!
//! This module provides the AdminExecutor which handles MongoDB administrative operations:
//! - Database management: show databases, use database
//! - Collection management: show collections
//! - Server commands and diagnostics

use futures::stream::TryStreamExt;
use mongodb::bson::{self, Document};
use tracing::info;

use crate::error::{ExecutionError, MongoshError, Result};
use crate::parser::AdminCommand;

use super::confirmation::confirm_admin_operation;
use super::context::ExecutionContext;
use super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Executor for administrative commands
pub struct AdminExecutor {
    /// Execution context
    context: ExecutionContext,
}

impl AdminExecutor {
    /// Create a new admin executor
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Result<Self>` - New executor or error
    pub async fn new(context: ExecutionContext) -> Result<Self> {
        Ok(Self { context })
    }

    /// Execute an administrative command
    ///
    /// # Arguments
    /// * `cmd` - Admin command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, cmd: AdminCommand) -> Result<ExecutionResult> {
        // Check if operation requires confirmation
        if !confirm_admin_operation(&cmd)? {
            return Ok(ExecutionResult {
                success: true,
                data: ResultData::Message("Operation cancelled by user".to_string()),
                stats: ExecutionStats::default(),
                error: None,
            });
        }

        match cmd {
            AdminCommand::ShowDatabases => self.show_databases().await,
            AdminCommand::ShowCollections => self.show_collections().await,
            AdminCommand::UseDatabase(name) => self.use_database(name).await,
            AdminCommand::ListIndexes(collection) => self.list_indexes(collection).await,
            AdminCommand::CreateIndex {
                collection,
                keys,
                options,
            } => self.create_index(collection, keys, options).await,
            AdminCommand::CreateIndexes {
                collection,
                indexes,
            } => self.create_indexes(collection, indexes).await,
            _ => Err(MongoshError::NotImplemented(
                "Admin command not yet implemented".to_string(),
            )),
        }
    }

    /// Show all databases
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - List of database names
    async fn show_databases(&self) -> Result<ExecutionResult> {
        info!("Listing databases");

        let client = self.context.get_client().await?;

        let db_names = client
            .list_database_names()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!("Found {} databases", db_names.len());

        Ok(ExecutionResult {
            success: true,
            data: ResultData::List(db_names),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Show collections in current database
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - List of collection names
    async fn show_collections(&self) -> Result<ExecutionResult> {
        let db_name = self.context.get_current_database().await;
        info!("Listing collections in database '{}'", db_name);

        let db = self.context.get_database().await?;

        let collection_names = db
            .list_collection_names()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!("Found {} collections", collection_names.len());

        Ok(ExecutionResult {
            success: true,
            data: ResultData::List(collection_names),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Switch to a different database
    ///
    /// # Arguments
    /// * `name` - Database name
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Success message
    async fn use_database(&self, name: String) -> Result<ExecutionResult> {
        info!("Switching to database '{}'", name);

        self.context.set_current_database(name.clone()).await;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(format!("switched to db {}", name)),
            stats: ExecutionStats::default(),
            error: None,
        })
    }

    /// List indexes on a collection
    ///
    /// # Arguments
    /// * `collection` - Collection name
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - List of indexes
    async fn list_indexes(&self, collection: String) -> Result<ExecutionResult> {
        let db_name = self.context.get_current_database().await;
        info!(
            "Listing indexes for collection '{}' in database '{}'",
            collection, db_name
        );

        let db = self.context.get_database().await?;
        let coll: mongodb::Collection<Document> = db.collection(&collection);

        // Get cursor for indexes
        let mut cursor = coll
            .list_indexes()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Collect all indexes into a vector
        let mut indexes = Vec::new();
        while let Some(index) = cursor
            .try_next()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?
        {
            // Convert IndexModel to Document
            let index_doc = bson::to_document(&index).map_err(|e| {
                ExecutionError::QueryFailed(format!("Failed to convert index to document: {}", e))
            })?;
            indexes.push(index_doc);
        }

        let count = indexes.len();
        info!("Found {} indexes", count);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Documents(indexes),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: count,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Parse index options from a document
    ///
    /// # Arguments
    /// * `options_doc` - Options document to parse
    ///
    /// # Returns
    /// * `Result<Option<mongodb::options::IndexOptions>>` - Parsed options or error
    fn parse_index_options(
        options_doc: Option<Document>,
    ) -> Result<Option<mongodb::options::IndexOptions>> {
        match options_doc {
            Some(opts) => {
                let index_opts = bson::from_document(opts).map_err(|e| {
                    ExecutionError::InvalidParameters(format!("Invalid index options: {}", e))
                })?;
                Ok(Some(index_opts))
            }
            None => Ok(None),
        }
    }

    /// Create an index on a collection
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `keys` - Index keys document
    /// * `options` - Optional index options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Index creation result
    async fn create_index(
        &self,
        collection: String,
        keys: Document,
        options: Option<Document>,
    ) -> Result<ExecutionResult> {
        use tracing::debug;

        debug!(
            "Creating index on collection '{}' with keys: {:?}",
            collection, keys
        );

        let db = self.context.get_database().await?;
        let coll: mongodb::Collection<Document> = db.collection(&collection);

        // Parse and validate index options
        let index_options = Self::parse_index_options(options)?;

        // Create index model
        let index_model = mongodb::IndexModel::builder()
            .keys(keys)
            .options(index_options)
            .build();

        // Create the index
        let result = coll
            .create_index(index_model)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        debug!("Created index with name: {}", result.index_name);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(format!("Created index: {}", result.index_name)),
            stats: ExecutionStats::default(),
            error: None,
        })
    }

    /// Create multiple indexes on a collection
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `indexes` - Vector of index specifications
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Index creation result
    async fn create_indexes(
        &self,
        collection: String,
        indexes: Vec<Document>,
    ) -> Result<ExecutionResult> {
        use tracing::debug;

        debug!(
            "Creating {} indexes on collection '{}'",
            indexes.len(),
            collection
        );

        let db = self.context.get_database().await?;
        let coll: mongodb::Collection<Document> = db.collection(&collection);

        // Create index models from documents
        let mut index_models = Vec::new();
        for (idx, index_doc) in indexes.into_iter().enumerate() {
            // Extract keys - MongoDB requires "key" field (not "keys")
            // Spec format: { key: { name: 1 }, name: "idx_name", unique: true, ... }
            let keys = index_doc
                .get_document("key")
                .or_else(|_| index_doc.get_document("keys"))
                .map_err(|_| {
                    ExecutionError::InvalidParameters(format!(
                        "Index specification at position {} must contain 'key' or 'keys' field",
                        idx
                    ))
                })?
                .clone();

            // Extract options - separate from keys
            let options_doc = if let Ok(opts_doc) = index_doc.get_document("options") {
                // Explicit options field
                Some(opts_doc.clone())
            } else {
                // Extract root-level option fields
                let mut opts = Document::new();
                let option_fields = [
                    "name",
                    "unique",
                    "background",
                    "sparse",
                    "expireAfterSeconds",
                    "partialFilterExpression",
                    "collation",
                    "weights",
                    "default_language",
                    "language_override",
                    "textIndexVersion",
                    "2dsphereIndexVersion",
                    "bits",
                    "min",
                    "max",
                    "bucketSize",
                    "storageEngine",
                    "wildcardProjection",
                    "hidden",
                ];

                for field in &option_fields {
                    if let Some(value) = index_doc.get(*field) {
                        opts.insert(*field, value.clone());
                    }
                }

                if !opts.is_empty() { Some(opts) } else { None }
            };

            // Parse options with proper error handling
            let index_options = Self::parse_index_options(options_doc).map_err(|e| {
                ExecutionError::InvalidParameters(format!(
                    "Invalid options for index at position {}: {}",
                    idx, e
                ))
            })?;

            let index_model = mongodb::IndexModel::builder()
                .keys(keys)
                .options(index_options)
                .build();

            index_models.push(index_model);
        }

        // Create the indexes
        let result = coll
            .create_indexes(index_models)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        let index_names = result.index_names.join(", ");
        debug!(
            "Created {} indexes: {}",
            result.index_names.len(),
            index_names
        );

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(format!("Created indexes: {}", index_names)),
            stats: ExecutionStats::default(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_admin_executor_creation() {
        // This is a placeholder test - would need proper setup with ConnectionManager
        // and SharedState to fully test
    }
}
