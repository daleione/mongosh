use std::{fmt, io};

use crate::error::mongo::format_mongodb_error;

/// Crate-wide `Result` type using [`MongoshError`] as the error.
///
/// This alias is re-exported by the parent `error` module and is intended
/// to be used throughout the crate for fallible operations.
pub type Result<T> = std::result::Result<T, MongoshError>;

/// Top-level error type for mongosh operations.
///
/// This type wraps more specific error kinds and provides a single
/// error type that can be used throughout the crate.
#[derive(Debug)]
pub enum MongoshError {
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

    /// MongoDB driver errors.
    MongoDb(mongodb::error::Error),

    /// Authentication errors.
    Auth(AuthError),

    /// Plugin-related errors.
    Plugin(PluginError),

    /// Script execution errors.
    Script(ScriptError),

    /// Generic error with a free-form message.
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
    Timeout,

    /// Invalid connection URI.
    InvalidUri(String),

    /// Connection lost.
    Disconnected,

    /// Connection pool exhausted.
    PoolExhausted,

    /// Not currently connected to MongoDB.
    NotConnected,

    /// Ping command failed.
    PingFailed(String),

    /// Command execution failed.
    CommandFailed(String),

    /// Session operation failed.
    SessionFailed(String),

    /// Transaction operation failed.
    TransactionFailed(String),
}

/// Parsing-specific errors.
#[derive(Debug)]
pub enum ParseError {
    /// Syntax error in command.
    SyntaxError(String),

    /// Invalid command format.
    InvalidCommand(String),

    /// Unexpected token while parsing.
    UnexpectedToken { expected: String, found: String },

    /// Invalid query syntax.
    InvalidQuery(String),

    /// Invalid aggregation pipeline.
    InvalidPipeline(String),
}

/// Execution-specific errors.
#[derive(Debug)]
pub enum ExecutionError {
    /// Query execution failed.
    QueryFailed(String),

    /// Operation not supported.
    UnsupportedOperation(String),

    /// Invalid operation parameters.
    InvalidParameters(String),

    /// Transaction error.
    TransactionError(String),

    /// Cursor error.
    CursorError(String),
}

/// Configuration-specific errors.
#[derive(Debug)]
pub enum ConfigError {
    /// Config file not found.
    FileNotFound(String),

    /// Invalid config format.
    InvalidFormat(String),

    /// Missing required field.
    MissingField(String),

    /// Invalid field value.
    InvalidValue { field: String, value: String },
}

/// Authentication-specific errors.
#[derive(Debug)]
pub enum AuthError {
    /// Authentication failed.
    AuthenticationFailed(String),

    /// Invalid credentials.
    InvalidCredentials,

    /// Permission denied.
    PermissionDenied(String),

    /// Token expired.
    TokenExpired,
}

/// Plugin-specific errors.
#[derive(Debug)]
pub enum PluginError {
    /// Plugin not found.
    NotFound(String),

    /// Plugin load failed.
    LoadFailed(String),

    /// Plugin initialization failed.
    InitFailed(String),

    /// Plugin execution failed.
    ExecutionFailed(String),

    /// Invalid plugin.
    Invalid(String),
}

/// Script execution errors.
#[derive(Debug)]
pub enum ScriptError {
    /// Script file not found.
    FileNotFound(String),

    /// Script parse error.
    ParseError(String),

    /// Script runtime error.
    RuntimeError(String),

    /// Script timeout.
    Timeout,
}

/* ========================= Display & Error impls ========================= */

