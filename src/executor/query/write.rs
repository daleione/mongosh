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

        let result = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let filter = filter.clone();
                let update = update.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    use mongodb::options::UpdateOptions;
                    let mut options = UpdateOptions::default();
                    // CRITICAL: Set comment for killOp support
                    options.comment = Some(Bson::String(handle.comment().to_string()));

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

        let result = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let filter = filter.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    use mongodb::options::DeleteOptions;
                    let mut options = DeleteOptions::default();
                    // CRITICAL: Set comment for killOp support
                    options.comment = Some(Bson::String(handle.comment().to_string()));

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
