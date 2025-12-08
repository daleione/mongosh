//! Command and query parser for mongosh
//!
//! This module provides parsing functionality for:
//! - MongoDB shell commands (show dbs, use db, etc.)
//! - MongoDB queries and operations
//! - JavaScript-like syntax
//! - Aggregation pipelines
//!
//! The parser uses a regex-based approach for db.collection.operation() syntax
//! and JSON parsing for document literals.

use mongodb::bson::{doc, Document};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::error::{ParseError, Result};

/// Main parser for mongosh commands
pub struct Parser {
    /// Current database context (for validation)
    current_database: Option<String>,
}

/// Represents a parsed command
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Database query command
    Query(QueryCommand),

    /// Administrative command
    Admin(AdminCommand),

    /// Utility command
    Utility(UtilityCommand),

    /// Script execution command
    Script(ScriptCommand),

    /// Help command
    Help(Option<String>),

    /// Exit/quit command
    Exit,
}

/// Query-related commands (CRUD operations)
#[derive(Debug, Clone, PartialEq)]
pub enum QueryCommand {
    /// Find documents
    Find {
        collection: String,
        filter: Document,
        options: FindOptions,
    },

    /// Insert one document
    InsertOne {
        collection: String,
        document: Document,
    },

    /// Insert many documents
    InsertMany {
        collection: String,
        documents: Vec<Document>,
    },

    /// Update one document
    UpdateOne {
        collection: String,
        filter: Document,
        update: Document,
        options: UpdateOptions,
    },

    /// Update many documents
    UpdateMany {
        collection: String,
        filter: Document,
        update: Document,
        options: UpdateOptions,
    },

    /// Delete one document
    DeleteOne {
        collection: String,
        filter: Document,
    },

    /// Delete many documents
    DeleteMany {
        collection: String,
        filter: Document,
    },

    /// Aggregation pipeline
    Aggregate {
        collection: String,
        pipeline: Vec<Document>,
        options: AggregateOptions,
    },

    /// Count documents
    Count {
        collection: String,
        filter: Option<Document>,
    },

    /// Find and modify
    FindAndModify {
        collection: String,
        query: Document,
        update: Option<Document>,
        remove: bool,
        options: FindAndModifyOptions,
    },
}

/// Administrative commands
#[derive(Debug, Clone, PartialEq)]
pub enum AdminCommand {
    /// Show databases
    ShowDatabases,

    /// Show collections
    ShowCollections,

    /// Use database
    UseDatabase(String),

    /// Create collection
    CreateCollection {
        name: String,
        options: Option<Document>,
    },

    /// Drop collection
    DropCollection(String),

    /// Drop database
    DropDatabase,

    /// Create index
    CreateIndex {
        collection: String,
        keys: Document,
        options: Option<Document>,
    },

    /// Drop index
    DropIndex { collection: String, name: String },

    /// List indexes
    ListIndexes(String),

    /// Get collection stats
    CollectionStats(String),

    /// Get database stats
    DatabaseStats,
}

/// Utility commands
#[derive(Debug, Clone, PartialEq)]
pub enum UtilityCommand {
    /// Print/echo value
    Print(String),

    /// Get server status
    ServerStatus,

    /// Get current time
    CurrentTime,

    /// Execute raw command
    RunCommand(Document),
}

/// Script execution command
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptCommand {
    /// Script content or path
    pub content: String,

    /// Whether content is a file path
    pub is_file: bool,
}

/// Options for find operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindOptions {
    /// Limit number of results
    pub limit: Option<i64>,

    /// Skip number of results
    pub skip: Option<u64>,

    /// Sort specification
    pub sort: Option<Document>,

    /// Projection specification
    pub projection: Option<Document>,

    /// Batch size
    pub batch_size: Option<u32>,
}

/// Options for update operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateOptions {
    /// Create document if not found
    pub upsert: bool,

    /// Array filters for update
    pub array_filters: Option<Vec<Document>>,
}

