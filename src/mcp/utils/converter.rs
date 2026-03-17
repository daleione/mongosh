//! Type conversion utilities between JSON, BSON, and MCP types
//!
//! ## BSON Type Handling
//!
//! ### Output (BSON → JSON)
//!
//! Results returned to AI clients use **MongoDB Extended JSON v2 Relaxed** format,
//! which preserves BSON type information:
//!
//! - `ObjectId`    → `{"$oid": "69297ddcb4c39276cb39b05b"}`
//! - `DateTime`    → `{"$date": "2025-11-28T10:47:07.965Z"}`
//! - `Decimal128`  → `{"$numberDecimal": "3.14"}`
//! - `Binary`      → `{"$binary": {"base64": "...", "subType": "00"}}`
//! - `Timestamp`   → `{"$timestamp": {"t": 1, "i": 2}}`
//! - `Int32/Int64` → plain JSON numbers (relaxed mode)
//! - `Double`      → plain JSON number
//!
//! ### Input (JSON → BSON)
//!
//! Filters and documents sent by AI clients are parsed with Extended JSON v2
//! awareness. Both relaxed and canonical forms are accepted:
//!
//! ```text
//! {"$oid": "..."}                           → ObjectId
//! {"$date": "2025-01-01T00:00:00Z"}         → DateTime (ISO 8601)
//! {"$date": "2025-01-01"}                   → DateTime (date-only, midnight UTC)
//! {"$date": 1740441600000}                  → DateTime (epoch ms)
//! {"$date": {"$numberLong": "..."}}         → DateTime (canonical)
//! {"$numberLong": "12345"}                  → Int64
//! {"$numberInt": "42"}                      → Int32
//! {"$numberDouble": "3.14"}                 → Double
//! {"$numberDecimal": "3.14"}                → Decimal128
//! {"$binary": {"base64": "...", "subType": "00"}} → Binary
//! {"$regularExpression": {"pattern": "...", "options": "..."}} → Regex
//! {"$timestamp": {"t": 1, "i": 2}}          → Timestamp
//! {"$minKey": 1}                            → MinKey
//! {"$maxKey": 1}                            → MaxKey
//! {"$undefined": true}                      → Undefined
//! ```
//!
//! MongoDB query operators (`$gt`, `$in`, `$match`, etc.) are **not** affected
//! because they are not in the Extended JSON type-marker whitelist.

use bson::{Bson, Document};
use rmcp::model::{CallToolResult, Content};
use serde_json::Value as JsonValue;

use crate::executor::ExecutionResult;

// ---------------------------------------------------------------------------
// Output: BSON → Extended JSON v2 Relaxed
// ---------------------------------------------------------------------------

/// Convert a BSON document to **Extended JSON v2 Relaxed** `serde_json::Value`.
///
/// This is the canonical output format for MCP results. It preserves BSON type
/// information so that AI clients can round-trip values back into queries.
pub fn bson_document_to_json(doc: &Document) -> JsonValue {
    // `Bson::into_relaxed_extjson()` is provided by the bson crate and produces
    // the exact format documented at:
    //   https://www.mongodb.com/docs/manual/reference/mongodb-extended-json/
    Bson::Document(doc.clone()).into_relaxed_extjson()
}

// ---------------------------------------------------------------------------
// Input: JSON → BSON (Extended JSON v2 aware)
// ---------------------------------------------------------------------------

/// Convert a JSON value to a BSON [`Document`].
///
/// The top-level value must be a JSON object (or `null`, which yields an empty
/// document). All nested values are parsed with Extended JSON v2 awareness so
/// that type markers such as `{"$oid": "..."}` are converted to the correct
/// BSON types.
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

