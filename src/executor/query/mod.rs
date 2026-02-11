//! Query executor for CRUD operations
//!
//! This module provides the QueryExecutor which handles all MongoDB CRUD operations:
//! - Read: find, findOne, count, distinct
//! - Write: insertOne, insertMany, updateOne, updateMany, deleteOne, deleteMany
//! - Aggregate: aggregate
//! - FindAndModify: findOneAndDelete, findOneAndUpdate, findOneAndReplace
//! - Explain: explain command support
//!
//! The module is organized into sub-modules by operation type:
//! - `read`: Read operations
//! - `write`: Write operations
//! - `aggregate`: Aggregation operations
//! - `find_and_modify`: FindAndModify operations
//! - `explain`: Explain operations

use std::time::Instant;

use crate::error::{MongoshError, Result};
use crate::parser::{QueryCommand, QueryMode};
use super::confirmation::confirm_query_operation;
use super::context::ExecutionContext;
use super::result::{ExecutionResult, ExecutionStats, ResultData};

// Sub-modules
mod read;
mod write;
mod aggregate;
mod find_and_modify;
mod explain;

/// Query executor for CRUD operations
pub struct QueryExecutor {
    /// Execution context
    context: ExecutionContext,
}

impl QueryExecutor {
    /// Create a new query executor
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Result<Self>` - New executor or error
    pub async fn new(context: ExecutionContext) -> Result<Self> {
        Ok(Self { context })
    }

    /// Execute a query command
    pub async fn execute(&self, cmd: QueryCommand, mode: QueryMode) -> Result<ExecutionResult> {
        // Check if operation requires confirmation
        if !confirm_query_operation(&cmd)? {
            return Ok(ExecutionResult {
                success: true,
                data: ResultData::Message("Operation cancelled by user".to_string()),
                stats: ExecutionStats::default(),
                error: None,
            });
        }

        let start = Instant::now();

        let result = match cmd {
            QueryCommand::Find {
                collection,
                filter,
                options,
            } => self.execute_find(collection, filter, options, mode).await,

            QueryCommand::FindOne {
                collection,
                filter,
                options,
            } => self.execute_find_one(collection, filter, options).await,

            QueryCommand::CountDocuments { collection, filter } => {
                self.execute_count(collection, Some(filter)).await
            }

            // Write operations
            QueryCommand::InsertOne {
                collection,
                document,
            } => self.execute_insert_one(collection, document).await,

            QueryCommand::InsertMany {
                collection,
                documents,
            } => self.execute_insert_many(collection, documents).await,

            QueryCommand::UpdateOne {
                collection,
                filter,
                update,
                options: _,
            } => self.execute_update_one(collection, filter, update).await,

            QueryCommand::UpdateMany {
                collection,
                filter,
                update,
                options: _,
            } => self.execute_update_many(collection, filter, update).await,

            QueryCommand::DeleteOne { collection, filter } => {
                self.execute_delete_one(collection, filter).await
            }

            QueryCommand::DeleteMany { collection, filter } => {
                self.execute_delete_many(collection, filter).await
            }

            QueryCommand::Aggregate {
                collection,
                pipeline,
                options,
            } => self.execute_aggregate(collection, pipeline, options, mode).await,

            QueryCommand::EstimatedDocumentCount { collection } => {
                self.execute_estimated_document_count(collection).await
            }

            QueryCommand::Distinct {
                collection,
                field,
                filter,
            } => self.execute_distinct(collection, field, filter).await,

            QueryCommand::ReplaceOne {
                collection,
                filter,
                replacement,
                options: _,
            } => {
                self.execute_replace_one(collection, filter, replacement)
                    .await
            }

            QueryCommand::FindOneAndDelete {
                collection,
                filter,
                options,
            } => {
                self.execute_find_one_and_delete(collection, filter, options)
                    .await
            }

            QueryCommand::FindOneAndUpdate {
                collection,
                filter,
                update,
                options,
            } => {
                self.execute_find_one_and_update(collection, filter, update, options)
                    .await
            }

            QueryCommand::FindOneAndReplace {
                collection,
                filter,
                replacement,
                options,
            } => {
                self.execute_find_one_and_replace(collection, filter, replacement, options)
                    .await
            }

            QueryCommand::FindAndModify {
                collection,
                query,
                sort,
                remove,
                update,
                new,
                fields,
                upsert,
                array_filters,
                max_time_ms,
                collation,
            } => {
                self.execute_find_and_modify(
                    collection,
                    query,
                    sort,
                    remove,
                    update,
                    new,
                    fields,
                    upsert,
                    array_filters,
                    max_time_ms,
                    collation,
                )
                .await
            }

            QueryCommand::Explain {
                collection,
                verbosity,
                query,
            } => self.execute_explain(collection, verbosity, *query).await,

            // New command variants - not yet implemented
            QueryCommand::BulkWrite { .. } => Err(MongoshError::NotImplemented(
                "bulkWrite not yet implemented".to_string(),
            )),
        };

        // Add execution time to result
        if let Ok(mut exec_result) = result {
            exec_result.stats.execution_time_ms = start.elapsed().as_millis() as u64;
            Ok(exec_result)
        } else {
            result
        }
    }
}
