//! Completion system for mongosh REPL
//!
//! This module provides an intelligent completion system that works with both
//! MongoDB shell syntax and SQL syntax. The system is built on a finite state
//! machine (FSM) approach that is error-tolerant and works with incomplete input.
//!
//! # Architecture
//!
//! The completion system consists of several components:
//!
//! - **TokenStream**: Wraps SQL and Mongo tokens with cursor awareness
//! - **FSM**: Determines the completion context based on token sequence
//! - **Context**: Standardized representation of what to complete
//! - **Provider**: Fetches completion candidates (collections, operations, etc.)
//! - **Engine**: Orchestrates the entire completion flow
//!
//! # Examples
//!
//! ```no_run
//! use mongosh::repl::completion::{CompletionEngine, MongoCandidateProvider};
//! use mongosh::repl::SharedState;
//! use std::sync::Arc;
//!
//! let shared_state = SharedState::new("test".to_string());
//! let provider = Arc::new(MongoCandidateProvider::new(shared_state, None));
//! let engine = CompletionEngine::new(provider);
//!
//! // Complete "db.us" with cursor at position 5
//! let (start, candidates) = engine.complete("db.us", 5);
//! // Returns collection names starting with "us"
//! ```

mod context;
mod engine;
mod fsm;
mod provider;
mod token_stream;

pub use engine::CompletionEngine;
pub use provider::MongoCandidateProvider;
