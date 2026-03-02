//! Write operations for query executor
//!
//! This module contains all write operations including:
//! - insertOne, insertMany
//! - updateOne, updateMany
//! - deleteOne, deleteMany
//! - replaceOne

use mongodb::Collection;
use mongodb::bson::{Bson, Document};
use tracing::{debug, info};

use crate::error::{ExecutionError, Result};
use super::super::killable::run_killable_command;
use super::super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Check if MongoDB server version supports comment field on write operations
///
/// The comment field for delete and update operations was added in MongoDB 4.4
///
/// # Arguments
/// * `version_str` - Server version string (e.g., "4.2.0", "4.4.0", "5.0.0")
///
/// # Returns
/// * `bool` - True if version >= 4.4.0, False otherwise
fn supports_write_comment(version_str: Option<&str>) -> bool {
    let version_str = match version_str {
        Some(v) => v,
        None => return false,
    };

    // Parse version string like "4.4.0" or "5.0.0-rc1"
    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // Extract major and minor version numbers
    let major: u32 = parts[0].parse().unwrap_or(0);
    let minor: u32 = parts[1].split('-').next().unwrap_or("0").parse().unwrap_or(0);

    // Comment field supported in MongoDB 4.4+
    major > 4 || (major == 4 && minor >= 4)
}

/// Write operations implementation
impl super::QueryExecutor {
    /// Execute insertOne command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `document` - Document to insert
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Insert result
    pub(super) async fn execute_insert_one(
        &self,
        collection: String,
        document: Document,
    ) -> Result<ExecutionResult> {
        debug!("Executing insertOne on collection '{}'", collection);

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let result = coll.insert_one(document).await?;
        let inserted_id = result.inserted_id.to_string();

        Ok(ExecutionResult {
            success: true,
            data: ResultData::InsertOne { inserted_id },
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(1),
            },
            error: None,
        })
    }

    /// Execute insertMany command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `documents` - Documents to insert
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Insert result
    pub(super) async fn execute_insert_many(
        &self,
        collection: String,
        documents: Vec<Document>,
    ) -> Result<ExecutionResult> {
        debug!("Executing insertMany on collection '{}'", collection);

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let count = documents.len();
        let result = coll.insert_many(documents).await?;
        let inserted_ids: Vec<String> = result
            .inserted_ids
            .values()
            .map(|v| v.to_string())
            .collect();

        Ok(ExecutionResult {
            success: true,
            data: ResultData::InsertMany { inserted_ids },
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(count as u64),
            },
            error: None,
        })
    }

    /// Execute updateOne command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    /// * `update` - Update document
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Update result
    pub(super) async fn execute_update_one(
        &self,
        collection: String,
        filter: Document,
        update: Document,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing updateOne on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let result = coll.update_one(filter, update).await?;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Update {
                matched: result.matched_count,
                modified: result.modified_count,
            },
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(result.modified_count),
            },
            error: None,
        })
    }

    /// Execute updateMany command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    /// * `update` - Update document
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Update result
    pub(super) async fn execute_update_many(
        &self,
        collection: String,
        filter: Document,
        update: Document,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing updateMany on collection '{}' with filter: {:?}",
            collection, filter
        );

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;
        let server_version = self.context.shared_state.get_server_version();

        let result = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let filter = filter.clone();
                let update = update.clone();
                let server_version = server_version.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    use mongodb::options::UpdateOptions;
                    let mut options = UpdateOptions::default();
                    // CRITICAL: Set comment for killOp support (only if server supports it)
                    if supports_write_comment(server_version.as_deref()) {
                        options.comment = Some(Bson::String(handle.comment().to_string()));
                    }

                    let result = coll
                        .update_many(filter, update)
                        .with_options(options)
                        .await?;

                    Ok(result)
                })
            },
        )
        .await?;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Update {
                matched: result.matched_count,
                modified: result.modified_count,
            },
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(result.modified_count),
            },
            error: None,
        })
    }

    /// Execute deleteOne command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Delete result
    pub(super) async fn execute_delete_one(
        &self,
        collection: String,
        filter: Document,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing deleteOne on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let result = coll.delete_one(filter).await?;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Delete {
                deleted: result.deleted_count,
            },
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(result.deleted_count),
            },
            error: None,
        })
    }

    /// Execute deleteMany command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Delete result
    pub(super) async fn execute_delete_many(
        &self,
        collection: String,
        filter: Document,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing deleteMany on collection '{}' with filter: {:?}",
            collection, filter
        );

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;
        let server_version = self.context.shared_state.get_server_version();

        let result = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let filter = filter.clone();
                let server_version = server_version.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    use mongodb::options::DeleteOptions;
                    let mut options = DeleteOptions::default();
                    // CRITICAL: Set comment for killOp support (only if server supports it)
                    if supports_write_comment(server_version.as_deref()) {
                        options.comment = Some(Bson::String(handle.comment().to_string()));
                    }

                    let result = coll
                        .delete_many(filter)
                        .with_options(options)
                        .await?;

                    Ok(result)
                })
            },
        )
        .await?;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Delete {
                deleted: result.deleted_count,
            },
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(result.deleted_count),
            },
            error: None,
        })
    }

    /// Execute replaceOne command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter to match document
    /// * `replacement` - Replacement document
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Replace result or error
    pub(super) async fn execute_replace_one(
        &self,
        collection: String,
        filter: Document,
        replacement: Document,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing replaceOne on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let result = coll
            .replace_one(filter, replacement)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!(
            "ReplaceOne result: matched={}, modified={}",
            result.matched_count, result.modified_count
        );

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Update {
                matched: result.matched_count,
                modified: result.modified_count,
            },
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(result.modified_count),
            },
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports_write_comment() {
        // MongoDB 4.4+ supports comment
        assert!(supports_write_comment(Some("4.4.0")));
        assert!(supports_write_comment(Some("4.4.1")));
        assert!(supports_write_comment(Some("5.0.0")));
        assert!(supports_write_comment(Some("5.0.1")));
        assert!(supports_write_comment(Some("6.0.0")));
        assert!(supports_write_comment(Some("7.0.0")));

        // MongoDB < 4.4 does not support comment
        assert!(!supports_write_comment(Some("4.2.0")));
        assert!(!supports_write_comment(Some("4.3.0")));
        assert!(!supports_write_comment(Some("4.0.0")));
        assert!(!supports_write_comment(Some("3.6.0")));

        // Edge cases
        assert!(!supports_write_comment(None));
        assert!(!supports_write_comment(Some("")));
        assert!(!supports_write_comment(Some("invalid")));

        // Version strings with release candidates or other suffixes
        assert!(supports_write_comment(Some("4.4.0-rc1")));
        assert!(supports_write_comment(Some("5.0.0-rc2")));
        assert!(!supports_write_comment(Some("4.2.0-rc1")));
    }
}
