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

    /// Server to connect to
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Port to connect to
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Database name to use
    #[arg(short = 'd', long, value_name = "NAME")]
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

    /// Execute JavaScript file
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    pub script_file: Option<PathBuf>,

    /// Evaluate JavaScript expression
    #[arg(long = "eval", value_name = "EXPR")]
    pub eval: Option<String>,

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

    /// Parse command-line arguments
    ///
    /// # Returns
    /// * `CliArgs` - Parsed arguments
    pub fn parse_args() -> CliArgs {
        CliArgs::parse()
    }

    /// Load configuration from file and merge with arguments
    ///
    /// # Arguments
    /// * `args` - Command-line arguments
    ///
    /// # Returns
    /// * `Result<Config>` - Loaded configuration or error
    fn load_config(args: &CliArgs) -> Result<Config> {
        // Load default config
        let mut config = Config::default();

        // Apply CLI arguments to override config values
        if let Some(format_str) = &args.format {
            // Parse format string to OutputFormat
            config.display.format = match format_str.to_lowercase().as_str() {
                "shell" => OutputFormat::Shell,
                "json" => OutputFormat::Json,
                "json-pretty" | "jsonpretty" => OutputFormat::JsonPretty,
                "table" => OutputFormat::Table,
                "compact" => OutputFormat::Compact,
                _ => {
                    eprintln!("Warning: Unknown format '{}', using default", format_str);
                    OutputFormat::Shell
                }
            };
        }

        // Apply no-color flag
        if args.no_color {
            config.display.color_output = false;
        }

        Ok(config)
    }

    /// Run the CLI application
    ///
    /// This is the main entry point that determines the execution mode
    /// and starts either the REPL or script execution.
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn run(&self) -> Result<()> {
        todo!("Determine execution mode and start appropriate handler")
    }

    /// Get the MongoDB connection URI
    ///
    /// Constructs the URI from arguments or uses default from config
    ///
    /// # Returns
    /// * `String` - Connection URI
    pub fn get_connection_uri(&self) -> String {
        if let Some(uri) = &self.args.uri {
            uri.clone()
        } else {
            self.build_connection_uri()
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

        // Then try to extract from URI if provided
        if let Some(uri) = &self.args.uri {
            if let Some(db) = extract_database_from_uri(uri) {
                return db;
            }
        }

        // Finally, fall back to default
        "test".to_string()
    }

    /// Check if running in interactive mode
    ///
    /// # Returns
    /// * `bool` - True if interactive mode
    pub fn is_interactive(&self) -> bool {
        self.args.script_file.is_none() && self.args.eval.is_none()
    }

    /// Check if running in script mode
    ///
    /// # Returns
    /// * `bool` - True if script mode
    pub fn is_script_mode(&self) -> bool {
        self.args.script_file.is_some() || self.args.eval.is_some()
    }

    /// Get the configuration
    ///
    /// # Returns
    /// * `&Config` - Reference to configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get mutable reference to configuration
    ///
    /// # Returns
    /// * `&mut Config` - Mutable reference to configuration
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
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
        todo!("Apply CLI arguments to override config values")
    }

    /// Validate configuration and arguments
    ///
    /// # Returns
    /// * `Result<()>` - Ok if valid, error otherwise
    pub fn validate(&self) -> Result<()> {
        todo!("Validate configuration and argument combinations")
    }

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
    fn generate_completion(&self, shell: &str) -> Result<()> {
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
        if show {
            println!("{:#?}", self.config);
        }
        if validate {
            self.config.validate()?;
            println!("Configuration is valid");
        }
        Ok(())
    }

    /// Print banner with version and connection info
    pub fn print_banner(&self) {
        if !self.args.quiet {
            println!("MongoDB Shell v{}", env!("CARGO_PKG_VERSION"));
            println!("Connecting to: {}", self.get_connection_uri());
            println!();
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
    fn test_is_interactive_mode() {
        let args = CliArgs::try_parse_from(vec!["mongosh"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert!(cli.is_interactive());
        assert!(!cli.is_script_mode());
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
        let args = CliArgs::try_parse_from(vec!["mongosh", "-d", "mydb"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.get_database(), "mydb");

        // Test with database in URI
        let args = CliArgs::try_parse_from(vec!["mongosh", "mongodb://localhost/admin"]).unwrap();
        let config = Config::default();
        let cli = CliInterface { args, config };
        assert_eq!(cli.get_database(), "admin");

        // Test explicit argument overrides URI
        let args =
            CliArgs::try_parse_from(vec!["mongosh", "mongodb://localhost/admin", "-d", "mydb"])
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
        let args = CliArgs::try_parse_from(vec!["mongosh", "-d", "mydb"]).unwrap();
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
        let args =
            CliArgs::try_parse_from(vec!["mongosh", "-u", "user", "-p", "pass", "-d", "mydb"])
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
            "-d",
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
            "-d",
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
}
