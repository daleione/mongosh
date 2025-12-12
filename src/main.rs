//! MongoDB Shell - Rust Edition
//!
//! A high-performance MongoDB shell implementation written in Rust.
//! Provides an interactive REPL interface, script execution, and full MongoDB operation support.
//!
//! # Features
//!
//! - Interactive REPL with syntax highlighting and auto-completion
//! - Script execution from files or command line
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
//!
//! # Execute script
//! mongosh --file script.js
//!
//! # Evaluate expression
//! mongosh --eval "db.users.find()"
//! ```

use tokio;
use tracing::{info, Level};
use tracing_subscriber;

mod cli;
mod config;
mod connection;
mod error;
mod executor;
mod formatter;
mod parser;
mod plugins;
mod repl;
mod script;
mod utils;

use cli::CliInterface;

use connection::ConnectionManager;
use error::Result;
use executor::{CommandRouter, ExecutionContext};
use formatter::Formatter;

use repl::{ReplContext, ReplEngine, SharedState};

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

    // Determine execution mode and run
    if cli.is_interactive() {
        run_interactive_mode(&cli).await
    } else {
        run_script_mode(&cli).await
    }
}

/// Run application in interactive REPL mode
///
/// # Arguments
/// * `cli` - CLI interface with configuration
///
/// # Returns
/// * `Result<()>` - Success or error
async fn run_interactive_mode(cli: &CliInterface) -> Result<()> {
    info!("Starting interactive REPL mode");

    // Get connection URI and database
    let uri = cli.get_connection_uri();
    let database = cli.get_database();

    // Connect to MongoDB
    let mut conn_manager = ConnectionManager::new(uri.clone(), cli.config().connection.clone());

    if !cli.args().no_connect {
        info!("Connecting to MongoDB: {}", uri);
        conn_manager.connect().await?;
        info!("Connected successfully");
    }

    // Create shared state for REPL and execution context
    let mut shared_state = SharedState::new(database.clone(), uri);
    shared_state.set_connected(None); // Mark as connected (version detection is optional)

    // Create execution context with shared state
    let exec_context = ExecutionContext::new(conn_manager, shared_state.clone());

    // Create command router
    let _router = CommandRouter::new(exec_context.clone()).await?;

    // Create and configure formatter
    let format = cli.config().display.format;
    let use_colors = cli.config().display.color_output && !cli.args().no_color;
    let formatter = Formatter::new(format, use_colors);

    // Create REPL engine with shared state
    let color_enabled = cli.config().display.color_output && !cli.args().no_color;
    let highlighting_enabled = true; // TODO: make configurable
    let mut repl = ReplEngine::new(
        shared_state.clone(),
        cli.config().history.clone(),
        color_enabled,
        highlighting_enabled,
    )?;

    // Main REPL loop
    info!("Starting REPL loop");
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
                eprintln!("Parse error: {}", e);
                continue;
            }
        };

        // Check for exit command
        if matches!(command, parser::Command::Exit) {
            break;
        }

        // Execute command
        match exec_context.execute(command).await {
            Ok(result) => {
                // Format and display result
                match formatter.format(&result) {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Format error: {}", e),
                }
                // No need to manually sync - shared_state is automatically updated!
            }
            Err(e) => {
                eprintln!("Execution error: {}", e);
            }
        }
    }

    // Save history before exit
    let history_path = &cli.config().history.file_path;
    let _ = repl.save_history(history_path);

    // ConnectionManager will be disconnected automatically when ExecutionContext is dropped
    println!("Goodbye!");
    Ok(())
}

/// Run application in script execution mode
///
/// # Arguments
/// * `cli` - CLI interface with configuration
///
/// # Returns
/// * `Result<()>` - Success or error
async fn run_script_mode(cli: &CliInterface) -> Result<()> {
    info!("Starting script execution mode");

    // Get connection URI and database
    let uri = cli.get_connection_uri();
    let database = cli.get_database();

    // Connect to MongoDB
    let mut conn_manager = ConnectionManager::new(uri.clone(), cli.config().connection.clone());

    if !cli.args().no_connect {
        info!("Connecting to MongoDB: {}", uri);
        conn_manager.connect().await?;
    }

    // Get MongoDB client
    let client = conn_manager.get_client()?.clone();

    // Create script executor
    let executor = script::ScriptExecutor::new(client, database);

    // Execute script
    let result = if let Some(file) = &cli.args().script_file {
        info!("Executing script file: {:?}", file);
        executor.execute_file(file).await?
    } else if let Some(eval) = &cli.args().eval {
        info!("Evaluating expression");
        executor.execute_string(eval).await?
    } else {
        return Err("No script or eval expression provided".into());
    };

    // Display results
    if !cli.args().quiet {
        if result.success {
            println!("{}", result.get_output());
            if let Some(value) = result.return_value {
                println!("Return value: {}", value);
            }
        } else {
            eprintln!("Script failed: {}", result.error.unwrap_or_default());
            std::process::exit(1);
        }
    }

    // Disconnect
    conn_manager.disconnect().await?;

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

    info!("Logging initialized at level: {:?}", level);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        // This test ensures all modules are properly declared
        // and can be compiled together
        assert!(true);
    }
}
