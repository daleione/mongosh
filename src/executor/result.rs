//! Execution result types
//!
//! This module defines the data structures for representing command execution results:
//! - ExecutionResult: Overall result of a command execution
//! - ResultData: Various types of data that can be returned
//! - ExecutionStats: Statistics about the execution

use mongodb::bson::Document;

/// Result of command execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Success status
    pub success: bool,

    /// Result data (documents, stats, etc.)
    pub data: ResultData,

    /// Execution statistics
    pub stats: ExecutionStats,

    /// Error message if failed
    pub error: Option<String>,
}

/// Data returned from command execution
#[derive(Debug, Clone)]
pub enum ResultData {
    /// List of documents
    Documents(Vec<Document>),

    /// Single document
    Document(Document),

    /// Insert one result
    InsertOne { inserted_id: String },

    /// Insert many result
    InsertMany { inserted_ids: Vec<String> },

    /// Update result
    Update { matched: u64, modified: u64 },

    /// Delete result
    Delete { deleted: u64 },

    /// Count result
    Count(u64),

    /// Text message
    Message(String),

    /// List of strings
    List(Vec<String>),

    /// No data
    None,
}

/// Execution statistics
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct ExecutionStats {
    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Number of documents returned
    pub documents_returned: usize,

    /// Number of documents affected
    pub documents_affected: Option<u64>,
}


impl ExecutionResult {
    /// Create a successful result
    pub fn success(data: ResultData, stats: ExecutionStats) -> Self {
        Self {
            success: true,
            data,
            stats,
            error: None,
        }
    }

    /// Create a failed result
    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: ResultData::None,
            stats: ExecutionStats::default(),
            error: Some(error),
        }
    }
}
