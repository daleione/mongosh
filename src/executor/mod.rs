//! Command execution engine for mongosh
//!
//! This module provides the execution layer that processes parsed commands
//! and performs the corresponding MongoDB operations. It has been refactored
//! into separate sub-modules for better organization:
//!
//! ## Module Structure
//!
//! - `context`: ExecutionContext for managing state and connections
//! - `result`: Result types (ExecutionResult, ResultData, ExecutionStats)
//! - `router`: CommandRouter for dispatching commands to executors
//! - `query`: QueryExecutor for CRUD operations
//! - `admin`: AdminExecutor for administrative commands
//! - `utility`: UtilityExecutor for utility commands
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐
//! │   Command   │
//! └──────┬──────┘
//!        │
//!        ▼
//! ┌─────────────────┐
//! │ CommandRouter   │
//! └────────┬────────┘
//!          │
//!    ┌─────┴─────┬──────────┬─────────┐
//!    ▼           ▼          ▼         ▼
//! ┌──────┐  ┌───────┐  ┌───────┐  ┌────────┐
//! │Query │  │Admin  │  │Utility│  │Help    │
//! │Exec  │  │Exec   │  │Exec   │  │        │
//! └──────┘  └───────┘  └───────┘  └────────┘
//!    │          │          │          │
//!    └──────────┴──────────┴──────────┘
//!                   │
//!                   ▼
//!            ┌──────────────┐
//!            │ExecutionResult│
//!            └──────────────┘
//! ```
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use mongosh::executor::ExecutionContext;
//! use mongosh::connection::ConnectionManager;
//! use mongosh::repl::SharedState;
//! use mongosh::parser::Command;
//!
//! async fn example(conn: ConnectionManager, state: SharedState) {
//!     let context = ExecutionContext::new(conn, state);
//!     let command = Command::Help(None);
//!     let result = context.execute(command).await.unwrap();
//!     println!("{:?}", result);
//! }
//! ```

// Module declarations
mod admin;
mod context;
mod query;
mod result;
mod router;
mod utility;

// Re-export public types
pub use context::ExecutionContext;
pub use result::{ExecutionResult, ResultData};
pub use router::CommandRouter;

// Re-export for convenience
use crate::error::Result;
use crate::parser::Command;

impl ExecutionContext {
    /// Execute a command using the command router
    ///
    /// This is the main entry point for command execution.
    ///
    /// # Arguments
    /// * `command` - Parsed command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, command: Command) -> Result<ExecutionResult> {
        let router = CommandRouter::new(self.clone()).await?;
        router.route(command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_stats_default() {
        let stats = ExecutionStats::default();
        assert_eq!(stats.execution_time_ms, 0);
        assert_eq!(stats.documents_returned, 0);
        assert!(stats.documents_affected.is_none());
    }

    #[test]
    fn test_result_data_variants() {
        let data = ResultData::Message("test".to_string());
        match data {
            ResultData::Message(msg) => assert_eq!(msg, "test"),
            _ => panic!("Expected Message variant"),
        }
    }

    #[test]
    fn test_result_builders() {
        let success = ExecutionResult::success(
            ResultData::Message("OK".to_string()),
            ExecutionStats::default(),
        );
        assert!(success.success);

        let error = ExecutionResult::error("Failed".to_string());
        assert!(!error.success);
        assert_eq!(error.error, Some("Failed".to_string()));
    }
}
