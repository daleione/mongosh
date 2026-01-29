//! Execution result types
//!
//! This module defines the data structures for representing command execution results:
//! - ExecutionResult: Overall result of a command execution
//! - ResultData: Various types of data that can be returned
//! - ExecutionStats: Statistics about the execution

use mongodb::bson::Document;

use super::export::StreamingQuery;

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
///
/// Note: The Stream variant cannot be cloned as it contains a trait object.
/// When cloning ResultData with Stream, it will panic.
pub enum ResultData {
    /// List of documents
    Documents(Vec<Document>),

    /// List of documents with pagination info
    DocumentsWithPagination {
        documents: Vec<Document>,
        has_more: bool,
        displayed: usize,
    },

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

    /// Streaming query result (for export operations)
    ///
    /// This variant holds a streaming query that can be used to
    /// fetch documents in batches without loading everything into memory.
    /// Cannot be cloned - used for export operations only.
    Stream(Box<dyn StreamingQuery>),
}

// Manual Debug implementation for ResultData because Stream variant contains trait object
impl std::fmt::Debug for ResultData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResultData::Documents(docs) => f.debug_tuple("Documents").field(docs).finish(),
            ResultData::DocumentsWithPagination { documents, has_more, displayed } => {
                f.debug_struct("DocumentsWithPagination")
                    .field("documents", documents)
                    .field("has_more", has_more)
                    .field("displayed", displayed)
                    .finish()
            }
            ResultData::Document(doc) => f.debug_tuple("Document").field(doc).finish(),
            ResultData::InsertOne { inserted_id } => {
                f.debug_struct("InsertOne").field("inserted_id", inserted_id).finish()
            }
            ResultData::InsertMany { inserted_ids } => {
                f.debug_struct("InsertMany").field("inserted_ids", inserted_ids).finish()
            }
            ResultData::Update { matched, modified } => {
                f.debug_struct("Update")
                    .field("matched", matched)
                    .field("modified", modified)
                    .finish()
            }
            ResultData::Delete { deleted } => {
                f.debug_struct("Delete").field("deleted", deleted).finish()
            }
            ResultData::Count(count) => f.debug_tuple("Count").field(count).finish(),
            ResultData::Message(msg) => f.debug_tuple("Message").field(msg).finish(),
            ResultData::List(list) => f.debug_tuple("List").field(list).finish(),
            ResultData::None => f.write_str("None"),
            ResultData::Stream(_) => f.write_str("Stream(<streaming query>)"),
        }
    }
}

// Manual Clone implementation for ResultData because Stream variant cannot be cloned
impl Clone for ResultData {
    fn clone(&self) -> Self {
        match self {
            ResultData::Documents(docs) => ResultData::Documents(docs.clone()),
            ResultData::DocumentsWithPagination { documents, has_more, displayed } => {
                ResultData::DocumentsWithPagination {
                    documents: documents.clone(),
                    has_more: *has_more,
                    displayed: *displayed,
                }
            }
            ResultData::Document(doc) => ResultData::Document(doc.clone()),
            ResultData::InsertOne { inserted_id } => ResultData::InsertOne {
                inserted_id: inserted_id.clone(),
            },
            ResultData::InsertMany { inserted_ids } => ResultData::InsertMany {
                inserted_ids: inserted_ids.clone(),
            },
            ResultData::Update { matched, modified } => ResultData::Update {
                matched: *matched,
                modified: *modified,
            },
            ResultData::Delete { deleted } => ResultData::Delete {
                deleted: *deleted,
            },
            ResultData::Count(count) => ResultData::Count(*count),
            ResultData::Message(msg) => ResultData::Message(msg.clone()),
            ResultData::List(list) => ResultData::List(list.clone()),
            ResultData::None => ResultData::None,
            ResultData::Stream(_) => {
                panic!("Cannot clone ResultData::Stream - streaming queries are not cloneable")
            }
        }
    }
}

/// Execution statistics
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Number of documents returned
    #[allow(dead_code)]
    pub documents_returned: usize,

    /// Number of documents affected
    pub documents_affected: Option<u64>,
}

impl ExecutionResult {
    /// Create a successful result
    #[allow(dead_code)]
    pub fn success(data: ResultData, stats: ExecutionStats) -> Self {
        Self {
            success: true,
            data,
            stats,
            error: None,
        }
    }

    /// Create a failed result
    #[allow(dead_code)]
    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: ResultData::None,
            stats: ExecutionStats::default(),
            error: Some(error),
        }
    }
}