/// Convert a JSON value to a BSON value with Extended JSON v2 support.
///
/// When the value is a JSON object, `try_parse_extended_json` is consulted
/// first. If the object matches a known Extended JSON type marker it is
/// converted to the corresponding BSON type. Otherwise the object is treated
/// as a regular document (which may contain MongoDB query/update operators).
fn json_to_bson(value: &JsonValue) -> Result<Bson, String> {
    match value {
        JsonValue::Null => Ok(Bson::Null),
        JsonValue::Bool(b) => Ok(Bson::Boolean(*b)),
        JsonValue::Number(n) => {
            // Prefer integer representation to avoid unnecessary floating-point.
            if let Some(i) = n.as_i64() {
                Ok(Bson::Int64(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Bson::Double(f))
            } else {
                Err(format!("Invalid number: {}", n))
            }
        }
        JsonValue::String(s) => Ok(Bson::String(s.clone())),
        JsonValue::Array(arr) => {
            let bson_arr: Result<Vec<Bson>, String> = arr.iter().map(json_to_bson).collect();
            Ok(Bson::Array(bson_arr?))
        }
        JsonValue::Object(map) => {
            // Check for Extended JSON type markers BEFORE treating as a document.
            if let Some(result) = try_parse_extended_json(map) {
                return result;
            }

            // Regular document (may contain query operators like $gt, $in …).
            let mut doc = Document::new();
            for (key, val) in map {
                doc.insert(key.clone(), json_to_bson(val)?);
            }
            Ok(Bson::Document(doc))
        }
    }
}

// ---------------------------------------------------------------------------
// Extended JSON type-marker detection
// ---------------------------------------------------------------------------

/// The set of single-key Extended JSON type markers that this parser recognises.
///
/// Keys that start with `$` but are **not** in this list are treated as MongoDB
/// query/update operators and are passed through unchanged.
const EXTENDED_JSON_SINGLE_KEY_MARKERS: &[&str] = &[
    "$oid",
    "$date",
    "$numberLong",
    "$numberInt",
    "$numberDouble",
    "$numberDecimal",
    "$minKey",
    "$maxKey",
    "$undefined",
];

/// Try to interpret a JSON object as an Extended JSON v2 type marker.
///
/// Returns `Some(Result<Bson, String>)` if the object matches a known marker,
/// or `None` if it should be treated as a regular document.
fn try_parse_extended_json(
    map: &serde_json::Map<String, JsonValue>,
) -> Option<Result<Bson, String>> {
    if map.len() == 1 {
        let (key, value) = map.iter().next().unwrap();

        // Only act on whitelisted type markers.
        if EXTENDED_JSON_SINGLE_KEY_MARKERS.contains(&key.as_str()) {
            return Some(match key.as_str() {
                "$oid" => parse_oid(value),
                "$date" => parse_date(value),
                "$numberLong" => parse_number_long(value),
                "$numberInt" => parse_number_int(value),
                "$numberDouble" => parse_number_double(value),
                "$numberDecimal" => parse_number_decimal(value),
                "$minKey" => Ok(Bson::MinKey),
                "$maxKey" => Ok(Bson::MaxKey),
                "$undefined" => Ok(Bson::Undefined),
                _ => unreachable!(),
            });
        }

        // Single-key nested-object markers.
        if key == "$binary" {
            if let JsonValue::Object(_) = value {
                return Some(parse_binary_v2(value));
            }
        }
        if key == "$regularExpression" {
            if let JsonValue::Object(_) = value {
                return Some(parse_regex(value));
            }
        }
        if key == "$timestamp" {
            if let JsonValue::Object(_) = value {
                return Some(parse_timestamp(value));
            }
        }
    }

    // Two-key legacy binary: {"$binary": "<base64>", "$type": "<hex>"}
    if map.len() == 2 && map.contains_key("$binary") && map.contains_key("$type") {
        return Some(parse_binary_legacy(map));
    }

    None
}

// ---------------------------------------------------------------------------
// Individual type parsers
// ---------------------------------------------------------------------------

/// `{"$oid": "<24-hex-char string>"}` → `ObjectId`
fn parse_oid(value: &JsonValue) -> Result<Bson, String> {
    let s = value
        .as_str()
        .ok_or_else(|| "$oid value must be a string".to_string())?;
    let oid = bson::oid::ObjectId::parse_str(s)
        .map_err(|e| format!("Invalid ObjectId '{}': {}", s, e))?;
    Ok(Bson::ObjectId(oid))
}

/// `{"$date": <value>}` → `DateTime`
///
/// Accepts three representations:
/// - ISO 8601 string: `"2025-01-01T00:00:00Z"` or date-only `"2025-01-01"`
/// - Epoch milliseconds integer: `1740441600000`
/// - Canonical form: `{"$numberLong": "1740441600000"}`
fn parse_date(value: &JsonValue) -> Result<Bson, String> {
    match value {
        JsonValue::String(s) => {
            // Try RFC 3339 / ISO 8601 with time component first.
            let millis = chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.timestamp_millis())
                .or_else(|_| {
                    // Fall back to date-only "YYYY-MM-DD" (midnight UTC).
                    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map_err(|e| e.to_string())
                        .and_then(|d| {
                            d.and_hms_milli_opt(0, 0, 0, 0)
                                .ok_or_else(|| "Invalid date".to_string())
                        })
                        .map(|ndt| ndt.and_utc().timestamp_millis())
                })
                .map_err(|e| format!("Invalid $date string '{}': {}", s, e))?;
            Ok(Bson::DateTime(bson::DateTime::from_millis(millis)))
        }
        JsonValue::Number(n) => {
            let millis = n.as_i64().ok_or_else(|| {
                "$date number must be an integer (epoch milliseconds)".to_string()
            })?;
            Ok(Bson::DateTime(bson::DateTime::from_millis(millis)))
        }
        JsonValue::Object(inner) => {
            // Canonical form: {"$numberLong": "<millis>"}
            let long_val = inner
                .get("$numberLong")
                .ok_or_else(|| "$date object must contain \"$numberLong\"".to_string())?;
            let s = long_val
                .as_str()
                .ok_or_else(|| "$numberLong value must be a string".to_string())?;
            let millis: i64 = s
                .parse()
                .map_err(|e| format!("Invalid $numberLong '{}': {}", s, e))?;
            Ok(Bson::DateTime(bson::DateTime::from_millis(millis)))
        }
        _ => {
            Err("$date value must be a string, integer, or {\"$numberLong\": \"...\"}".to_string())
        }
    }
}

