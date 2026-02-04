//! Command type definitions for mongosh
//!
//! This module defines all command types that can be parsed and executed,
//! including queries, administrative commands, utilities, and scripts.

use mongodb::bson::Document;
use serde::{Deserialize, Serialize};

use crate::error::ParseError;

/// Query execution mode
///
/// Determines how query results are returned and processed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryMode {
    /// Interactive mode with pagination
    ///
    /// Returns results in batches and saves cursor state for pagination.
    /// Used for normal interactive queries in the shell.
    Interactive { batch_size: u32 },

    /// Streaming mode for export
    ///
    /// Returns a streaming interface for processing all results.
    /// Used for export operations to avoid loading all data into memory.
    Streaming { batch_size: u32 },
}

impl Default for QueryMode {
    fn default() -> Self {
        QueryMode::Interactive { batch_size: 20 }
    }
}

/// Represents a parsed command
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Database query command (CRUD operations)
    Query(QueryCommand),

    /// Administrative command (show, use, create, drop, etc.)
    Admin(AdminCommand),

    /// Utility command (print, serverStatus, etc.)
    Utility(UtilityCommand),

    /// Configuration command (set format, color, etc.)
    Config(ConfigCommand),

    /// Piped command (query with post-processing)
    Pipe(Box<Command>, PipeCommand),

    /// Help command with optional topic
    Help(Option<String>),

    /// Exit/quit command
    Exit,
}

/// Query-related commands (CRUD operations)
#[derive(Debug, Clone, PartialEq)]
pub enum QueryCommand {
    /// Find documents matching a filter
    Find {
        collection: String,
        filter: Document,
        options: FindOptions,
    },

    /// Find one document matching a filter
    FindOne {
        collection: String,
        filter: Document,
        options: FindOptions,
    },

    /// Insert a single document
    InsertOne {
        collection: String,
        document: Document,
    },

    /// Insert multiple documents
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

    /// Update multiple documents
    UpdateMany {
        collection: String,
        filter: Document,
        update: Document,
        options: UpdateOptions,
    },

    /// Replace one document
    ReplaceOne {
        collection: String,
        filter: Document,
        replacement: Document,
        options: UpdateOptions,
    },

    /// Delete one document
    DeleteOne {
        collection: String,
        filter: Document,
    },

    /// Delete multiple documents
    DeleteMany {
        collection: String,
        filter: Document,
    },

    /// Run an aggregation pipeline
    Aggregate {
        collection: String,
        pipeline: Vec<Document>,
        options: AggregateOptions,
    },

    /// Count documents matching a filter
    CountDocuments {
        collection: String,
        filter: Document,
    },

    /// Estimate document count (fast but approximate)
    EstimatedDocumentCount { collection: String },

    /// Find one document and delete it
    FindOneAndDelete {
        collection: String,
        filter: Document,
        options: FindAndModifyOptions,
    },

    /// Find one document and update it
    FindOneAndUpdate {
        collection: String,
        filter: Document,
        update: Document,
        options: FindAndModifyOptions,
    },

    /// Find one document and replace it
    FindOneAndReplace {
        collection: String,
        filter: Document,
        replacement: Document,
        options: FindAndModifyOptions,
    },

    /// Find and modify a document (unified interface)
    FindAndModify {
        collection: String,
        query: Document,
        sort: Option<Document>,
        remove: bool,
        update: Option<Document>,
        new: bool,
        fields: Option<Document>,
        upsert: bool,
        array_filters: Option<Vec<Document>>,
        max_time_ms: Option<u64>,
        collation: Option<Document>,
    },

    /// Create a distinct query
    Distinct {
        collection: String,
        field: String,
        filter: Option<Document>,
    },

    /// Bulk write operations
    BulkWrite {
        collection: String,
        operations: Vec<Document>,
        ordered: bool,
    },

    /// Explain query execution plan
    Explain {
        collection: String,
        verbosity: ExplainVerbosity,
        query: Box<QueryCommand>,
    },
}

