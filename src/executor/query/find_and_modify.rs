//! FindAndModify operations for query executor
//!
//! This module contains all findAndModify operations including:
//! - findOneAndDelete
//! - findOneAndUpdate
//! - findOneAndReplace
//! - findAndModify

use mongodb::Collection;
use mongodb::bson::Document;
use tracing::{debug, info};

use crate::error::{ExecutionError, Result};
use crate::parser::FindAndModifyOptions;
use super::super::result::{ExecutionResult, ExecutionStats, ResultData};

/// FindAndModify operations implementation
impl super::QueryExecutor {
    /// Execute findOneAndDelete command
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter to match document
    /// * `options` - FindAndModify options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Found document or null
    pub(super) async fn execute_find_one_and_delete(
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
    pub(super) async fn execute_find_one_and_update(
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
    pub(super) async fn execute_find_one_and_replace(
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
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn execute_find_and_modify(
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
