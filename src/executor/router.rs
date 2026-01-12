//! Command router for dispatching commands to executors
//!
//! This module provides the CommandRouter which dispatches parsed commands
//! to the appropriate executor based on command type:
//! - Query commands → QueryExecutor
//! - Admin commands → AdminExecutor
//! - Utility commands → UtilityExecutor

use std::collections::HashMap;
use std::fs;
use std::time::Instant;
use tabled::{builder::Builder, settings::Style};
use tracing::debug;

use crate::config::{Config, OutputFormat};
use crate::error::Result;
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
  db.collection.replaceOne(filter, doc)      - Replace one document
  db.collection.deleteOne(filter)            - Delete one document
  db.collection.deleteMany(filter)           - Delete multiple documents
  db.collection.findOneAndDelete(filter)     - Find and delete one document
  db.collection.findOneAndUpdate(filter, upd)- Find and update one document
  db.collection.findOneAndReplace(filter, doc)- Find and replace one document
  db.collection.countDocuments(filter)       - Count documents
  db.collection.estimatedDocumentCount()     - Get estimated document count (fast)
  db.collection.distinct(field, filter?)     - Get distinct values for a field

Administrative:
  show dbs                                    - List databases
  show collections                            - List collections
  use <database>                              - Switch database
  db.collection.createIndex(keys, options?)  - Create an index
  db.collection.getIndexes()                 - List indexes
  db.collection.dropIndex(name)              - Drop a single index
  db.collection.dropIndexes()                - Drop all indexes (except _id)
  db.collection.drop()                       - Drop the entire collection

Configuration:
  format [shell|json|json-pretty|table|compact] - Set/get output format
  color [on|off]                                - Enable/disable color output
  config                                        - Show current configuration

