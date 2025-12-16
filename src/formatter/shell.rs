//! Shell-style formatting for MongoDB documents
//!
//! This module provides shell-style formatting compatible with mongosh:
//! - BSON value formatting with type wrappers (ObjectId, ISODate, Long, etc.)
//! - Pretty-printed nested documents and arrays
//! - Optional color highlighting for different value types
//! - Indentation support for readable output

use mongodb::bson::{Bson, Document};

use super::colorizer::Colorizer;

/// Shell-style formatter (mongosh compatible)
pub struct ShellFormatter {
    /// Colorizer for output highlighting
    colorizer: Colorizer,

    /// Indentation level
    indent: usize,
}

impl ShellFormatter {
    /// Create a new shell formatter
    ///
    /// # Arguments
    /// * `use_colors` - Enable colored output
    ///
    /// # Returns
    /// * `Self` - New formatter
    pub fn new(use_colors: bool) -> Self {
        Self {
            colorizer: Colorizer::new(use_colors),
            indent: 2,
        }
    }

    /// Format a BSON document in shell style
    ///
    /// # Arguments
    /// * `doc` - Document to format
    ///
    /// # Returns
    /// * `String` - Formatted document
    pub fn format_document(&self, doc: &Document) -> String {
        self.format_document_with_indent(doc, 0)
    }

    /// Format a BSON document with indentation
    fn format_document_with_indent(&self, doc: &Document, indent_level: usize) -> String {
        if doc.is_empty() {
            return "{}".to_string();
        }

        let mut result = String::from("{\n");
        let indent = " ".repeat((indent_level + 1) * self.indent);

        let entries: Vec<_> = doc.iter().collect();
        for (i, (key, value)) in entries.iter().enumerate() {
            let formatted_value = self.format_bson_value(value, indent_level + 1);
            result.push_str(&indent);

            // Key without quotes (shell style)
            result.push_str(&self.colorizer.field_key(key));

            result.push_str(": ");
            result.push_str(&formatted_value);

            // Add comma except for last item
            if i < entries.len() - 1 {
                result.push(',');
            }
            result.push('\n');
        }

        result.push_str(&" ".repeat(indent_level * self.indent));
        result.push('}');
        result
    }

    /// Format a BSON value in shell style
    fn format_bson_value(&self, value: &Bson, indent_level: usize) -> String {
        match value {
            Bson::ObjectId(oid) => self.colorizer.type_wrapper("ObjectId", &oid.to_string()),
            Bson::DateTime(dt) => {
                // Convert to ISO 8601 format
                let iso = dt.try_to_rfc3339_string().unwrap_or_else(|_| {
                    // Fallback to timestamp if conversion fails
                    format!("{}", dt.timestamp_millis())
                });
                self.colorizer.iso_date(&iso)
            }
            Bson::Int64(n) => self.colorizer.type_wrapper("Long", &n.to_string()),
            Bson::Decimal128(d) => self.colorizer.type_wrapper("NumberDecimal", &d.to_string()),
            Bson::String(s) => self.colorizer.string(s),
            Bson::Int32(n) => self.colorizer.number(&n.to_string()),
            Bson::Double(f) => self.colorizer.number(&f.to_string()),
            Bson::Boolean(b) => self.colorizer.number(&b.to_string()),
            Bson::Null => self.colorizer.null("null"),
            Bson::Array(arr) => self.format_array(arr, indent_level),
            Bson::Document(doc) => self.format_document_with_indent(doc, indent_level),
            Bson::Binary(bin) => {
                // Convert BinarySubtype to u8
                let subtype_num = match bin.subtype {
                    mongodb::bson::spec::BinarySubtype::Generic => 0u8,
                    mongodb::bson::spec::BinarySubtype::Function => 1u8,
                    mongodb::bson::spec::BinarySubtype::BinaryOld => 2u8,
                    mongodb::bson::spec::BinarySubtype::UuidOld => 3u8,
                    mongodb::bson::spec::BinarySubtype::Uuid => 4u8,
                    mongodb::bson::spec::BinarySubtype::Md5 => 5u8,
                    mongodb::bson::spec::BinarySubtype::Encrypted => 6u8,
                    mongodb::bson::spec::BinarySubtype::Column => 7u8,
                    mongodb::bson::spec::BinarySubtype::Sensitive => 8u8,
                    mongodb::bson::spec::BinarySubtype::UserDefined(n) => n,
                    _ => 0u8, // Default to generic for unknown subtypes
                };
                self.colorizer
                    .bin_data(subtype_num, &hex::encode(&bin.bytes))
            }
            Bson::RegularExpression(regex) => self.colorizer.regex(&regex.pattern, &regex.options),
            Bson::Timestamp(ts) => self.colorizer.timestamp(ts.time, ts.increment),
            _ => format!("{:?}", value),
        }
    }

    /// Format a BSON array in shell style
    fn format_array(&self, arr: &[Bson], indent_level: usize) -> String {
        if arr.is_empty() {
            return "[]".to_string();
        }

        let mut result = String::from("[\n");
        let indent = " ".repeat((indent_level + 1) * self.indent);

        for (i, value) in arr.iter().enumerate() {
            result.push_str(&indent);
            result.push_str(&self.format_bson_value(value, indent_level + 1));

            if i < arr.len() - 1 {
                result.push(',');
            }
            result.push('\n');
        }

        result.push_str(&" ".repeat(indent_level * self.indent));
        result.push(']');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn test_shell_formatter_objectid() {
        use mongodb::bson::oid::ObjectId;
        let formatter = ShellFormatter::new(false);
        let oid = ObjectId::parse_str("65705d84dfc3f3b5094e1f72").unwrap();
        let doc = doc! { "_id": oid };
        let result = formatter.format_document(&doc);
        assert!(result.contains("_id:"));
        assert!(result.contains("ObjectId('65705d84dfc3f3b5094e1f72')"));
    }

    #[test]
    fn test_shell_formatter_datetime() {
        use mongodb::bson::DateTime;
        let formatter = ShellFormatter::new(false);
        let dt = DateTime::from_millis(1701862788373);
        let doc = doc! { "created_time": dt };
        let result = formatter.format_document(&doc);
        assert!(result.contains("created_time:"));
        assert!(result.contains("ISODate("));
        assert!(result.contains("2023-12-06"));
    }

    #[test]
    fn test_shell_formatter_long() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "user_id": 1i64 };
        let result = formatter.format_document(&doc);
        assert!(result.contains("user_id:"));
        assert!(result.contains("Long('1')"));
    }

    #[test]
    fn test_shell_formatter_string() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "nickname": "dalei" };
        let result = formatter.format_document(&doc);
        assert!(result.contains("nickname:"));
        assert!(result.contains("'dalei'"));
    }

    #[test]
    fn test_shell_formatter_null() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "oauth2": null };
        let result = formatter.format_document(&doc);
        assert!(result.contains("oauth2:"));
        assert!(result.contains("null"));
    }

    #[test]
    fn test_shell_formatter_nested_document() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! {
            "user": {
                "name": "test",
                "age": 25
            }
        };
        let result = formatter.format_document(&doc);
        assert!(result.contains("user:"));
        assert!(result.contains("name:"));
        assert!(result.contains("'test'"));
        assert!(result.contains("age:"));
        assert!(result.contains("25"));
    }

    #[test]
    fn test_shell_formatter_array() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "tags": ["rust", "mongodb"] };
        let result = formatter.format_document(&doc);
        assert!(result.contains("tags:"));
        assert!(result.contains("'rust'"));
        assert!(result.contains("'mongodb'"));
    }
}
