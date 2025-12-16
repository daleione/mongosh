//! Configuration management for mongosh
//!
//! This module handles loading, parsing, and managing configuration from various sources:
//! - Configuration files (TOML format)
//! - Environment variables
//! - Command-line arguments
//!
//! Configuration precedence (highest to lowest):
//! 1. Command-line arguments
//! 2. Environment variables
//! 3. Configuration file
//! 4. Default values

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::Result;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Connection configuration
    pub connection: ConnectionConfig,

    /// Display configuration
    pub display: DisplayConfig,

    /// History configuration
    pub history: HistoryConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Plugin configuration
    pub plugins: PluginConfig,
}

/// Connection-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Default MongoDB connection URI
    #[serde(default = "default_uri")]
    pub default_uri: String,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Number of retry attempts on connection failure
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,

    /// Maximum pool size
    #[serde(default = "default_max_pool_size")]
    pub max_pool_size: u32,

    /// Minimum pool size
    #[serde(default = "default_min_pool_size")]
    pub min_pool_size: u32,

    /// Connection idle timeout in seconds
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
}

/// Display and output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Output format (json, json-pretty, table, compact)
    #[serde(default = "default_format")]
    pub format: OutputFormat,

    /// Enable colored output
    #[serde(default = "default_color_output")]
    pub color_output: bool,

    /// Number of results per page
    #[serde(default = "default_page_size")]
    pub page_size: usize,

    /// Enable syntax highlighting
    #[serde(default = "default_syntax_highlighting")]
    pub syntax_highlighting: bool,

    /// Show execution time
    #[serde(default = "default_show_timing")]
    pub show_timing: bool,
}

/// Output format options
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Shell format (mongosh compatible)
    ///
    /// Uses MongoDB shell syntax with type wrappers:
    /// - ObjectId('...'), ISODate('...'), Long('...')
    /// - Pretty-printed nested documents and arrays
    /// - Colored output support
    Shell,

    /// Compact JSON format (single-line)
    ///
    /// Minified JSON without whitespace or indentation.
    /// Suitable for: logging, piping to other tools, minimal output
    /// Example: `{"_id":"123","name":"John"}`
    Json,

    /// Pretty-printed JSON format (multi-line)
    ///
    /// Human-readable JSON with indentation and newlines.
    /// Suitable for: terminal display, debugging, reading
    /// Example:
    /// ```json
    /// {
    ///   "_id": "123",
    ///   "name": "John"
    /// }
    /// ```
    JsonPretty,

    /// Table format (ASCII table layout)
    ///
    /// Displays documents as an ASCII table with columns.
    /// Suitable for: comparing multiple documents, structured data view
    Table,

    /// Compact summary format
    ///
    /// Displays only summary information, not full document content.
    /// Suitable for: quick checks, counting results
    /// Example: "5 document(s) returned"
    Compact,
}

/// Command history configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Maximum number of history entries
    #[serde(default = "default_max_history_size")]
    pub max_size: usize,

    /// Path to history file
    #[serde(default = "default_history_file")]
    pub file_path: PathBuf,

    /// Enable history persistence
    #[serde(default = "default_persist_history")]
    pub persist: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (error, warn, info, debug, trace)
    #[serde(default = "default_log_level")]
    pub level: LogLevel,

    /// Path to log file (None for stdout)
    #[serde(default)]
    pub file_path: Option<PathBuf>,

    /// Enable timestamps in logs
    #[serde(default = "default_log_timestamps")]
    pub timestamps: bool,
}

/// Log level options
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Plugin system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Enable plugin system
    #[serde(default = "default_plugins_enabled")]
    pub enabled: bool,

    /// Directory containing plugins
    #[serde(default = "default_plugin_directory")]
    pub directory: PathBuf,

    /// List of enabled plugin names
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
}

// Default value functions
fn default_uri() -> String {
    "mongodb://localhost:27017".to_string()
}

fn default_timeout() -> u64 {
    30
}

fn default_retry_attempts() -> u32 {
    3
}

fn default_max_pool_size() -> u32 {
    10
}

fn default_min_pool_size() -> u32 {
    2
}

fn default_idle_timeout() -> u64 {
    300
}

fn default_format() -> OutputFormat {
    OutputFormat::Shell // Shell format is the most user-friendly default
}

fn default_color_output() -> bool {
    true
}

fn default_page_size() -> usize {
    20
}

fn default_syntax_highlighting() -> bool {
    true
}

fn default_show_timing() -> bool {
    true
}

fn default_max_history_size() -> usize {
    1000
}