impl QueryCommand {
    /// Get the collection name for this query command
    pub fn collection(&self) -> &str {
        match self {
            QueryCommand::Find { collection, .. }
            | QueryCommand::FindOne { collection, .. }
            | QueryCommand::InsertOne { collection, .. }
            | QueryCommand::InsertMany { collection, .. }
            | QueryCommand::UpdateOne { collection, .. }
            | QueryCommand::UpdateMany { collection, .. }
            | QueryCommand::ReplaceOne { collection, .. }
            | QueryCommand::DeleteOne { collection, .. }
            | QueryCommand::DeleteMany { collection, .. }
            | QueryCommand::Aggregate { collection, .. }
            | QueryCommand::CountDocuments { collection, .. }
            | QueryCommand::EstimatedDocumentCount { collection }
            | QueryCommand::FindOneAndDelete { collection, .. }
            | QueryCommand::FindOneAndUpdate { collection, .. }
            | QueryCommand::FindOneAndReplace { collection, .. }
            | QueryCommand::FindAndModify { collection, .. }
            | QueryCommand::Distinct { collection, .. }
            | QueryCommand::BulkWrite { collection, .. }
            | QueryCommand::Explain { collection, .. } => collection,
        }
    }

    /// Check if this query command supports explain
    pub fn supports_explain(&self) -> bool {
        matches!(
            self,
            QueryCommand::Find { .. }
                | QueryCommand::FindOne { .. }
                | QueryCommand::Aggregate { .. }
                | QueryCommand::CountDocuments { .. }
                | QueryCommand::Distinct { .. }
        )
    }
}

/// Explain verbosity modes
#[derive(Debug, Clone, PartialEq)]
pub enum ExplainVerbosity {
    /// Query planner information only (default)
    QueryPlanner,
    /// Query planner and execution statistics
    ExecutionStats,
    /// All plans execution information
    AllPlansExecution,
}

impl ExplainVerbosity {
    /// Verbosity mode constants
    pub const QUERY_PLANNER: &'static str = "queryPlanner";
    pub const EXECUTION_STATS: &'static str = "executionStats";
    pub const ALL_PLANS_EXECUTION: &'static str = "allPlansExecution";

    /// Backwards compatibility constants
    pub const COMPAT_TRUE: &'static str = "true";
    pub const COMPAT_FALSE: &'static str = "false";

    /// Parse verbosity from string
    pub fn from_str(s: &str) -> Result<Self, ParseError> {
        match s {
            Self::QUERY_PLANNER => Ok(ExplainVerbosity::QueryPlanner),
            Self::EXECUTION_STATS => Ok(ExplainVerbosity::ExecutionStats),
            Self::ALL_PLANS_EXECUTION => Ok(ExplainVerbosity::AllPlansExecution),
            Self::COMPAT_TRUE => Ok(ExplainVerbosity::AllPlansExecution), // backwards compatibility
            Self::COMPAT_FALSE => Ok(ExplainVerbosity::QueryPlanner), // backwards compatibility
            _ => Err(ParseError::InvalidCommand(format!(
                "Invalid explain verbosity: '{}'. Valid options are: '{}', '{}', '{}' (or boolean true/false for compatibility)",
                s, Self::QUERY_PLANNER, Self::EXECUTION_STATS, Self::ALL_PLANS_EXECUTION
            ))),
        }
    }

    /// Parse verbosity from boolean (backwards compatibility)
    pub fn from_bool(value: bool) -> Self {
        if value {
            ExplainVerbosity::AllPlansExecution
        } else {
            ExplainVerbosity::QueryPlanner
        }
    }

    /// Convert to MongoDB command string
    pub fn as_str(&self) -> &'static str {
        match self {
            ExplainVerbosity::QueryPlanner => Self::QUERY_PLANNER,
            ExplainVerbosity::ExecutionStats => Self::EXECUTION_STATS,
            ExplainVerbosity::AllPlansExecution => Self::ALL_PLANS_EXECUTION,
        }
    }

    /// Validate explain arguments and parse verbosity
    pub fn parse_from_args(args: &[crate::parser::mongo_ast::Expr]) -> Result<Self, ParseError> {
        use crate::parser::mongo_ast::Expr;

        if args.is_empty() {
            return Ok(Self::default());
        }

        if args.len() > 1 {
            return Err(ParseError::InvalidCommand(
                format!("explain() expects at most 1 argument, got {}", args.len())
            ));
        }

        match args.first().unwrap() {
            Expr::String(verb_str) => Self::from_str(verb_str),
            Expr::Boolean(b) => Ok(Self::from_bool(*b)),
            _ => Err(ParseError::InvalidCommand(
                "explain() expects a string verbosity ('queryPlanner', 'executionStats', 'allPlansExecution') or boolean".to_string()
            )),
        }
    }
}

impl Default for ExplainVerbosity {
    fn default() -> Self {
        ExplainVerbosity::QueryPlanner
    }
}

