//! Command-line interface for mongosh
//!
//! This module handles:
//! - Command-line argument parsing using clap
//! - Configuration loading and validation
//! - Application initialization and startup
//! - Mode selection (interactive vs script execution)
//! - Connection string parsing

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::{Config, OutputFormat};
use crate::error::Result;

/// Extract database name from MongoDB connection URI
///
/// # Arguments
/// * `uri` - MongoDB connection URI
///
/// # Returns
/// * `Option<String>` - Database name if found in URI
fn extract_database_from_uri(uri: &str) -> Option<String> {
    // Parse URI to extract database name
    // Format: mongodb://[username:password@]host[:port][/database][?options]

    // Find the part after the last '/' and before '?'
    if let Some(after_slash) = uri.split("://").nth(1) {
        // Find the database part: after host/port and before query params
        if let Some(path_part) = after_slash.split('/').nth(1) {
            // Remove query parameters if any
            let db_name = path_part.split('?').next().unwrap_or("");
            if !db_name.is_empty() {
                return Some(db_name.to_string());
            }
        }
    }
    None
}

/// MongoDB Shell - A high-performance Rust implementation
#[derive(Parser, Debug)]
#[command(
    name = "mongosh",
    version,
    about = "MongoDB Shell written in Rust",
    long_about = "A high-performance MongoDB Shell implementation in Rust with support for
interactive REPL, script execution, and all MongoDB operations."
)]
pub struct CliArgs {
    /// MongoDB connection URI
    ///
    /// Format: mongodb://[username:password@]host[:port][/database][?options]
    #[arg(value_name = "URI")]
    pub uri: Option<String>,

    /// Datasource name from config file
    ///
    /// Use a named datasource defined in the config file.
    /// Example: mongosh -d card_prod
    #[arg(short = 'd', long, value_name = "NAME")]
    pub datasource: Option<String>,

    /// Server to connect to
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Port to connect to
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Database name to use
    #[arg(long, value_name = "NAME")]
    pub database: Option<String>,

    /// Username for authentication
    #[arg(short = 'u', long, value_name = "USERNAME")]
    pub username: Option<String>,

    /// Password for authentication
    #[arg(short = 'p', long, value_name = "PASSWORD")]
    pub password: Option<String>,

    /// Authentication database
    #[arg(long, value_name = "NAME", default_value = "admin")]
    pub auth_database: String,

    /// Configuration file path
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    /// Output format (json, json-pretty, table, compact)
    #[arg(long, value_name = "FORMAT")]
    pub format: Option<String>,

    /// Disable colored output
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Quiet mode (minimal output)
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Verbose mode (detailed logging)
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Very verbose mode (debug logging)
    #[arg(long = "vv")]
    pub very_verbose: bool,

    /// Connection timeout in seconds
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<u64>,

    /// Disable automatic connection
    #[arg(long)]
    pub no_connect: bool,

    /// Enable TLS/SSL
    #[arg(long)]
    pub tls: bool,

    /// TLS certificate file
    #[arg(long, value_name = "FILE")]
    pub tls_cert_file: Option<PathBuf>,

    /// TLS CA certificate file
    #[arg(long, value_name = "FILE")]
    pub tls_ca_file: Option<PathBuf>,

    /// Disable TLS certificate validation
    #[arg(long)]
    pub tls_insecure: bool,

    /// Subcommands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Subcommands for mongosh
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show version information
    Version,

    /// Generate shell completion script
    Completion {
        /// Shell type (bash, zsh, fish, powershell)
        #[arg(value_name = "SHELL")]
        shell: String,
    },

    /// Show configuration
    Config {
        /// Show effective configuration
        #[arg(long)]
        show: bool,

        /// Validate configuration file
        #[arg(long)]
        validate: bool,
    },
}

/// CLI interface handler
pub struct CliInterface {
    /// Parsed command-line arguments
    args: CliArgs,

    /// Loaded configuration
    config: Config,
}

