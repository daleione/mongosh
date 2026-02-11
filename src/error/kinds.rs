//! Error types for the MongoDB shell application.
//!
//! This module defines a streamlined error hierarchy that wraps MongoDB driver
//! errors and provides additional application-specific error types.

use std::{fmt, io};

use crate::error::mongo::format_mongodb_error;

/// Crate-wide `Result` type using [`MongoshError`] as the error.
pub type Result<T> = std::result::Result<T, MongoshError>;

/// Top-level error type for mongosh operations.
///
/// This type provides a unified error interface for the entire application,
/// wrapping MongoDB driver errors and application-specific errors.
#[derive(Debug)]
pub enum MongoshError {
    /// MongoDB driver errors (automatically formatted as structured JSON).
    MongoDb(mongodb::error::Error),

    /// Connection-related errors.
    Connection(ConnectionError),

    /// Command parsing errors.
    Parse(ParseError),

    /// Command execution errors.
    Execution(ExecutionError),

    /// Configuration errors.
    Config(ConfigError),

    /// I/O errors.
    Io(io::Error),

    /// Generic error with a message.
    Generic(String),

    /// Feature not yet implemented.
    NotImplemented(String),
}

/// Connection-specific errors.
#[derive(Debug)]
pub enum ConnectionError {
    /// Failed to establish a connection.
    ConnectionFailed(String),

    /// Connection timeout.
    #[allow(dead_code)]
    Timeout,

    /// Invalid connection URI.
    InvalidUri(String),

    /// Not currently connected to MongoDB.
    NotConnected,

    /// Ping command failed.
    PingFailed(String),

    /// Session operation failed.
    #[allow(dead_code)]
    SessionFailed(String),

    /// Transaction operation failed.
    #[allow(dead_code)]
    TransactionFailed(String),
}

/// Parsing-specific errors.
#[derive(Debug)]
pub enum ParseError {
    /// Syntax error in input.
    SyntaxError(String),

    /// Invalid command format.
    InvalidCommand(String),

    /// Invalid query syntax.
    InvalidQuery(String),

    /// Invalid aggregation pipeline.
    #[allow(dead_code)]
    InvalidPipeline(String),
}

/// Execution-specific errors.
#[derive(Debug)]
pub enum ExecutionError {
    /// Query execution failed.
    QueryFailed(String),

    /// Invalid operation parameters.
    InvalidParameters(String),

    /// Cursor error.
    CursorError(String),

    /// Invalid operation.
    InvalidOperation(String),
}

/// Configuration-specific errors.
#[derive(Debug)]
pub enum ConfigError {
    /// Config file not found.
    #[allow(dead_code)]
    FileNotFound(String),

    /// Invalid config format.
    #[allow(dead_code)]
    InvalidFormat(String),

    /// Missing required field.
    #[allow(dead_code)]
    MissingField(String),

    /// Generic configuration error.
    Generic(String),
}

// ============================================================================
// Display implementations
// ============================================================================

impl fmt::Display for MongoshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MongoshError::MongoDb(e) => format_mongodb_error(f, e),
            MongoshError::Connection(e) => write!(f, "ConnectionError: {}", e),
            MongoshError::Parse(e) => write!(f, "ParseError: {}", e),
            MongoshError::Execution(e) => write!(f, "ExecutionError: {}", e),
            MongoshError::Config(e) => write!(f, "ConfigError: {}", e),
            MongoshError::Io(e) => write!(f, "IoError: {}", e),
            MongoshError::Generic(msg) => write!(f, "{}", msg),
            MongoshError::NotImplemented(msg) => write!(f, "NotImplemented: {}", msg),
        }
    }
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionError::ConnectionFailed(msg) => write!(f, "{}", msg),
            ConnectionError::Timeout => write!(f, "Connection timeout"),
            ConnectionError::InvalidUri(uri) => write!(f, "Invalid connection URI: {}", uri),
            ConnectionError::NotConnected => write!(f, "Not connected to MongoDB"),
            ConnectionError::PingFailed(msg) => write!(f, "{}", msg),
            ConnectionError::SessionFailed(msg) => write!(f, "{}", msg),
            ConnectionError::TransactionFailed(msg) => write!(f, "{}", msg),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::SyntaxError(msg) => write!(f, "{}", msg),
            ParseError::InvalidCommand(msg) => write!(f, "{}", msg),
            ParseError::InvalidQuery(msg) => write!(f, "{}", msg),
            ParseError::InvalidPipeline(msg) => write!(f, "{}", msg),
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::QueryFailed(msg) => write!(f, "{}", msg),
            ExecutionError::InvalidParameters(msg) => write!(f, "{}", msg),
            ExecutionError::CursorError(msg) => write!(f, "{}", msg),
            ExecutionError::InvalidOperation(msg) => write!(f, "{}", msg),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::FileNotFound(path) => write!(f, "{}", path),
            ConfigError::InvalidFormat(msg) => write!(f, "{}", msg),
            ConfigError::MissingField(field) => write!(f, "{}", field),
            ConfigError::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

// ============================================================================
// Error trait implementations
// ============================================================================

impl std::error::Error for MongoshError {}
impl std::error::Error for ConnectionError {}
impl std::error::Error for ParseError {}
impl std::error::Error for ExecutionError {}
impl std::error::Error for ConfigError {}

// ============================================================================
// Conversions to MongoshError
// ============================================================================

impl From<mongodb::error::Error> for MongoshError {
    fn from(err: mongodb::error::Error) -> Self {
        MongoshError::MongoDb(err)
    }
}

impl From<ConnectionError> for MongoshError {
    fn from(err: ConnectionError) -> Self {
        MongoshError::Connection(err)
    }
}

impl From<ParseError> for MongoshError {
    fn from(err: ParseError) -> Self {
        MongoshError::Parse(err)
    }
}

impl From<ExecutionError> for MongoshError {
    fn from(err: ExecutionError) -> Self {
        MongoshError::Execution(err)
    }
}

impl From<ConfigError> for MongoshError {
    fn from(err: ConfigError) -> Self {
        MongoshError::Config(err)
    }
}

impl From<io::Error> for MongoshError {
    fn from(err: io::Error) -> Self {
        MongoshError::Io(err)
    }
}

impl From<String> for MongoshError {
    fn from(msg: String) -> Self {
        MongoshError::Generic(msg)
    }
}

impl From<&str> for MongoshError {
    fn from(msg: &str) -> Self {
        MongoshError::Generic(msg.to_string())
    }
}

impl From<bson::ser::Error> for MongoshError {
    fn from(err: bson::ser::Error) -> Self {
        MongoshError::Generic(format!("BSON serialization error: {}", err))
    }
}

impl From<bson::de::Error> for MongoshError {
    fn from(err: bson::de::Error) -> Self {
        MongoshError::Generic(format!("BSON deserialization error: {}", err))
    }
}

impl From<reedline::ReedlineError> for MongoshError {
    fn from(err: reedline::ReedlineError) -> Self {
        MongoshError::Generic(format!("Reedline error: {}", err))
    }
}
