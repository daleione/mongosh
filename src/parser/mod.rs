//! Command and query parser for mongosh
//!
//! This module provides a comprehensive parsing system for MongoDB shell commands
//! using Oxc AST parser for JavaScript syntax and simple string matching for
//! shell-specific commands.
//!
//! # Architecture
//!
//! The parser is split into multiple focused modules:
//! - `command`: Command type definitions (Command, QueryCommand, AdminCommand, etc.)
//! - `ast_parser`: Main AST-based parser orchestrator
//! - `db_operation`: Parser for db.collection.operation() syntax
//! - `expr_converter`: JavaScript expression to BSON converter
//! - `shell_commands`: Parser for shell commands (show, use, help, etc.)
//!
//! # Examples
//!
//! ```no_run
//! use mongosh::parser::Parser;
//!
//! let mut parser = Parser::new();
//!
//! // Parse a find query
//! let cmd = parser.parse("db.users.find({ age: { $gt: 18 } })").unwrap();
//!
//! // Parse a shell command
//! let cmd = parser.parse("show dbs").unwrap();
//!
//! // Parse an aggregation
//! let cmd = parser.parse("db.logs.aggregate([{ $match: {} }])").unwrap();
//! ```

mod command;
mod db_operation;
mod expr_converter;
mod shell_commands;
mod sql_context;
mod sql_expr;
mod sql_lexer;
mod sql_parser;

// Re-export public API
pub use command::*;

use crate::error::{ParseError, Result};

/// Main parser for mongosh commands
///
/// This parser handles all types of MongoDB shell commands including:
/// - Database operations (CRUD, aggregation, etc.)
/// - Administrative commands (show, use, create, drop, etc.)
/// - Utility commands (print, help, etc.)
/// - Script execution
pub struct Parser {}

impl Parser {
    /// Create a new parser instance
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mongosh::parser::Parser;
    ///
    /// let parser = Parser::new();
    /// ```
    pub fn new() -> Self {
        Self {}
    }

    /// Parse an input string into a Command
    ///
    /// This is the main entry point for parsing. It automatically detects
    /// the type of command and routes to the appropriate parser.
    ///
    /// # Arguments
    ///
    /// * `input` - The input string to parse
    ///
    /// # Returns
    ///
    /// * `Result<Command>` - The parsed command or an error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mongosh::parser::Parser;
    ///
    /// let mut parser = Parser::new();
    ///
    /// // Parse a query
    /// let cmd = parser.parse("db.users.find({ name: 'Alice' })").unwrap();
    ///
    /// // Parse a shell command
    /// let cmd = parser.parse("show collections").unwrap();
    /// ```
    pub fn parse(&mut self, input: &str) -> Result<Command> {
        // Trim whitespace and trailing semicolons
        let trimmed = input.trim().trim_end_matches(';').trim();

        // Handle empty input
        if trimmed.is_empty() {
            return Err(ParseError::InvalidCommand("Empty input".to_string()).into());
        }

        // Check if it's a SQL SELECT command
        if sql_parser::SqlParser::is_sql_command(trimmed) {
            return sql_parser::SqlParser::parse_to_command(trimmed);
        }

        // Check if it's a shell command (show, use, help, exit, quit)
        if shell_commands::ShellCommandParser::is_shell_command(trimmed) {
            return shell_commands::ShellCommandParser::parse(trimmed);
        }

        // Check if it's a database operation (db.collection.operation)
        if trimmed.starts_with("db.") {
            return db_operation::DbOperationParser::parse(trimmed);
        }

        // If nothing matches, it's an invalid command
        Err(ParseError::InvalidCommand(trimmed.to_string()).into())
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let _parser = Parser::new();
        // Parser created successfully
    }

    #[test]
    fn test_parse_exit() {
        let mut parser = Parser::new();
        let cmd = parser.parse("exit").unwrap();
        assert!(matches!(cmd, Command::Exit));

        let cmd = parser.parse("quit").unwrap();
        assert!(matches!(cmd, Command::Exit));
    }

    #[test]
    fn test_parse_help() {
        let mut parser = Parser::new();
        let cmd = parser.parse("help").unwrap();
        assert!(matches!(cmd, Command::Help(None)));

        let cmd = parser.parse("help find").unwrap();
        assert!(matches!(cmd, Command::Help(Some(_))));
    }

    #[test]
    fn test_parse_show_databases() {
        let mut parser = Parser::new();
        let cmd = parser.parse("show dbs").unwrap();
        assert!(matches!(cmd, Command::Admin(AdminCommand::ShowDatabases)));

        let cmd = parser.parse("show databases").unwrap();
        assert!(matches!(cmd, Command::Admin(AdminCommand::ShowDatabases)));
    }

    #[test]
    fn test_parse_show_collections() {
        let mut parser = Parser::new();
        let cmd = parser.parse("show collections").unwrap();
        assert!(matches!(cmd, Command::Admin(AdminCommand::ShowCollections)));
    }

