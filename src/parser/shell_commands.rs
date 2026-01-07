//! Shell command parser
//!
//! This module handles parsing of MongoDB shell-specific commands like:
//! - show dbs, show collections, show users, etc.
//! - use <database>
//! - help [topic]
//! - exit, quit
//!
//! These commands don't use JavaScript syntax, so they're parsed with simple string matching.

use crate::error::{ParseError, Result};
use crate::parser::command::{AdminCommand, Command, ConfigCommand};

/// Parser for shell-specific commands
pub struct ShellCommandParser;

impl ShellCommandParser {
    /// Check if input is a shell command
    pub fn is_shell_command(input: &str) -> bool {
        input.starts_with("show ")
            || input.starts_with("use ")
            || input.starts_with("help")
            || input.starts_with("config")
            || input == "format"
            || input.starts_with("format ")
            || input == "color"
            || input.starts_with("color ")
            || input == "query"
            || input.starts_with("query ")
            || matches!(input, "exit" | "quit" | "it")
    }

    /// Parse a shell command
    pub fn parse(input: &str) -> Result<Command> {
        let trimmed = input.trim();

        // Exit commands
        if matches!(trimmed, "exit" | "quit") {
            return Ok(Command::Exit);
        }

        // Iteration command (for pagination)
        if trimmed == "it" {
            return Ok(Command::Utility(
                crate::parser::command::UtilityCommand::Iterate,
            ));
        }

        // Help command
        if trimmed.starts_with("help") {
            return Self::parse_help(trimmed);
        }

        // Show commands
        if trimmed.starts_with("show ") {
            return Self::parse_show(trimmed);
        }

        // Use command
        if trimmed.starts_with("use ") {
            return Self::parse_use(trimmed);
        }

        // Config commands
        if trimmed.starts_with("config")
            || trimmed.starts_with("format")
            || trimmed.starts_with("color")
        {
            return Self::parse_config(trimmed);
        }

        // Query commands (named queries)
        if trimmed == "query" || trimmed.starts_with("query ") {
            return Self::parse_query(trimmed);
        }

        Err(ParseError::InvalidCommand(format!("Unknown shell command: {}", input)).into())
    }

    /// Parse help command
    fn parse_help(input: &str) -> Result<Command> {
        let topic = input
            .strip_prefix("help")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(String::from);

        Ok(Command::Help(topic))
    }

    /// Parse show command
    fn parse_show(input: &str) -> Result<Command> {
        let rest = input.strip_prefix("show ").unwrap().trim();

        let cmd = match rest {
            "dbs" | "databases" => AdminCommand::ShowDatabases,
            "collections" | "tables" => AdminCommand::ShowCollections,
            "users" => AdminCommand::ShowUsers,
            "roles" => AdminCommand::ShowRoles,
            "profile" => AdminCommand::ShowProfile,
            "logs" => AdminCommand::ShowLogs(None),
            other if other.starts_with("log ") => {
                let log_type = other.strip_prefix("log ").unwrap().trim().to_string();
                AdminCommand::ShowLogs(Some(log_type))
            }
            _ => {
                return Err(ParseError::InvalidCommand(format!(
                    "Unknown show command: show {}",
                    rest
                ))
                .into());
            }
        };

        Ok(Command::Admin(cmd))
    }

    /// Parse use command
    fn parse_use(input: &str) -> Result<Command> {
        let db_name = input.strip_prefix("use ").unwrap().trim();

        if db_name.is_empty() {
            return Err(
                ParseError::InvalidCommand("Database name cannot be empty".to_string()).into(),
            );
        }

        // Validate database name (basic validation)
        if !Self::is_valid_db_name(db_name) {
            return Err(
                ParseError::InvalidCommand(format!("Invalid database name: {}", db_name)).into(),
            );
        }

        Ok(Command::Admin(AdminCommand::UseDatabase(
            db_name.to_string(),
        )))
    }

