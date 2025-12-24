pub mod kinds;
pub mod mongo;

// Re-export commonly used error types and the crate-wide Result alias
pub use kinds::{ConnectionError, ExecutionError, MongoshError, ParseError, PluginError, Result};