    #[test]
    fn test_parse_use_database() {
        let mut parser = Parser::new();
        let cmd = parser.parse("use mydb").unwrap();
        if let Command::Admin(AdminCommand::UseDatabase(name)) = cmd {
            assert_eq!(name, "mydb");
        } else {
            panic!("Expected UseDatabase command");
        }
    }

    #[test]
    fn test_parse_find_empty() {
        let mut parser = Parser::new();
        let cmd = parser.parse("db.users.find()").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert!(filter.is_empty());
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_find_with_filter() {
        let mut parser = Parser::new();
        let cmd = parser.parse("db.users.find({ age: 25 })").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_i64("age").unwrap(), 25);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_find_with_operators() {
        let mut parser = Parser::new();
        let cmd = parser.parse("db.users.find({ age: { $gt: 18 } })").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            let age_cond = filter.get_document("age").unwrap();
            assert_eq!(age_cond.get_i64("$gt").unwrap(), 18);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_insert_one() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.users.insertOne({ name: 'Alice', age: 30 })")
            .unwrap();
        if let Command::Query(QueryCommand::InsertOne {
            collection,
            document,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(document.get_str("name").unwrap(), "Alice");
            assert_eq!(document.get_i64("age").unwrap(), 30);
        } else {
            panic!("Expected InsertOne command");
        }
    }

    #[test]
    fn test_parse_insert_many() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.users.insertMany([{ name: 'Alice' }, { name: 'Bob' }])")
            .unwrap();
        if let Command::Query(QueryCommand::InsertMany {
            collection,
            documents,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(documents.len(), 2);
        } else {
            panic!("Expected InsertMany command");
        }
    }

    #[test]
    fn test_parse_update_one() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.users.updateOne({ name: 'Alice' }, { $set: { age: 31 } })")
            .unwrap();
        if let Command::Query(QueryCommand::UpdateOne {
            collection,
            filter,
            update,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
            let set_doc = update.get_document("$set").unwrap();
            assert_eq!(set_doc.get_i64("age").unwrap(), 31);
        } else {
            panic!("Expected UpdateOne command");
        }
    }

    #[test]
    fn test_parse_delete_one() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.users.deleteOne({ name: 'Alice' })")
            .unwrap();
        if let Command::Query(QueryCommand::DeleteOne {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
        } else {
            panic!("Expected DeleteOne command");
        }
    }

    #[test]
    fn test_parse_aggregate() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.users.aggregate([{ $match: { age: { $gt: 18 } } }])")
            .unwrap();
        if let Command::Query(QueryCommand::Aggregate {
            collection,
            pipeline,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(pipeline.len(), 1);
        } else {
            panic!("Expected Aggregate command");
        }
    }

    #[test]
    fn test_parse_empty_input() {
        let mut parser = Parser::new();
        assert!(parser.parse("").is_err());
        assert!(parser.parse("   ").is_err());
        assert!(parser.parse(";;;").is_err());
    }

    #[test]
    fn test_parse_invalid_command() {
        let mut parser = Parser::new();
        assert!(parser.parse("invalid command").is_err());
        assert!(parser.parse("db.users.invalidOp()").is_err());
    }

    #[test]
    fn test_parse_with_semicolon() {
        let mut parser = Parser::new();
        let cmd = parser.parse("db.users.find();").unwrap();
        assert!(matches!(cmd, Command::Query(QueryCommand::Find { .. })));
    }

    #[test]
    fn test_parse_chained_limit() {
        let mut parser = Parser::new();
        let cmd = parser.parse("db.users.find().limit(1)").unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = cmd {
            assert_eq!(options.limit, Some(1));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_chained_skip_and_limit() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.users.find({ age: { $gt: 18 } }).limit(10).skip(5)")
            .unwrap();
        if let Command::Query(QueryCommand::Find {
            filter, options, ..
        }) = cmd
        {
            assert_eq!(options.limit, Some(10));
            assert_eq!(options.skip, Some(5));
            let age_cond = filter.get_document("age").unwrap();
            assert_eq!(age_cond.get_i64("$gt").unwrap(), 18);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_chained_sort() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.users.find().sort({ name: 1, age: -1 })")
            .unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = cmd {
            assert!(options.sort.is_some());
            let sort = options.sort.unwrap();
            assert_eq!(sort.get_i64("name").unwrap(), 1);
            assert_eq!(sort.get_i64("age").unwrap(), -1);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_complex_chained_query() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse("db.products.find({ category: 'electronics' }).sort({ price: -1 }).limit(20).skip(10)")
            .unwrap();
        if let Command::Query(QueryCommand::Find {
            collection,
            filter,
            options,
        }) = cmd
        {
            assert_eq!(collection, "products");
            assert_eq!(filter.get_str("category").unwrap(), "electronics");
            assert_eq!(options.limit, Some(20));
            assert_eq!(options.skip, Some(10));
            assert!(options.sort.is_some());
        } else {
            panic!("Expected Find command");
        }
    }
}
