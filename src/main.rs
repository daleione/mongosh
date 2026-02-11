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
async fn run_interactive_mode(cli: &CliInterface) -> Result<()> {
    let (conn_manager, server_version) = setup_connection(cli).await?;
    let shared_state = initialize_shared_state(cli, server_version)?;
    let config_path = cli.config_path().map(|p| p.to_path_buf());
    let exec_context =
        create_execution_context(conn_manager, shared_state.clone(), config_path).await?;
    let mut repl = create_repl_engine(cli, shared_state.clone(), exec_context.clone())?;

    run_repl_loop(cli, &mut repl, &exec_context, &shared_state).await?;

    println!("Goodbye!");
    Ok(())
}

/// Setup connection to MongoDB
async fn setup_connection(cli: &CliInterface) -> Result<(ConnectionManager, Option<String>)> {
    let uri = cli.get_connection_uri();
    let mut conn_manager = ConnectionManager::new(uri, cli.config().connection.clone());

    if cli.args().no_connect {
        return Ok((conn_manager, None));
    }

    conn_manager.connect().await?;

    let version = conn_manager.get_client().ok().and_then(|client| {
        futures::executor::block_on(conn_manager.get_server_version(client)).ok()
    });

    if let Some(ref ver) = version {
        cli.print_connection_info(ver);
    }

    Ok((conn_manager, version))
}

/// Initialize shared state with configuration
fn initialize_shared_state(
    cli: &CliInterface,
    server_version: Option<String>,
) -> Result<SharedState> {
    let database = cli.get_database();
    let mut shared_state = SharedState::with_config(database, &cli.config().display);
    shared_state.set_connected(server_version);

    if cli.args().no_color {
        shared_state.set_color_enabled(false);
    }

    Ok(shared_state)
}

/// Create execution context with connected manager
async fn create_execution_context(
    conn_manager: ConnectionManager,
    shared_state: SharedState,
    config_path: Option<std::path::PathBuf>,
) -> Result<ExecutionContext> {
    let exec_context = ExecutionContext::with_config_path(conn_manager, shared_state, config_path);
    CommandRouter::new(exec_context.clone()).await?;
    Ok(exec_context)
}

/// Create REPL engine with configuration
fn create_repl_engine(
    cli: &CliInterface,
    shared_state: SharedState,
    exec_context: ExecutionContext,
) -> Result<ReplEngine> {
    ReplEngine::new(
        shared_state,
        cli.config().history.clone(),
        cli.config().display.syntax_highlighting,
        Some(Arc::new(exec_context)),
    )
}

/// Main REPL loop
async fn run_repl_loop(
    cli: &CliInterface,
    repl: &mut ReplEngine,
    exec_context: &ExecutionContext,
    shared_state: &SharedState,
) -> Result<()> {
    while repl.is_running() {
        // Reset cancellation token for each command
        let mut context_clone = exec_context.clone();
        context_clone.reset_cancel_token();

        let input = match repl.read_line()? {
            Some(line) if !line.trim().is_empty() => line,
            Some(_) => continue,
            None => break,
        };

        let command = match repl.process_input(&input) {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
        };

        if matches!(command, parser::Command::Exit) {
            break;
        }

        // Setup Ctrl+C handler for this command execution
        let cancel_token = context_clone.get_cancel_token();
        let cancel_token_clone = cancel_token.clone();

        let ctrl_c_handle = tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    cancel_token_clone.cancel();
                }
                Err(err) => {
                    eprintln!("Failed to listen for Ctrl+C: {}", err);
                }
            }
        });

        execute_and_display(cli, &context_clone, shared_state, command).await;

        // Cancel the Ctrl+C listener for the next command
        ctrl_c_handle.abort();
    }

    Ok(())
}

/// Execute command and display result
async fn execute_and_display(
    cli: &CliInterface,
    exec_context: &ExecutionContext,
    shared_state: &SharedState,
    command: parser::Command,
) {
    let is_config_cmd = matches!(command, parser::Command::Config(_));
    let is_execute_named_query = matches!(
        command,
        parser::Command::Config(parser::ConfigCommand::ExecuteNamedQuery { .. })
    );

    match exec_context.execute(command).await {
        Ok(result) => {
            if is_execute_named_query {
                display_result(cli, shared_state, &result);
            } else if is_config_cmd {
                if let executor::ResultData::Message(msg) = &result.data {
                    println!("{}", msg);
                }
            } else {
                display_result(cli, shared_state, &result);
            }
        }
        Err(e) => eprintln!("{}", e),
    }
}

/// Display execution result with proper formatting
fn display_result(
    cli: &CliInterface,
    shared_state: &SharedState,
    result: &executor::ExecutionResult,
) {
    let mut display_config = cli.config().display.clone();
    display_config.format = shared_state.get_format();
    display_config.color_output = shared_state.get_color_enabled();

    let formatter = Formatter::from_config(&display_config);

    match formatter.format(result) {
        Ok(output) => println!("{}", output),
        Err(e) => eprintln!("Format error: {}", e),
    }
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
