//! Error handling module for MongoDB operations.
//!
//! This module provides comprehensive error handling for MongoDB operations with:
//! - Structured error information extraction from MongoDB driver errors
//! - Consistent JSON error formatting for APIs and logging
//! - Application-specific error types
//!
//! # Example
//!
//! ```rust,no_run
//! use mongosh::error::{Result, MongoshError};
//! use mongosh::error::mongo::ErrorResponse;
//!
//! fn example_operation() -> Result<()> {
//!     // MongoDB operations automatically convert errors
//!     // to structured JSON format
//!     Ok(())
//! }
//!
//! fn handle_error(err: &mongodb::error::Error) {
//!     let response = ErrorResponse::from_mongodb_error(err);
//!     println!("{}", response.to_json_pretty().unwrap());
//! }
//! ```

pub mod kinds;
pub mod mongo;

// Re-export commonly used types
pub use kinds::{ConfigError, ConnectionError, ExecutionError, MongoshError, ParseError, Result};
pub use mongo::{ErrorDetails, ErrorInfo, ErrorResponse};
