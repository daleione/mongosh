//! Configuration management for mongosh
//!
//! This module handles loading, parsing, and managing configuration from various sources:
//! - Configuration files (TOML format)
//! - Environment variables
//! - Command-line arguments
//!
//! Configuration precedence (highest to lowest):
//! 1. Command-line arguments
//! 2. Configuration file
//! 3. Default values

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{ConfigError, MongoshError, Result};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Connection configuration
    #[serde(default)]
    pub connection: ConnectionConfig,

    /// Display configuration
    #[serde(default)]
    pub display: DisplayConfig,

    /// History configuration
    #[serde(default)]
    pub history: HistoryConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Named query
    #[serde(default)]
    pub named_query: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            display: DisplayConfig::default(),
            history: HistoryConfig::default(),
            logging: LoggingConfig::default(),
            named_query: HashMap::new(),
        }
    }
}

impl Config {
    /// Get the default configuration file path (~/.mongoshrc)
    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mongoshrc")
    }

    /// Load configuration from file
    ///
    /// # Arguments
    /// * `path` - Path to configuration file (None for default path)
    ///
    /// # Returns
    /// * `Result<Self>` - Loaded configuration or error
    pub fn load_from_file(path: Option<&Path>) -> Result<Self> {
        let config_path = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_config_path);

        // If config file doesn't exist, create default one and return default config
        if !config_path.exists() {
            let default_config = Self::default();
            if let Err(e) = default_config.save_to_file(Some(&config_path)) {
                eprintln!("Warning: Failed to create default config file: {}", e);
            }
            return Ok(default_config);
        }

        // Read and parse config file
        let content = fs::read_to_string(&config_path).map_err(|e| {
            MongoshError::Config(ConfigError::Generic(format!(
                "Failed to read config file '{}': {}",
                config_path.display(),
                e
            )))
        })?;

        let config: Config = toml::from_str(&content).map_err(|e| {
            MongoshError::Config(ConfigError::Generic(format!(
                "Failed to parse config file '{}': {}",
                config_path.display(),
                e
            )))
        })?;

        Ok(config)
    }

    /// Save configuration to file
    ///
    /// # Arguments
    /// * `path` - Path to save configuration (None for default path)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn save_to_file(&self, path: Option<&Path>) -> Result<()> {
        let config_path = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_config_path);

        // Generate TOML content with comments
        let toml_content = self.to_toml_with_comments()?;

        // Write to file
        let mut file = fs::File::create(&config_path).map_err(|e| {
            MongoshError::Config(ConfigError::Generic(format!(
                "Failed to create config file '{}': {}",
                config_path.display(),
                e
            )))
        })?;

        file.write_all(toml_content.as_bytes()).map_err(|e| {
            MongoshError::Config(ConfigError::Generic(format!(
                "Failed to write config file '{}': {}",
                config_path.display(),
                e
            )))
        })?;

        Ok(())
    }

    /// Convert configuration to TOML string with comments
    pub fn to_toml_with_comments(&self) -> Result<String> {
        // Load the default template with comments
        const TEMPLATE: &str = include_str!("../../config.default.toml");

        // Parse template as a Document (preserves comments and formatting)
        let mut doc = TEMPLATE.parse::<toml_edit::DocumentMut>().map_err(|e| {
            MongoshError::Config(ConfigError::Generic(format!(
                "Failed to parse template: {}",
                e
            )))
        })?;

        // Update values in the document
        Self::update_toml_document(&mut doc, self)?;

        Ok(doc.to_string())
    }

    /// Update TOML document with current configuration values
    fn update_toml_document(doc: &mut toml_edit::DocumentMut, config: &Config) -> Result<()> {
        Self::update_section(doc, "connection", |table| {
            // Update datasources
            let mut datasources = toml_edit::Table::new();
            for (name, uri) in &config.connection.datasources {
                datasources[name] = toml_edit::value(uri.as_str());
            }
            table["datasources"] = toml_edit::Item::Table(datasources);

            // Update default_datasource
            if let Some(ref default_ds) = config.connection.default_datasource {
                table["default_datasource"] = toml_edit::value(default_ds.as_str());
            }

            table["timeout"] = toml_edit::value(config.connection.timeout as i64);
            table["retry_attempts"] = toml_edit::value(config.connection.retry_attempts as i64);
            table["max_pool_size"] = toml_edit::value(config.connection.max_pool_size as i64);
            table["min_pool_size"] = toml_edit::value(config.connection.min_pool_size as i64);
            table["idle_timeout"] = toml_edit::value(config.connection.idle_timeout as i64);
        });

        Self::update_section(doc, "display", |table| {
            table["format"] = toml_edit::value(config.display.format.as_str());
            table["color_output"] = toml_edit::value(config.display.color_output);
            table["page_size"] = toml_edit::value(config.display.page_size as i64);
            table["syntax_highlighting"] = toml_edit::value(config.display.syntax_highlighting);
            table["show_timing"] = toml_edit::value(config.display.show_timing);
            table["json_indent"] = toml_edit::value(config.display.json_indent as i64);
        });

        Self::update_section(doc, "history", |table| {
            table["max_size"] = toml_edit::value(config.history.max_size as i64);
            table["file_path"] = toml_edit::value(config.history.file_path.display().to_string());
            table["persist"] = toml_edit::value(config.history.persist);
        });

        Self::update_section(doc, "logging", |table| {
            table["level"] = toml_edit::value(config.logging.level.as_str());
            table["timestamps"] = toml_edit::value(config.logging.timestamps);
            if let Some(ref path) = config.logging.file_path {
                table["file_path"] = toml_edit::value(path.display().to_string());
            }
        });

        Self::update_section(doc, "named_query", |table| {
            for (name, query) in &config.named_query {
                table[name] = toml_edit::value(query.as_str());
            }
        });

        Ok(())
    }

    /// Helper to update a TOML section
    fn update_section<F>(doc: &mut toml_edit::DocumentMut, section: &str, updater: F)
    where
        F: FnOnce(&mut toml_edit::Table),
    {
        if let Some(table) = doc.get_mut(section).and_then(|v| v.as_table_mut()) {
            updater(table);
        }
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        Self::validate_range(self.connection.timeout, 1, 300, "Connection timeout")?;
        Self::validate_range(self.connection.retry_attempts, 0, 10, "Retry attempts")?;
        Self::validate_range(self.connection.max_pool_size, 1, 100, "Max pool size")?;
        Self::validate_range(self.connection.idle_timeout, 60, 3600, "Idle timeout")?;

        if self.connection.min_pool_size > self.connection.max_pool_size {
            return Err(Self::config_error(
                "Min pool size cannot be greater than max pool size",
            ));
        }

        Self::validate_range(self.display.page_size, 1, 1000, "Page size")?;
        Self::validate_range(self.display.json_indent, 0, 8, "JSON indent")?;
        Self::validate_range(self.history.max_size, 0, 10000, "Max history size")?;

        Ok(())
    }

    /// Helper to validate numeric ranges
    fn validate_range<T>(value: T, min: T, max: T, field_name: &str) -> Result<()>
    where
        T: PartialOrd + std::fmt::Display,
    {
        if value < min || value > max {
            return Err(Self::config_error(&format!(
                "{} must be between {} and {}",
                field_name, min, max
            )));
        }
        Ok(())
    }

    /// Helper to create configuration errors
    fn config_error(msg: &str) -> MongoshError {
        MongoshError::Config(ConfigError::Generic(msg.to_string()))
    }
}

