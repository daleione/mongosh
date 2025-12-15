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
use crate::parser::command::{AdminCommand, Command};

/// Parser for shell-specific commands
pub struct ShellCommandParser;

impl ShellCommandParser {
    /// Check if input is a shell command
    pub fn is_shell_command(input: &str) -> bool {
        input.starts_with("show ")
            || input.starts_with("use ")
            || input.starts_with("help")
            || matches!(input, "exit" | "quit")
    }

    /// Parse a shell command
    pub fn parse(input: &str) -> Result<Command> {
        let trimmed = input.trim();

        // Exit commands
        if matches!(trimmed, "exit" | "quit") {
            return Ok(Command::Exit);
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
                .into())
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
        if let Command::Admin(AdminCommand::UseDatabase(name)) = result {
            assert_eq!(name, "mydb");
        } else {
            panic!("Expected UseDatabase command");
        }
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
}
