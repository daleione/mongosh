//! Shell-style formatting for MongoDB documents
//!
//! This module provides shell-style formatting compatible with mongosh:
//! - BSON value formatting with type wrappers (ObjectId, ISODate, Long, etc.)
//! - Pretty-printed nested documents and arrays
//! - Optional color highlighting for different value types
//! - Indentation support for readable output

use mongodb::bson::{Bson, Document};

use super::bson_utils::{BsonConverter, ShellStyleConverter};
use super::colorizer::Colorizer;

/// Shell-style formatter (mongosh compatible)
pub struct ShellFormatter {
    /// Converter for BSON values
    converter: ShellStyleConverter,

    /// Colorizer for output highlighting (kept for compatibility)
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
            converter: ShellStyleConverter::new(use_colors),
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
            let formatted_value = self.converter.convert_with_indent(value, indent_level + 1);
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
    ///
    /// # Arguments
    /// * `value` - BSON value to format
    ///
    /// # Returns
    /// * `String` - Formatted value
    #[allow(dead_code)]
    pub fn format_value(&self, value: &Bson) -> String {
        self.converter.convert(value)
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
