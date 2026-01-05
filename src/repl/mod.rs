//! REPL (Read-Eval-Print Loop) module root for mongosh
//!
//! This module is split into several submodules responsible for different
//! aspects of the interactive shell:
//!
//! - `cursor_state`  : pagination cursor state
//! - `shared_state`  : shared mutable state between REPL and execution context
//! - `engine`        : `ReplEngine`, the main interactive loop and editor
//! - `completer`     : Completion provider for reedline
//! - `highlighter`   : Syntax highlighting for reedline
//! - `hinter`        : Inline hints for reedline
//! - `validator`     : Line validation for reedline
//! - `completion`    : Intelligent completion system for MongoDB shell and SQL
//!
//! External code should typically depend on `ReplEngine` and `SharedState`.
//! More specialized types (e.g. completer, highlighter, validator)
//! are re‑exported for convenience but are mostly internal details of the
//! REPL implementation.

mod completer;
pub mod completion;
mod cursor_state;
mod engine;
mod highlighter;
mod hinter;
mod prompt;
mod shared_state;
mod validator;

pub use cursor_state::CursorState;
pub use engine::ReplEngine;
pub use shared_state::SharedState;

#[cfg(test)]
mod tests;