    /// Parse config commands (format, color, config)
    fn parse_config(input: &str) -> Result<Command> {
        let trimmed = input.trim();

        // Handle "format" command
        if trimmed.starts_with("format") {
            let rest = trimmed.strip_prefix("format").unwrap().trim();
            if rest.is_empty() {
                return Ok(Command::Config(ConfigCommand::GetFormat));
            }
            return Ok(Command::Config(ConfigCommand::SetFormat(rest.to_string())));
        }

        // Handle "color" command
        if trimmed.starts_with("color") {
            let rest = trimmed.strip_prefix("color").unwrap().trim();
            if rest.is_empty() {
                return Ok(Command::Config(ConfigCommand::GetColor));
            }

            let enabled = match rest {
                "on" | "true" | "yes" | "1" => true,
                "off" | "false" | "no" | "0" => false,
                _ => {
                    return Err(ParseError::InvalidCommand(format!(
                        "Invalid color value: '{}'. Use 'on' or 'off'",
                        rest
                    ))
                    .into());
                }
            };
            return Ok(Command::Config(ConfigCommand::SetColor(enabled)));
        }

        // Handle "config" command (show all settings)
        if trimmed == "config" {
            return Ok(Command::Config(ConfigCommand::ShowConfig));
        }

        Err(ParseError::InvalidCommand(format!("Unknown config command: {}", input)).into())
    }

    /// Parse query commands (named queries)
    fn parse_query(input: &str) -> Result<Command> {
        let trimmed = input.trim();
        let rest = trimmed.strip_prefix("query").unwrap().trim();

        // List all queries: "query" or "query list"
        if rest.is_empty() || rest == "list" {
            return Ok(Command::Config(ConfigCommand::ListNamedQueries));
        }

        // Save query: "query save <name> <query>"
        if rest.starts_with("save ") {
            let save_rest = rest.strip_prefix("save ").unwrap().trim();
            let parts: Vec<&str> = save_rest.splitn(2, ' ').collect();

            if parts.len() < 2 {
                return Err(ParseError::InvalidCommand(
                    "Usage: query save <name> <query>".to_string(),
                )
                .into());
            }

            let name = parts[0].to_string();
            let query = parts[1].to_string();

            if name.is_empty() || query.is_empty() {
                return Err(ParseError::InvalidCommand(
                    "Query name and query string cannot be empty".to_string(),
                )
                .into());
            }

            return Ok(Command::Config(ConfigCommand::SaveNamedQuery {
                name,
                query,
            }));
        }

        // Delete query: "query delete <name>"
        if rest.starts_with("delete ") {
            let name = rest.strip_prefix("delete ").unwrap().trim().to_string();

            if name.is_empty() {
                return Err(
                    ParseError::InvalidCommand("Usage: query delete <name>".to_string()).into(),
                );
            }

            return Ok(Command::Config(ConfigCommand::DeleteNamedQuery(name)));
        }

        // Execute query: "query <name> [args...]"
        let parts: Vec<String> = Self::parse_query_args(rest);

        if parts.is_empty() {
            return Err(
                ParseError::InvalidCommand("Query name cannot be empty".to_string()).into(),
            );
        }

        let name = parts[0].clone();
        let args = parts[1..].to_vec();

        Ok(Command::Config(ConfigCommand::ExecuteNamedQuery {
            name,
            args,
        }))
    }

