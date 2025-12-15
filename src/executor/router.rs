//! Command router for dispatching commands to executors
//!
//! This module provides the CommandRouter which dispatches parsed commands
//! to the appropriate executor based on command type:
//! - Query commands → QueryExecutor
//! - Admin commands → AdminExecutor
//! - Utility commands → UtilityExecutor

use std::time::Instant;
use tracing::debug;

use crate::error::{MongoshError, Result};
use crate::parser::Command;

use super::admin::AdminExecutor;
use super::context::ExecutionContext;
use super::query::QueryExecutor;
use super::result::{ExecutionResult, ExecutionStats, ResultData};
use super::utility::UtilityExecutor;

/// Command router that dispatches commands to appropriate executors
pub struct CommandRouter {
    /// Execution context
    context: ExecutionContext,
}

impl CommandRouter {
    /// Create a new command router
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Result<Self>` - New router or error
    pub async fn new(context: ExecutionContext) -> Result<Self> {
        Ok(Self { context })
    }

    /// Route command to appropriate executor
    ///
    /// # Arguments
    /// * `command` - Parsed command
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn route(&self, command: Command) -> Result<ExecutionResult> {
        debug!("Routing command: {:?}", command);

        let start = Instant::now();

        let result = match command {
            Command::Query(query_cmd) => {
                let executor = QueryExecutor::new(self.context.clone()).await?;
                executor.execute(query_cmd).await
            }
            Command::Admin(admin_cmd) => {
                let executor = AdminExecutor::new(self.context.clone()).await?;
                executor.execute(admin_cmd).await
            }
            Command::Utility(util_cmd) => {
                let executor = UtilityExecutor::new();
                executor.execute(util_cmd).await
            }
            Command::Help(topic) => self.execute_help(topic).await,
            Command::Exit => Ok(ExecutionResult {
                success: true,
                data: ResultData::Message("Exiting...".to_string()),
                stats: ExecutionStats::default(),
                error: None,
            }),
            _ => Err(MongoshError::NotImplemented(
                "Command type not yet implemented".to_string(),
            )),
        };

        let elapsed = start.elapsed().as_millis() as u64;
        debug!("Command executed in {}ms", elapsed);

        result
    }

    /// Execute help command
    ///
    /// # Arguments
    /// * `topic` - Optional help topic
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Help text
    async fn execute_help(&self, topic: Option<String>) -> Result<ExecutionResult> {
        let help_text = if let Some(t) = topic {
            format!("Help for: {}\n(Not yet implemented)", t)
        } else {
            r#"MongoDB Shell Commands:

Database Operations:
  db.collection.find(filter, projection)     - Find documents
  db.collection.findOne(filter, projection)  - Find one document
  db.collection.insertOne(document)          - Insert one document
  db.collection.insertMany([documents])      - Insert multiple documents
  db.collection.updateOne(filter, update)    - Update one document
  db.collection.updateMany(filter, update)   - Update multiple documents
  db.collection.deleteOne(filter)            - Delete one document
  db.collection.deleteMany(filter)           - Delete multiple documents
  db.collection.countDocuments(filter)       - Count documents

Administrative:
  show dbs                                    - List databases
  show collections                            - List collections
  use <database>                              - Switch database

Utility:
  help                                        - Show this help
  help <command>                              - Show help for specific command
  exit / quit                                 - Exit shell
"#
            .to_string()
        };

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(help_text),
            stats: ExecutionStats::default(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_command_router_help() {
        // This is a placeholder test - would need proper setup with ConnectionManager
        // and SharedState to fully test
    }
}
