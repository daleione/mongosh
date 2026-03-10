//! Security management for MCP server
//!
//! This module provides security configuration and access control for MongoDB operations
//! exposed through the MCP protocol.

mod manager;

pub use manager::{SecurityConfig, SecurityManager};