impl CliInterface {
    /// Create a new CLI interface
    ///
    /// # Returns
    /// * `Result<Self>` - New CLI interface or error
    pub fn new() -> Result<Self> {
        let args = CliArgs::parse();
        let config = Self::load_config(&args)?;

        Ok(Self { args, config })
    }

    /// Load configuration from file and merge with arguments
    ///
    /// # Arguments
    /// * `args` - Command-line arguments
    ///
    /// # Returns
    /// * `Result<Config>` - Loaded configuration or error
    fn load_config(args: &CliArgs) -> Result<Config> {
        // Load config from file (or create default if not exists)
        let config_path = args.config_file.as_deref();
        let mut config = Config::load_from_file(config_path)?;

        // Validate loaded configuration
        if let Err(e) = config.validate() {
            eprintln!("Warning: Configuration validation failed: {}", e);
            eprintln!("Using default configuration instead.");
            config = Config::default();
        }

        // Apply CLI arguments to override config values
        Self::apply_args_to_config(&mut config, args);

        Ok(config)
    }

    /// Get the MongoDB connection URI
    ///
    /// Determines the connection URI with the following priority:
    /// 1. Datasource from config (if -d/--datasource is specified)
    /// 2. Explicit URI argument
    /// 3. Build from individual connection arguments (--host, --port, etc.)
    /// 4. Default datasource from config
    ///
    /// # Returns
    /// * `String` - Connection URI
    pub fn get_connection_uri(&self) -> String {
        // Priority 1: Check if datasource is specified via -d flag
        if let Some(ref datasource_name) = self.args.datasource {
            if let Some(uri) = self.config.connection.get_datasource(Some(datasource_name)) {
                return uri;
            } else {
                eprintln!(
                    "Warning: Datasource '{}' not found in config",
                    datasource_name
                );
                eprintln!(
                    "Available datasources: {}",
                    self.config.connection.list_datasources().join(", ")
                );
            }
        }

        // Priority 2: Explicit URI from command line
        if let Some(uri) = &self.args.uri {
            return uri.clone();
        }

        // Priority 3: Check if we should build from individual args
        if self.args.host.is_some() || self.args.username.is_some() {
            return self.build_connection_uri();
        }

        // Priority 4: Use default datasource from config
        if let Some(uri) = self.config.connection.get_datasource(None) {
            return uri;
        }

        // Final fallback: build default URI
        self.build_connection_uri()
    }

    /// Get sanitized connection URI for display (hides credentials)
    ///
    /// # Returns
    /// * `String` - Sanitized URI with credentials replaced by ***
    pub fn get_sanitized_connection_uri(&self) -> String {
        let uri = self.get_connection_uri();
        Self::sanitize_uri(&uri)
    }

    /// Sanitize URI by hiding credentials
    ///
    /// # Arguments
    /// * `uri` - The URI to sanitize
    ///
    /// # Returns
    /// * `String` - Sanitized URI
    fn sanitize_uri(uri: &str) -> String {
        // Hide everything between :// and @
        if let Some(proto_end) = uri.find("://") {
            if let Some(host_start) = uri.find('@') {
                let proto = &uri[..proto_end + 3];
                let host = &uri[host_start..];
                return format!("{}***{}", proto, host);
            }
        }
        // If no @ found but contains credentials pattern, hide it
        if uri.contains('@') {
            "mongodb://***".to_string()
        } else {
            uri.to_string()
        }
    }

