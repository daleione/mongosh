//! Read operations for query executor
//!
//! This module contains all read operations including:
//! - find, findOne
//! - count, estimatedDocumentCount
//! - distinct

use futures::stream::TryStreamExt;
use mongodb::Collection;
use mongodb::bson::{Bson, Document};
use tracing::{debug, info};

use crate::error::{ExecutionError, Result};
use crate::parser::{FindOptions, QueryMode};

use super::super::export::streaming::FindStreamingQuery;
use super::super::killable::run_killable_command;
use super::super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Read operations implementation
impl super::QueryExecutor {
    /// Execute findOne command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    /// * `options` - Find options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Query result with single document
    pub(super) async fn execute_find_one(
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
    pub(super) async fn execute_find(
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
    pub(super) async fn execute_find_streaming(
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

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;

        // Execute find with killOp support
        let cursor = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let filter = filter.clone();
                let options = options.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    // Build MongoDB find options
                    let mut find_opts = mongodb::options::FindOptions::default();

                    // CRITICAL: Set comment for killOp support
                    find_opts.comment = Some(Bson::String(handle.comment().to_string()));

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

                    Ok(cursor)
                })
            },
        )
        .await?;

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
    pub(super) async fn execute_find_interactive(
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

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;

        // Execute find with killOp support
        let mut cursor = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let filter = filter.clone();
                let options = options.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    // Build MongoDB find options
                    let mut find_opts = mongodb::options::FindOptions::default();

                    // CRITICAL: Set comment for killOp support
                    find_opts.comment = Some(Bson::String(handle.comment().to_string()));

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
                    let cursor = coll
                        .find(filter)
                        .with_options(find_opts)
                        .await
                        .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

                    Ok(cursor)
                })
            },
        )
        .await?;

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
    pub(super) async fn execute_count(
        &self,
        collection: String,
        filter: Option<Document>,
    ) -> Result<ExecutionResult> {
        info!("Executing count on collection '{}'", collection);

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;

        let count = run_killable_command(
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

                    let count = if let Some(f) = filter {
                        use mongodb::options::CountOptions;
                        let mut options = CountOptions::default();
                        // CRITICAL: Set comment for killOp support
                        options.comment = Some(Bson::String(handle.comment().to_string()));

                        coll.count_documents(f)
                            .with_options(options)
                            .await
                            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?
                    } else {
                        use mongodb::options::EstimatedDocumentCountOptions;
                        let mut options = EstimatedDocumentCountOptions::default();
                        // CRITICAL: Set comment for killOp support
                        options.comment = Some(Bson::String(handle.comment().to_string()));

                        coll.estimated_document_count()
                            .with_options(options)
                            .await
                            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?
                    };

                    info!("Count result: {}", count);
                    Ok(count)
                })
            },
        )
        .await?;

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

    /// Execute estimatedDocumentCount command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Count result or error
    pub(super) async fn execute_estimated_document_count(
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

    /// Execute distinct command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `field` - Field to get distinct values for
    /// * `filter` - Optional query filter
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Distinct values result or error
    pub(super) async fn execute_distinct(
        &self,
        collection: String,
        field: String,
        filter: Option<Document>,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing distinct on collection '{}' for field '{}'",
            collection, field
        );

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;

        let values = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let field = field.clone();
                let filter = filter.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    use mongodb::options::DistinctOptions;
                    let mut options = DistinctOptions::default();
                    // CRITICAL: Set comment for killOp support
                    options.comment = Some(Bson::String(handle.comment().to_string()));

                    let filter_doc = filter.unwrap_or_else(|| Document::new());
                    let values = coll
                        .distinct(&field, filter_doc)
                        .with_options(options)
                        .await
                        .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

                    Ok(values)
                })
            },
        )
        .await?;

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
}
