//! JSON formatting for MongoDB documents
//!
//! This module provides JSON formatting with BSON type simplification:
//! - Pretty-printed and compact JSON output
//! - BSON type conversion to standard JSON types
//! - Optional color highlighting for JSON output
//! - Support for ObjectId, DateTime, Int64, Decimal128, Binary, etc.

use colored_json::prelude::*;
use mongodb::bson::{Bson, Document};

use crate::error::Result;
use crate::executor::ResultData;

/// JSON formatter with pretty printing support
pub struct JsonFormatter {
    /// Enable pretty printing
    pretty: bool,

    /// Indentation level
    indent: usize,

    /// Enable colored output
    use_colors: bool,
}

impl JsonFormatter {
    /// Create a new JSON formatter
    ///
    /// # Arguments
    /// * `pretty` - Enable pretty printing
    /// * `use_colors` - Enable colored output
    ///
    /// # Returns
    /// * `Self` - New formatter
    pub fn new(pretty: bool, use_colors: bool) -> Self {
        Self {
            pretty,
            indent: 2,
            use_colors,
        }
    }

    /// Format result data as JSON
    ///
    /// # Arguments
    /// * `data` - Result data to format
    ///
    /// # Returns
    /// * `Result<String>` - JSON string or error
    pub fn format(&self, data: &ResultData) -> Result<String> {
        match data {
            ResultData::Documents(docs) => self.format_documents(docs),
            ResultData::Document(doc) => self.format_document(doc),
            ResultData::Message(msg) => Ok(format!("\"{}\"", msg)),
            ResultData::List(items) => {
                let list_str = items.join("\n");
                Ok(list_str)
            }
            ResultData::InsertOne { inserted_id } => {
                Ok(format!("{{ \"insertedId\": \"{}\" }}", inserted_id))
            }
            ResultData::InsertMany { inserted_ids } => {
                let ids_json = inserted_ids
                    .iter()
                    .map(|id| format!("\"{}\"", id))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(format!("{{ \"insertedIds\": [{}] }}", ids_json))
            }
            ResultData::Update { matched, modified } => Ok(format!(
                "{{ \"matchedCount\": {}, \"modifiedCount\": {} }}",
                matched, modified
            )),
            ResultData::Delete { deleted } => Ok(format!("{{ \"deletedCount\": {} }}", deleted)),
            ResultData::Count(count) => Ok(format!("{}", count)),
            ResultData::None => Ok("null".to_string()),
        }
    }

    /// Format documents as JSON array
    ///
    /// # Arguments
    /// * `docs` - Documents to format
    ///
    /// # Returns
    /// * `Result<String>` - JSON array string
    fn format_documents(&self, docs: &[Document]) -> Result<String> {
        // Convert BSON documents to simplified JSON
        let json_docs: Vec<serde_json::Value> = docs
            .iter()
            .map(|doc| self.bson_to_simplified_json(doc))
            .collect();

        let json_str = if self.pretty {
            serde_json::to_string_pretty(&json_docs).unwrap_or_else(|_| format!("{:?}", docs))
        } else {
            serde_json::to_string(&json_docs).unwrap_or_else(|_| format!("{:?}", docs))
        };

        // Only apply colors for pretty-printed JSON
        // Compact JSON should remain as-is for piping/logging
        if self.use_colors && self.pretty {
            Ok(json_str.to_colored_json_auto().unwrap_or(json_str))
        } else {
            Ok(json_str)
        }
    }

    /// Format single document as JSON object
    ///
    /// # Arguments
    /// * `doc` - Document to format
    ///
    /// # Returns
    /// * `Result<String>` - JSON object string
    pub fn format_document(&self, doc: &Document) -> Result<String> {
        // Convert BSON document to simplified JSON
        let json_value = self.bson_to_simplified_json(doc);

        let json_str = if self.pretty {
            serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| format!("{:?}", doc))
        } else {
            serde_json::to_string(&json_value).unwrap_or_else(|_| format!("{:?}", doc))
        };

