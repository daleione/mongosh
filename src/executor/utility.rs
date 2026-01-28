//! Utility executor for helper commands
//!
//! This module provides the UtilityExecutor which handles utility operations:
//! - Print statements and output
//! - Helper functions
//! - Miscellaneous non-database commands
//! - Cursor iteration (it command)

use crate::error::{MongoshError, Result};
use crate::parser::UtilityCommand;
use tracing::info;

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
        }
    }

    /// Execute iteration command (get next batch from cursor)
    ///
    /// Continues fetching documents from the live cursor stored in shared state.
    /// This eliminates the need for skip() operations and provides optimal performance.
    async fn execute_iterate(&self) -> Result<ExecutionResult> {
        use futures::stream::TryStreamExt;

        // Get mutable access to cursor state
        let mut cursor_guard = self.context.shared_state.get_cursor_mut().await;

        // Check if cursor exists
        let cursor_state = cursor_guard.as_mut().ok_or_else(|| {
            MongoshError::Generic("No active cursor. Please run a query first.".to_string())
        })?;

        // Check if cursor has expired (10 minute timeout)
        if cursor_state.is_expired() {
            // Clear the expired cursor
            *cursor_guard = None;
            drop(cursor_guard);
            self.context.shared_state.clear_cursor().await;

            return Err(MongoshError::Generic(
                "Cursor has expired (10 minute timeout). Please re-run your query.".to_string(),
            )
            .into());
        }

        let batch_size = cursor_state.batch_size;

        // Fetch next batch from the live cursor (no skip needed!)
        let mut documents = Vec::new();
        let mut count = 0;

        while count < batch_size as usize {
            match cursor_state.cursor.try_next().await {
                Ok(Some(doc)) => {
                    documents.push(doc);
                    count += 1;
                }
                Ok(None) => {
                    // Cursor exhausted - no more documents
                    break;
                }
                Err(e) => {
                    // Cursor error - clear state and return error
                    *cursor_guard = None;
                    drop(cursor_guard);
                    self.context.shared_state.clear_cursor().await;

                    return Err(
                        crate::error::ExecutionError::CursorError(e.to_string()).into()
                    );
                }
            }
        }

        info!("Retrieved {} documents from cursor", count);

        // Update documents retrieved count
        cursor_state.update_retrieved(count);

        // Check if there might be more documents
        let has_more = count == batch_size as usize;

        // If no more documents, clear the cursor
        if !has_more {
            let total_retrieved = cursor_state.documents_retrieved();
            *cursor_guard = None;
            drop(cursor_guard);
            self.context.shared_state.clear_cursor().await;

            info!("Cursor exhausted. Total {} documents retrieved.", total_retrieved);
        }

        // Create result
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
                execution_time_ms: 0,
                documents_returned: count,
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
            crate::repl::SharedState::new("test".to_string()),
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
