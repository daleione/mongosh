//! MongoDB error handling and structured error information extraction.
//!
//! This module provides utilities to extract structured error information from
//! MongoDB driver errors and format them as JSON for consistent error reporting.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Structured error response for MongoDB errors.
///
/// This structure wraps ErrorInfo and provides JSON serialization methods.
/// When serialized to JSON, it directly outputs the error information without
/// an outer "error" wrapper.
#[derive(Debug, Clone)]
pub struct ErrorResponse {
    pub error: ErrorInfo,
}

impl ErrorResponse {
    /// Create a new error response from a MongoDB error.
    pub fn from_mongodb_error(error: &mongodb::error::Error) -> Self {
        Self {
            error: ErrorInfo::from_mongodb_error(error),
        }
    }

    /// Convert to pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.error)
    }

    /// Convert to compact JSON string (single line).
    #[allow(dead_code)]
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.error)
    }
}

/// Structured error information extracted from MongoDB errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error type classification (e.g., "mongo.write_error", "mongo.command_error")
    #[serde(rename = "type")]
    pub error_type: String,

    /// MongoDB error code (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,

    /// Human-readable error name derived from the error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_name: Option<String>,

    /// Descriptive error message
    pub message: String,

    /// Additional structured error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,

    /// Server-side labels associated with the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

