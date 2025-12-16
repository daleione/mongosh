//! Command type definitions for mongosh
//!
//! This module defines all command types that can be parsed and executed,
//! including queries, administrative commands, utilities, and scripts.

use mongodb::bson::Document;
use serde::{Deserialize, Serialize};

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

    /// Script execution command
    Script(ScriptCommand),

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
}

/// Administrative commands
#[derive(Debug, Clone, PartialEq)]
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

    /// Create a collection
    CreateCollection {
        name: String,
        options: Option<Document>,
    },

    /// Drop a collection
    DropCollection(String),

    /// Drop current database
    DropDatabase,

    /// Rename a collection
    RenameCollection {
        from: String,
        to: String,
        drop_target: bool,
    },

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

    /// Drop an index
    DropIndex { collection: String, name: String },

    /// Drop all indexes on a collection
    DropIndexes(String),

    /// List indexes on a collection
    ListIndexes(String),

    /// Get collection statistics
    CollectionStats(String),

    /// Get database statistics
    DatabaseStats,

    /// Get server status
    ServerStatus,

    /// Get current operations
    CurrentOp { include_all: bool },

    /// Kill an operation
    KillOp(i64),

    /// Validate a collection
    ValidateCollection { collection: String, full: bool },

    /// Compact a collection
    CompactCollection(String),

    /// Repair database
    RepairDatabase,
}

/// Utility commands
#[derive(Debug, Clone, PartialEq)]
/// Utility commands for shell operations
pub enum UtilityCommand {
    /// Print/echo a value
    Print(String),

    /// Print in JSON format
    PrintJson(Document),

    /// Get current time
    CurrentTime,

    /// Execute a raw database command
    RunCommand(Document),

    /// Get build info
    BuildInfo,

    /// Get host info
    HostInfo,

    /// Get connection status
    ConnectionStatus,

    /// Get database version
    Version,
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
}

/// Script execution command
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptCommand {
    /// Script content or file path
    pub content: String,

    /// Whether content is a file path
    pub is_file: bool,
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

impl Command {
    /// Check if this is an exit command
    pub fn is_exit(&self) -> bool {
        matches!(self, Command::Exit)
    }

    /// Check if this is a help command
    pub fn is_help(&self) -> bool {
        matches!(self, Command::Help(_))
    }

    /// Check if this is a query command
    pub fn is_query(&self) -> bool {
        matches!(self, Command::Query(_))
    }

    /// Check if this is an admin command
    pub fn is_admin(&self) -> bool {
        matches!(self, Command::Admin(_))
    }

    /// Get command name for display
    pub fn name(&self) -> &str {
        match self {
            Command::Query(q) => q.name(),
            Command::Admin(a) => a.name(),
            Command::Utility(u) => u.name(),
            Command::Config(_) => "config",
            Command::Script(_) => "script",
            Command::Help(_) => "help",
            Command::Exit => "exit",
        }
    }
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
            | QueryCommand::EstimatedDocumentCount { collection, .. }
            | QueryCommand::FindOneAndDelete { collection, .. }
            | QueryCommand::FindOneAndUpdate { collection, .. }
            | QueryCommand::FindOneAndReplace { collection, .. }
            | QueryCommand::Distinct { collection, .. }
            | QueryCommand::BulkWrite { collection, .. } => collection,
        }
    }

    /// Get command name
    pub fn name(&self) -> &str {
        match self {
            QueryCommand::Find { .. } => "find",
            QueryCommand::FindOne { .. } => "findOne",
            QueryCommand::InsertOne { .. } => "insertOne",
            QueryCommand::InsertMany { .. } => "insertMany",
            QueryCommand::UpdateOne { .. } => "updateOne",
            QueryCommand::UpdateMany { .. } => "updateMany",
            QueryCommand::ReplaceOne { .. } => "replaceOne",
            QueryCommand::DeleteOne { .. } => "deleteOne",
            QueryCommand::DeleteMany { .. } => "deleteMany",
            QueryCommand::Aggregate { .. } => "aggregate",
            QueryCommand::CountDocuments { .. } => "countDocuments",
            QueryCommand::EstimatedDocumentCount { .. } => "estimatedDocumentCount",
            QueryCommand::FindOneAndDelete { .. } => "findOneAndDelete",
            QueryCommand::FindOneAndUpdate { .. } => "findOneAndUpdate",
            QueryCommand::FindOneAndReplace { .. } => "findOneAndReplace",
            QueryCommand::Distinct { .. } => "distinct",
            QueryCommand::BulkWrite { .. } => "bulkWrite",
        }
    }
}

impl AdminCommand {
    /// Get command name
    pub fn name(&self) -> &str {
        match self {
            AdminCommand::ShowDatabases => "show dbs",
            AdminCommand::ShowCollections => "show collections",
            AdminCommand::ShowUsers => "show users",
            AdminCommand::ShowRoles => "show roles",
            AdminCommand::ShowProfile => "show profile",
            AdminCommand::ShowLogs(_) => "show logs",
            AdminCommand::UseDatabase(_) => "use",
            AdminCommand::CreateCollection { .. } => "createCollection",
            AdminCommand::DropCollection(_) => "dropCollection",
            AdminCommand::DropDatabase => "dropDatabase",
            AdminCommand::RenameCollection { .. } => "renameCollection",
            AdminCommand::CreateIndex { .. } => "createIndex",
            AdminCommand::CreateIndexes { .. } => "createIndexes",
            AdminCommand::DropIndex { .. } => "dropIndex",
            AdminCommand::DropIndexes(_) => "dropIndexes",
            AdminCommand::ListIndexes(_) => "listIndexes",
            AdminCommand::CollectionStats(_) => "collStats",
            AdminCommand::DatabaseStats => "dbStats",
            AdminCommand::ServerStatus => "serverStatus",
            AdminCommand::CurrentOp { .. } => "currentOp",
            AdminCommand::KillOp(_) => "killOp",
            AdminCommand::ValidateCollection { .. } => "validate",
            AdminCommand::CompactCollection(_) => "compact",
            AdminCommand::RepairDatabase => "repairDatabase",
        }
    }
}

impl UtilityCommand {
    /// Get command name
    pub fn name(&self) -> &str {
        match self {
            UtilityCommand::Print(_) => "print",
            UtilityCommand::PrintJson(_) => "printjson",
            UtilityCommand::CurrentTime => "Date",
            UtilityCommand::RunCommand(_) => "runCommand",
            UtilityCommand::BuildInfo => "buildInfo",
            UtilityCommand::HostInfo => "hostInfo",
            UtilityCommand::ConnectionStatus => "connectionStatus",
            UtilityCommand::Version => "version",
        }
    }
}

impl ConfigCommand {
    /// Get command name
    pub fn name(&self) -> &str {
        match self {
            ConfigCommand::SetFormat(_) => "setFormat",
            ConfigCommand::GetFormat => "getFormat",
            ConfigCommand::SetColor(_) => "setColor",
            ConfigCommand::GetColor => "getColor",
            ConfigCommand::ShowConfig => "config",
        }
    }
}