fn default_history_file() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mongosh_history")
}

fn default_persist_history() -> bool {
    true
}

fn default_log_level() -> LogLevel {
    LogLevel::Warn
}

fn default_log_timestamps() -> bool {
    true
}

fn default_plugins_enabled() -> bool {
    true
}

fn default_plugin_directory() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mongosh")
        .join("plugins")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            display: DisplayConfig::default(),
            history: HistoryConfig::default(),
            logging: LoggingConfig::default(),
            plugins: PluginConfig::default(),
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            default_uri: default_uri(),
            timeout: default_timeout(),
            retry_attempts: default_retry_attempts(),
            max_pool_size: default_max_pool_size(),
            min_pool_size: default_min_pool_size(),
            idle_timeout: default_idle_timeout(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            color_output: default_color_output(),
            page_size: default_page_size(),
            syntax_highlighting: default_syntax_highlighting(),
            show_timing: default_show_timing(),
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_size: default_max_history_size(),
            file_path: default_history_file(),
            persist: default_persist_history(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file_path: None,
            timestamps: default_log_timestamps(),
        }
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_plugins_enabled(),
            directory: default_plugin_directory(),
            enabled_plugins: Vec::new(),
        }
    }
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from a file
    ///
    /// # Arguments
    /// * `path` - Path to the configuration file (TOML format)
    ///
    /// # Returns
    /// * `Result<Config>` - Loaded configuration or error
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        todo!("Load configuration from TOML file")
    }

    /// Load configuration from multiple sources with proper precedence
    ///
    /// # Returns
    /// * `Result<Config>` - Merged configuration or error
    pub fn load() -> Result<Self> {
        todo!("Load configuration from all sources with precedence")
    }

    /// Load configuration from environment variables
    ///
    /// Environment variables are prefixed with MONGOSH_
    /// Example: MONGOSH_CONNECTION_TIMEOUT=60
    ///
    /// # Returns
    /// * `Result<Config>` - Configuration from environment or default
    pub fn from_env() -> Result<Self> {
        todo!("Load configuration from environment variables")
    }

    /// Get the default configuration file path
    ///
    /// # Returns
    /// * `PathBuf` - Path to default configuration file
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mongosh")
            .join("config.toml")
    }

    /// Save configuration to a file
    ///
    /// # Arguments
    /// * `path` - Path where to save the configuration
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        todo!("Save configuration to TOML file")
    }

    /// Merge this configuration with another, giving priority to the other
    ///
    /// # Arguments
    /// * `other` - Configuration to merge with (takes precedence)
    ///
    /// # Returns
    /// * `Config` - Merged configuration
    pub fn merge(&self, other: &Config) -> Config {
        todo!("Merge two configurations with proper precedence")
    }

    /// Validate the configuration
    ///
    /// # Returns
    /// * `Result<()>` - Ok if valid, error otherwise
    pub fn validate(&self) -> Result<()> {
        todo!("Validate configuration values")
    }

    /// Get connection timeout as Duration
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_secs(self.connection.timeout)
    }

    /// Get idle timeout as Duration
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.connection.idle_timeout)
    }
}

impl ConnectionConfig {
    /// Parse and validate the connection URI
    ///
    /// # Returns
    /// * `Result<()>` - Ok if URI is valid, error otherwise
    pub fn validate_uri(&self) -> Result<()> {
        todo!("Validate MongoDB connection URI format")
    }
}

impl LogLevel {
    /// Convert to tracing::Level
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self {
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Trace => tracing::Level::TRACE,
        }
    }
}

impl OutputFormat {
    /// Check if format requires pretty printing
    pub fn is_pretty(&self) -> bool {
        matches!(self, OutputFormat::JsonPretty | OutputFormat::Table)
    }

    /// Check if format is JSON-based
    pub fn is_json(&self) -> bool {
        matches!(self, OutputFormat::Json | OutputFormat::JsonPretty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.connection.default_uri, "mongodb://localhost:27017");
        assert_eq!(config.display.format, OutputFormat::Shell);
        assert!(config.display.color_output);
    }

    #[test]
    fn test_output_format_checks() {
        assert!(OutputFormat::JsonPretty.is_pretty());
        assert!(OutputFormat::JsonPretty.is_json());
        assert!(!OutputFormat::Compact.is_pretty());
        assert!(OutputFormat::Table.is_pretty());
    }

    #[test]
    fn test_connection_timeout() {
        let config = Config::default();
        assert_eq!(config.connection_timeout(), Duration::from_secs(30));
    }
}