/// Options for aggregate operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateOptions {
    /// Allow disk use for large aggregations
    pub allow_disk_use: bool,

    /// Batch size
    pub batch_size: Option<u32>,

    /// Max time in milliseconds
    pub max_time_ms: Option<u64>,
}

/// Options for findAndModify operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindAndModifyOptions {
    /// Return new document after modification
    pub return_new: bool,

    /// Create document if not found
    pub upsert: bool,

    /// Sort specification
    pub sort: Option<Document>,

    /// Projection specification
    pub projection: Option<Document>,
}

impl Parser {
    /// Create a new parser
    ///
    /// # Returns
    /// * `Self` - New parser instance
    pub fn new() -> Self {
        Self {
            current_database: None,
        }
    }

    /// Set current database context
    ///
    /// # Arguments
    /// * `database` - Database name
    pub fn set_database(&mut self, database: String) {
        self.current_database = Some(database);
    }

    /// Parse input string into a command
    ///
    /// # Arguments
    /// * `input` - Input string to parse
    ///
    /// # Returns
    /// * `Result<Command>` - Parsed command or parse error
    pub fn parse(&mut self, input: &str) -> Result<Command> {
        // Trim whitespace and trailing semicolons
        let trimmed = input.trim().trim_end_matches(';').trim();

        // Handle empty input
        if trimmed.is_empty() {
            return Err(ParseError::InvalidCommand("Empty input".to_string()).into());
        }

        // Check for exit commands
        if matches!(trimmed, "exit" | "quit") {
            return Ok(Command::Exit);
        }

        // Check for help commands
        if trimmed.starts_with("help") {
            let topic = trimmed
                .strip_prefix("help")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(String::from);
            return Ok(Command::Help(topic));
        }

        // Check for shell commands (show, use, etc.)
        if Self::is_shell_command(trimmed) {
            return self.parse_shell_command(trimmed);
        }

        // Check for db.collection.operation() syntax
        if trimmed.starts_with("db.") {
            return self.parse_db_operation(trimmed);
        }

        // If nothing matches, it's an invalid command
        Err(ParseError::InvalidCommand(format!("Unknown command: {}", trimmed)).into())
    }

    /// Parse a query filter document
    ///
    /// # Arguments
    /// * `query` - Query string to parse
    ///
    /// # Returns
    /// * `Result<Document>` - Parsed BSON document or error
    pub fn parse_query(&self, query: &str) -> Result<Document> {
        self.parse_document(query)
    }

    /// Parse an aggregation pipeline
    ///
    /// # Arguments
    /// * `pipeline` - Pipeline string to parse
    ///
    /// # Returns
    /// * `Result<Vec<Document>>` - Parsed pipeline stages or error
    pub fn parse_aggregation(&self, pipeline: &str) -> Result<Vec<Document>> {
        let trimmed = pipeline.trim();

        // Pipeline should be an array of documents
        if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
            return Err(
                ParseError::InvalidPipeline("Pipeline must be an array".to_string()).into(),
            );
        }

