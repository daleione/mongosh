use std::fmt;

use serde::{Deserialize, Serialize};

/// Structured error information extracted from MongoDB errors.
///
/// This is intended to be serialized to JSON and consumed by other
/// components (e.g. logging, APIs).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub(crate) error_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) details: Option<ErrorDetails>,
}

/// Additional error details extracted from MongoDB error details document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) collection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) key: Option<bson::Document>,
}

impl ErrorInfo {
    /// Convert error info to pretty-printed JSON string.
    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert error info to compact JSON string (single line).
    pub fn to_json_compact(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Format MongoDB error messages as pretty JSON wrapped in an `error` field.
///
/// Intended to be used by the parent module's `Display` implementation for
/// `MongoshError::MongoDb`.
pub fn format_mongodb_error(
    f: &mut fmt::Formatter<'_>,
    error: &mongodb::error::Error,
) -> fmt::Result {
    let info = extract_error_info(error);

    // Wrap in "error" field as requested.
    let wrapper = serde_json::json!({ "error": info });

    // Format as pretty JSON.
    let json_output = serde_json::to_string_pretty(&wrapper).map_err(|_| fmt::Error)?;
    write!(f, "\n{json_output}")
}

/// Extract structured information from a MongoDB error using the driver API.
///
/// This avoids string parsing where possible by using the driver's typed error
/// structures directly.
pub fn extract_error_info(error: &mongodb::error::Error) -> ErrorInfo {
    use mongodb::error::{ErrorKind, WriteFailure};

    let mut info = ErrorInfo::default();

    match error.kind.as_ref() {
        ErrorKind::Write(write_failure) => {
            info.error_type = Some("mongo.write_error".to_string());

            match write_failure {
                WriteFailure::WriteError(write_error) => {
                    info.code = Some(write_error.code);
                    info.message = Some(write_error.message.clone());
                    info.name = get_error_name(write_error.code);
                    info.details = Some(extract_error_details_from_write_error(write_error));
                }
                WriteFailure::WriteConcernError(wc_error) => {
                    info.code = Some(wc_error.code);
                    info.message = Some(wc_error.message.clone());
                    info.name = get_error_name(wc_error.code);
                }
                _ => {}
            }
        }
        ErrorKind::Command(command_error) => {
            info.error_type = Some("mongo.command_error".to_string());
            info.code = Some(command_error.code);
            info.message = Some(command_error.message.clone());
            info.name = get_error_name(command_error.code);
        }
        ErrorKind::BulkWrite(bulk_error) => {
            info.error_type = Some("mongo.bulk_write_error".to_string());
            // BulkWriteError doesn't expose structured fields we want, so use Debug.
            info.message = Some(format!("{bulk_error:?}"));
        }
        ErrorKind::InsertMany(insert_error) => {
            info.error_type = Some("mongo.insert_many_error".to_string());

            if let Some(write_errors) = &insert_error.write_errors {
                if let Some(first_error) = write_errors.first() {
                    info.code = Some(first_error.code);
                    info.message = Some(first_error.message.clone());
                    info.name = get_error_name(first_error.code);
                    info.details =
                        Some(extract_error_details_from_indexed_write_error(first_error));
                }
            } else if let Some(wc_error) = &insert_error.write_concern_error {
                info.code = Some(wc_error.code);
                info.message = Some(wc_error.message.clone());
                info.name = get_error_name(wc_error.code);
            }
        }
        ErrorKind::Authentication { message, .. } => {
            info.error_type = Some("mongo.authentication_error".to_string());
            info.message = Some(message.clone());
        }
        ErrorKind::InvalidArgument { message, .. } => {
            info.error_type = Some("mongo.invalid_argument".to_string());
            info.message = Some(message.clone());
        }
        ErrorKind::ServerSelection { message, .. } => {
            info.error_type = Some("mongo.server_selection_error".to_string());
            info.message = Some(message.clone());
        }
        _ => {
            // For other error types, fall back to the Display representation.
            info.message = Some(error.to_string());
        }
    }

    // Simplify message for known error types to avoid redundancy.
    if let Some(code) = info.code {
        if code == 11000 || code == 11001 {
            info.message = Some("Duplicate key error".to_string());
        }
    }

    info
}

/// Get a human-readable error name from a MongoDB error code.
fn get_error_name(code: i32) -> Option<String> {
    let name = match code {
        11000 | 11001 => "DuplicateKey",
        13 => "Unauthorized",
        18 => "AuthenticationFailed",
        26 => "NamespaceNotFound",
        50 => "MaxTimeMSExpired",
        121 => "DocumentValidationFailure",
        _ => return None,
    };

    Some(name.to_string())
}

/// Extract collection, index, and key information from `WriteError` details.
fn extract_error_details_from_write_error(
    write_error: &mongodb::error::WriteError,
) -> ErrorDetails {
    extract_from_details_and_message(&write_error.details, &write_error.message)
}

/// Extract collection, index, and key information from `IndexedWriteError` details.
fn extract_error_details_from_indexed_write_error(
    write_error: &mongodb::error::IndexedWriteError,
) -> ErrorDetails {
    extract_from_details_and_message(&write_error.details, &write_error.message)
}

/// Extract error details from an optional BSON document and a message string.
///
/// The BSON document is preferred; if it does not contain the necessary
/// information, a best-effort extraction from the message string is attempted.
fn extract_from_details_and_message(
    error_details: &Option<bson::Document>,
    message: &str,
) -> ErrorDetails {
    let mut details = ErrorDetails {
        collection: None,
        index: None,
        key: None,
    };

    // Try to extract from the errInfo/details document if available.
    if let Some(doc) = error_details {
        // Extract namespace (collection).
        if let Some(bson::Bson::String(ns)) = doc.get("namespace") {
            details.collection = Some(ns.clone());
        } else if let Some(bson::Bson::String(ns)) = doc.get("ns") {
            details.collection = Some(ns.clone());
        }

        // Extract index name.
        if let Some(bson::Bson::String(idx)) = doc.get("index") {
            details.index = Some(idx.clone());
        } else if let Some(bson::Bson::String(idx)) = doc.get("indexName") {
            details.index = Some(idx.clone());
        }

        // Extract duplicate key.
        if let Some(bson::Bson::Document(key_doc)) = doc.get("keyPattern") {
            details.key = Some(key_doc.clone());
        } else if let Some(bson::Bson::Document(key_doc)) = doc.get("keyValue") {
            details.key = Some(key_doc.clone());
        }
    }

    // Fallback: extract from message if details document doesn't have the info.
    if details.collection.is_none() {
        if let Some(coll_start) = message.find("collection: ") {
            let after = &message[coll_start + "collection: ".len()..];
            if let Some(space_pos) = after.find(' ') {
                details.collection = Some(after[..space_pos].to_string());
            }
        }
    }

    if details.index.is_none() {
        if let Some(idx_start) = message.find("index: ") {
            let after = &message[idx_start + "index: ".len()..];
            if let Some(space_pos) = after.find(' ') {
                details.index = Some(after[..space_pos].to_string());
            }
        }
    }

    details
}
