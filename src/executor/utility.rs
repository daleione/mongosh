//! Utility executor for helper commands
//!
//! This module provides the UtilityExecutor which handles utility operations:
//! - Print statements and output
//! - Helper functions
//! - Miscellaneous non-database commands

use crate::error::{MongoshError, Result};
use crate::parser::UtilityCommand;

use super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Executor for utility commands
pub struct UtilityExecutor {}

impl UtilityExecutor {
    /// Create a new utility executor
    ///
    /// # Returns
    /// * `Self` - New executor
    pub fn new() -> Self {
        Self {}
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
            _ => Err(MongoshError::NotImplemented(
                "Utility command not yet implemented".to_string(),
            )),
        }
    }
}

impl Default for UtilityExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_utility_executor_print() {
        let executor = UtilityExecutor::new();
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
