//! JSON formatting for MongoDB documents
//!
//! This module provides JSON formatting with BSON type simplification:
//! - Pretty-printed and compact JSON output
//! - BSON type conversion to standard JSON types
//! - Optional color highlighting for JSON output
//! - Support for ObjectId, DateTime, Int64, Decimal128, Binary, etc.

use colored_json::prelude::*;
use mongodb::bson::{Bson, Document};

use super::bson_utils::{BsonConverter, JsonConverter};
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

    /// Converter for BSON to JSON
    converter: JsonConverter,
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
    pub fn new(pretty: bool, use_colors: bool, indent: usize) -> Self {
        Self {
            pretty,
            indent,
            use_colors,
            converter: JsonConverter::simplified(),
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
            ResultData::DocumentsWithPagination { documents, .. } => {
                self.format_documents(documents)
            }
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
            self.to_pretty_string(&json_docs)
                .unwrap_or_else(|_| format!("{:?}", docs))
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
            self.to_pretty_string(&json_value)
                .unwrap_or_else(|_| format!("{:?}", doc))
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

    /// Convert a value to pretty-printed JSON with custom indentation
    ///
    /// # Arguments
    /// * `value` - The value to serialize
    ///
    /// # Returns
    /// * `Result<String, serde_json::Error>` - Pretty JSON string with custom indent
    fn to_pretty_string<T: serde::Serialize>(
        &self,
        value: &T,
    ) -> std::result::Result<String, serde_json::Error> {
        let mut buf = Vec::new();
        let indent = " ".repeat(self.indent);
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        value.serialize(&mut ser)?;
        Ok(String::from_utf8(buf).unwrap())
    }

    /// Convert BSON document to simplified JSON
    ///
    /// Converts BSON types to human-readable JSON using the JsonConverter
    fn bson_to_simplified_json(&self, doc: &Document) -> serde_json::Value {
        self.converter.convert(&Bson::Document(doc.clone()))
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new(true, false, 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn test_json_formatter() {
        let formatter = JsonFormatter::new(false, false, 2);
        let doc = doc! { "name": "test", "value": 42 };
        let result = formatter.format_document(&doc).unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_json_formatter_simplified_objectid() {
        use mongodb::bson::oid::ObjectId;
        let formatter = JsonFormatter::new(true, false, 2);
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
        let formatter = JsonFormatter::new(true, false, 2);
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
        let formatter = JsonFormatter::new(true, false, 2);
        let doc = doc! { "user_id": 1i64 };
        let result = formatter.format_document(&doc).unwrap();
        // Should be a number, not Long('1')
        assert!(result.contains("\"user_id\""));
        assert!(result.contains("1"));
        assert!(!result.contains("Long"));
    }

    #[test]
    fn test_json_formatter_complete_document() {
        use mongodb::bson::{DateTime, oid::ObjectId};
        let formatter = JsonFormatter::new(true, false, 2);
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
        let formatter = JsonFormatter::new(false, false, 2);
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
        let compact = JsonFormatter::new(false, false, 2);
        let pretty = JsonFormatter::new(true, false, 2);
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
