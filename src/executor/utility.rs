//! Utility executor for helper commands
//!
//! This module provides the UtilityExecutor which handles utility operations:
//! - Print statements and output
//! - Helper functions
//! - Miscellaneous non-database commands
//! - Cursor iteration (it command)

use crate::error::{MongoshError, Result};
use crate::parser::UtilityCommand;

use super::context::ExecutionContext;
use super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Executor for utility commands
pub struct UtilityExecutor {
    /// Execution context
    context: ExecutionContext,
}

impl UtilityExecutor {
    /// Create a new utility executor
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Self` - New executor
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Execute a utility command
    ///
    /// # Arguments
    /// * `cmd` - Utility command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, cmd: UtilityCommand) -> Result<ExecutionResult> {
        match cmd {
            UtilityCommand::Print(text) => Ok(ExecutionResult {
                success: true,
                data: ResultData::Message(text),
                stats: ExecutionStats::default(),
                error: None,
            }),
            UtilityCommand::Iterate => self.execute_iterate().await,
            _ => Err(MongoshError::NotImplemented(
                "Utility command not yet implemented".to_string(),
            )),
        }
    }

    /// Execute iteration command (get next batch from cursor)
    ///
    /// This is a SEPARATE operation from execute_find - it handles pagination
    /// by directly querying the database with skip parameter.
    async fn execute_iterate(&self) -> Result<ExecutionResult> {
        use futures::stream::TryStreamExt;
        use mongodb::Collection;
        use mongodb::bson::Document;

        // Check if there's an active cursor
        if !self.context.shared_state.has_active_cursor() {
            return Ok(ExecutionResult {
                success: false,
                data: ResultData::Message("No more documents to iterate".to_string()),
                stats: ExecutionStats::default(),
                error: Some("No active cursor".to_string()),
            });
        }

        // Get and remove cursor state (we'll update it)
        let mut cursor_state: crate::repl::CursorState = self
            .context
            .shared_state
            .get_cursor_state()
            .ok_or_else(|| MongoshError::Generic("Cursor state not found".to_string()))?;

        // Check if there are more documents
        if !cursor_state.has_more() {
            return Ok(ExecutionResult {
                success: false,
                data: ResultData::Message("No more documents".to_string()),
                stats: ExecutionStats::default(),
                error: Some("Cursor has no more documents".to_string()),
            });
        }

        // Get database and collection
        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&cursor_state.collection);

        // Build find options with skip for pagination
        let mut find_opts = mongodb::options::FindOptions::default();

        // Get batch size from cursor state
        let batch_size = cursor_state.options.batch_size.unwrap_or(20);
        find_opts.batch_size = Some(batch_size);

        // Skip the documents we've already retrieved
        find_opts.skip = Some(cursor_state.get_skip());

        // Apply limit if set
        if let Some(limit) = cursor_state.options.limit {
            find_opts.limit = Some(limit);
        }

        // Apply sort and projection from original query
        if let Some(ref sort) = cursor_state.options.sort {
            find_opts.sort = Some(sort.clone());
        }

        if let Some(ref projection) = cursor_state.options.projection {
            find_opts.projection = Some(projection.clone());
        }

        // Execute query to get next batch
        let mut cursor = coll
            .find(cursor_state.filter.clone())
            .with_options(find_opts)
            .await
            .map_err(|e| crate::error::ExecutionError::QueryFailed(e.to_string()))?;

        // Fetch next batch
        let mut documents = Vec::new();
        let mut batch_count = 0;

        while batch_count < batch_size as usize {
            match cursor
                .try_next()
                .await
                .map_err(|e| crate::error::ExecutionError::CursorError(e.to_string()))?
            {
                Some(doc) => {
                    documents.push(doc);
                    batch_count += 1;
                }
                None => break,
            }
        }

        // Update cursor state with new batch
        cursor_state.update(batch_count, cursor_state.total_matched);

        // Determine if there are still more documents
        let has_more = if let Some(total) = cursor_state.total_matched {
            // Use the total we got from the first query
            cursor_state.documents_retrieved < total
        } else {
            // Heuristic: if we got a full batch, there might be more
            batch_count >= batch_size as usize
        };

        cursor_state.has_more = has_more;

        // Save total_matched before moving cursor_state
        let total_matched = cursor_state.total_matched;

        // Save updated cursor state (or clear if no more)
        if has_more {
            self.context
                .shared_state
                .set_cursor_state(Some(cursor_state));
        } else {
            self.context.shared_state.clear_cursor_state();
        }

        // Create result
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
                execution_time_ms: 0,
                documents_returned: batch_count,
                documents_affected: None,
            },
            error: None,
        })
    }
}

impl Default for UtilityExecutor {
    fn default() -> Self {
        // Create a minimal context for testing
        Self::new(ExecutionContext::new(
            crate::connection::ConnectionManager::new(
                "mongodb://localhost:27017".to_string(),
                crate::config::ConnectionConfig::default(),
            ),
            crate::repl::SharedState::new(
                "test".to_string(),
                "mongodb://localhost:27017".to_string(),
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_utility_executor_print() {
        let executor = UtilityExecutor::default();
        let result = executor
            .execute(UtilityCommand::Print("Hello".to_string()))
            .await
            .unwrap();

        assert!(result.success);
        match result.data {
            ResultData::Message(msg) => assert_eq!(msg, "Hello"),
            _ => panic!("Expected Message result"),
        }
    }

    #[test]
    fn test_utility_executor_default() {
        let _executor = UtilityExecutor::default();
    }
}
