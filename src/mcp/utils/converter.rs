//! Type conversion utilities between JSON, BSON, and MCP types

use bson::{Bson, Document};
use rmcp::model::{CallToolResult, Content};
use serde_json::Value as JsonValue;

use crate::executor::ExecutionResult;

/// Convert JSON value to BSON document
///
/// # Arguments
/// * `value` - JSON value to convert
///
/// # Returns
/// Result containing BSON document or error message
pub fn json_to_bson_document(value: &JsonValue) -> Result<Document, String> {
    match value {
        JsonValue::Object(map) => {
            let mut doc = Document::new();
            for (key, val) in map {
                let bson_val = json_to_bson(val)?;
                doc.insert(key.clone(), bson_val);
            }
            Ok(doc)
        }
        JsonValue::Null => Ok(Document::new()),
        _ => Err(format!("Expected JSON object, got {:?}", value)),
    }
}

/// Convert JSON value to BSON value
fn json_to_bson(value: &JsonValue) -> Result<Bson, String> {
    match value {
        JsonValue::Null => Ok(Bson::Null),
        JsonValue::Bool(b) => Ok(Bson::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Bson::Int64(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Bson::Double(f))
            } else {
                Err("Invalid number".to_string())
            }
        }
        JsonValue::String(s) => Ok(Bson::String(s.clone())),
        JsonValue::Array(arr) => {
            let bson_arr: Result<Vec<Bson>, String> =
                arr.iter().map(json_to_bson).collect();
            Ok(Bson::Array(bson_arr?))
        }
        JsonValue::Object(map) => {
            let mut doc = Document::new();
            for (key, val) in map {
                doc.insert(key.clone(), json_to_bson(val)?);
            }
            Ok(Bson::Document(doc))
        }
    }
}

/// Convert BSON document to JSON value
///
/// # Arguments
/// * `doc` - BSON document to convert
///
/// # Returns
/// JSON value representation of the document
pub fn bson_document_to_json(doc: &Document) -> JsonValue {
    bson_to_json(&Bson::Document(doc.clone()))
}

/// Convert BSON value to JSON value
fn bson_to_json(bson: &Bson) -> JsonValue {
    match bson {
        Bson::Double(f) => JsonValue::Number(
            serde_json::Number::from_f64(*f).unwrap_or_else(|| serde_json::Number::from(0)),
        ),
        Bson::String(s) => JsonValue::String(s.clone()),
        Bson::Array(arr) => JsonValue::Array(arr.iter().map(bson_to_json).collect()),
        Bson::Document(doc) => {
            let mut map = serde_json::Map::new();
            for (key, val) in doc {
                map.insert(key.clone(), bson_to_json(val));
            }
            JsonValue::Object(map)
        }
        Bson::Boolean(b) => JsonValue::Bool(*b),
        Bson::Null => JsonValue::Null,
        Bson::Int32(i) => JsonValue::Number((*i).into()),
        Bson::Int64(i) => JsonValue::Number((*i).into()),
        Bson::ObjectId(oid) => JsonValue::String(oid.to_hex()),
        Bson::DateTime(dt) => JsonValue::String(dt.to_string()),
        Bson::Binary(bin) => JsonValue::String(format!("Binary({})", bin.bytes.len())),
        Bson::RegularExpression(regex) => {
            JsonValue::String(format!("/{}/{}", regex.pattern, regex.options))
        }
        Bson::JavaScriptCode(code) => JsonValue::String(format!("Code({})", code)),
        Bson::JavaScriptCodeWithScope(code) => {
            JsonValue::String(format!("Code({}) with scope", code.code))
        }
        Bson::Timestamp(ts) => JsonValue::String(format!("Timestamp({}, {})", ts.time, ts.increment)),
        Bson::Decimal128(d) => JsonValue::String(d.to_string()),
        Bson::Undefined => JsonValue::Null,
        Bson::MaxKey => JsonValue::String("MaxKey".to_string()),
        Bson::MinKey => JsonValue::String("MinKey".to_string()),
        Bson::DbPointer(dbp) => JsonValue::String(format!("DBPointer({:?})", dbp)),
        Bson::Symbol(s) => JsonValue::String(format!("Symbol({})", s)),
    }
}