impl ErrorInfo {
    /// Extract structured error information from a MongoDB driver error.
    pub fn from_mongodb_error(error: &mongodb::error::Error) -> Self {
        use mongodb::error::ErrorKind;

        match error.kind.as_ref() {
            ErrorKind::Write(write_failure) => Self::from_write_failure(write_failure),
            ErrorKind::Command(command_error) => Self::from_command_error(command_error),
            ErrorKind::InsertMany(insert_error) => Self::from_insert_many_error(insert_error),
            ErrorKind::BulkWrite(bulk_error) => Self::from_bulk_write_error(bulk_error),
            ErrorKind::Authentication { message, .. } => Self {
                error_type: "mongo.authentication_error".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::InvalidArgument { message, .. } => Self {
                error_type: "mongo.invalid_argument".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::ServerSelection { message, .. } => Self {
                error_type: "mongo.server_selection_error".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::ConnectionPoolCleared { message, .. } => Self {
                error_type: "mongo.connection_pool_cleared".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::InvalidResponse { message, .. } => Self {
                error_type: "mongo.invalid_response".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::Transaction { message, .. } => Self {
                error_type: "mongo.transaction_error".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::IncompatibleServer { message, .. } => Self {
                error_type: "mongo.incompatible_server".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::DnsResolve { message, .. } => Self {
                error_type: "mongo.dns_resolve_error".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::InvalidTlsConfig { message, .. } => Self {
                error_type: "mongo.invalid_tls_config".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::Internal { message, .. } => Self {
                error_type: "mongo.internal_error".to_string(),
                code: None,
                code_name: None,
                message: message.clone(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::Io(io_error) => Self {
                error_type: "mongo.io_error".to_string(),
                code: None,
                code_name: None,
                message: io_error.to_string(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::BsonSerialization(bson_error) => Self {
                error_type: "mongo.bson_serialization_error".to_string(),
                code: None,
                code_name: None,
                message: bson_error.to_string(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::BsonDeserialization(bson_error) => Self {
                error_type: "mongo.bson_deserialization_error".to_string(),
                code: None,
                code_name: None,
                message: bson_error.to_string(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::SessionsNotSupported => Self {
                error_type: "mongo.sessions_not_supported".to_string(),
                code: None,
                code_name: None,
                message: "Sessions are not supported on this deployment".to_string(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::MissingResumeToken => Self {
                error_type: "mongo.missing_resume_token".to_string(),
                code: None,
                code_name: None,
                message: "Resume token is missing from change stream document".to_string(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::Shutdown => Self {
                error_type: "mongo.shutdown".to_string(),
                code: None,
                code_name: None,
                message: "Client has been shut down".to_string(),
                details: None,
                labels: get_error_labels(error),
            },
            ErrorKind::GridFs(gridfs_error) => Self {
                error_type: "mongo.gridfs_error".to_string(),
                code: None,
                code_name: None,
                message: format!("{:?}", gridfs_error),
                details: None,
                labels: get_error_labels(error),
            },
            _ => Self {
                error_type: "mongo.unknown_error".to_string(),
                code: None,
                code_name: None,
                message: error.to_string(),
                details: None,
                labels: get_error_labels(error),
            },
        }
    }

    fn from_write_failure(write_failure: &mongodb::error::WriteFailure) -> Self {
        use mongodb::error::WriteFailure;

        match write_failure {
            WriteFailure::WriteError(write_error) => {
                let details = extract_write_error_details(write_error);
                Self {
                    error_type: "mongo.write_error".to_string(),
                    code: Some(write_error.code),
                    code_name: write_error
                        .code_name
                        .clone()
                        .or_else(|| get_standard_error_name(write_error.code)),
                    message: simplify_error_message(write_error.code, &write_error.message),
                    details: Some(details),
                    labels: None,
                }
            }
            WriteFailure::WriteConcernError(wc_error) => Self {
                error_type: "mongo.write_concern_error".to_string(),
                code: Some(wc_error.code),
                code_name: if wc_error.code_name.is_empty() {
                    get_standard_error_name(wc_error.code)
                } else {
                    Some(wc_error.code_name.clone())
                },
                message: wc_error.message.clone(),
                details: wc_error.details.as_ref().map(|doc| ErrorDetails {
                    namespace: extract_namespace_from_doc(doc),
                    collection: extract_collection_from_doc(doc),
                    database: extract_database_from_doc(doc),
                    index: extract_index_from_doc(doc),
                    key: extract_key_from_doc(doc),
                    raw: Some(doc.clone()),
                }),
                labels: None,
            },
            _ => Self {
                error_type: "mongo.write_failure".to_string(),
                code: None,
                code_name: None,
                message: format!("{:?}", write_failure),
                details: None,
                labels: None,
            },
        }
    }

    fn from_command_error(command_error: &mongodb::error::CommandError) -> Self {
        Self {
            error_type: "mongo.command_error".to_string(),
            code: Some(command_error.code),
            code_name: if command_error.code_name.is_empty() {
                get_standard_error_name(command_error.code)
            } else {
                Some(command_error.code_name.clone())
            },
            message: command_error.message.clone(),
            details: None,
            labels: None,
        }
    }

    fn from_insert_many_error(insert_error: &mongodb::error::InsertManyError) -> Self {
        // Prioritize write errors over write concern errors
        if let Some(write_errors) = &insert_error.write_errors {
            if let Some(first_error) = write_errors.first() {
                let details = extract_indexed_write_error_details(first_error);
                return Self {
                    error_type: "mongo.insert_many_error".to_string(),
                    code: Some(first_error.code),
                    code_name: first_error
                        .code_name
                        .clone()
                        .or_else(|| get_standard_error_name(first_error.code)),
                    message: simplify_error_message(first_error.code, &first_error.message),
                    details: Some(details),
                    labels: None,
                };
            }
        }

        if let Some(wc_error) = &insert_error.write_concern_error {
            return Self {
                error_type: "mongo.insert_many_error".to_string(),
                code: Some(wc_error.code),
                code_name: if wc_error.code_name.is_empty() {
                    get_standard_error_name(wc_error.code)
                } else {
                    Some(wc_error.code_name.clone())
                },
                message: wc_error.message.clone(),
                details: wc_error.details.as_ref().map(|doc| ErrorDetails {
                    namespace: extract_namespace_from_doc(doc),
                    collection: extract_collection_from_doc(doc),
                    database: extract_database_from_doc(doc),
                    index: extract_index_from_doc(doc),
                    key: extract_key_from_doc(doc),
                    raw: Some(doc.clone()),
                }),
                labels: None,
            };
        }

        // Fallback
        Self {
            error_type: "mongo.insert_many_error".to_string(),
            code: None,
            code_name: None,
            message: "Insert many operation failed".to_string(),
            details: None,
            labels: None,
        }
    }

    fn from_bulk_write_error(bulk_error: &mongodb::error::BulkWriteError) -> Self {
        Self {
            error_type: "mongo.bulk_write_error".to_string(),
            code: None,
            code_name: None,
            message: format!("{:?}", bulk_error),
            details: None,
            labels: None,
        }
    }
}

/// Additional structured error details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// Full namespace (database.collection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Collection name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,

    /// Database name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,

    /// Index name involved in the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,

    /// Key or key pattern involved in the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<bson::Document>,

    /// Raw error details document from MongoDB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<bson::Document>,
}

/// Format a MongoDB error as a JSON error response.
pub fn format_mongodb_error_json(error: &mongodb::error::Error) -> String {
    let response = ErrorResponse::from_mongodb_error(error);
    response
        .to_json_pretty()
        .unwrap_or_else(|_| format!(r#"{{"error": {{"message": "{}"}}}}"#, error))
}

/// Format MongoDB error for Display trait implementation.
pub fn format_mongodb_error(
    f: &mut fmt::Formatter<'_>,
    error: &mongodb::error::Error,
) -> fmt::Result {
    write!(f, "{}", format_mongodb_error_json(error))
}

// ============================================================================
// Helper functions for extracting error details
// ============================================================================

/// Extract error details from a WriteError.
fn extract_write_error_details(write_error: &mongodb::error::WriteError) -> ErrorDetails {
    let mut details = ErrorDetails {
        namespace: None,
        collection: None,
        database: None,
        index: None,
        key: None,
        raw: write_error.details.clone(),
    };

    if let Some(doc) = &write_error.details {
        details.namespace = extract_namespace_from_doc(doc);
        details.collection = extract_collection_from_doc(doc);
        details.database = extract_database_from_doc(doc);
        details.index = extract_index_from_doc(doc);
        details.key = extract_key_from_doc(doc);
    }

    // If still missing collection/index info, try parsing from message
    if details.collection.is_none() || details.index.is_none() {
        enhance_details_from_message(&mut details, &write_error.message);
    }

    details
}

/// Extract error details from an IndexedWriteError.
fn extract_indexed_write_error_details(
    write_error: &mongodb::error::IndexedWriteError,
) -> ErrorDetails {
    let mut details = ErrorDetails {
        namespace: None,
        collection: None,
        database: None,
        index: None,
        key: None,
        raw: write_error.details.clone(),
    };

    if let Some(doc) = &write_error.details {
        details.namespace = extract_namespace_from_doc(doc);
        details.collection = extract_collection_from_doc(doc);
        details.database = extract_database_from_doc(doc);
        details.index = extract_index_from_doc(doc);
        details.key = extract_key_from_doc(doc);
    }

    // If still missing collection/index info, try parsing from message
    if details.collection.is_none() || details.index.is_none() {
        enhance_details_from_message(&mut details, &write_error.message);
    }

    details
}

/// Extract namespace from a BSON document.
fn extract_namespace_from_doc(doc: &bson::Document) -> Option<String> {
    // Try common field names
    for key in ["ns", "namespace"] {
        if let Ok(ns) = doc.get_str(key) {
            return Some(ns.to_string());
        }
    }
    None
}

/// Extract collection name from a BSON document.
fn extract_collection_from_doc(doc: &bson::Document) -> Option<String> {
    // First try direct collection field
    if let Ok(coll) = doc.get_str("collection") {
        return Some(coll.to_string());
    }

    // Try extracting from namespace
    if let Some(ns) = extract_namespace_from_doc(doc) {
        return parse_collection_from_namespace(&ns);
    }

    None
}

/// Extract database name from a BSON document.
fn extract_database_from_doc(doc: &bson::Document) -> Option<String> {
    // First try direct database field
    if let Ok(db) = doc.get_str("database") {
        return Some(db.to_string());
    }

    // Try direct db field
    if let Ok(db) = doc.get_str("db") {
        return Some(db.to_string());
    }

    // Try extracting from namespace
    if let Some(ns) = extract_namespace_from_doc(doc) {
        return parse_database_from_namespace(&ns);
    }

    None
}

/// Extract index name from a BSON document.
fn extract_index_from_doc(doc: &bson::Document) -> Option<String> {
    for key in ["index", "indexName"] {
        if let Ok(idx) = doc.get_str(key) {
            return Some(idx.to_string());
        }
    }
    None
}

/// Extract key or key pattern from a BSON document.
fn extract_key_from_doc(doc: &bson::Document) -> Option<bson::Document> {
    // Try various field names where key information might be stored
    for key in ["keyValue", "keyPattern", "duplicateKey", "key"] {
        if let Ok(key_doc) = doc.get_document(key) {
            return Some(key_doc.clone());
        }
    }
    None
}

/// Parse collection name from namespace string (format: "database.collection").
fn parse_collection_from_namespace(namespace: &str) -> Option<String> {
    namespace.split('.').nth(1).map(|s| s.to_string())
}

/// Parse database name from namespace string (format: "database.collection").
fn parse_database_from_namespace(namespace: &str) -> Option<String> {
    namespace.split('.').next().map(|s| s.to_string())
}

/// Enhance error details by parsing the error message string.
/// This is a fallback when structured data is not available.
fn enhance_details_from_message(details: &mut ErrorDetails, message: &str) {
    // Only use this as a last resort
    if details.namespace.is_none() {
        if let Some(ns) = extract_field_from_message(message, "collection:") {
            details.namespace = Some(ns.clone());
            if details.collection.is_none() {
                details.collection = parse_collection_from_namespace(&ns);
            }
            if details.database.is_none() {
                details.database = parse_database_from_namespace(&ns);
            }
        }
    }

    if details.index.is_none() {
        details.index = extract_field_from_message(message, "index:");
    }
}

/// Extract a field value from an error message string.
fn extract_field_from_message(message: &str, field_prefix: &str) -> Option<String> {
    message.find(field_prefix).and_then(|start| {
        let after = &message[start + field_prefix.len()..].trim_start();
        // Extract until next space or special character
        let end = after
            .find(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    })
}

/// Get error labels from a MongoDB error.
fn get_error_labels(error: &mongodb::error::Error) -> Option<Vec<String>> {
    if error.labels().is_empty() {
        None
    } else {
        Some(error.labels().iter().cloned().collect())
    }
}

/// Get a standard error name from a MongoDB error code.
///
/// Based on MongoDB error codes documentation:
/// https://github.com/mongodb/mongo/blob/master/src/mongo/base/error_codes.yml
fn get_standard_error_name(code: i32) -> Option<String> {
    let name = match code {
        // Write errors
        11000 | 11001 => "DuplicateKey",
        121 => "DocumentValidationFailure",

        // Authentication & Authorization
        13 => "Unauthorized",
        18 => "AuthenticationFailed",

        // Namespace errors
        26 => "NamespaceNotFound",
        48 => "NamespaceExists",

        // Query & Execution
        50 => "MaxTimeMSExpired",
        96 => "OperationFailed",

        // Index errors
        85 => "IndexOptionsConflict",
        86 => "IndexKeySpecsConflict",

        // Replication
        10107 => "NotPrimary",
        10058 => "WriteConcernFailed",

        // Transaction errors
        225 => "NoSuchTransaction",
        228 => "TransactionCommitted",
        244 => "TransactionTooOld",
        251 => "NoSuchTransaction",
        256 => "TransactionAborted",

        // Network errors
        89 => "NetworkTimeout",
        133 => "FailedToParse",

        // Command errors
        59 => "CommandNotFound",
        72 => "InvalidOptions",

        _ => return None,
    };

    Some(name.to_string())
}

/// Simplify error messages for known error codes to avoid redundancy.
fn simplify_error_message(code: i32, original_message: &str) -> String {
    match code {
        11000 | 11001 => "Duplicate key error".to_string(),
        121 => "Document validation failed".to_string(),
        13 => "Unauthorized access".to_string(),
        18 => "Authentication failed".to_string(),
        26 => "Namespace not found".to_string(),
        50 => "Operation exceeded time limit".to_string(),
        _ => original_message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_namespace() {
        assert_eq!(
            parse_collection_from_namespace("mydb.mycollection"),
            Some("mycollection".to_string())
        );
        assert_eq!(
            parse_database_from_namespace("mydb.mycollection"),
            Some("mydb".to_string())
        );
    }

    #[test]
    fn test_get_standard_error_name() {
        assert_eq!(
            get_standard_error_name(11000),
            Some("DuplicateKey".to_string())
        );
        assert_eq!(
            get_standard_error_name(13),
            Some("Unauthorized".to_string())
        );
        assert_eq!(get_standard_error_name(999999), None);
    }

    #[test]
    fn test_simplify_error_message() {
        assert_eq!(
            simplify_error_message(11000, "E11000 duplicate key error..."),
            "Duplicate key error"
        );
        assert_eq!(
            simplify_error_message(96, "Operation failed"),
            "Operation failed"
        );
    }
}

#[cfg(test)]
mod json_format_tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_json_format_no_outer_error_wrapper() {
        let error_info = ErrorInfo {
            error_type: "mongo.write_error".to_string(),
            code: Some(11000),
            code_name: Some("DuplicateKey".to_string()),
            message: "Duplicate key error".to_string(),
            details: None,
            labels: None,
        };

        let response = ErrorResponse { error: error_info };
        let json_str = response.to_json_pretty().unwrap();

        println!("JSON Output:\n{}", json_str);

        // Parse JSON to verify structure
        let json_value: Value = serde_json::from_str(&json_str).unwrap();

        // Confirm top-level contains fields directly, not nested under "error"
        assert!(json_value.get("type").is_some(), "should have 'type' field");
        assert!(json_value.get("code").is_some(), "should have 'code' field");
        assert!(
            json_value.get("error").is_none(),
            "should not have outer 'error' wrapper"
        );

        assert_eq!(json_value["type"], "mongo.write_error");
        assert_eq!(json_value["code"], 11000);
    }
}