impl fmt::Display for MongoshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MongoshError::Connection(e) => write!(f, "Connection error: {e}"),
            MongoshError::Parse(e) => write!(f, "{e}"),
            MongoshError::Execution(e) => write!(f, "Execution error: {e}"),
            MongoshError::Config(e) => write!(f, "Configuration error: {e}"),
            MongoshError::Io(e) => write!(f, "I/O error: {e}"),
            MongoshError::MongoDb(e) => format_mongodb_error(f, e),
            MongoshError::Auth(e) => write!(f, "Authentication error: {e}"),
            MongoshError::Plugin(e) => write!(f, "Plugin error: {e}"),
            MongoshError::Script(e) => write!(f, "Script error: {e}"),
            MongoshError::Generic(msg) => write!(f, "{msg}"),
            MongoshError::NotImplemented(msg) => write!(f, "Not implemented: {msg}"),
        }
    }
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionError::ConnectionFailed(msg) => write!(f, "Failed to connect: {msg}"),
            ConnectionError::Timeout => write!(f, "Connection timeout"),
            ConnectionError::InvalidUri(uri) => write!(f, "Invalid connection URI: {uri}"),
            ConnectionError::Disconnected => write!(f, "Connection lost"),
            ConnectionError::PoolExhausted => write!(f, "Connection pool exhausted"),
            ConnectionError::NotConnected => write!(f, "Not connected to MongoDB"),
            ConnectionError::PingFailed(msg) => write!(f, "Ping failed: {msg}"),
            ConnectionError::CommandFailed(msg) => write!(f, "Command failed: {msg}"),
            ConnectionError::SessionFailed(msg) => {
                write!(f, "Session operation failed: {msg}")
            }
            ConnectionError::TransactionFailed(msg) => {
                write!(f, "Transaction operation failed: {msg}")
            }
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::SyntaxError(msg) => write!(f, "Syntax error: {msg}"),
            ParseError::InvalidCommand(cmd) => write!(f, "Invalid command: {cmd}"),
            ParseError::UnexpectedToken { expected, found } => {
                write!(f, "Expected '{expected}', found '{found}'")
            }
            ParseError::InvalidQuery(msg) => write!(f, "Invalid query: {msg}"),
            ParseError::InvalidPipeline(msg) => write!(f, "Invalid pipeline: {msg}"),
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::QueryFailed(msg) => write!(f, "Query failed: {msg}"),
            ExecutionError::UnsupportedOperation(op) => {
                write!(f, "Unsupported operation: {op}")
            }
            ExecutionError::InvalidParameters(msg) => write!(f, "Invalid parameters: {msg}"),
            ExecutionError::TransactionError(msg) => write!(f, "Transaction error: {msg}"),
            ExecutionError::CursorError(msg) => write!(f, "Cursor error: {msg}"),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::FileNotFound(path) => write!(f, "Config file not found: {path}"),
            ConfigError::InvalidFormat(msg) => write!(f, "Invalid config format: {msg}"),
            ConfigError::MissingField(field) => write!(f, "Missing required field: {field}"),
            ConfigError::InvalidValue { field, value } => {
                write!(f, "Invalid value '{value}' for field '{field}'")
            }
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::AuthenticationFailed(msg) => {
                write!(f, "Authentication failed: {msg}")
            }
            AuthError::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthError::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            AuthError::TokenExpired => write!(f, "Authentication token expired"),
        }
    }
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::NotFound(name) => write!(f, "Plugin not found: {name}"),
            PluginError::LoadFailed(msg) => write!(f, "Failed to load plugin: {msg}"),
            PluginError::InitFailed(msg) => write!(f, "Failed to initialize plugin: {msg}"),
            PluginError::ExecutionFailed(msg) => {
                write!(f, "Plugin execution failed: {msg}")
            }
            PluginError::Invalid(msg) => write!(f, "Invalid plugin: {msg}"),
        }
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::FileNotFound(path) => write!(f, "Script file not found: {path}"),
            ScriptError::ParseError(msg) => write!(f, "Script parse error: {msg}"),
            ScriptError::RuntimeError(msg) => write!(f, "Script runtime error: {msg}"),
            ScriptError::Timeout => write!(f, "Script execution timeout"),
        }
    }
}

impl std::error::Error for MongoshError {}
impl std::error::Error for ConnectionError {}
impl std::error::Error for ParseError {}
impl std::error::Error for ExecutionError {}
impl std::error::Error for ConfigError {}
impl std::error::Error for AuthError {}
impl std::error::Error for PluginError {}
impl std::error::Error for ScriptError {}

/* ========================= Conversions to MongoshError ========================= */

impl From<io::Error> for MongoshError {
    fn from(err: io::Error) -> Self {
        MongoshError::Io(err)
    }
}

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

impl From<AuthError> for MongoshError {
    fn from(err: AuthError) -> Self {
        MongoshError::Auth(err)
    }
}

impl From<PluginError> for MongoshError {
    fn from(err: PluginError) -> Self {
        MongoshError::Plugin(err)
    }
}

impl From<ScriptError> for MongoshError {
    fn from(err: ScriptError) -> Self {
        MongoshError::Script(err)
    }
}

impl From<String> for MongoshError {
    fn from(msg: String) -> Self {
        MongoshError::Generic(msg)
    }
}

impl From<&str> for MongoshError {
    fn from(msg: &str) -> Self {
        MongoshError::Generic(msg.to_owned())
    }
}

impl From<rustyline::error::ReadlineError> for MongoshError {
    fn from(err: rustyline::error::ReadlineError) -> Self {
        MongoshError::Generic(format!("Readline error: {err}"))
    }
}
