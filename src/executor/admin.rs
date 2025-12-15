//! Admin executor for administrative commands
//!
//! This module provides the AdminExecutor which handles MongoDB administrative operations:
//! - Database management: show databases, use database
//! - Collection management: show collections
//! - Server commands and diagnostics

use tracing::info;

use crate::error::{ExecutionError, MongoshError, Result};
use crate::parser::AdminCommand;

use super::context::ExecutionContext;
use super::result::{ExecutionResult, ExecutionStats, ResultData};

/// Executor for administrative commands
pub struct AdminExecutor {
    /// Execution context
    context: ExecutionContext,
}

impl AdminExecutor {
    /// Create a new admin executor
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Result<Self>` - New executor or error
    pub async fn new(context: ExecutionContext) -> Result<Self> {
        Ok(Self { context })
    }

    /// Execute an administrative command
    ///
    /// # Arguments
    /// * `cmd` - Admin command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, cmd: AdminCommand) -> Result<ExecutionResult> {
        match cmd {
            AdminCommand::ShowDatabases => self.show_databases().await,
            AdminCommand::ShowCollections => self.show_collections().await,
            AdminCommand::UseDatabase(name) => self.use_database(name).await,
            _ => Err(MongoshError::NotImplemented(
                "Admin command not yet implemented".to_string(),
            )),
        }
    }

    /// Show all databases
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - List of database names
    async fn show_databases(&self) -> Result<ExecutionResult> {
        info!("Listing databases");

        let client = self.context.get_client().await?;

        let db_names = client
            .list_database_names()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!("Found {} databases", db_names.len());

        Ok(ExecutionResult {
            success: true,
            data: ResultData::List(db_names),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Show collections in current database
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - List of collection names
    async fn show_collections(&self) -> Result<ExecutionResult> {
        let db_name = self.context.get_current_database().await;
        info!("Listing collections in database '{}'", db_name);

        let db = self.context.get_database().await?;

        let collection_names = db
            .list_collection_names()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!("Found {} collections", collection_names.len());

        Ok(ExecutionResult {
            success: true,
            data: ResultData::List(collection_names),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Switch to a different database
    ///
    /// # Arguments
    /// * `name` - Database name
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Success message
    async fn use_database(&self, name: String) -> Result<ExecutionResult> {
        info!("Switching to database '{}'", name);

        self.context.set_current_database(name.clone()).await;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(format!("switched to db {}", name)),
            stats: ExecutionStats::default(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_admin_executor_creation() {
        // This is a placeholder test - would need proper setup with ConnectionManager
        // and SharedState to fully test
    }
}
