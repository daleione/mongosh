//! Aggregate operations for query executor
//!
//! This module contains aggregation pipeline operations including:
//! - aggregate (interactive and streaming modes)

use futures::stream::TryStreamExt;
use mongodb::Collection;
use mongodb::bson::{self, Bson, Document};
use mongodb::options::{AggregateOptions as MongoAggregateOptions, Hint};
use tracing::{debug, info};

use crate::error::{ExecutionError, Result};
use crate::parser::AggregateOptions;
use super::super::export::streaming::AggregateStreamingQuery;
use super::super::killable::run_killable_command;
use super::super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Aggregate operations implementation
impl super::QueryExecutor {
    /// Execute an aggregation pipeline
    pub(super) async fn execute_aggregate(
        &self,
        collection: String,
        pipeline: Vec<Document>,
        options: AggregateOptions,
        mode: crate::parser::QueryMode,
    ) -> Result<ExecutionResult> {
        match mode {
            crate::parser::QueryMode::Interactive { .. } => {
                self.execute_aggregate_interactive(collection, pipeline, options).await
            }
            crate::parser::QueryMode::Streaming { batch_size } => {
                self.execute_aggregate_streaming(collection, pipeline, options, batch_size).await
            }
        }
    }

    /// Execute an aggregation pipeline in interactive mode
    pub(super) async fn execute_aggregate_interactive(
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

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;

        // Execute aggregate with killOp support
        let documents = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let pipeline = pipeline.clone();
                let options = options.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    // Build MongoDB aggregate options
                    let mut agg_opts = MongoAggregateOptions::default();

                    // CRITICAL: Set comment for killOp support
                    agg_opts.comment = Some(Bson::String(handle.comment().to_string()));

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
                        .aggregate(pipeline)
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

                    Ok(documents)
                })
            },
        )
        .await?;

        let count = documents.len();

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

    /// Execute an aggregation pipeline in streaming mode for export
    pub(super) async fn execute_aggregate_streaming(
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

        let client = self.context.get_client().await?;
        let client_id = self.context.get_client_id();
        let cancel_token = self.context.get_cancel_token();
        let db_name = self.context.get_current_database().await;

        // Execute aggregate with killOp support
        let cursor = run_killable_command(
            client,
            client_id,
            cancel_token,
            |client, handle| {
                let db_name = db_name.clone();
                let collection = collection.clone();
                let pipeline = pipeline.clone();
                let options = options.clone();

                Box::pin(async move {
                    let coll: Collection<Document> = client
                        .database(&db_name)
                        .collection(&collection);

                    // Build MongoDB aggregate options
                    let mut agg_opts = MongoAggregateOptions::default();

                    // CRITICAL: Set comment for killOp support
                    agg_opts.comment = Some(Bson::String(handle.comment().to_string()));

                    if options.allow_disk_use {
                        agg_opts.allow_disk_use = Some(true);
                        debug!("Applied allow_disk_use: true");
                    }

                    if let Some(batch_size_opt) = options.batch_size {
                        agg_opts.batch_size = Some(batch_size_opt);
                        debug!("Applied batch_size from options: {}", batch_size_opt);
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

                    // Execute aggregation
                    let cursor = coll
                        .aggregate(pipeline)
                        .with_options(agg_opts)
                        .await
                        .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

                    Ok(cursor)
                })
            },
        )
        .await?;

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
}
