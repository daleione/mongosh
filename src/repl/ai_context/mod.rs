//! AI context pre-generation module
//!
//! This module provides functionality to sample MongoDB metadata (schemas, indexes)
//! and use the DeepSeek Chat API to generate structured context files that are
//! injected into FIM prompts for more accurate field completion.
//!
//! ## Architecture
//!
//! ```text
//! :ai-gen command
//!   ├─► Sampler     — collects metadata from MongoDB (collections, indexes, sample docs)
//!   ├─► Generator   — calls Chat API to produce structured context files
//!   └─► Reader      — reads context files at FIM prompt build time
//! ```

// These modules are fully used when the `ai-completion` feature is enabled.
// Without it, only the router's `execute_ai_generate` stub references them,
// so silence dead-code warnings for the default (no-feature) build.
#![cfg_attr(not(feature = "ai-completion"), allow(dead_code, unused_imports))]

pub mod reader;
pub mod sampler;

#[cfg(feature = "ai-completion")]
pub mod generator;

pub use reader::ContextReader;
pub use sampler::Sampler;

#[cfg(feature = "ai-completion")]
pub use generator::ContextGenerator;
