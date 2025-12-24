//! Command router for dispatching commands to executors
//!
//! This module provides the CommandRouter which dispatches parsed commands
//! to the appropriate executor based on command type:
//! - Query commands → QueryExecutor
//! - Admin commands → AdminExecutor
//! - Utility commands → UtilityExecutor

use std::time::Instant;
use tracing::debug;

use crate::config::OutputFormat;
use crate::error::{MongoshError, Result};
use crate::parser::{Command, ConfigCommand};

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
                let executor = UtilityExecutor::new(self.context.clone());
                executor.execute(util_cmd).await
            }
            Command::Config(config_cmd) => self.execute_config(config_cmd).await,
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

Configuration:
  format [shell|json|json-pretty|table|compact] - Set/get output format
  color [on|off]                                - Enable/disable color output
  config                                        - Show current configuration

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

    /// Execute config command
    ///
    /// # Arguments
    /// * `cmd` - Config command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Config result
    async fn execute_config(&self, cmd: ConfigCommand) -> Result<ExecutionResult> {
        let shared_state = &self.context.shared_state;

        let message = match cmd {
            ConfigCommand::SetFormat(format_str) => {
                let format = match format_str.to_lowercase().as_str() {
                    "shell" => OutputFormat::Shell,
                    "json" => OutputFormat::Json,
                    "json-pretty" | "jsonpretty" => OutputFormat::JsonPretty,
                    "table" => OutputFormat::Table,
                    "compact" => OutputFormat::Compact,
                    _ => {
                        return Ok(ExecutionResult {
                            success: false,
                            data: ResultData::Message(format!(
                                "Invalid format: '{}'\n\nSupported formats: shell, json, json-pretty, table, compact",
                                format_str
                            )),
                            stats: ExecutionStats::default(),
                            error: Some("Invalid format".to_string()),
                        });
                    }
                };

                shared_state.set_format(format);
                format!("Output format set to: {}", format_str)
            }
            ConfigCommand::GetFormat => {
                let format = shared_state.get_format();
                let format_str = match format {
                    OutputFormat::Shell => "shell",
                    OutputFormat::Json => "json",
                    OutputFormat::JsonPretty => "json-pretty",
                    OutputFormat::Table => "table",
                    OutputFormat::Compact => "compact",
                };
                format!(
                    "Current format: {}\n\nSupported formats: shell, json, json-pretty, table, compact",
                    format_str
                )
            }
            ConfigCommand::SetColor(enabled) => {
                shared_state.set_color_enabled(enabled);
                format!(
                    "Color output {}",
                    if enabled { "enabled" } else { "disabled" }
                )
            }
            ConfigCommand::GetColor => {
                let enabled = shared_state.get_color_enabled();
                format!(
                    "Color output: {}",
                    if enabled { "enabled" } else { "disabled" }
                )
            }
            ConfigCommand::ShowConfig => {
                let format = shared_state.get_format();
                let format_str = match format {
                    OutputFormat::Shell => "shell",
                    OutputFormat::Json => "json",
                    OutputFormat::JsonPretty => "json-pretty",
                    OutputFormat::Table => "table",
                    OutputFormat::Compact => "compact",
                };
                let color = if shared_state.get_color_enabled() {
                    "enabled"
                } else {
                    "disabled"
                };

                format!(
                    r#"Current Configuration:
  format: {}
  color: {}

Available Commands:
  format [shell|json|json-pretty|table|compact]   - Set/get output format
  color [on|off]                                  - Set/get color output
  config                                          - Show this configuration"#,
                    format_str, color
                )
            }
        };

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(message),
            stats: ExecutionStats::default(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_command_router_help() {
        // This is a placeholder test - would need proper setup with ConnectionManager
        // and SharedState to fully test
    }
}