        // Only apply colors for pretty-printed JSON
        // Compact JSON should remain as-is for piping/logging
        if self.use_colors && self.pretty {
            Ok(json_str.to_colored_json_auto().unwrap_or(json_str))
        } else {
            Ok(json_str)
        }
    }

    /// Convert BSON document to simplified JSON
    ///
    /// Converts BSON types to human-readable JSON:
    /// - ObjectId -> String
    /// - DateTime -> ISO 8601 String
    /// - Int64 -> Number
    /// - Decimal128 -> Number
    /// - Binary -> Base64 String
    fn bson_to_simplified_json(&self, doc: &Document) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        for (key, value) in doc {
            map.insert(key.clone(), self.bson_value_to_json(value));
        }

        serde_json::Value::Object(map)
    }

    /// Convert BSON value to simplified JSON value
    fn bson_value_to_json(&self, value: &Bson) -> serde_json::Value {
        use serde_json::Value as JsonValue;

        match value {
            Bson::ObjectId(oid) => JsonValue::String(oid.to_string()),
            Bson::DateTime(dt) => {
                let iso = dt
                    .try_to_rfc3339_string()
                    .unwrap_or_else(|_| format!("{}", dt.timestamp_millis()));
                JsonValue::String(iso)
            }
            Bson::Int64(n) => JsonValue::Number((*n).into()),
            Bson::Int32(n) => JsonValue::Number((*n).into()),
            Bson::Double(f) => serde_json::Number::from_f64(*f)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            Bson::Decimal128(d) => {
                // Convert Decimal128 to string then to number if possible
                let s = d.to_string();
                s.parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(JsonValue::Number)
                    .unwrap_or_else(|| JsonValue::String(s))
            }
            Bson::String(s) => JsonValue::String(s.clone()),
            Bson::Boolean(b) => JsonValue::Bool(*b),
            Bson::Null => JsonValue::Null,
            Bson::Array(arr) => {
                let json_arr: Vec<JsonValue> =
                    arr.iter().map(|v| self.bson_value_to_json(v)).collect();
                JsonValue::Array(json_arr)
            }
            Bson::Document(doc) => self.bson_to_simplified_json(doc),
            Bson::Binary(bin) => {
                // Convert binary to base64 string
                use base64::Engine;
                let base64_str = base64::engine::general_purpose::STANDARD.encode(&bin.bytes);
                JsonValue::String(base64_str)
            }
            Bson::RegularExpression(regex) => {
                JsonValue::String(format!("/{}/{}", regex.pattern, regex.options))
            }
            Bson::Timestamp(ts) => {
                // Convert timestamp to milliseconds
                let millis = (ts.time as i64) * 1000 + (ts.increment as i64);
                JsonValue::Number(millis.into())
            }
            _ => JsonValue::String(format!("{:?}", value)),
        }
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new(true, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn test_json_formatter() {
        let formatter = JsonFormatter::new(false, false);
        let doc = doc! { "name": "test", "value": 42 };
        let result = formatter.format_document(&doc).unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_json_formatter_simplified_objectid() {
        use mongodb::bson::oid::ObjectId;
        let formatter = JsonFormatter::new(true, false);
        let oid = ObjectId::parse_str("65705d84dfc3f3b5094e1f72").unwrap();
        let doc = doc! { "_id": oid };
        let result = formatter.format_document(&doc).unwrap();
        // Should be simplified to string, not extended JSON
        assert!(result.contains("\"_id\""));
        assert!(result.contains("\"65705d84dfc3f3b5094e1f72\""));
        assert!(!result.contains("$oid"));
    }

    #[test]
    fn test_json_formatter_simplified_datetime() {
        use mongodb::bson::DateTime;
        let formatter = JsonFormatter::new(true, false);
        let dt = DateTime::from_millis(1701862788373);
        let doc = doc! { "created_time": dt };
        let result = formatter.format_document(&doc).unwrap();
        // Should be ISO 8601 string, not extended JSON
        assert!(result.contains("\"created_time\""));
        assert!(result.contains("2023-12-06"));
        assert!(!result.contains("$date"));
        assert!(!result.contains("$numberLong"));
    }

    #[test]
    fn test_json_formatter_simplified_long() {
        let formatter = JsonFormatter::new(true, false);
        let doc = doc! { "user_id": 1i64 };
        let result = formatter.format_document(&doc).unwrap();
        // Should be a number, not Long('1')
        assert!(result.contains("\"user_id\""));
        assert!(result.contains("1"));
        assert!(!result.contains("Long"));
    }

    #[test]
    fn test_json_formatter_complete_document() {
        use mongodb::bson::{oid::ObjectId, DateTime};
        let formatter = JsonFormatter::new(true, false);
        let oid = ObjectId::parse_str("65705d84dfc3f3b5094e1f72").unwrap();
        let dt = DateTime::from_millis(1701862788373);
        let doc = doc! {
            "_id": oid,
            "user_id": 1i64,
            "nickname": "dalei",
            "oauth2": null,
            "created_time": dt
        };
        let result = formatter.format_document(&doc).unwrap();

        // Verify all fields are simplified
        assert!(result.contains("\"_id\": \"65705d84dfc3f3b5094e1f72\""));
        assert!(result.contains("\"user_id\": 1"));
        assert!(result.contains("\"nickname\": \"dalei\""));
        assert!(result.contains("\"oauth2\": null"));
        assert!(result.contains("\"created_time\": \"2023-12-06"));

        // Verify no extended JSON formats
        assert!(!result.contains("$oid"));
        assert!(!result.contains("$date"));
    }

    #[test]
    fn test_json_formatter_compact() {
        let formatter = JsonFormatter::new(false, false);
        let doc = doc! { "name": "test", "value": 42 };
        let result = formatter.format_document(&doc).unwrap();

        // Compact JSON should be single line without extra whitespace
        assert!(
            !result.contains('\n'),
            "Compact JSON should not contain newlines"
        );
        assert!(
            !result.contains("  "),
            "Compact JSON should not contain double spaces"
        );

        // But should still contain the data
        assert!(result.contains("name"));
        assert!(result.contains("test"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_json_formatter_compact_vs_pretty() {
        let compact = JsonFormatter::new(false, false);
        let pretty = JsonFormatter::new(true, false);
        let doc = doc! { "a": 1, "b": 2, "c": 3 };

        let compact_result = compact.format_document(&doc).unwrap();
        let pretty_result = pretty.format_document(&doc).unwrap();

        // Compact should be much shorter
        assert!(compact_result.len() < pretty_result.len());

        // Pretty should have newlines
        assert!(pretty_result.contains('\n'));
        assert!(!compact_result.contains('\n'));
    }
}