/// Connection-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Named datasources with their connection URIs
    /// Example: {"card_prod": "mongodb://prod:27017", "card_dev": "mongodb://dev:27017"}
    #[serde(default = "default_datasources")]
    pub datasources: HashMap<String, String>,

    /// Default datasource name to use when not specified
    #[serde(default = "default_datasource_name")]
    pub default_datasource: Option<String>,

    /// Deprecated: Default MongoDB connection URI (for backward compatibility)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_uri: Option<String>,

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

impl ConnectionConfig {
    /// Get datasource URI by name, with fallback logic
    ///
    /// # Arguments
    /// * `name` - Optional datasource name
    ///
    /// # Returns
    /// * `Option<String>` - URI if found, None otherwise
    pub fn get_datasource(&self, name: Option<&str>) -> Option<String> {
        // If a specific name is provided, look it up
        if let Some(ds_name) = name {
            if let Some(uri) = self.datasources.get(ds_name) {
                return Some(uri.clone());
            }
            return None;
        }

        // Try default datasource
        if let Some(ref default_name) = self.default_datasource {
            if let Some(uri) = self.datasources.get(default_name) {
                return Some(uri.clone());
            }
        }

        // Fallback to legacy default_uri for backward compatibility
        if let Some(ref uri) = self.default_uri {
            return Some(uri.clone());
        }

        // If only one datasource exists, use it
        if self.datasources.len() == 1 {
            return self.datasources.values().next().cloned();
        }

        None
    }

