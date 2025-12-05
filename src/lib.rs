//! MongoDB Shell Library
//!
//! This library provides the core functionality for the MongoDB Shell (mongosh) implementation.
//! It can be used as a standalone library to build MongoDB tools and applications.
//!
//! # Modules
//!
//! - `cli`: Command-line interface and argument parsing
//! - `config`: Configuration management
//! - `connection`: MongoDB connection management
//! - `error`: Error types and handling
//! - `executor`: Command execution engine
//! - `formatter`: Output formatting and display
//! - `parser`: Command and query parsing
//! - `plugins`: Plugin system for extensibility
//! - `repl`: Interactive REPL engine
//! - `script`: Script execution
//! - `utils`: Utility functions and helpers
//!
//! # Example
//!
//! ```no_run
//! use mongosh::{config::Config, connection::ConnectionManager};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::default();
//!     let mut manager = ConnectionManager::new(
//!         "mongodb://localhost:27017".to_string(),
//!         config.connection,
//!     );
//!
//!     manager.connect().await?;
//!     println!("Connected to MongoDB");
//!
//!     manager.disconnect().await?;
//!     Ok(())
//! }
//! ```

pub mod cli;
pub mod config;
pub mod connection;
pub mod error;
pub mod executor;
pub mod formatter;
pub mod parser;
pub mod plugins;
pub mod repl;
pub mod script;
pub mod utils;

// Re-export commonly used types
pub use config::Config;
pub use connection::ConnectionManager;
pub use error::{MongoshError, Result};
pub use executor::{CommandRouter, ExecutionResult};
pub use formatter::Formatter;
pub use parser::{Command, Parser};
pub use repl::{ReplContext, ReplEngine};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get library version string
///
/// # Returns
/// * `&str` - Version string
pub fn version() -> &'static str {
    VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }
}