    /// Parse query arguments, respecting quoted strings
    fn parse_query_args(input: &str) -> Vec<String> {
        let mut args = Vec::new();
        let mut current_arg = String::new();
        let mut in_quotes = false;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '"' | '\'' => {
                    in_quotes = !in_quotes;
                }
                ' ' if !in_quotes => {
                    if !current_arg.is_empty() {
                        args.push(current_arg.clone());
                        current_arg.clear();
                    }
                }
                _ => {
                    current_arg.push(ch);
                }
            }
        }

        if !current_arg.is_empty() {
            args.push(current_arg);
        }

        args
    }

    /// Validate database name
    fn is_valid_db_name(name: &str) -> bool {
        // MongoDB database name restrictions:
        // - Cannot be empty
        // - Cannot contain /\. "$*<>:|?
        // - Cannot be longer than 64 characters
        // - Case sensitive (but we don't enforce case restrictions here)

        if name.is_empty() || name.len() > 64 {
            return false;
        }

        // Check for invalid characters
        for ch in name.chars() {
            if matches!(
                ch,
                '/' | '\\' | '.' | ' ' | '"' | '$' | '*' | '<' | '>' | ':' | '|' | '?'
            ) {
                return false;
            }
            // Also check for null character
            if ch == '\0' {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_shell_command() {
        assert!(ShellCommandParser::is_shell_command("show dbs"));
        assert!(ShellCommandParser::is_shell_command("use mydb"));
        assert!(ShellCommandParser::is_shell_command("help"));
        assert!(ShellCommandParser::is_shell_command("exit"));
        assert!(ShellCommandParser::is_shell_command("quit"));
        assert!(ShellCommandParser::is_shell_command("format"));
        assert!(ShellCommandParser::is_shell_command("format json"));
        assert!(ShellCommandParser::is_shell_command("color on"));
        assert!(ShellCommandParser::is_shell_command("config"));
        assert!(!ShellCommandParser::is_shell_command("db.users.find()"));
        assert!(!ShellCommandParser::is_shell_command("print('hello')"));
    }

    #[test]
    fn test_parse_exit() {
        let result = ShellCommandParser::parse("exit").unwrap();
        assert!(matches!(result, Command::Exit));

        let result = ShellCommandParser::parse("quit").unwrap();
        assert!(matches!(result, Command::Exit));
    }

    #[test]
    fn test_parse_help() {
        let result = ShellCommandParser::parse("help").unwrap();
        assert!(matches!(result, Command::Help(None)));

        let result = ShellCommandParser::parse("help find").unwrap();
        if let Command::Help(Some(topic)) = result {
            assert_eq!(topic, "find");
        } else {
            panic!("Expected Help command with topic");
        }
    }

    #[test]
    fn test_parse_show_databases() {
        let result = ShellCommandParser::parse("show dbs").unwrap();
        assert!(matches!(
            result,
            Command::Admin(AdminCommand::ShowDatabases)
        ));

        let result = ShellCommandParser::parse("show databases").unwrap();
        assert!(matches!(
            result,
            Command::Admin(AdminCommand::ShowDatabases)
        ));
    }

    #[test]
    fn test_parse_show_collections() {
        let result = ShellCommandParser::parse("show collections").unwrap();
        assert!(matches!(
            result,
            Command::Admin(AdminCommand::ShowCollections)
        ));

        let result = ShellCommandParser::parse("show tables").unwrap();
        assert!(matches!(
            result,
            Command::Admin(AdminCommand::ShowCollections)
        ));
    }

    #[test]
    fn test_parse_show_users() {
        let result = ShellCommandParser::parse("show users").unwrap();
        assert!(matches!(result, Command::Admin(AdminCommand::ShowUsers)));
    }

    #[test]
    fn test_parse_use_database() {
        let result = ShellCommandParser::parse("use mydb").unwrap();
        if let Command::Admin(AdminCommand::UseDatabase(db)) = result {
            assert_eq!(db, "mydb");
        } else {
            panic!("Expected UseDatabase command");
        }
    }

    #[test]
    fn test_parse_config_format() {
        let result = ShellCommandParser::parse("format").unwrap();
        assert!(matches!(result, Command::Config(ConfigCommand::GetFormat)));

        let result = ShellCommandParser::parse("format json").unwrap();
        if let Command::Config(ConfigCommand::SetFormat(fmt)) = result {
            assert_eq!(fmt, "json");
        } else {
            panic!("Expected SetFormat command");
        }
    }

    #[test]
    fn test_parse_config_color() {
        let result = ShellCommandParser::parse("color").unwrap();
        assert!(matches!(result, Command::Config(ConfigCommand::GetColor)));

        let result = ShellCommandParser::parse("color on").unwrap();
        assert!(matches!(
            result,
            Command::Config(ConfigCommand::SetColor(true))
        ));

        let result = ShellCommandParser::parse("color off").unwrap();
        assert!(matches!(
            result,
            Command::Config(ConfigCommand::SetColor(false))
        ));
    }

    #[test]
    fn test_parse_config_show() {
        let result = ShellCommandParser::parse("config").unwrap();
        assert!(matches!(result, Command::Config(ConfigCommand::ShowConfig)));
    }

    #[test]
    fn test_invalid_db_name() {
        assert!(ShellCommandParser::parse("use my/db").is_err());
        assert!(ShellCommandParser::parse("use my.db").is_err());
        assert!(ShellCommandParser::parse("use my db").is_err());
        assert!(ShellCommandParser::parse("use ").is_err());
    }

    #[test]
    fn test_valid_db_name() {
        assert!(ShellCommandParser::is_valid_db_name("mydb"));
        assert!(ShellCommandParser::is_valid_db_name("my-db"));
        assert!(ShellCommandParser::is_valid_db_name("my_db"));
        assert!(ShellCommandParser::is_valid_db_name("MyDB123"));
        assert!(!ShellCommandParser::is_valid_db_name("my.db"));
        assert!(!ShellCommandParser::is_valid_db_name("my/db"));
        assert!(!ShellCommandParser::is_valid_db_name("my db"));
        assert!(!ShellCommandParser::is_valid_db_name(""));
    }

    #[test]
    fn test_parse_query_list() {
        let result = ShellCommandParser::parse("query").unwrap();
        assert!(matches!(
            result,
            Command::Config(ConfigCommand::ListNamedQueries)
        ));

        let result = ShellCommandParser::parse("query list").unwrap();
        assert!(matches!(
            result,
            Command::Config(ConfigCommand::ListNamedQueries)
        ));
    }

    #[test]
    fn test_parse_query_save() {
        let result = ShellCommandParser::parse("query save simple select * from abc").unwrap();
        if let Command::Config(ConfigCommand::SaveNamedQuery { name, query }) = result {
            assert_eq!(name, "simple");
            assert_eq!(query, "select * from abc");
        } else {
            panic!("Expected SaveNamedQuery command");
        }
    }

    #[test]
    fn test_parse_query_delete() {
        let result = ShellCommandParser::parse("query delete simple").unwrap();
        if let Command::Config(ConfigCommand::DeleteNamedQuery(name)) = result {
            assert_eq!(name, "simple");
        } else {
            panic!("Expected DeleteNamedQuery command");
        }
    }

    #[test]
    fn test_parse_query_execute() {
        let result = ShellCommandParser::parse("query simple").unwrap();
        if let Command::Config(ConfigCommand::ExecuteNamedQuery { name, args }) = result {
            assert_eq!(name, "simple");
            assert_eq!(args.len(), 0);
        } else {
            panic!("Expected ExecuteNamedQuery command");
        }

        let result = ShellCommandParser::parse("query user_by_name John Doe").unwrap();
        if let Command::Config(ConfigCommand::ExecuteNamedQuery { name, args }) = result {
            assert_eq!(name, "user_by_name");
            assert_eq!(args, vec!["John", "Doe"]);
        } else {
            panic!("Expected ExecuteNamedQuery command");
        }
    }

    #[test]
    fn test_parse_query_args_with_quotes() {
        let args = ShellCommandParser::parse_query_args("name \"John Doe\" 42");
        assert_eq!(args, vec!["name", "John Doe", "42"]);

        let args = ShellCommandParser::parse_query_args("'Skelly McDermott' admin");
        assert_eq!(args, vec!["Skelly McDermott", "admin"]);
    }
}
