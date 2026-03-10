//! MCP (Model Context Protocol) integration module
//!
//! This module provides MCP server functionality for MongoDB Shell,
//! allowing AI models to interact with MongoDB through natural language.

pub mod server;
pub mod security;
pub mod tools;
pub mod utils;

pub use server::MongoShellServer;
pub use security::SecurityConfig;