/// `{"$numberLong": "<integer string>"}` → `Int64`
fn parse_number_long(value: &JsonValue) -> Result<Bson, String> {
    let s = value
        .as_str()
        .ok_or_else(|| "$numberLong value must be a string".to_string())?;
    let n: i64 = s
        .parse()
        .map_err(|e| format!("Invalid $numberLong '{}': {}", s, e))?;
    Ok(Bson::Int64(n))
}

/// `{"$numberInt": "<integer string>"}` → `Int32`
fn parse_number_int(value: &JsonValue) -> Result<Bson, String> {
    let s = value
        .as_str()
        .ok_or_else(|| "$numberInt value must be a string".to_string())?;
    let n: i32 = s
        .parse()
        .map_err(|e| format!("Invalid $numberInt '{}': {}", s, e))?;
    Ok(Bson::Int32(n))
}

/// `{"$numberDouble": "<float string>"}` → `Double`
fn parse_number_double(value: &JsonValue) -> Result<Bson, String> {
    let s = value
        .as_str()
        .ok_or_else(|| "$numberDouble value must be a string".to_string())?;
    let f: f64 = s
        .parse()
        .map_err(|e| format!("Invalid $numberDouble '{}': {}", s, e))?;
    Ok(Bson::Double(f))
}

/// `{"$numberDecimal": "<decimal string>"}` → `Decimal128`
fn parse_number_decimal(value: &JsonValue) -> Result<Bson, String> {
    let s = value
        .as_str()
        .ok_or_else(|| "$numberDecimal value must be a string".to_string())?;
    let d: bson::Decimal128 = s
        .parse()
        .map_err(|e| format!("Invalid $numberDecimal '{}': {}", s, e))?;
    Ok(Bson::Decimal128(d))
}

/// `{"$binary": {"base64": "...", "subType": "XX"}}` → `Binary` (v2 format)
fn parse_binary_v2(value: &JsonValue) -> Result<Bson, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "$binary value must be an object".to_string())?;

    let base64_str = obj
        .get("base64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "$binary.base64 must be a string".to_string())?;

    let sub_type_str = obj
        .get("subType")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "$binary.subType must be a string".to_string())?;

    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_str)
        .map_err(|e| format!("Invalid base64 in $binary: {}", e))?;

    let sub_type_byte = u8::from_str_radix(sub_type_str.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Invalid $binary.subType '{}': {}", sub_type_str, e))?;

    Ok(Bson::Binary(bson::Binary {
        subtype: bson::spec::BinarySubtype::from(sub_type_byte),
        bytes,
    }))
}