    /// Build connection URI from individual arguments
    ///
    /// Constructs a MongoDB connection URI from CLI arguments including:
    /// - Authentication credentials (username/password)
    /// - Host and port (defaults to localhost:27017)
    /// - Database name
    /// - Authentication database (via authSource parameter)
    /// - TLS/SSL options
    ///
    /// # Returns
    /// * `String` - Constructed connection URI in the format:
    ///   `mongodb://[username:password@]host:port[/database][?options]`
    ///
    /// # Examples
    /// ```
    /// // Default: mongodb://localhost:27017
    /// // With auth: mongodb://user:pass@localhost:27017/?authSource=admin
    /// // Full: mongodb://user:pass@host:port/db?authSource=admin&tls=true
    /// ```
    fn build_connection_uri(&self) -> String {
        let mut uri = String::from("mongodb://");

        // Add authentication credentials if provided
        if let Some(username) = &self.args.username {
            uri.push_str(username);
            if let Some(password) = &self.args.password {
                uri.push(':');
                uri.push_str(password);
            }
            uri.push('@');
        }

        // Add host (default to localhost if not provided)
        let host = self.args.host.as_deref().unwrap_or("localhost");
        uri.push_str(host);

        // Add port (default to 27017 if not provided)
        let port = self.args.port.unwrap_or(27017);
        uri.push(':');
        uri.push_str(&port.to_string());

        // Add database if provided
        if let Some(db) = &self.args.database {
            uri.push('/');
            uri.push_str(db);
        }

        // Add authentication database as query parameter if username is provided
        // and auth_database is not the default or different from database
        if self.args.username.is_some() {
            let needs_auth_db_param = if let Some(db) = &self.args.database {
                // If database is specified, add authSource only if different
                &self.args.auth_database != db
            } else {
                // If no database specified, add authSource if not default "admin"
                true
            };

            if needs_auth_db_param {
                if self.args.database.is_some() {
                    uri.push_str("?authSource=");
                } else {
                    uri.push_str("/?authSource=");
                }
                uri.push_str(&self.args.auth_database);
            }
        }

        // Add TLS options if enabled
        if self.args.tls {
            let separator = if uri.contains('?') { "&" } else { "?" };
            uri.push_str(separator);
            uri.push_str("tls=true");

            if self.args.tls_insecure {
                uri.push_str("&tlsAllowInvalidCertificates=true");
            }
        }

        uri
    }

    /// Get the database name to use
    ///
    /// Priority:
    /// 1. --database/-d command line argument
    /// 2. Database name from connection URI
    /// 3. Default to "test"
    ///
    /// # Returns
    /// * `String` - Database name
    pub fn get_database(&self) -> String {
        // First check if database is explicitly provided via CLI argument
        if let Some(db) = &self.args.database {
            return db.clone();
        }

        // Then try to extract from the actual connection URI being used
        let uri = self.get_connection_uri();
        if let Some(db) = extract_database_from_uri(&uri) {
            return db;
        }

        // Finally, fall back to default
        "test".to_string()
    }

    /// Get the configuration
    ///
    /// # Returns
    /// * `&Config` - Reference to configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the CLI arguments
    ///
    /// # Returns
    /// * `&CliArgs` - Reference to arguments
    pub fn args(&self) -> &CliArgs {
        &self.args
    }

    /// Apply CLI arguments to configuration
    ///
    /// Overrides configuration values with CLI arguments where provided
    ///
    /// # Arguments
    /// * `config` - Configuration to modify
    fn apply_args_to_config(config: &mut Config, args: &CliArgs) {
        Self::apply_display_args(config, args);
        Self::apply_logging_args(config, args);
        Self::apply_connection_args(config, args);
    }

    /// Apply display-related CLI arguments to configuration
    fn apply_display_args(config: &mut Config, args: &CliArgs) {
        if let Some(format_str) = &args.format {
            config.display.format = Self::parse_output_format(format_str);
        }

        if args.no_color {
            config.display.color_output = false;
        }
    }

    /// Apply logging-related CLI arguments to configuration
    fn apply_logging_args(config: &mut Config, args: &CliArgs) {
        use crate::config::LogLevel;

        config.logging.level = if args.very_verbose {
            LogLevel::Trace
        } else if args.verbose {
            LogLevel::Debug
        } else if args.quiet {
            LogLevel::Error
        } else {
            config.logging.level
        };
    }

