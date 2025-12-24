//! Query executor for CRUD operations
//!
//! This module provides the QueryExecutor which handles all MongoDB CRUD operations:
//! - Read: find, findOne, count
//! - Write: insertOne, insertMany, updateOne, updateMany, deleteOne, deleteMany
//! - Aggregate: aggregate

use std::time::Instant;

use futures::stream::TryStreamExt;
use mongodb::Collection;
use mongodb::bson::{self, Document};
use mongodb::options::{AggregateOptions as MongoAggregateOptions, Hint};
use tracing::{debug, info};

use crate::error::{ExecutionError, MongoshError, Result};
use crate::parser::{AggregateOptions, FindOptions, QueryCommand};

use super::context::ExecutionContext;
use super::result::{ExecutionResult, ExecutionStats, ResultData};

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
    ///
    /// # Arguments
    /// * `cmd` - Query command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, cmd: QueryCommand) -> Result<ExecutionResult> {
        let start = Instant::now();

        let result = match cmd {
            QueryCommand::Find {
                collection,
                filter,
                options,
            } => self.execute_find(collection, filter, options).await,

            QueryCommand::FindOne {
                collection,
                filter,
                options,
            } => self.execute_find_one(collection, filter, options).await,

            QueryCommand::CountDocuments { collection, filter } => {
                self.execute_count(collection, Some(filter)).await
            }

            // Write operations (Phase 3)
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
            } => self.execute_aggregate(collection, pipeline, options).await,

            // New command variants - not yet implemented
            QueryCommand::ReplaceOne { .. } => Err(MongoshError::NotImplemented(
                "replaceOne not yet implemented".to_string(),
            )),
            QueryCommand::EstimatedDocumentCount { .. } => Err(MongoshError::NotImplemented(
                "estimatedDocumentCount not yet implemented".to_string(),
            )),
            QueryCommand::FindOneAndDelete { .. } => Err(MongoshError::NotImplemented(
                "findOneAndDelete not yet implemented".to_string(),
            )),
            QueryCommand::FindOneAndUpdate { .. } => Err(MongoshError::NotImplemented(
                "findOneAndUpdate not yet implemented".to_string(),
            )),
            QueryCommand::FindOneAndReplace { .. } => Err(MongoshError::NotImplemented(
                "findOneAndReplace not yet implemented".to_string(),
            )),
            QueryCommand::Distinct { .. } => Err(MongoshError::NotImplemented(
                "distinct not yet implemented".to_string(),
            )),
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

    // ===== Read Operations =====

    /// Execute findOne command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    /// * `options` - Find options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Query result with single document
    async fn execute_find_one(
        &self,
        collection: String,
        filter: Document,
        options: FindOptions,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing findOne on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build find options
        let mut find_options = mongodb::options::FindOptions::default();
        find_options.projection = options.projection;
        find_options.sort = options.sort;
        find_options.limit = Some(1); // FindOne always limits to 1

        // Execute find query
        let mut cursor = coll.find(filter).with_options(find_options).await?;

        // Get first document
        let doc = cursor.try_next().await?;

        match doc {
            Some(document) => Ok(ExecutionResult {
                success: true,
                data: ResultData::Document(document),
                stats: ExecutionStats {
                    execution_time_ms: 0,
                    documents_returned: 1,
                    documents_affected: None,
                },
                error: None,
            }),
            None => Ok(ExecutionResult {
                success: true,
                data: ResultData::None,
                stats: ExecutionStats {
                    execution_time_ms: 0,
                    documents_returned: 0,
                    documents_affected: None,
                },
                error: None,
            }),
        }
    }

    /// Execute find command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    /// * `options` - Find options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Query result
    async fn execute_find(
        &self,
        collection: String,
        filter: Document,
        options: FindOptions,
    ) -> Result<ExecutionResult> {
        info!(
            "Executing find on collection '{}' with filter: {:?}",
            collection, filter
        );

        // This function only handles NEW queries - clear any previous cursor state
        self.context.shared_state.clear_cursor_state();

        // Get database and collection
        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build MongoDB find options
        let mut find_opts = mongodb::options::FindOptions::default();

        // Determine batch size (default 20 like MongoDB shell)
        let batch_size = options.batch_size.unwrap_or(20);
        find_opts.batch_size = Some(batch_size);
        debug!("Applied batch_size: {}", batch_size);

        // Apply user-specified limit if any
        if let Some(limit) = options.limit {
            find_opts.limit = Some(limit);
            debug!("Applied limit: {}", limit);
        }

        // Apply skip if specified
        if let Some(skip) = options.skip {
            find_opts.skip = Some(skip);
            debug!("Applied skip: {}", skip);
        }

        if let Some(ref sort) = options.sort {
            find_opts.sort = Some(sort.clone());
            debug!("Applied sort");
        }

        if let Some(ref projection) = options.projection {
            find_opts.projection = Some(projection.clone());
            debug!("Applied projection");
        }

        // Execute query
        let mut cursor = coll
            .find(filter.clone())
            .with_options(find_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Collect first batch of results
        let mut documents = Vec::new();
        let mut batch_count = 0;

        while batch_count < batch_size as usize {
            match cursor
                .try_next()
                .await
                .map_err(|e| ExecutionError::CursorError(e.to_string()))?
            {
                Some(doc) => {
                    documents.push(doc);
                    batch_count += 1;
                }
                None => break,
            }
        }

        info!("Retrieved {} documents in first batch", batch_count);

        // Get total count ONCE at the beginning (optimization #1)
        // Only if: no limit, reasonable batch size, and we got results
        let total_matched = if options.limit.is_none() && batch_count > 0 && batch_count < 100 {
            match coll.count_documents(filter.clone()).await {
                Ok(count) => {
                    debug!("Total documents matched: {}", count);
                    Some(count as usize)
                }
                Err(e) => {
                    debug!("Failed to get total count: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Determine if there are more documents (optimization #3: improved logic)
        let has_more = self.has_more_documents(
            batch_count,
            batch_size as usize,
            total_matched,
            options.limit.map(|l| l as usize),
        );

        // Create cursor state for pagination if there are more documents
        if has_more {
            let mut cursor_state = crate::repl::CursorState::new(
                collection.clone(),
                filter.clone(),
                options.clone(),
                total_matched,
            );
            // Update with the first batch we just retrieved
            cursor_state.update(batch_count, total_matched);
            self.context
                .shared_state
                .set_cursor_state(Some(cursor_state));
            debug!(
                "Saved cursor state for pagination with {} documents retrieved",
                batch_count
            );
        }

        // Create result with pagination info
        let result_data = if has_more {
            ResultData::DocumentsWithPagination {
                documents,
                has_more: true,
                displayed: batch_count,
                total: total_matched,
            }
        } else {
            ResultData::Documents(documents)
        };

        Ok(ExecutionResult {
            success: true,
            data: result_data,
            stats: ExecutionStats {
                execution_time_ms: 0, // Will be set by caller
                documents_returned: batch_count,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Helper method to determine if there are more documents to fetch
    /// This encapsulates the has_more logic (optimization #3)
    fn has_more_documents(
        &self,
        batch_count: usize,
        batch_size: usize,
        total_matched: Option<usize>,
        limit: Option<usize>,
    ) -> bool {
        // If we got less than a full batch, definitely no more
        if batch_count < batch_size {
            return false;
        }

        // If we know the total, compare with what we retrieved
        if let Some(total) = total_matched {
            return batch_count < total;
        }

        // If there's a limit and we've reached it, no more
        if let Some(lim) = limit {
            if batch_count >= lim {
                return false;
            }
        }

        // We got a full batch and don't know the total - assume there might be more
        true
    }

    /// Execute count operation
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Optional query filter
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Count result or error
    async fn execute_count(
        &self,
        collection: String,
        filter: Option<Document>,
    ) -> Result<ExecutionResult> {
        info!("Executing count on collection '{}'", collection);

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let count = if let Some(f) = filter {
            coll.count_documents(f)
                .await
                .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?
        } else {
            coll.estimated_document_count()
                .await
                .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?
        };

        info!("Count result: {}", count);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Count(count),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(count),
            },
            error: None,
        })
    }

    // ===== Write Operations =====

    /// Execute insertOne command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `document` - Document to insert
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Insert result
    async fn execute_insert_one(
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
    async fn execute_insert_many(
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
    async fn execute_update_one(
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
    async fn execute_update_many(
        &self,
        collection: String,
        filter: Document,
        update: Document,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing updateMany on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let result = coll.update_many(filter, update).await?;

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
    async fn execute_delete_one(
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
    async fn execute_delete_many(
        &self,
        collection: String,
        filter: Document,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing deleteMany on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let result = coll.delete_many(filter).await?;

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

    /// Execute an aggregation pipeline
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `pipeline` - Aggregation pipeline stages
    /// * `options` - Aggregation options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    async fn execute_aggregate(
        &self,
        collection: String,
        pipeline: Vec<Document>,
        options: AggregateOptions,
    ) -> Result<ExecutionResult> {
        info!(
            "Executing aggregate on collection '{}' with {} pipeline stages",
            collection,
            pipeline.len()
        );

        // Get database and collection
        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build MongoDB aggregate options
        let mut agg_opts = MongoAggregateOptions::default();

        if options.allow_disk_use {
            agg_opts.allow_disk_use = Some(true);
            debug!("Applied allow_disk_use: true");
        }

        if let Some(batch_size) = options.batch_size {
            agg_opts.batch_size = Some(batch_size);
            debug!("Applied batch_size: {}", batch_size);
        }

        if let Some(max_time_ms) = options.max_time_ms {
            agg_opts.max_time = Some(std::time::Duration::from_millis(max_time_ms));
            debug!("Applied max_time_ms: {}", max_time_ms);
        }

        if let Some(collation_doc) = options.collation {
            match bson::from_document(collation_doc) {
                Ok(collation) => {
                    agg_opts.collation = Some(collation);
                    debug!("Applied collation");
                }
                Err(e) => {
                    return Err(ExecutionError::InvalidParameters(format!(
                        "Invalid collation: {}",
                        e
                    ))
                    .into());
                }
            }
        }

        if let Some(hint_doc) = options.hint {
            agg_opts.hint = Some(Hint::Keys(hint_doc));
            debug!("Applied hint");
        }

        if let Some(read_concern_doc) = options.read_concern {
            match bson::from_document(read_concern_doc) {
                Ok(read_concern) => {
                    agg_opts.read_concern = Some(read_concern);
                    debug!("Applied read_concern");
                }
                Err(e) => {
                    return Err(ExecutionError::InvalidParameters(format!(
                        "Invalid read concern: {}",
                        e
                    ))
                    .into());
                }
            }
        }

        if let Some(let_vars) = options.let_vars {
            agg_opts.let_vars = Some(let_vars);
            debug!("Applied let_vars");
        }

        // Execute aggregation
        let mut cursor = coll
            .aggregate(pipeline.clone())
            .with_options(agg_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Collect results
        let mut documents = Vec::new();

        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e| ExecutionError::CursorError(e.to_string()))?
        {
            documents.push(doc);
        }

        let count = documents.len();
        info!("Aggregation returned {} documents", count);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Documents(documents),
            stats: ExecutionStats {
                execution_time_ms: 0, // Will be set by caller
                documents_returned: count,
                documents_affected: None,
            },
            error: None,
        })
    }
}