/// Administrative commands
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum AdminCommand {
    /// Show all databases
    ShowDatabases,

    /// Show collections in current database
    ShowCollections,

    /// Show users in current database
    ShowUsers,

    /// Show roles in current database
    ShowRoles,

    /// Show database profile information
    ShowProfile,

    /// Show logs
    ShowLogs(Option<String>),

    /// Switch to a database
    UseDatabase(String),

    /// Create an index
    CreateIndex {
        collection: String,
        keys: Document,
        options: Option<Document>,
    },

    /// Create multiple indexes
    CreateIndexes {
        collection: String,
        indexes: Vec<Document>,
    },

    /// List indexes on a collection
    ListIndexes(String),

    /// Drop a single index from a collection
    DropIndex { collection: String, index: String },

    /// Drop multiple indexes from a collection
    DropIndexes {
        collection: String,
        indexes: Option<Vec<String>>,
    },

    /// Drop a collection
    DropCollection(String),

    /// Rename a collection
    RenameCollection {
        collection: String,
        target: String,
        drop_target: bool,
    },

    /// Get collection statistics
    CollectionStats {
        collection: String,
        scale: Option<i32>,
    },
}

/// Pipe commands for post-processing query results
#[derive(Debug, Clone, PartialEq)]
pub enum PipeCommand {
    /// Export results to a file
    Export {
        format: ExportFormat,
        file: Option<String>,
    },

    /// Explain query execution plan
    Explain,
}

/// Export format types
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    /// JSON Lines format (one JSON object per line)
    JsonL,
    /// CSV format
    Csv,
}

/// Utility commands
#[derive(Debug, Clone, PartialEq)]
/// Utility commands for shell operations
pub enum UtilityCommand {
    /// Print/echo a value
    #[allow(dead_code)]
    Print(String),

    /// Iterate through more results (it command)
    Iterate,
}

/// Configuration commands for runtime settings
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigCommand {
    /// Set output format (shell, json, json-pretty, table, compact)
    SetFormat(String),

    /// Get current format
    GetFormat,

    /// Enable/disable colors
    SetColor(bool),

    /// Get current color setting
    GetColor,

    /// Show all current settings
    ShowConfig,

    /// List all named queries
    ListNamedQueries,

    /// Execute a named query with arguments
    ExecuteNamedQuery { name: String, args: Vec<String> },

    /// Save a named query
    SaveNamedQuery { name: String, query: String },

    /// Delete a named query
    DeleteNamedQuery(String),
}

/// Options for find operations
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FindOptions {
    /// Maximum number of documents to return
    pub limit: Option<i64>,

    /// Number of documents to skip
    pub skip: Option<u64>,

    /// Sort specification
    pub sort: Option<Document>,

    /// Projection specification (fields to include/exclude)
    pub projection: Option<Document>,

    /// Batch size for cursor
    pub batch_size: Option<u32>,

    /// Enable collation
    pub collation: Option<Document>,

    /// Hint for index to use
    pub hint: Option<Document>,

    /// Maximum time in milliseconds
    pub max_time_ms: Option<u64>,

    /// Read concern level
    pub read_concern: Option<Document>,
}

/// Options for update operations
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct UpdateOptions {
    /// Create document if not found
    pub upsert: bool,

    /// Array filters for positional updates
    pub array_filters: Option<Vec<Document>>,

    /// Collation
    pub collation: Option<Document>,

    /// Hint for index to use
    pub hint: Option<Document>,

    /// Write concern
    pub write_concern: Option<Document>,
}

/// Options for aggregate operations
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AggregateOptions {
    /// Allow writing to temporary files for large aggregations
    pub allow_disk_use: bool,

    /// Batch size for cursor
    pub batch_size: Option<u32>,

    /// Maximum time in milliseconds
    pub max_time_ms: Option<u64>,

    /// Collation
    pub collation: Option<Document>,

    /// Hint for index to use
    pub hint: Option<Document>,

    /// Read concern level
    pub read_concern: Option<Document>,

    /// Let variables for aggregation expressions
    pub let_vars: Option<Document>,
}

/// Options for findAndModify operations
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FindAndModifyOptions {
    /// Return the modified document instead of the original
    pub return_new: bool,

    /// Create document if not found
    pub upsert: bool,

    /// Sort specification
    pub sort: Option<Document>,

    /// Projection specification
    pub projection: Option<Document>,

    /// Collation
    pub collation: Option<Document>,

    /// Array filters
    pub array_filters: Option<Vec<Document>>,

    /// Maximum time in milliseconds
    pub max_time_ms: Option<u64>,

    /// Hint for index to use
    pub hint: Option<Document>,
}