    /// Apply connection-related CLI arguments to configuration
    fn apply_connection_args(config: &mut Config, args: &CliArgs) {
        if let Some(timeout) = args.timeout {
            config.connection.timeout = timeout;
        }
    }

    /// Parse output format string
    fn parse_output_format(format_str: &str) -> OutputFormat {
        match format_str.to_lowercase().as_str() {
            "shell" => OutputFormat::Shell,
            "json" => OutputFormat::Json,
            "json-pretty" | "jsonpretty" => OutputFormat::JsonPretty,
            "table" => OutputFormat::Table,
            "compact" => OutputFormat::Compact,
            _ => {
                eprintln!("Warning: Unknown format '{}', using default", format_str);
                OutputFormat::Shell
            }
        }
    }

    /// Validate configuration and arguments

    /// Handle subcommands
    ///
    /// # Returns
    /// * `Result<bool>` - True if subcommand was handled, false to continue
    pub async fn handle_subcommand(&self) -> Result<bool> {
        match &self.args.command {
            Some(Commands::Version) => {
                self.show_version();
                Ok(true)
            }
            Some(Commands::Completion { shell }) => {
                self.generate_completion(shell)?;
                Ok(true)
            }
            Some(Commands::Config { show, validate }) => {
                self.handle_config_command(*show, *validate)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Show version information
    fn show_version(&self) {
        println!("mongosh version {}", env!("CARGO_PKG_VERSION"));
        println!("Rust version: {}", env!("CARGO_PKG_RUST_VERSION"));
    }

    /// Generate shell completion script
    ///
    /// # Arguments
    /// * `shell` - Shell type
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    fn generate_completion(&self, _shell: &str) -> Result<()> {
        todo!("Generate shell completion script for specified shell")
    }

    /// Handle config subcommand
    ///
    /// # Arguments
    /// * `show` - Whether to show configuration
    /// * `validate` - Whether to validate configuration
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    fn handle_config_command(&self, show: bool, validate: bool) -> Result<()> {
        if validate {
            self.validate_config_file()?;
        }

        if show {
            self.show_config()?;
        }

        Ok(())
    }

    /// Validate configuration file
    fn validate_config_file(&self) -> Result<()> {
        let path = self.get_config_path();
        println!("Validating configuration file: {}", path.display());

        if !path.exists() {
            println!("❌ Configuration file does not exist");
            return Ok(());
        }

        match Config::load_from_file(self.args.config_file.as_deref()) {
            Ok(config) => match config.validate() {
                Ok(_) => println!("✅ Configuration is valid"),
                Err(e) => println!("❌ Configuration validation failed: {}", e),
            },
            Err(e) => println!("❌ Failed to load configuration: {}", e),
        }

        Ok(())
    }

    /// Show effective configuration
    fn show_config(&self) -> Result<()> {
        let path = self.get_config_path();
        println!("Configuration file: {}", path.display());
        println!();
        println!("=== Effective Configuration ===");
        println!();

        match self.config.to_toml_with_comments() {
            Ok(toml_str) => println!("{}", toml_str),
            Err(e) => {
                eprintln!("Error formatting configuration: {}", e);
                println!("{:#?}", self.config);
            }
        }

        Ok(())
    }

    /// Get configuration file path (from args or default)
    fn get_config_path(&self) -> PathBuf {
        self.args
            .config_file
            .as_ref()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Config::default_config_path)
    }

    /// Print banner with version and connection info
    pub fn print_banner(&self) {
        if !self.args.quiet {
            println!("Connecting to: {}", self.get_sanitized_connection_uri());
            println!("Using Mongosh: {}", env!("CARGO_PKG_VERSION"));
        }
    }

    /// Print version information after connection
    ///
    /// # Arguments
    /// * `mongodb_version` - MongoDB server version
    pub fn print_connection_info(&self, mongodb_version: &str) {
        if !self.args.quiet {
            println!("Using MongoDB: {}", mongodb_version);
        }
    }
}

impl Default for CliInterface {
    fn default() -> Self {
        Self::new().expect("Failed to create CLI interface")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_args_parsing() {
        // Test with no arguments
        let args = CliArgs::try_parse_from(vec!["mongosh"]).unwrap();
        assert!(args.uri.is_none());
        assert!(args.database.is_none());
    }

    #[test]
    fn test_cli_args_with_uri() {
        let args = CliArgs::try_parse_from(vec!["mongosh", "mongodb://localhost:27017"]).unwrap();
        assert_eq!(args.uri, Some("mongodb://localhost:27017".to_string()));
    }

    #[test]
    fn test_cli_args_with_flags() {
        let args = CliArgs::try_parse_from(vec!["mongosh", "--no-color", "--quiet"]).unwrap();
        assert!(args.no_color);
        assert!(args.quiet);
    }

    #[test]
    fn test_extract_database_from_uri() {
        assert_eq!(
            extract_database_from_uri("mongodb://localhost:27017/mydb"),
            Some("mydb".to_string())
        );
        assert_eq!(
            extract_database_from_uri("mongodb://localhost:27017/mydb?retryWrites=true"),
            Some("mydb".to_string())
        );
        assert_eq!(
            extract_database_from_uri("mongodb://user:pass@localhost:27017/admin"),
            Some("admin".to_string())
        );
        assert_eq!(extract_database_from_uri("mongodb://localhost:27017"), None);
        assert_eq!(
            extract_database_from_uri("mongodb://localhost:27017/"),
            None
        );
    }

    #[test]
    fn test_get_database_priority() {
        // Test with explicit database argument
        let args = CliArgs::try_parse_from(vec!["mongosh", "--database", "mydb"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.get_database(), "mydb");

        // Test with database in URI
        let args = CliArgs::try_parse_from(vec!["mongosh", "mongodb://localhost/admin"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.get_database(), "admin");

        // Test explicit argument overrides URI
        let args = CliArgs::try_parse_from(vec![
            "mongosh",
            "mongodb://localhost/test",
            "--database",
            "mydb",
        ])
        .unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.get_database(), "mydb");

        // Test default
        let args = CliArgs::try_parse_from(vec!["mongosh"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.get_database(), "test");
    }

    #[test]
    fn test_build_connection_uri_defaults() {
        // Test with no arguments - should use default host and port
        let args = CliArgs::try_parse_from(vec!["mongosh"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.build_connection_uri(), "mongodb://localhost:27017");
    }

    #[test]
    fn test_build_connection_uri_with_host() {
        // Test with custom host
        let args = CliArgs::try_parse_from(vec!["mongosh", "--host", "192.168.0.5"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.build_connection_uri(), "mongodb://192.168.0.5:27017");
    }

    #[test]
    fn test_build_connection_uri_with_host_and_port() {
        // Test with custom host and port
        let args =
            CliArgs::try_parse_from(vec!["mongosh", "--host", "192.168.0.5", "--port", "9999"])
                .unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.build_connection_uri(), "mongodb://192.168.0.5:9999");
    }

    #[test]
    fn test_build_connection_uri_with_database() {
        // Test with database
        let args = CliArgs::try_parse_from(vec!["mongosh", "--database", "mydb"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.build_connection_uri(), "mongodb://localhost:27017/mydb");
    }

    #[test]
    fn test_build_connection_uri_with_auth() {
        // Test with username and password
        let args =
            CliArgs::try_parse_from(vec!["mongosh", "-u", "admin", "-p", "password123"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(
            cli.build_connection_uri(),
            "mongodb://admin:password123@localhost:27017/?authSource=admin"
        );
    }

    #[test]
    fn test_build_connection_uri_with_auth_and_database() {
        // Test with username, password, and database
        let args = CliArgs::try_parse_from(vec![
            "mongosh",
            "-u",
            "user",
            "-p",
            "pass",
            "--database",
            "mydb",
        ])
        .unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(
            cli.build_connection_uri(),
            "mongodb://user:pass@localhost:27017/mydb?authSource=admin"
        );
    }

    #[test]
    fn test_build_connection_uri_with_auth_database() {
        // Test with custom auth database
        let args = CliArgs::try_parse_from(vec![
            "mongosh",
            "-u",
            "user",
            "-p",
            "pass",
            "--database",
            "mydb",
            "--auth-database",
            "auth_db",
        ])
        .unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(
            cli.build_connection_uri(),
            "mongodb://user:pass@localhost:27017/mydb?authSource=auth_db"
        );
    }

    #[test]
    fn test_build_connection_uri_with_tls() {
        // Test with TLS enabled
        let args = CliArgs::try_parse_from(vec!["mongosh", "--tls"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(
            cli.build_connection_uri(),
            "mongodb://localhost:27017?tls=true"
        );
    }

    #[test]
    fn test_build_connection_uri_with_tls_insecure() {
        // Test with TLS and insecure option
        let args = CliArgs::try_parse_from(vec!["mongosh", "--tls", "--tls-insecure"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(
            cli.build_connection_uri(),
            "mongodb://localhost:27017?tls=true&tlsAllowInvalidCertificates=true"
        );
    }

    #[test]
    fn test_build_connection_uri_complete() {
        // Test with all parameters
        let args = CliArgs::try_parse_from(vec![
            "mongosh",
            "--host",
            "192.168.0.5",
            "--port",
            "9999",
            "-u",
            "admin",
            "-p",
            "secret",
            "--database",
            "testdb",
            "--tls",
        ])
        .unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(
            cli.build_connection_uri(),
            "mongodb://admin:secret@192.168.0.5:9999/testdb?authSource=admin&tls=true"
        );
    }

    #[test]
    fn test_build_connection_uri_username_only() {
        // Test with username but no password
        let args = CliArgs::try_parse_from(vec!["mongosh", "-u", "admin"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(
            cli.build_connection_uri(),
            "mongodb://admin@localhost:27017/?authSource=admin"
        );
    }

    #[test]
    fn test_get_connection_uri_prefers_explicit_uri() {
        // Test that explicit URI takes precedence
        let args = CliArgs::try_parse_from(vec![
            "mongosh",
            "mongodb://example.com:27017/db",
            "--host",
            "localhost",
        ])
        .unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.get_connection_uri(), "mongodb://example.com:27017/db");
    }

    #[test]
    fn test_sanitize_uri_with_credentials() {
        let uri = "mongodb://user:password@localhost:27017/db";
        let sanitized = CliInterface::sanitize_uri(uri);
        assert_eq!(sanitized, "mongodb://***@localhost:27017/db");
        assert!(!sanitized.contains("password"));
        assert!(!sanitized.contains("user"));
    }

    #[test]
    fn test_sanitize_uri_without_credentials() {
        let uri = "mongodb://localhost:27017/db";
        let sanitized = CliInterface::sanitize_uri(uri);
        assert_eq!(sanitized, "mongodb://localhost:27017/db");
    }

    #[test]
    fn test_sanitize_uri_srv_with_credentials() {
        let uri = "mongodb+srv://myuser:mypass@cluster0.ab123.mongodb.net/myFirstDatabase";
        let sanitized = CliInterface::sanitize_uri(uri);
        assert_eq!(
            sanitized,
            "mongodb+srv://***@cluster0.ab123.mongodb.net/myFirstDatabase"
        );
        assert!(!sanitized.contains("myuser"));
        assert!(!sanitized.contains("mypass"));
    }

    #[test]
    fn test_get_sanitized_connection_uri() {
        let args = CliArgs::try_parse_from(vec![
            "mongosh",
            "mongodb://admin:secret@localhost:27017/testdb",
        ])
        .unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        let sanitized = cli.get_sanitized_connection_uri();
        assert!(!sanitized.contains("admin"));
        assert!(!sanitized.contains("secret"));
        assert!(sanitized.contains("***"));
    }
}
