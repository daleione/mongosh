//! MongoDB Shell - Rust Edition
//!
//! A high-performance MongoDB shell implementation written in Rust.
//! Provides an interactive REPL interface and full MongoDB operation support.
//!
//! # Features
//!
//! - Interactive REPL with syntax highlighting and auto-completion
//! - Full CRUD operation support
//! - Aggregation pipeline execution
//! - Connection management with pooling
//! - Plugin system for extensibility
//! - Multiple output formats (JSON, table, compact)
//! - Configuration management
//!
//! # Usage
//!
//! ```bash
//! # Interactive mode
//! mongosh mongodb://localhost:27017
//! ```

use std::sync::Arc;
use tracing::Level;

mod cli;
mod config;
mod connection;
mod error;
mod executor;
mod formatter;
mod parser;
mod repl;

use cli::CliInterface;

use connection::ConnectionManager;
use error::Result;
use executor::{CommandRouter, ExecutionContext};
use formatter::Formatter;

use repl::{ReplEngine, SharedState};

/// Application entry point
#[tokio::main]
async fn main() {
    // Initialize the application and handle any errors
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Main application logic
///
/// This function orchestrates the application startup:
/// 1. Parse command-line arguments
/// 2. Load configuration
/// 3. Initialize logging
/// 4. Handle subcommands or start main application
///
/// # Returns
/// * `Result<()>` - Success or error
async fn run() -> Result<()> {
    // Parse command-line arguments and load configuration
    let cli = CliInterface::new()?;

    // Initialize logging based on verbosity
    initialize_logging(&cli);

    // Handle subcommands (version, completion, config)
    if cli.handle_subcommand().await? {
        return Ok(());
    }

    // Print banner if not in quiet mode
    cli.print_banner();

    // Run in interactive mode
    run_interactive_mode(&cli).await
}

/// Run application in interactive REPL mode
///
/// # Arguments
/// * `cli` - CLI interface with configuration
///
/// # Returns
/// * `Result<()>` - Success or error
async fn run_interactive_mode(cli: &CliInterface) -> Result<()> {
    // Get connection URI and database
    let uri = cli.get_connection_uri();
    let database = cli.get_database();

    // Connect to MongoDB
    let mut conn_manager = ConnectionManager::new(uri.clone(), cli.config().connection.clone());

    let server_version = if !cli.args().no_connect {
        conn_manager.connect().await?;

        // Get MongoDB server version
        let version = if let Ok(client) = conn_manager.get_client() {
            conn_manager.get_server_version(client).await.ok()
        } else {
            None
        };

        // Print connection info with MongoDB version
        if let Some(ref ver) = version {
            cli.print_connection_info(ver);
        }

        version
    } else {
        None
    };

    // Create shared state for REPL and execution context
    let mut shared_state = SharedState::new(database.clone());
    shared_state.set_connected(server_version);

    // Create execution context with shared state
    let exec_context = ExecutionContext::new(conn_manager, shared_state.clone());

    // Create command router
    let _router = CommandRouter::new(exec_context.clone()).await?;

    // Create and configure formatter
    let format = cli.config().display.format;
    let use_colors = cli.config().display.color_output && !cli.args().no_color;
    let _formatter = Formatter::new(format, use_colors);

    // Create REPL engine with shared state
    let color_enabled = cli.config().display.color_output && !cli.args().no_color;
    shared_state.set_color_enabled(color_enabled);
    let highlighting_enabled = true; // TODO: make configurable
    let mut repl = ReplEngine::new(
        shared_state.clone(),
        cli.config().history.clone(),
        highlighting_enabled,
        Some(Arc::new(exec_context.clone())),
    )?;

    // Main REPL loop
    while repl.is_running() {
        // Read user input
        let input = match repl.read_line()? {
            Some(line) => line,
            None => {
                // EOF reached (Ctrl+D)
                break;
            }
        };

        // Skip empty lines
        if input.trim().is_empty() {
            continue;
        }

        // Parse command
        let command = match repl.process_input(&input) {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
        };

        // Check for exit command
        if matches!(command, parser::Command::Exit) {
            break;
        }

        // Check if this is a config command - output directly without formatting
        let is_config_cmd = matches!(command, parser::Command::Config(_));

        // Execute command
        match exec_context.execute(command).await {
            Ok(result) => {
                if is_config_cmd {
                    // Config commands output directly, no formatting
                    if let executor::ResultData::Message(msg) = &result.data {
                        println!("{}", msg);
                    }
                } else {
                    // Update formatter with current settings from shared_state
                    let current_format = shared_state.get_format();
                    let current_color = shared_state.get_color_enabled();
                    let current_formatter = Formatter::new(current_format, current_color);

                    // Format and display result
                    match current_formatter.format(&result) {
                        Ok(output) => println!("{}", output),
                        Err(e) => eprintln!("Format error: {}", e),
                    }
                }
                // No need to manually sync - shared_state is automatically updated!
            }
            Err(e) => {
                eprintln!("Execution error: {}", e);
            }
        }
    }

    // History is automatically saved by FileBackedHistory
    // ConnectionManager will be disconnected automatically when ExecutionContext is dropped
    println!("Goodbye!");
    Ok(())
}

/// Initialize logging system based on verbosity level
///
/// # Arguments
/// * `cli` - CLI interface with verbosity settings
fn initialize_logging(cli: &CliInterface) {
    let level = if cli.args().very_verbose {
        Level::TRACE
    } else if cli.args().verbose {
        Level::DEBUG
    } else {
        cli.config().logging.level.to_tracing_level()
    };

    // Build subscriber with level filter
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false);

    // Configure timestamps
    if cli.config().logging.timestamps {
        subscriber.init();
    } else {
        subscriber.without_time().init();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_structure() {
        // This test ensures all modules are properly declared
        // and can be compiled together
        assert!(true);
    }
}