/// `{"$binary": "<base64>", "$type": "<hex>"}` → `Binary` (legacy v1 format)
fn parse_binary_legacy(map: &serde_json::Map<String, JsonValue>) -> Result<Bson, String> {
    let base64_str = map
        .get("$binary")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "$binary value must be a string".to_string())?;

    let type_str = map
        .get("$type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "$type value must be a string".to_string())?;

    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_str)
        .map_err(|e| format!("Invalid base64 in $binary: {}", e))?;

    let sub_type_byte = u8::from_str_radix(type_str.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Invalid $type '{}': {}", type_str, e))?;

    Ok(Bson::Binary(bson::Binary {
        subtype: bson::spec::BinarySubtype::from(sub_type_byte),
        bytes,
    }))
}

/// `{"$regularExpression": {"pattern": "...", "options": "..."}}` → `Regex`
fn parse_regex(value: &JsonValue) -> Result<Bson, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "$regularExpression value must be an object".to_string())?;

    let pattern = obj
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "$regularExpression.pattern must be a string".to_string())?;

    let options = obj.get("options").and_then(|v| v.as_str()).unwrap_or("");

    Ok(Bson::RegularExpression(bson::Regex {
        pattern: pattern.to_string(),
        options: options.to_string(),
    }))
}

/// `{"$timestamp": {"t": <u32>, "i": <u32>}}` → `Timestamp`
fn parse_timestamp(value: &JsonValue) -> Result<Bson, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "$timestamp value must be an object".to_string())?;

    let t = obj
        .get("t")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "$timestamp.t must be a non-negative integer".to_string())?
        as u32;

    let i = obj
        .get("i")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "$timestamp.i must be a non-negative integer".to_string())?
        as u32;

    Ok(Bson::Timestamp(bson::Timestamp {
        time: t,
        increment: i,
    }))
}

// ---------------------------------------------------------------------------
// ExecutionResult → MCP CallToolResult
// ---------------------------------------------------------------------------

