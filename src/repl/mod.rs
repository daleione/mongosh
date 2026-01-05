//! REPL (Read-Eval-Print Loop) module root for mongosh
//!
//! This module is split into several submodules responsible for different
//! aspects of the interactive shell:
//!
//! - `cursor_state`  : pagination cursor state
//! - `shared_state`  : shared mutable state between REPL and execution context
//! - `engine`        : `ReplEngine`, the main interactive loop and editor
//! - `helper`        : `ReplHelper` and trait impls (completion, hints, etc.)
//! - `completion`    : Intelligent completion system for MongoDB shell and SQL
//!
//! External code should typically depend on `ReplEngine` and `SharedState`.
//! More specialized types (e.g. helpers, prompt generator, command completer)
//! are re‑exported for convenience but are mostly internal details of the
//! REPL implementation.

pub mod completion;
mod cursor_state;
mod engine;
mod helper;
mod shared_state;

pub use cursor_state::CursorState;
pub use engine::ReplEngine;
pub use shared_state::SharedState;

#[cfg(test)]
mod tests;