Named Queries:
  query                                       - List all named queries
  query <name> [args...]                      - Execute a named query with arguments
  query save <name> <query>                   - Save a new named query
  query delete <name>                         - Delete a named query

  Parameter substitution:
    '$1', '$2'...                             - String parameters (with quotes in template)
    $1, $2...                                 - Numeric/raw parameters (no quotes in template)
    $*                                        - Raw aggregation: 18, 25, 30
    $@                                        - String aggregation: 'admin', 'user'

  Examples:
    query save user "db.users.find({name: '\$1', age: \$2})"
    query user John 25                        -> {name: 'John', age: 25}

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
            ConfigCommand::ListNamedQueries => {
                return self.list_named_query().await;
            }
            ConfigCommand::ExecuteNamedQuery { name, args } => {
                return self.execute_named_query(&name, &args).await;
            }
            ConfigCommand::SaveNamedQuery { name, query } => {
                return self.save_named_query(&name, &query).await;
            }
            ConfigCommand::DeleteNamedQuery(name) => {
                return self.delete_named_query(&name).await;
            }
        };

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(message),
            stats: ExecutionStats::default(),
            error: None,
        })
    }

    /// Load named query from config file
    async fn load_named_query(&self) -> Result<HashMap<String, String>> {
        let config_path = self
            .context
            .config_path
            .as_ref()
            .map(|p| p.clone())
            .unwrap_or_else(|| Config::default_config_path());

        if !config_path.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&config_path).map_err(|e| {
            crate::error::MongoshError::Config(crate::error::ConfigError::Generic(format!(
                "Failed to read config file: {}",
                e
            )))
        })?;

        let config: Config = toml::from_str(&content).map_err(|e| {
            crate::error::MongoshError::Config(crate::error::ConfigError::Generic(format!(
                "Failed to parse config file: {}",
                e
            )))
        })?;

        Ok(config.named_query)
    }

    /// Save config with updated named query
    async fn save_config_with_query(&self, query: HashMap<String, String>) -> Result<()> {
        let config_path = self
            .context
            .config_path
            .as_ref()
            .map(|p| p.clone())
            .unwrap_or_else(|| Config::default_config_path());

        let mut config = if config_path.exists() {
            let content = fs::read_to_string(&config_path).map_err(|e| {
                crate::error::MongoshError::Config(crate::error::ConfigError::Generic(format!(
                    "Failed to read config file: {}",
                    e
                )))
            })?;
            toml::from_str(&content).unwrap_or_else(|_| Config::default())
        } else {
            Config::default()
        };

        config.named_query = query;
        config.save_to_file(Some(&config_path))?;

        Ok(())
    }

    /// List all named query
    async fn list_named_query(&self) -> Result<ExecutionResult> {
        let query = self.load_named_query().await?;

        if query.is_empty() {
            return Ok(ExecutionResult {
                success: true,
                data: ResultData::Message("No named queries defined.".to_string()),
                stats: ExecutionStats::default(),
                error: None,
            });
        }

        // Build table using tabled library
        let mut builder = Builder::default();

        // Add header row
        builder.push_record(vec!["Name", "Query"]);

        // Add data rows
        for (name, q) in query.iter() {
            builder.push_record(vec![name.as_str(), q.as_str()]);
        }

        let mut table = builder.build();
        table.with(Style::ascii());

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(table.to_string()),
            stats: ExecutionStats::default(),
            error: None,
        })
    }

    /// Execute a named query with parameter substitution
    async fn execute_named_query(&self, name: &str, args: &[String]) -> Result<ExecutionResult> {
        let query = self.load_named_query().await?;

        let query_template = query.get(name).ok_or_else(|| {
            crate::error::MongoshError::Config(crate::error::ConfigError::Generic(format!(
                "Named query '{}' not found",
                name
            )))
        })?;

        // Substitute parameters
        let substituted_query = self.substitute_parameters(query_template, args);

        // Parse and execute the query
        let mut parser = crate::parser::Parser::new();
        let command = parser.parse(&substituted_query)?;
        Box::pin(self.route(command)).await
    }

    /// Substitute parameters in query template
    fn substitute_parameters(&self, template: &str, args: &[String]) -> String {
        let mut result = template.to_string();

        // First, handle positional parameters ($1, $2, $3, etc.)
        // We need to be careful about whether the parameter is in quotes or not
        for (i, arg) in args.iter().enumerate() {
            let placeholder = format!("${}", i + 1);
            let quoted_placeholder = format!("'{}'", placeholder);
            let double_quoted_placeholder = format!("\"{}\"", placeholder);

            // If parameter is in quotes, keep it as string (remove the placeholder quotes)
            if result.contains(&quoted_placeholder) {
                result = result.replace(&quoted_placeholder, &format!("'{}'", arg));
            } else if result.contains(&double_quoted_placeholder) {
                result = result.replace(&double_quoted_placeholder, &format!("\"{}\"", arg));
            } else {
                // Not in quotes - use raw value (could be number or unquoted string)
                result = result.replace(&placeholder, arg);
            }
        }

        // Then handle aggregation parameters
        if result.contains("$@") {
            // String aggregation: quote each argument
            let quoted_args: Vec<String> = args.iter().map(|s| format!("'{}'", s)).collect();
            let aggregated = quoted_args.join(", ");
            result = result.replace("$@", &aggregated);
        }

        if result.contains("$*") {
            // Raw aggregation: no quotes (for numeric arrays)
            let aggregated = args.join(", ");
            result = result.replace("$*", &aggregated);
        }

        result
    }

    /// Save a named query
    async fn save_named_query(&self, name: &str, query: &str) -> Result<ExecutionResult> {
        let mut query_map = self.load_named_query().await?;
        query_map.insert(name.to_string(), query.to_string());
        self.save_config_with_query(query_map).await?;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(format!("Named query '{}' saved", name)),
            stats: ExecutionStats::default(),
            error: None,
        })
    }

    /// Delete a named query
    async fn delete_named_query(&self, name: &str) -> Result<ExecutionResult> {
        let mut query = self.load_named_query().await?;

        if query.remove(name).is_none() {
            return Ok(ExecutionResult {
                success: false,
                data: ResultData::Message(format!("Named query '{}' not found", name)),
                stats: ExecutionStats::default(),
                error: Some(format!("Query '{}' does not exist", name)),
            });
        }

        self.save_config_with_query(query).await?;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(format!("{}: Deleted", name)),
            stats: ExecutionStats::default(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_parameters_with_numbers() {
        let router = CommandRouter {
            context: ExecutionContext::new(
                crate::connection::ConnectionManager::new(
                    "mongodb://localhost:27017".to_string(),
                    crate::config::ConnectionConfig::default(),
                ),
                crate::repl::SharedState::new("test".to_string()),
            ),
        };

        // Numeric parameter without quotes
        let template = "db.users.find({age: $1})";
        let result = router.substitute_parameters(template, &["18".to_string()]);
        assert_eq!(result, "db.users.find({age: 18})");

        // Numeric parameter with quotes (should keep quotes)
        let template = "db.users.find({age: '$1'})";
        let result = router.substitute_parameters(template, &["18".to_string()]);
        assert_eq!(result, "db.users.find({age: '18'})");

        // String parameter with quotes
        let template = "db.users.findOne({name: '$1'})";
        let result = router.substitute_parameters(template, &["davin".to_string()]);
        assert_eq!(result, "db.users.findOne({name: 'davin'})");

        // Multiple parameters mixed (string and number)
        let template = "db.users.find({name: '$1', age: $2})";
        let result =
            router.substitute_parameters(template, &["davin".to_string(), "25".to_string()]);
        assert_eq!(result, "db.users.find({name: 'davin', age: 25})");

        // Complex scenario: multiple mixed types
        let template = "db.users.find({name: '$1', age: $2, city: '$3', active: $4})";
        let result = router.substitute_parameters(
            template,
            &[
                "John".to_string(),
                "30".to_string(),
                "New York".to_string(),
                "true".to_string(),
            ],
        );
        assert_eq!(
            result,
            "db.users.find({name: 'John', age: 30, city: 'New York', active: true})"
        );
    }

    #[test]
    fn test_substitute_parameters_with_aggregation() {
        let router = CommandRouter {
            context: ExecutionContext::new(
                crate::connection::ConnectionManager::new(
                    "mongodb://localhost:27017".to_string(),
                    crate::config::ConnectionConfig::default(),
                ),
                crate::repl::SharedState::new("test".to_string()),
            ),
        };

        // Raw aggregation (numeric)
        let template = "db.users.find({age: {$in: [$*]}})";
        let result = router.substitute_parameters(
            template,
            &["18".to_string(), "25".to_string(), "30".to_string()],
        );
        assert_eq!(result, "db.users.find({age: {$in: [18, 25, 30]}})");

        // String aggregation (quoted)
        let template = "db.users.find({category: {$in: [$@]}})";
        let result =
            router.substitute_parameters(template, &["admin".to_string(), "user".to_string()]);
        assert_eq!(
            result,
            "db.users.find({category: {$in: ['admin', 'user']}})"
        );
    }

    #[tokio::test]
    async fn test_command_router_help() {
        // This is a placeholder test - would need proper setup with ConnectionManager
        // and SharedState to fully test
    }
}