/// Convert an [`ExecutionResult`] to an MCP [`CallToolResult`].
///
/// Documents are serialised using Extended JSON v2 Relaxed so that the AI
/// client can observe and reproduce BSON types such as `ObjectId` and
/// `DateTime`.
pub fn execution_result_to_mcp_tool_result(result: ExecutionResult) -> CallToolResult {
    use crate::executor::ResultData;

    if !result.success {
        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
        return CallToolResult::error(vec![Content::text(error_msg)]);
    }

    match result.data {
        ResultData::Documents(docs) => {
            let json_docs: Vec<JsonValue> = docs.iter().map(bson_document_to_json).collect();

            let output = serde_json::json!({
                "documents": json_docs,
                "count": json_docs.len(),
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::DocumentsWithPagination {
            documents,
            has_more,
            displayed,
        } => {
            let json_docs: Vec<JsonValue> = documents.iter().map(bson_document_to_json).collect();

            let output = serde_json::json!({
                "documents": json_docs,
                "count": json_docs.len(),
                "hasMore": has_more,
                "displayed": displayed,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::Document(doc) => {
            let json_doc = bson_document_to_json(&doc);
            let output = serde_json::json!({
                "document": json_doc,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::InsertOne { inserted_id } => {
            let output = serde_json::json!({
                "insertedId": inserted_id,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::InsertMany { inserted_ids } => {
            let output = serde_json::json!({
                "insertedIds": inserted_ids,
                "insertedCount": inserted_ids.len(),
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::Update { matched, modified } => {
            let output = serde_json::json!({
                "matchedCount": matched,
                "modifiedCount": modified,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::Delete { deleted } => {
            let output = serde_json::json!({
                "deletedCount": deleted,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::Count(count) => {
            let output = serde_json::json!({
                "count": count,
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::Message(msg) => CallToolResult::success(vec![Content::text(msg)]),
        ResultData::List(items) => {
            let output = serde_json::json!({
                "items": items,
                "count": items.len(),
                "executionTimeMs": result.stats.execution_time_ms
            });

            CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )])
        }
        ResultData::None => {
            CallToolResult::success(vec![Content::text("Operation completed successfully")])
        }
        ResultData::Stream(_) => CallToolResult::error(vec![Content::text(
            "Stream results are not supported in MCP context",
        )]),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bson::{doc, oid::ObjectId};

    // ---- Output tests (BSON → Extended JSON v2 Relaxed) --------------------

    #[test]
    fn test_output_objectid_extended_json() {
        let oid = ObjectId::parse_str("69297ddcb4c39276cb39b05b").unwrap();
        let doc = doc! { "_id": oid };
        let json = bson_document_to_json(&doc);
        // {"_id": {"$oid": "69297ddcb4c39276cb39b05b"}}
        assert_eq!(json["_id"]["$oid"], "69297ddcb4c39276cb39b05b");
    }

    #[test]
    fn test_output_datetime_extended_json() {
        let dt = bson::DateTime::from_millis(1732789627965);
        let doc = doc! { "create_time": dt };
        let json = bson_document_to_json(&doc);
        // {"create_time": {"$date": "2024-11-28T10:47:07.965Z"}}
        assert!(
            json["create_time"]["$date"].is_string(),
            "DateTime should be wrapped in {{\"$date\": \"...\"}}, got: {}",
            json
        );
        let date_str = json["create_time"]["$date"].as_str().unwrap();
        assert!(
            date_str.contains("2024-11-28"),
            "ISO date string should contain date, got: {}",
            date_str
        );
    }

    #[test]
    fn test_output_numbers_unchanged_in_relaxed_mode() {
        let doc = doc! { "count": 42i32, "big": 9999i64 };
        let json = bson_document_to_json(&doc);
        // In relaxed mode integers are plain JSON numbers.
        assert_eq!(json["count"], 42);
        assert_eq!(json["big"], 9999);
    }

    #[test]
    fn test_output_double_unchanged_in_relaxed_mode() {
        let doc = doc! { "ratio": 3.14f64 };
        let json = bson_document_to_json(&doc);
        let v = json["ratio"].as_f64().unwrap();
        assert!((v - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_output_string_boolean_null_unchanged() {
        let doc = doc! { "name": "Alice", "active": true, "deleted_at": bson::Bson::Null };
        let json = bson_document_to_json(&doc);
        assert_eq!(json["name"], "Alice");
        assert_eq!(json["active"], true);
        assert!(json["deleted_at"].is_null());
    }

    #[test]
    fn test_output_nested_document() {
        let oid = ObjectId::parse_str("69297ddcb4c39276cb39b05b").unwrap();
        let doc = doc! {
            "_id": oid,
            "meta": {
                "group_id": ObjectId::parse_str("6920127eb40f0636d6b49042").unwrap()
            }
        };
        let json = bson_document_to_json(&doc);
        assert!(json["_id"]["$oid"].is_string());
        assert!(json["meta"]["group_id"]["$oid"].is_string());
    }

    // ---- Input tests (JSON → BSON with Extended JSON) ----------------------

    #[test]
    fn test_input_objectid() {
        let json = serde_json::json!({"$oid": "69297ddcb4c39276cb39b05b"});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::ObjectId(_)));
        if let Bson::ObjectId(oid) = bson {
            assert_eq!(oid.to_hex(), "69297ddcb4c39276cb39b05b");
        }
    }

    #[test]
    fn test_input_objectid_invalid() {
        let json = serde_json::json!({"$oid": "not-a-valid-oid"});
        assert!(json_to_bson(&json).is_err());
    }

    #[test]
    fn test_input_date_iso8601() {
        let json = serde_json::json!({"$date": "2025-01-01T00:00:00Z"});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::DateTime(_)));
        if let Bson::DateTime(dt) = bson {
            assert_eq!(dt.timestamp_millis(), 1735689600000);
        }
    }

    #[test]
    fn test_input_date_only() {
        // "2025-01-01" should be interpreted as midnight UTC.
        let json = serde_json::json!({"$date": "2025-01-01"});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::DateTime(_)));
        if let Bson::DateTime(dt) = bson {
            assert_eq!(dt.timestamp_millis(), 1735689600000);
        }
    }

    #[test]
    fn test_input_date_epoch_millis() {
        let json = serde_json::json!({"$date": 1735689600000i64});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::DateTime(_)));
        if let Bson::DateTime(dt) = bson {
            assert_eq!(dt.timestamp_millis(), 1735689600000);
        }
    }

    #[test]
    fn test_input_date_canonical_number_long() {
        let json = serde_json::json!({"$date": {"$numberLong": "1735689600000"}});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::DateTime(_)));
        if let Bson::DateTime(dt) = bson {
            assert_eq!(dt.timestamp_millis(), 1735689600000);
        }
    }

    #[test]
    fn test_input_number_long() {
        let json = serde_json::json!({"$numberLong": "9007199254740993"});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::Int64(9007199254740993)));
    }

    #[test]
    fn test_input_number_int() {
        let json = serde_json::json!({"$numberInt": "42"});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::Int32(42)));
    }

    #[test]
    fn test_input_number_double() {
        let json = serde_json::json!({"$numberDouble": "3.14"});
        if let Bson::Double(f) = json_to_bson(&json).unwrap() {
            assert!((f - 3.14).abs() < 1e-10);
        } else {
            panic!("expected Double");
        }
    }

    #[test]
    fn test_input_min_max_key() {
        assert!(matches!(
            json_to_bson(&serde_json::json!({"$minKey": 1})).unwrap(),
            Bson::MinKey
        ));
        assert!(matches!(
            json_to_bson(&serde_json::json!({"$maxKey": 1})).unwrap(),
            Bson::MaxKey
        ));
    }

    #[test]
    fn test_input_undefined() {
        let json = serde_json::json!({"$undefined": true});
        assert!(matches!(json_to_bson(&json).unwrap(), Bson::Undefined));
    }

    #[test]
    fn test_input_binary_v2() {
        use base64::Engine as _;
        let bytes = vec![0xde, 0xad, 0xbe, 0xef];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let json = serde_json::json!({"$binary": {"base64": b64, "subType": "00"}});
        if let Bson::Binary(bin) = json_to_bson(&json).unwrap() {
            assert_eq!(bin.bytes, bytes);
        } else {
            panic!("expected Binary");
        }
    }

    #[test]
    fn test_input_binary_legacy() {
        use base64::Engine as _;
        let bytes = vec![0x01, 0x02, 0x03];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let json = serde_json::json!({"$binary": b64, "$type": "00"});
        if let Bson::Binary(bin) = json_to_bson(&json).unwrap() {
            assert_eq!(bin.bytes, bytes);
        } else {
            panic!("expected Binary");
        }
    }

    #[test]
    fn test_input_regular_expression() {
        let json = serde_json::json!({
            "$regularExpression": {"pattern": "^foo", "options": "i"}
        });
        if let Bson::RegularExpression(re) = json_to_bson(&json).unwrap() {
            assert_eq!(re.pattern, "^foo");
            assert_eq!(re.options, "i");
        } else {
            panic!("expected RegularExpression");
        }
    }

    #[test]
    fn test_input_timestamp() {
        let json = serde_json::json!({"$timestamp": {"t": 1234, "i": 5}});
        if let Bson::Timestamp(ts) = json_to_bson(&json).unwrap() {
            assert_eq!(ts.time, 1234);
            assert_eq!(ts.increment, 5);
        } else {
            panic!("expected Timestamp");
        }
    }

    // ---- Operator pass-through tests --------------------------------------

    #[test]
    fn test_query_operator_not_confused_with_extended_json() {
        // $gt is not an Extended JSON type marker; it must survive as-is.
        let json = serde_json::json!({"$gt": 100});
        let bson = json_to_bson(&json).unwrap();
        assert!(matches!(bson, Bson::Document(_)));
        if let Bson::Document(doc) = bson {
            assert!(doc.contains_key("$gt"));
        }
    }

    #[test]
    fn test_mixed_operators_and_extended_json() {
        // filter: {"create_time": {"$gte": {"$date": "2025-01-01T00:00:00Z"}}}
        let json = serde_json::json!({
            "create_time": {
                "$gte": {"$date": "2025-01-01T00:00:00Z"}
            }
        });
        let doc = json_to_bson_document(&json).unwrap();
        let inner = doc.get_document("create_time").unwrap();
        let gte_val = inner.get("$gte").unwrap();
        assert!(
            matches!(gte_val, Bson::DateTime(_)),
            "$gte value should be DateTime, got: {:?}",
            gte_val
        );
    }

    #[test]
    fn test_objectid_in_dollar_in_array() {
        // filter: {"_id": {"$in": [{"$oid": "..."}, {"$oid": "..."}]}}
        let json = serde_json::json!({
            "_id": {
                "$in": [
                    {"$oid": "69b3cd8d552ada26281cb872"},
                    {"$oid": "69b3dcc990541c9ee4572717"}
                ]
            }
        });
        let doc = json_to_bson_document(&json).unwrap();
        let inner = doc.get_document("_id").unwrap();
        let in_arr = inner.get_array("$in").unwrap();
        assert!(
            matches!(&in_arr[0], Bson::ObjectId(_)),
            "first element should be ObjectId"
        );
        assert!(
            matches!(&in_arr[1], Bson::ObjectId(_)),
            "second element should be ObjectId"
        );
    }

    // ---- Round-trip tests -------------------------------------------------

    #[test]
    fn test_roundtrip_objectid() {
        let oid = ObjectId::new();
        let doc = doc! { "_id": oid };
        let json = bson_document_to_json(&doc);
        let doc2 = json_to_bson_document(&json).unwrap();
        assert_eq!(
            doc.get_object_id("_id").unwrap(),
            doc2.get_object_id("_id").unwrap()
        );
    }

    #[test]
    fn test_roundtrip_datetime() {
        // Use a round millisecond so there is no sub-ms precision loss.
        let dt = bson::DateTime::from_millis(1735689600000);
        let doc = doc! { "ts": dt };
        let json = bson_document_to_json(&doc);
        let doc2 = json_to_bson_document(&json).unwrap();
        assert_eq!(
            doc.get_datetime("ts").unwrap().timestamp_millis(),
            doc2.get_datetime("ts").unwrap().timestamp_millis()
        );
    }

    #[test]
    fn test_roundtrip_nested() {
        let oid = ObjectId::new();
        let dt = bson::DateTime::from_millis(1735689600000);
        let doc = doc! {
            "_id": oid,
            "meta": {
                "created_at": dt,
                "score": 42i32,
            },
            "tags": ["a", "b"]
        };
        let json = bson_document_to_json(&doc);
        let doc2 = json_to_bson_document(&json).unwrap();
        assert_eq!(
            doc.get_object_id("_id").unwrap(),
            doc2.get_object_id("_id").unwrap()
        );
        let meta2 = doc2.get_document("meta").unwrap();
        assert_eq!(
            meta2.get_datetime("created_at").unwrap().timestamp_millis(),
            dt.timestamp_millis()
        );
    }

    // ---- Existing basic tests (regression) --------------------------------

    #[test]
    fn test_json_to_bson_document_basic() {
        let json = serde_json::json!({
            "name": "test",
            "age": 25,
            "active": true
        });
        let doc = json_to_bson_document(&json).unwrap();
        assert_eq!(doc.get_str("name").unwrap(), "test");
        assert_eq!(doc.get_i64("age").unwrap(), 25);
        assert!(doc.get_bool("active").unwrap());
    }

    #[test]
    fn test_json_to_bson_document_nested() {
        let json = serde_json::json!({ "user": { "name": "test", "age": 25 } });
        let doc = json_to_bson_document(&json).unwrap();
        let user = doc.get_document("user").unwrap();
        assert_eq!(user.get_str("name").unwrap(), "test");
    }

    #[test]
    fn test_json_to_bson_document_array() {
        let json = serde_json::json!({ "tags": ["rust", "mongodb", "mcp"] });
        let doc = json_to_bson_document(&json).unwrap();
        let tags = doc.get_array("tags").unwrap();
        assert_eq!(tags.len(), 3);
    }

    #[test]
    fn test_json_to_bson_document_null() {
        let doc = json_to_bson_document(&JsonValue::Null).unwrap();
        assert!(doc.is_empty());
    }

    #[test]
    fn test_json_to_bson_document_invalid() {
        assert!(json_to_bson_document(&serde_json::json!("string")).is_err());
    }

    #[test]
    fn test_bson_document_to_json_basic() {
        let doc = doc! { "name": "test", "age": 25 };
        let json = bson_document_to_json(&doc);
        assert_eq!(json["name"], "test");
        assert_eq!(json["age"], 25);
    }
}
