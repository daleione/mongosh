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
use crate::parser::{AggregateOptions, ExplainVerbosity, FindAndModifyOptions, FindOptions, QueryCommand, QueryMode};

use super::confirmation::confirm_query_operation;
use super::context::ExecutionContext;
use super::export::streaming::{AggregateStreamingQuery, FindStreamingQuery};
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
    async fn execute_find(
        &self,
        collection: String,
        filter: Document,
        options: FindOptions,
        mode: QueryMode,
    ) -> Result<ExecutionResult> {
        match mode {
            QueryMode::Interactive { batch_size } => {
                self.execute_find_interactive(collection, filter, options, batch_size).await
            }
            QueryMode::Streaming { batch_size } => {
                self.execute_find_streaming(collection, filter, options, batch_size).await
            }
        }
    }

    /// Execute find in streaming mode for export
    async fn execute_find_streaming(
        &self,
        collection: String,
        filter: Document,
        options: FindOptions,
        batch_size: u32,
    ) -> Result<ExecutionResult> {
        info!(
            "Executing find (streaming) on collection '{}' with filter: {:?}",
            collection, filter
        );

        // Get database and collection
        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build MongoDB find options
        let mut find_opts = mongodb::options::FindOptions::default();

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

        // Execute query and create cursor
        let cursor = coll
            .find(filter)
            .with_options(find_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Create streaming query wrapper
        let streaming_query = FindStreamingQuery::new_find(cursor, batch_size);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Stream(Box::new(streaming_query)),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Execute find in interactive mode with pagination
    async fn execute_find_interactive(
        &self,
        collection: String,
        filter: Document,
        options: FindOptions,
        batch_size: u32,
    ) -> Result<ExecutionResult> {
        info!(
            "Executing find on collection '{}' with filter: {:?}",
            collection, filter
        );

        // Clear any previous cursor state - this is a new query
        self.context.shared_state.clear_cursor().await;

        // Get database and collection
        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build MongoDB find options
        let mut find_opts = mongodb::options::FindOptions::default();

        // Use provided batch size
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

        // Execute query and create cursor
        let mut cursor = coll
            .find(filter)
            .with_options(find_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Fetch first batch of documents
        let mut documents = Vec::new();
        let mut count = 0;

        while count < batch_size as usize {
            match cursor
                .try_next()
                .await
                .map_err(|e| ExecutionError::CursorError(e.to_string()))?
            {
                Some(doc) => {
                    documents.push(doc);
                    count += 1;
                }
                None => break, // No more documents
            }
        }

        info!("Retrieved {} documents in first batch", count);

        // Check if there might be more documents
        // If we got a full batch, there's likely more
        let has_more = count == batch_size as usize;

        // If there are more documents, save the live cursor for pagination
        if has_more {
            let mut cursor_state = crate::repl::CursorState::new(
                collection.clone(),
                cursor, // Store the LIVE cursor
                batch_size,
            );
            cursor_state.update_retrieved(count);

            self.context.shared_state.set_cursor(cursor_state).await;

            debug!(
                "Saved live cursor for pagination with {} documents retrieved",
                count
            );
        }

        // Create result with pagination info
        let result_data = if has_more {
            ResultData::DocumentsWithPagination {
                documents,
                has_more: true,
                displayed: count,
            }
        } else {
            ResultData::Documents(documents)
        };

        Ok(ExecutionResult {
            success: true,
            data: result_data,
            stats: ExecutionStats {
                execution_time_ms: 0, // Will be set by caller
                documents_returned: count,
                documents_affected: None,
            },
            error: None,
        })
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

    /// Execute explain command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `verbosity` - Explain verbosity level
    /// * `query` - Query command to explain
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Explain output as document
    async fn execute_explain(
        &self,
        collection: String,
        verbosity: ExplainVerbosity,
        query: QueryCommand,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing explain on collection '{}' with verbosity: {:?}",
            collection, verbosity
        );

        let db = self.context.get_database().await?;

        // Convert verbosity to MongoDB command string
        let verbosity_str = verbosity.as_str();

        // Build explain command based on query type
        let explain_result = match query {
            // Prevent nested explain
            QueryCommand::Explain { .. } => {
                return Err(MongoshError::Execution(ExecutionError::InvalidOperation(
                    "Cannot nest explain() commands".to_string(),
                )));
            }

            QueryCommand::Find { filter, options, .. } => {
                self.build_find_explain(&collection, filter, options, verbosity_str, &db).await?
            }

            QueryCommand::FindOne { filter, options, .. } => {
                self.build_find_one_explain(&collection, filter, options, verbosity_str, &db).await?
            }

            QueryCommand::Aggregate { pipeline, options, .. } => {
                self.build_aggregate_explain(&collection, pipeline, options, verbosity_str, &db).await?
            }

            QueryCommand::CountDocuments { filter, .. } => {
                self.build_count_explain(&collection, filter, verbosity_str, &db).await?
            }

            QueryCommand::Distinct { field, filter, .. } => {
                self.build_distinct_explain(&collection, field, filter, verbosity_str, &db).await?
            }

            _ => {
                return Err(MongoshError::Execution(ExecutionError::InvalidOperation(
                    format!("explain() does not support this query type. Supported: find, findOne, aggregate, count, distinct"),
                )));
            }
        };

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Document(explain_result),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 1,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Build explain for find command with all options
    async fn build_find_explain(
        &self,
        collection: &str,
        filter: Document,
        options: FindOptions,
        verbosity: &str,
        db: &mongodb::Database,
    ) -> Result<Document> {
        let mut find_cmd = Document::new();
        find_cmd.insert("find", collection);
        find_cmd.insert("filter", filter);

        if let Some(projection) = options.projection {
            find_cmd.insert("projection", projection);
        }
        if let Some(sort) = options.sort {
            find_cmd.insert("sort", sort);
        }
        if let Some(limit) = options.limit {
            find_cmd.insert("limit", limit);
        }
        if let Some(skip) = options.skip {
            // Check for overflow when converting u64 to i64
            let skip_i64 = i64::try_from(skip).map_err(|_| {
                MongoshError::Execution(ExecutionError::InvalidOperation(
                    format!("skip value {} is too large (max: {})", skip, i64::MAX),
                ))
            })?;
            find_cmd.insert("skip", skip_i64);
        }
        if let Some(batch_size) = options.batch_size {
            // Check for overflow when converting u32 to i32
            let batch_size_i32 = i32::try_from(batch_size).map_err(|_| {
                MongoshError::Execution(ExecutionError::InvalidOperation(
                    format!("batchSize value {} is too large (max: {})", batch_size, i32::MAX),
                ))
            })?;
            find_cmd.insert("batchSize", batch_size_i32);
        }
        if let Some(hint) = options.hint {
            find_cmd.insert("hint", hint);
        }
        if let Some(max_time_ms) = options.max_time_ms {
            let max_time_i64 = i64::try_from(max_time_ms).map_err(|_| {
                MongoshError::Execution(ExecutionError::InvalidOperation(
                    format!("maxTimeMS value {} is too large (max: {})", max_time_ms, i64::MAX),
                ))
            })?;
            find_cmd.insert("maxTimeMS", max_time_i64);
        }
        if let Some(collation) = options.collation {
            find_cmd.insert("collation", collation);
        }

        let mut explain_cmd = Document::new();
        explain_cmd.insert("explain", find_cmd);
        explain_cmd.insert("verbosity", verbosity);

        Ok(db.run_command(explain_cmd).await?)
    }

    /// Build explain for findOne command
    async fn build_find_one_explain(
        &self,
        collection: &str,
        filter: Document,
        options: FindOptions,
        verbosity: &str,
        db: &mongodb::Database,
    ) -> Result<Document> {
        let mut find_cmd = Document::new();
        find_cmd.insert("find", collection);
        find_cmd.insert("filter", filter);
        find_cmd.insert("limit", 1);

        if let Some(projection) = options.projection {
            find_cmd.insert("projection", projection);
        }
        if let Some(sort) = options.sort {
            find_cmd.insert("sort", sort);
        }
        if let Some(hint) = options.hint {
            find_cmd.insert("hint", hint);
        }
        if let Some(collation) = options.collation {
            find_cmd.insert("collation", collation);
        }

        let mut explain_cmd = Document::new();
        explain_cmd.insert("explain", find_cmd);
        explain_cmd.insert("verbosity", verbosity);

        Ok(db.run_command(explain_cmd).await?)
    }

    /// Build explain for aggregate command
    async fn build_aggregate_explain(
        &self,
        collection: &str,
        pipeline: Vec<Document>,
        options: AggregateOptions,
        verbosity: &str,
        db: &mongodb::Database,
    ) -> Result<Document> {
        let mut agg_cmd = Document::new();
        agg_cmd.insert("aggregate", collection);
        agg_cmd.insert("pipeline", pipeline);
        agg_cmd.insert("cursor", Document::new());

        if let Some(batch_size) = options.batch_size {
            let batch_size_i32 = i32::try_from(batch_size).map_err(|_| {
                MongoshError::Execution(ExecutionError::InvalidOperation(
                    format!("batchSize value {} is too large (max: {})", batch_size, i32::MAX),
                ))
            })?;
            let mut cursor_doc = Document::new();
            cursor_doc.insert("batchSize", batch_size_i32);
            agg_cmd.insert("cursor", cursor_doc);
        }
        if let Some(max_time_ms) = options.max_time_ms {
            let max_time_i64 = i64::try_from(max_time_ms).map_err(|_| {
                MongoshError::Execution(ExecutionError::InvalidOperation(
                    format!("maxTimeMS value {} is too large (max: {})", max_time_ms, i64::MAX),
                ))
            })?;
            agg_cmd.insert("maxTimeMS", max_time_i64);
        }

        let mut explain_cmd = Document::new();
        explain_cmd.insert("explain", agg_cmd);
        explain_cmd.insert("verbosity", verbosity);

        Ok(db.run_command(explain_cmd).await?)
    }

    /// Build explain for count command
    async fn build_count_explain(
        &self,
        collection: &str,
        filter: Document,
        verbosity: &str,
        db: &mongodb::Database,
    ) -> Result<Document> {
        // Count uses aggregate with $match and $count stages
        let mut pipeline = Vec::new();
        if !filter.is_empty() {
            let mut match_stage = Document::new();
            match_stage.insert("$match", filter);
            pipeline.push(match_stage);
        }
        let mut count_stage = Document::new();
        count_stage.insert("$count", "count");
        pipeline.push(count_stage);

        let mut agg_cmd = Document::new();
        agg_cmd.insert("aggregate", collection);
        agg_cmd.insert("pipeline", pipeline);
        agg_cmd.insert("cursor", Document::new());

        let mut explain_cmd = Document::new();
        explain_cmd.insert("explain", agg_cmd);
        explain_cmd.insert("verbosity", verbosity);

        Ok(db.run_command(explain_cmd).await?)
    }

    /// Build explain for distinct command
    async fn build_distinct_explain(
        &self,
        collection: &str,
        field: String,
        filter: Option<Document>,
        verbosity: &str,
        db: &mongodb::Database,
    ) -> Result<Document> {
        let mut distinct_cmd = Document::new();
        distinct_cmd.insert("distinct", collection);
        distinct_cmd.insert("key", field);
        if let Some(filter_doc) = filter {
            distinct_cmd.insert("query", filter_doc);
        }

        let mut explain_cmd = Document::new();
        explain_cmd.insert("explain", distinct_cmd);
        explain_cmd.insert("verbosity", verbosity);

        Ok(db.run_command(explain_cmd).await?)
    }

    /// Execute an aggregation pipeline
    async fn execute_aggregate(
        &self,
        collection: String,
        pipeline: Vec<Document>,
        options: AggregateOptions,
        mode: QueryMode,
    ) -> Result<ExecutionResult> {
        match mode {
            QueryMode::Interactive { .. } => {
                self.execute_aggregate_interactive(collection, pipeline, options).await
            }
            QueryMode::Streaming { batch_size } => {
                self.execute_aggregate_streaming(collection, pipeline, options, batch_size).await
            }
        }
    }

    /// Execute an aggregation pipeline in interactive mode
    async fn execute_aggregate_interactive(
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

    /// Execute estimatedDocumentCount command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Count result or error
    async fn execute_estimated_document_count(
        &self,
        collection: String,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing estimatedDocumentCount on collection '{}'",
            collection
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let count = coll
            .estimated_document_count()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!("Estimated document count: {}", count);

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

    /// Execute an aggregation pipeline in streaming mode for export
    async fn execute_aggregate_streaming(
        &self,
        collection: String,
        pipeline: Vec<Document>,
        options: AggregateOptions,
        batch_size: u32,
    ) -> Result<ExecutionResult> {
        info!(
            "Executing aggregate (streaming) on collection '{}' with {} pipeline stages",
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
        }

        if let Some(batch_size_opt) = options.batch_size {
            agg_opts.batch_size = Some(batch_size_opt);
        }

        if let Some(max_time) = options.max_time_ms {
            agg_opts.max_time = Some(std::time::Duration::from_millis(max_time));
        }

        if let Some(collation_doc) = options.collation {
            match bson::from_document(collation_doc) {
                Ok(collation) => {
                    agg_opts.collation = Some(collation);
                    debug!("Applied collation");
                }
                Err(e) => {
                    return Err(ExecutionError::QueryFailed(format!(
                        "Invalid collation: {}",
                        e
                    ))
                    .into());
                }
            }
        }

        if let Some(ref hint_doc) = options.hint {
            agg_opts.hint = Some(Hint::Keys(hint_doc.clone()));
        }

        // Execute aggregation
        let cursor = coll
            .aggregate(pipeline)
            .with_options(agg_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Create streaming query wrapper
        let streaming_query = AggregateStreamingQuery::new_aggregate(cursor, batch_size);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Stream(Box::new(streaming_query)),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Execute distinct command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `field` - Field to get distinct values for
    /// * `filter` - Optional query filter
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Distinct values result or error
    async fn execute_distinct(
        &self,
        collection: String,
        field: String,
        filter: Option<Document>,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing distinct on collection '{}' for field '{}'",
            collection, field
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let filter_doc = filter.unwrap_or_else(|| Document::new());
        let values = coll
            .distinct(&field, filter_doc)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Convert Bson values to Documents for display
        let count = values.len();
        let docs: Vec<Document> = values
            .into_iter()
            .map(|v| {
                let mut doc = Document::new();
                doc.insert("value", v);
                doc
            })
            .collect();

        info!("Distinct returned {} unique values", count);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Documents(docs),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: count,
                documents_affected: None,
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
    async fn execute_replace_one(
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

    /// Execute findOneAndDelete command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter to match document
    /// * `options` - FindAndModify options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Found document or null
    async fn execute_find_one_and_delete(
        &self,
        collection: String,
        filter: Document,
        options: FindAndModifyOptions,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing findOneAndDelete on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build options
        let mut find_opts = mongodb::options::FindOneAndDeleteOptions::default();

        if let Some(sort) = options.sort {
            find_opts.sort = Some(sort);
        }

        if let Some(projection) = options.projection {
            find_opts.projection = Some(projection);
        }

        if let Some(max_time_ms) = options.max_time_ms {
            find_opts.max_time = Some(std::time::Duration::from_millis(max_time_ms));
        }

        let result = coll
            .find_one_and_delete(filter)
            .with_options(find_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        match result {
            Some(doc) => {
                info!("FindOneAndDelete found and deleted document");
                Ok(ExecutionResult {
                    success: true,
                    data: ResultData::Document(doc),
                    stats: ExecutionStats {
                        execution_time_ms: 0,
                        documents_returned: 1,
                        documents_affected: Some(1),
                    },
                    error: None,
                })
            }
            None => {
                info!("FindOneAndDelete found no matching document");
                Ok(ExecutionResult {
                    success: true,
                    data: ResultData::Message("No document found".to_string()),
                    stats: ExecutionStats {
                        execution_time_ms: 0,
                        documents_returned: 0,
                        documents_affected: Some(0),
                    },
                    error: None,
                })
            }
        }
    }

    /// Execute findOneAndUpdate command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter to match document
    /// * `update` - Update operations
    /// * `options` - FindAndModify options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Found document (before or after update)
    async fn execute_find_one_and_update(
        &self,
        collection: String,
        filter: Document,
        update: Document,
        options: FindAndModifyOptions,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing findOneAndUpdate on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build options
        let mut find_opts = mongodb::options::FindOneAndUpdateOptions::default();

        if options.return_new {
            find_opts.return_document = Some(mongodb::options::ReturnDocument::After);
        }

        if options.upsert {
            find_opts.upsert = Some(true);
        }

        if let Some(sort) = options.sort {
            find_opts.sort = Some(sort);
        }

        if let Some(projection) = options.projection {
            find_opts.projection = Some(projection);
        }

        if let Some(array_filters) = options.array_filters {
            find_opts.array_filters = Some(array_filters);
        }

        if let Some(max_time_ms) = options.max_time_ms {
            find_opts.max_time = Some(std::time::Duration::from_millis(max_time_ms));
        }

        let result = coll
            .find_one_and_update(filter, update)
            .with_options(find_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        match result {
            Some(doc) => {
                info!("FindOneAndUpdate found and updated document");
                Ok(ExecutionResult {
                    success: true,
                    data: ResultData::Document(doc),
                    stats: ExecutionStats {
                        execution_time_ms: 0,
                        documents_returned: 1,
                        documents_affected: Some(1),
                    },
                    error: None,
                })
            }
            None => {
                info!("FindOneAndUpdate found no matching document");
                Ok(ExecutionResult {
                    success: true,
                    data: ResultData::Message("No document found".to_string()),
                    stats: ExecutionStats {
                        execution_time_ms: 0,
                        documents_returned: 0,
                        documents_affected: Some(0),
                    },
                    error: None,
                })
            }
        }
    }

    /// Execute findOneAndReplace command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter to match document
    /// * `replacement` - Replacement document
    /// * `options` - FindAndModify options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Found document (before or after replacement)
    async fn execute_find_one_and_replace(
        &self,
        collection: String,
        filter: Document,
        replacement: Document,
        options: FindAndModifyOptions,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing findOneAndReplace on collection '{}' with filter: {:?}",
            collection, filter
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build options
        let mut find_opts = mongodb::options::FindOneAndReplaceOptions::default();

        if options.return_new {
            find_opts.return_document = Some(mongodb::options::ReturnDocument::After);
        }

        if options.upsert {
            find_opts.upsert = Some(true);
        }

        if let Some(sort) = options.sort {
            find_opts.sort = Some(sort);
        }

        if let Some(projection) = options.projection {
            find_opts.projection = Some(projection);
        }

        if let Some(max_time_ms) = options.max_time_ms {
            find_opts.max_time = Some(std::time::Duration::from_millis(max_time_ms));
        }

        let result = coll
            .find_one_and_replace(filter, replacement)
            .with_options(find_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        match result {
            Some(doc) => {
                info!("FindOneAndReplace found and replaced document");
                Ok(ExecutionResult {
                    success: true,
                    data: ResultData::Document(doc),
                    stats: ExecutionStats {
                        execution_time_ms: 0,
                        documents_returned: 1,
                        documents_affected: Some(1),
                    },
                    error: None,
                })
            }
            None => {
                info!("FindOneAndReplace found no matching document");
                Ok(ExecutionResult {
                    success: true,
                    data: ResultData::Message("No document found".to_string()),
                    stats: ExecutionStats {
                        execution_time_ms: 0,
                        documents_returned: 0,
                        documents_affected: Some(0),
                    },
                    error: None,
                })
            }
        }
    }

    /// Execute findAndModify command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `query` - Query filter
    /// * `sort` - Sort specification
    /// * `remove` - Whether to remove the document
    /// * `update` - Update specification
    /// * `new` - Return updated document instead of original
    /// * `fields` - Projection specification
    /// * `upsert` - Create document if not found
    /// * `array_filters` - Array filters for updates
    /// * `max_time_ms` - Maximum execution time
    /// * `collation` - Collation specification
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result with the document
    async fn execute_find_and_modify(
        &self,
        collection: String,
        query: Document,
        sort: Option<Document>,
        remove: bool,
        update: Option<Document>,
        new: bool,
        fields: Option<Document>,
        upsert: bool,
        array_filters: Option<Vec<Document>>,
        max_time_ms: Option<u64>,
        collation: Option<Document>,
    ) -> Result<ExecutionResult> {
        use tracing::{debug, info};

        debug!(
            "Executing findAndModify on collection '{}' (remove: {}, new: {})",
            collection, remove, new
        );

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        if remove {
            // Delete operation
            let mut find_opts = mongodb::options::FindOneAndDeleteOptions::default();

            if let Some(s) = sort {
                find_opts.sort = Some(s);
            }
            if let Some(proj) = fields {
                find_opts.projection = Some(proj);
            }
            if let Some(max_time) = max_time_ms {
                find_opts.max_time = Some(std::time::Duration::from_millis(max_time));
            }
            if let Some(_coll_spec) = collation {
                // Note: Collation conversion from BSON document is not directly supported
                // Users should use the driver's Collation builder instead
                // For now, we skip this option
            }

            let result = coll
                .find_one_and_delete(query)
                .with_options(find_opts)
                .await
                .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

            match result {
                Some(doc) => {
                    info!("FindAndModify removed document");
                    Ok(ExecutionResult {
                        success: true,
                        data: ResultData::Document(doc),
                        stats: ExecutionStats {
                            execution_time_ms: 0,
                            documents_returned: 1,
                            documents_affected: Some(1),
                        },
                        error: None,
                    })
                }
                None => {
                    info!("FindAndModify found no matching document to remove");
                    Ok(ExecutionResult {
                        success: true,
                        data: ResultData::Message("null".to_string()),
                        stats: ExecutionStats {
                            execution_time_ms: 0,
                            documents_returned: 0,
                            documents_affected: Some(0),
                        },
                        error: None,
                    })
                }
            }
        } else if let Some(update_doc) = update {
            // Update operation
            let mut find_opts = mongodb::options::FindOneAndUpdateOptions::default();

            if let Some(s) = sort {
                find_opts.sort = Some(s);
            }
            if let Some(proj) = fields {
                find_opts.projection = Some(proj);
            }
            if upsert {
                find_opts.upsert = Some(true);
            }
            if new {
                find_opts.return_document = Some(mongodb::options::ReturnDocument::After);
            }
            if let Some(filters) = array_filters {
                find_opts.array_filters = Some(filters);
            }
            if let Some(max_time) = max_time_ms {
                find_opts.max_time = Some(std::time::Duration::from_millis(max_time));
            }
            if let Some(_coll_spec) = collation {
                // Note: Collation conversion from BSON document is not directly supported
                // Users should use the driver's Collation builder instead
                // For now, we skip this option
            }

            let result = coll
                .find_one_and_update(query, update_doc)
                .with_options(find_opts)
                .await
                .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

            match result {
                Some(doc) => {
                    info!("FindAndModify updated document");
                    Ok(ExecutionResult {
                        success: true,
                        data: ResultData::Document(doc),
                        stats: ExecutionStats {
                            execution_time_ms: 0,
                            documents_returned: 1,
                            documents_affected: Some(1),
                        },
                        error: None,
                    })
                }
                None => {
                    info!("FindAndModify found no matching document to update");
                    Ok(ExecutionResult {
                        success: true,
                        data: ResultData::Message("null".to_string()),
                        stats: ExecutionStats {
                            execution_time_ms: 0,
                            documents_returned: 0,
                            documents_affected: Some(0),
                        },
                        error: None,
                    })
                }
            }
        } else {
            Err(ExecutionError::QueryFailed(
                "findAndModify requires either remove or update".to_string(),
            ).into())
        }
    }
}