/// Convert ExecutionResult to MCP CallToolResult
///
/// # Arguments
/// * `result` - Execution result from MongoDB operation
///
/// # Returns
/// MCP CallToolResult containing the operation result
pub fn execution_result_to_mcp_tool_result(result: ExecutionResult) -> CallToolResult {
    use crate::executor::ResultData;

    if !result.success {
        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
        return CallToolResult::error(vec![Content::text(error_msg)]);
    }

    match result.data {
        ResultData::Documents(docs) => {
            let json_docs: Vec<JsonValue> = docs
                .iter()
                .map(bson_document_to_json)
                .collect();

            let output = serde_json::json!({
                "documents": json_docs,
                "count": json_docs.len(),
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::DocumentsWithPagination { documents, has_more, displayed } => {
            let json_docs: Vec<JsonValue> = documents
                .iter()
                .map(bson_document_to_json)
                .collect();

            let output = serde_json::json!({
                "documents": json_docs,
                "count": json_docs.len(),
                "hasMore": has_more,
                "displayed": displayed,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::Document(doc) => {
            let json_doc = bson_document_to_json(&doc);
            let output = serde_json::json!({
                "document": json_doc,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::InsertOne { inserted_id } => {
            let output = serde_json::json!({
                "insertedId": inserted_id,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::InsertMany { inserted_ids } => {
            let output = serde_json::json!({
                "insertedIds": inserted_ids,
                "insertedCount": inserted_ids.len(),
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::Update { matched, modified } => {
            let output = serde_json::json!({
                "matchedCount": matched,
                "modifiedCount": modified,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::Delete { deleted } => {
            let output = serde_json::json!({
                "deletedCount": deleted,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::Count(count) => {
            let output = serde_json::json!({
                "count": count,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::Message(msg) => {
            CallToolResult::success(vec![Content::text(msg)])
        }
        ResultData::List(items) => {
            let output = serde_json::json!({
                "items": items,
                "count": items.len(),
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            )])
        }
        ResultData::None => {
            CallToolResult::success(vec![Content::text("Operation completed successfully")])
        }
        ResultData::Stream(_) => {
            CallToolResult::error(vec![Content::text(
                "Stream results are not supported in MCP context"
            )])
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_bson_document() {
        let json = serde_json::json!({
            "name": "test",
            "age": 25,
            "active": true
        });

        let doc = json_to_bson_document(&json).unwrap();
        assert_eq!(doc.get_str("name").unwrap(), "test");
        assert_eq!(doc.get_i64("age").unwrap(), 25);
        assert_eq!(doc.get_bool("active").unwrap(), true);
    }

    #[test]
    fn test_json_to_bson_document_nested() {
        let json = serde_json::json!({
            "user": {
                "name": "test",
                "age": 25
            }
        });

        let doc = json_to_bson_document(&json).unwrap();
        let user = doc.get_document("user").unwrap();
        assert_eq!(user.get_str("name").unwrap(), "test");
    }

    #[test]
    fn test_json_to_bson_document_array() {
        let json = serde_json::json!({
            "tags": ["rust", "mongodb", "mcp"]
        });

        let doc = json_to_bson_document(&json).unwrap();
        let tags = doc.get_array("tags").unwrap();
        assert_eq!(tags.len(), 3);
    }

    #[test]
    fn test_json_to_bson_document_null() {
        let json = JsonValue::Null;
        let doc = json_to_bson_document(&json).unwrap();
        assert!(doc.is_empty());
    }

    #[test]
    fn test_json_to_bson_document_invalid() {
        let json = serde_json::json!("string");
        let result = json_to_bson_document(&json);
        assert!(result.is_err());
    }

    #[test]
    fn test_bson_document_to_json() {
        let mut doc = Document::new();
        doc.insert("name", "test");
        doc.insert("age", 25);

        let json = bson_document_to_json(&doc);
        assert_eq!(json["name"], "test");
        assert_eq!(json["age"], 25);
    }




}