        // Parse as JSON array
        let json_value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| ParseError::InvalidPipeline(e.to_string()))?;

        // Convert to vector of BSON documents
        if let serde_json::Value::Array(stages) = json_value {
            let documents: Result<Vec<Document>> = stages
                .into_iter()
                .map(|stage| {
                    let bson = mongodb::bson::to_bson(&stage)
                        .map_err(|e| ParseError::InvalidPipeline(e.to_string()))?;
                    bson.as_document().cloned().ok_or_else(|| {
                        ParseError::InvalidPipeline("Stage must be a document".to_string()).into()
                    })
                })
                .collect();
            documents
        } else {
            Err(ParseError::InvalidPipeline("Pipeline must be an array".to_string()).into())
        }
    }

    /// Parse a document literal (JSON-like object)
    ///
    /// # Arguments
    /// * `input` - Document string to parse
    ///
    /// # Returns
    /// * `Result<Document>` - Parsed document or error
    pub fn parse_document(&self, input: &str) -> Result<Document> {
        let trimmed = input.trim();

        // Empty object
        if trimmed == "{}" {
            return Ok(Document::new());
        }

        // Try to parse as JSON
        self.parse_json_to_bson(trimmed)
    }

    /// Parse JSON string to BSON document
    ///
    /// Handles MongoDB-specific types like ObjectId, Date, etc.
    ///
    /// # Arguments
    /// * `json` - JSON string
    ///
    /// # Returns
    /// * `Result<Document>` - BSON document
    fn parse_json_to_bson(&self, json: &str) -> Result<Document> {
        // Parse as JSON value first
        let json_value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| ParseError::InvalidQuery(e.to_string()))?;

        // Convert to BSON
        let bson = mongodb::bson::to_bson(&json_value)
            .map_err(|e| ParseError::InvalidQuery(e.to_string()))?;

        // Ensure it's a document
        bson.as_document()
            .cloned()
            .ok_or_else(|| ParseError::InvalidQuery("Expected a document".to_string()).into())
    }

    /// Check if input is a shell command (show, use, etc.)
    ///
    /// # Arguments
    /// * `input` - Input string to check
    ///
    /// # Returns
    /// * `bool` - True if input starts with a shell command keyword
    fn is_shell_command(input: &str) -> bool {
        let keywords = ["show", "use"];
        keywords.iter().any(|kw| input.starts_with(kw))
    }

    /// Parse a shell command
    ///
    /// # Arguments
    /// * `input` - Shell command string
    ///
    /// # Returns
    /// * `Result<Command>` - Parsed command or error
    fn parse_shell_command(&self, input: &str) -> Result<Command> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        if parts.is_empty() {
            return Err(ParseError::InvalidCommand("Empty command".to_string()).into());
        }

        match parts[0] {
            "show" => {
                if parts.len() < 2 {
                    return Err(ParseError::InvalidCommand(
                        "'show' requires an argument".to_string(),
                    )
                    .into());
                }
                match parts[1] {
                    "dbs" | "databases" => Ok(Command::Admin(AdminCommand::ShowDatabases)),
                    "collections" | "tables" => Ok(Command::Admin(AdminCommand::ShowCollections)),
                    _ => Err(ParseError::InvalidCommand(format!(
                        "Unknown show command: {}",
                        parts[1]
                    ))
                    .into()),
                }
            }
            "use" => {
                if parts.len() < 2 {
                    return Err(ParseError::InvalidCommand(
                        "'use' requires a database name".to_string(),
                    )
                    .into());
                }
                Ok(Command::Admin(AdminCommand::UseDatabase(
                    parts[1].to_string(),
                )))
            }
            _ => Err(ParseError::InvalidCommand(format!("Unknown command: {}", parts[0])).into()),
        }
    }

    /// Parse a database operation (db.collection.operation())
    ///
    /// # Arguments
    /// * `input` - Operation string
    ///
    /// # Returns
    /// * `Result<Command>` - Parsed query command or error
    fn parse_db_operation(&self, input: &str) -> Result<Command> {
        // Use regex to parse db.collection.operation() pattern
        static DB_OP_REGEX: OnceLock<Regex> = OnceLock::new();
        let regex = DB_OP_REGEX.get_or_init(|| {
            Regex::new(r"^db\.([a-zA-Z_][a-zA-Z0-9_]*)\.([a-zA-Z]+)\((.*)\)$").unwrap()
        });

        if let Some(captures) = regex.captures(input.trim()) {
            let collection = captures.get(1).unwrap().as_str().to_string();
            let operation = captures.get(2).unwrap().as_str();
            let args_str = captures.get(3).unwrap().as_str().trim();

            // Parse based on operation type
            match operation {
                "find" => self.parse_find_operation(collection, args_str),
                "insertOne" => self.parse_insert_one_operation(collection, args_str),
                "insertMany" => self.parse_insert_many_operation(collection, args_str),
                "updateOne" => self.parse_update_one_operation(collection, args_str),
                "updateMany" => self.parse_update_many_operation(collection, args_str),
                "deleteOne" => self.parse_delete_one_operation(collection, args_str),
                "deleteMany" => self.parse_delete_many_operation(collection, args_str),
                "count" | "countDocuments" => self.parse_count_operation(collection, args_str),
                "aggregate" => self.parse_aggregate_operation(collection, args_str),
                _ => Err(
                    ParseError::InvalidCommand(format!("Unknown operation: {}", operation)).into(),
                ),
            }
        } else {
            Err(ParseError::SyntaxError(format!(
                "Invalid db.collection.operation() syntax: {}",
                input
            ))
            .into())
        }
    }

    /// Parse find operation: db.collection.find(filter, projection)
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `args` - Arguments string
    ///
    /// # Returns
    /// * `Result<Command>` - Parsed find command
    fn parse_find_operation(&self, collection: String, args: &str) -> Result<Command> {
        let (filter, options) = if args.is_empty() {
            // No arguments: find all
            (Document::new(), FindOptions::default())
        } else {
            // Parse arguments
            let parsed_args = self.parse_function_arguments(args)?;

            let filter = if parsed_args.is_empty() {
                Document::new()
            } else {
                self.parse_document(&parsed_args[0])?
            };

            let mut options = FindOptions::default();

            // Second argument is projection
            if parsed_args.len() > 1 {
                options.projection = Some(self.parse_document(&parsed_args[1])?);
            }

            (filter, options)
        };

        Ok(Command::Query(QueryCommand::Find {
            collection,
            filter,
            options,
        }))
    }

    /// Parse insertOne operation: db.collection.insertOne(document)
    fn parse_insert_one_operation(&self, collection: String, args: &str) -> Result<Command> {
        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "insertOne requires a document argument".to_string(),
            )
            .into());
        }

        let parsed_args = self.parse_function_arguments(args)?;
        if parsed_args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "insertOne requires a document argument".to_string(),
            )
            .into());
        }

        let document = self.parse_document(&parsed_args[0])?;

        Ok(Command::Query(QueryCommand::InsertOne {
            collection,
            document,
        }))
    }

    /// Parse insertMany operation: db.collection.insertMany([documents])
    fn parse_insert_many_operation(&self, collection: String, args: &str) -> Result<Command> {
        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "insertMany requires an array of documents".to_string(),
            )
            .into());
        }

        let parsed_args = self.parse_function_arguments(args)?;
        if parsed_args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "insertMany requires an array of documents".to_string(),
            )
            .into());
        }

        // Parse array of documents
        let array_str = &parsed_args[0];
        let json_value: serde_json::Value = serde_json::from_str(array_str)
            .map_err(|e| ParseError::InvalidCommand(e.to_string()))?;

        let documents = if let serde_json::Value::Array(arr) = json_value {
            let docs: Result<Vec<Document>> = arr
                .into_iter()
                .map(|v| {
                    let bson = mongodb::bson::to_bson(&v)
                        .map_err(|e| ParseError::InvalidCommand(e.to_string()))?;
                    bson.as_document().cloned().ok_or_else(|| {
                        ParseError::InvalidCommand("Expected document in array".to_string()).into()
                    })
                })
                .collect();
            docs?
        } else {
            return Err(ParseError::InvalidCommand(
                "insertMany requires an array of documents".to_string(),
            )
            .into());
        };

        Ok(Command::Query(QueryCommand::InsertMany {
            collection,
            documents,
        }))
    }

    /// Parse updateOne operation: db.collection.updateOne(filter, update, options)
    fn parse_update_one_operation(&self, collection: String, args: &str) -> Result<Command> {
        let parsed_args = self.parse_function_arguments(args)?;
        if parsed_args.len() < 2 {
            return Err(ParseError::InvalidCommand(
                "updateOne requires filter and update arguments".to_string(),
            )
            .into());
        }

        let filter = self.parse_document(&parsed_args[0])?;
        let update = self.parse_document(&parsed_args[1])?;

        let options = if parsed_args.len() > 2 {
            // Parse options document
            let opts_doc = self.parse_document(&parsed_args[2])?;
            let mut options = UpdateOptions::default();
            if let Ok(upsert) = opts_doc.get_bool("upsert") {
                options.upsert = upsert;
            }
            options
        } else {
            UpdateOptions::default()
        };

        Ok(Command::Query(QueryCommand::UpdateOne {
            collection,
            filter,
            update,
            options,
        }))
    }

    /// Parse updateMany operation: db.collection.updateMany(filter, update, options)
    fn parse_update_many_operation(&self, collection: String, args: &str) -> Result<Command> {
        let parsed_args = self.parse_function_arguments(args)?;
        if parsed_args.len() < 2 {
            return Err(ParseError::InvalidCommand(
                "updateMany requires filter and update arguments".to_string(),
            )
            .into());
        }

        let filter = self.parse_document(&parsed_args[0])?;
        let update = self.parse_document(&parsed_args[1])?;

        let options = if parsed_args.len() > 2 {
            let opts_doc = self.parse_document(&parsed_args[2])?;
            let mut options = UpdateOptions::default();
            if let Ok(upsert) = opts_doc.get_bool("upsert") {
                options.upsert = upsert;
            }
            options
        } else {
            UpdateOptions::default()
        };

        Ok(Command::Query(QueryCommand::UpdateMany {
            collection,
            filter,
            update,
            options,
        }))
    }

    /// Parse deleteOne operation: db.collection.deleteOne(filter)
    fn parse_delete_one_operation(&self, collection: String, args: &str) -> Result<Command> {
        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "deleteOne requires a filter argument".to_string(),
            )
            .into());
        }

        let parsed_args = self.parse_function_arguments(args)?;
        let filter = self.parse_document(&parsed_args[0])?;

        Ok(Command::Query(QueryCommand::DeleteOne {
            collection,
            filter,
        }))
    }

    /// Parse deleteMany operation: db.collection.deleteMany(filter)
    fn parse_delete_many_operation(&self, collection: String, args: &str) -> Result<Command> {
        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "deleteMany requires a filter argument".to_string(),
            )
            .into());
        }

        let parsed_args = self.parse_function_arguments(args)?;
        let filter = self.parse_document(&parsed_args[0])?;

        Ok(Command::Query(QueryCommand::DeleteMany {
            collection,
            filter,
        }))
    }

    /// Parse count operation: db.collection.count(filter)
    fn parse_count_operation(&self, collection: String, args: &str) -> Result<Command> {
        let filter = if args.is_empty() {
            None
        } else {
            let parsed_args = self.parse_function_arguments(args)?;
            if parsed_args.is_empty() {
                None
            } else {
                Some(self.parse_document(&parsed_args[0])?)
            }
        };

        Ok(Command::Query(QueryCommand::Count { collection, filter }))
    }

    /// Parse aggregate operation: db.collection.aggregate(pipeline)
    fn parse_aggregate_operation(&self, collection: String, args: &str) -> Result<Command> {
        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "aggregate requires a pipeline argument".to_string(),
            )
            .into());
        }

        let parsed_args = self.parse_function_arguments(args)?;
        let pipeline = self.parse_aggregation(&parsed_args[0])?;

        Ok(Command::Query(QueryCommand::Aggregate {
            collection,
            pipeline,
            options: AggregateOptions::default(),
        }))
    }

    /// Parse function arguments, handling nested parentheses and braces
    ///
    /// # Arguments
    /// * `args` - Arguments string (without outer parentheses)
    ///
    /// # Returns
    /// * `Result<Vec<String>>` - Vector of argument strings
    fn parse_function_arguments(&self, args: &str) -> Result<Vec<String>> {
        if args.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut arguments = Vec::new();
        let mut current_arg = String::new();
        let mut depth_paren = 0;
        let mut depth_brace = 0;
        let mut depth_bracket = 0;
        let mut in_string = false;
        let mut escape_next = false;
        let mut string_delimiter = '\0';

        for ch in args.chars() {
            if escape_next {
                current_arg.push(ch);
                escape_next = false;
                continue;
            }

            if ch == '\\' {
                escape_next = true;
                current_arg.push(ch);
                continue;
            }

            if (ch == '"' || ch == '\'')
                && depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
            {
                if in_string && ch == string_delimiter {
                    in_string = false;
                    string_delimiter = '\0';
                } else if !in_string {
                    in_string = true;
                    string_delimiter = ch;
                }
                current_arg.push(ch);
                continue;
            }

            if in_string {
                current_arg.push(ch);
                continue;
            }

            match ch {
                '(' => {
                    depth_paren += 1;
                    current_arg.push(ch);
                }
                ')' => {
                    depth_paren -= 1;
                    current_arg.push(ch);
                }
                '{' => {
                    depth_brace += 1;
                    current_arg.push(ch);
                }
                '}' => {
                    depth_brace -= 1;
                    current_arg.push(ch);
                }
                '[' => {
                    depth_bracket += 1;
                    current_arg.push(ch);
                }
                ']' => {
                    depth_bracket -= 1;
                    current_arg.push(ch);
                }
                ',' => {
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                        // End of argument
                        arguments.push(current_arg.trim().to_string());
                        current_arg.clear();
                    } else {
                        current_arg.push(ch);
                    }
                }
                _ => {
                    current_arg.push(ch);
                }
            }
        }

        // Add last argument
        if !current_arg.trim().is_empty() {
            arguments.push(current_arg.trim().to_string());
        }

        Ok(arguments)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            limit: None,
            skip: None,
            sort: None,
            projection: None,
            batch_size: None,
        }
    }
}

