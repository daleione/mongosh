//! Explain operations for query executor
//!
//! This module contains explain command support for various query types:
//! - find, findOne
//! - aggregate
//! - count
//! - distinct

use mongodb::bson::Document;
use tracing::debug;

use crate::error::{ExecutionError, MongoshError, Result};
use crate::parser::{AggregateOptions, ExplainVerbosity, FindOptions, QueryCommand};
use super::super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Explain operations implementation
impl super::QueryExecutor {
    /// Execute explain command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `verbosity` - Explain verbosity level
    /// * `query` - Query command to explain
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Explain output as document
    pub(super) async fn execute_explain(
        &self,
        collection: String,
        verbosity: ExplainVerbosity,
        query: QueryCommand,
    ) -> Result<ExecutionResult> {
        debug!(
            "Executing explain on collection '{}' with verbosity: {:?}",
            collection,
 verbosity
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
                    "explain() does not support this query type. Supported: find, findOne, aggregate, count, distinct".to_string(),
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
    pub(super) async fn build_find_explain(
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
    pub(super) async fn build_find_one_explain(
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
    pub(super) async fn build_aggregate_explain(
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
    pub(super) async fn build_count_explain(
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
    pub(super) async fn build_distinct_explain(
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
}