    /// List all available datasource names
    pub fn list_datasources(&self) -> Vec<String> {
        let mut names: Vec<String> = self.datasources.keys().cloned().collect();
        names.sort();
        names
    }
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

    /// JSON indentation (number of spaces)
    #[serde(default = "default_json_indent")]
    pub json_indent: usize,
}

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl OutputFormat {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Shell => "shell",
            OutputFormat::Json => "json",
            OutputFormat::JsonPretty => "json-pretty",
            OutputFormat::Table => "table",
            OutputFormat::Compact => "compact",
        }
    }
}

impl serde::Serialize for OutputFormat {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for OutputFormat {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "shell" => Ok(OutputFormat::Shell),
            "json" => Ok(OutputFormat::Json),
            "json-pretty" | "jsonpretty" | "json_pretty" => Ok(OutputFormat::JsonPretty),
            "table" => Ok(OutputFormat::Table),
            "compact" => Ok(OutputFormat::Compact),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["shell", "json", "json-pretty", "table", "compact"],
            )),
        }
    }
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

impl LogLevel {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

// Default value functions for serde
#[inline]
fn default_datasources() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("local".to_string(), "mongodb://localhost:27017".to_string());
    map
}

#[inline]
fn default_datasource_name() -> Option<String> {
    Some("local".to_string())
}

#[inline]
fn default_timeout() -> u64 {
    30
}

#[inline]
fn default_retry_attempts() -> u32 {
    3
}

#[inline]
fn default_max_pool_size() -> u32 {
    10
}

#[inline]
fn default_min_pool_size() -> u32 {
    2
}

#[inline]
fn default_idle_timeout() -> u64 {
    300
}

#[inline]
fn default_format() -> OutputFormat {
    OutputFormat::Shell
}

#[inline]
fn default_color_output() -> bool {
    true
}

#[inline]
fn default_page_size() -> usize {
    20
}

#[inline]
fn default_syntax_highlighting() -> bool {
    true
}

#[inline]
fn default_show_timing() -> bool {
    true
}

#[inline]
fn default_json_indent() -> usize {
    2
}

#[inline]
fn default_max_history_size() -> usize {
    1000
}

#[inline]
fn default_history_file() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mongosh_history")
}

#[inline]
fn default_persist_history() -> bool {
    true
}

#[inline]
fn default_log_level() -> LogLevel {
    LogLevel::Warn
}

#[inline]
fn default_log_timestamps() -> bool {
    true
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            datasources: default_datasources(),
            default_datasource: default_datasource_name(),
            default_uri: None,
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
            json_indent: default_json_indent(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(
            config.connection.datasources.get("local"),
            Some(&"mongodb://localhost:27017".to_string())
        );
        assert_eq!(
            config.connection.default_datasource,
            Some("local".to_string())
        );
        assert_eq!(config.display.format, OutputFormat::Shell);
        assert!(config.display.color_output);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        // Test invalid timeout
        config.connection.timeout = 0;
        assert!(config.validate().is_err());

        // Test invalid pool size
        config = Config::default();
        config.connection.min_pool_size = 20;
        config.connection.max_pool_size = 10;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_toml_serialization() {
        let config = Config::default();
        let toml_str = config.to_toml_with_comments().unwrap();
        assert!(toml_str.contains("[connection]"));
        assert!(toml_str.contains("[display]"));
        assert!(toml_str.contains("[history]"));
        assert!(toml_str.contains("[logging]"));
    }
}