impl Default for UpdateOptions {
    fn default() -> Self {
        Self {
            upsert: false,
            array_filters: None,
        }
    }
}

impl Default for AggregateOptions {
    fn default() -> Self {
        Self {
            allow_disk_use: false,
            batch_size: None,
            max_time_ms: None,
        }
    }
}

impl Default for FindAndModifyOptions {
    fn default() -> Self {
        Self {
            return_new: false,
            upsert: false,
            sort: None,
            projection: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = Parser::new();
        assert!(parser.current_database.is_none());
    }

    #[test]
    fn test_is_shell_command() {
        assert!(Parser::is_shell_command("show dbs"));
        assert!(Parser::is_shell_command("use mydb"));
        assert!(!Parser::is_shell_command("db.users.find()"));
    }

    #[test]
    fn test_parse_exit() {
        let mut parser = Parser::new();
        assert_eq!(parser.parse("exit").unwrap(), Command::Exit);
        assert_eq!(parser.parse("quit").unwrap(), Command::Exit);
    }

    #[test]
    fn test_parse_help() {
        let mut parser = Parser::new();
        assert_eq!(parser.parse("help").unwrap(), Command::Help(None));
        assert_eq!(
            parser.parse("help find").unwrap(),
            Command::Help(Some("find".to_string()))
        );
    }

    #[test]
    fn test_parse_show_databases() {
        let mut parser = Parser::new();
        let cmd = parser.parse("show dbs").unwrap();
        assert_eq!(cmd, Command::Admin(AdminCommand::ShowDatabases));
    }

    #[test]
    fn test_parse_show_collections() {
        let mut parser = Parser::new();
        let cmd = parser.parse("show collections").unwrap();
        assert_eq!(cmd, Command::Admin(AdminCommand::ShowCollections));
    }

    #[test]
    fn test_parse_use_database() {
        let mut parser = Parser::new();
        let cmd = parser.parse("use testdb").unwrap();
        assert_eq!(
            cmd,
            Command::Admin(AdminCommand::UseDatabase("testdb".to_string()))
        );
    }

    #[test]
    fn test_parse_find_empty() {
        let mut parser = Parser::new();
        let cmd = parser.parse("db.users.find()").unwrap();
        match cmd {
            Command::Query(QueryCommand::Find {
                collection,
                filter,
                options: _,
            }) => {
                assert_eq!(collection, "users");
                assert_eq!(filter, Document::new());
            }
            _ => panic!("Expected Find command"),
        }
    }

    #[test]
    fn test_parse_find_with_filter() {
        let mut parser = Parser::new();
        let cmd = parser.parse(r#"db.users.find({"age": 25})"#).unwrap();
        match cmd {
            Command::Query(QueryCommand::Find {
                collection,
                filter,
                options: _,
            }) => {
                assert_eq!(collection, "users");
                assert_eq!(filter.get_i64("age"), Ok(25));
            }
            _ => panic!("Expected Find command"),
        }
    }

    #[test]
    fn test_parse_find_with_projection() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse(r#"db.users.find({"age": 25}, {"name": 1})"#)
            .unwrap();
        match cmd {
            Command::Query(QueryCommand::Find {
                collection,
                filter,
                options,
            }) => {
                assert_eq!(collection, "users");
                assert_eq!(filter.get_i64("age"), Ok(25));
                assert!(options.projection.is_some());
            }
            _ => panic!("Expected Find command"),
        }
    }

    #[test]
    fn test_parse_empty_document() {
        let parser = Parser::new();
        let doc = parser.parse_document("{}").unwrap();
        assert_eq!(doc, Document::new());
    }

    #[test]
    fn test_parse_simple_document() {
        let parser = Parser::new();
        let doc = parser
            .parse_document(r#"{"name": "Alice", "age": 30}"#)
            .unwrap();
        assert_eq!(doc.get_str("name"), Ok("Alice"));
        assert_eq!(doc.get_i64("age"), Ok(30));
    }

    #[test]
    fn test_parse_nested_document() {
        let parser = Parser::new();
        let doc = parser
            .parse_document(r#"{"user": {"name": "Bob", "age": 25}}"#)
            .unwrap();
        let user_doc = doc.get_document("user").unwrap();
        assert_eq!(user_doc.get_str("name"), Ok("Bob"));
    }

    #[test]
    fn test_parse_function_arguments_simple() {
        let parser = Parser::new();
        let args = parser
            .parse_function_arguments(r#"{"a": 1}, {"b": 2}"#)
            .unwrap();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], r#"{"a": 1}"#);
        assert_eq!(args[1], r#"{"b": 2}"#);
    }

    #[test]
    fn test_parse_function_arguments_nested() {
        let parser = Parser::new();
        let args = parser
            .parse_function_arguments(r#"{"user": {"name": "Alice"}}, {"age": 25}"#)
            .unwrap();
        assert_eq!(args.len(), 2);
        assert!(args[0].contains("Alice"));
    }

    #[test]
    fn test_parse_aggregation() {
        let parser = Parser::new();
        let pipeline = parser
            .parse_aggregation(r#"[{"$match": {"age": 25}}, {"$group": {"_id": "$city"}}]"#)
            .unwrap();
        assert_eq!(pipeline.len(), 2);
    }

    #[test]
    fn test_parse_insert_one() {
        let mut parser = Parser::new();
        let cmd = parser
            .parse(r#"db.users.insertOne({"name": "Alice", "age": 30})"#)
            .unwrap();
        match cmd {
            Command::Query(QueryCommand::InsertOne {
                collection,
                document,
            }) => {
                assert_eq!(collection, "users");
                assert_eq!(document.get_str("name"), Ok("Alice"));
            }
            _ => panic!("Expected InsertOne command"),
        }
    }

    #[test]
    fn test_find_options_default() {
        let options = FindOptions::default();
        assert!(options.limit.is_none());
        assert!(options.skip.is_none());
    }

    #[test]
    fn test_invalid_command() {
        let mut parser = Parser::new();
        assert!(parser.parse("invalid command").is_err());
    }

    #[test]
    fn test_invalid_db_operation() {
        let mut parser = Parser::new();
        assert!(parser.parse("db.users.unknownOp()").is_err());
    }
}
