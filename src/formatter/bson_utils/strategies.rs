//! Strategy implementations for BSON conversion
//!
//! This module provides concrete implementations of the converter traits:
//! - PlainTextConverter: Simple string conversion for data export
//! - ShellStyleConverter: MongoDB shell-style formatting with colors
//! - CompactConverter: Compact display for table cells
//! - JsonConverter: JSON value conversion

use mongodb::bson::{
    Binary, Bson, DateTime, Decimal128, Document, Regex, Timestamp, oid::ObjectId,
};
use serde_json::Value as JsonValue;

use super::converter::{BsonConverter, BsonJsonConverter, BsonStringConverter};
use super::helpers::*;
use crate::formatter::colorizer::Colorizer;

/// Plain text converter for simple string conversion
///
/// Used primarily for data export (CSV, Excel, etc.)
/// Provides straightforward string representation without formatting
pub struct PlainTextConverter;

impl PlainTextConverter {
    /// Create a new plain text converter
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlainTextConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl BsonConverter for PlainTextConverter {
    type Output = String;

    fn convert(&self, value: &Bson) -> String {
        self.convert_to_string(value)
    }
}

impl BsonStringConverter for PlainTextConverter {
    fn format_string(&self, s: &str) -> String {
        s.to_string()
    }

    fn format_int32(&self, n: i32) -> String {
        n.to_string()
    }

    fn format_int64(&self, n: i64) -> String {
        n.to_string()
    }

    fn format_double(&self, f: f64) -> String {
        f.to_string()
    }

    fn format_boolean(&self, b: bool) -> String {
        b.to_string()
    }

    fn format_null(&self) -> String {
        String::new()
    }

    fn format_object_id(&self, oid: &ObjectId) -> String {
        oid.to_string()
    }

    fn format_datetime(&self, dt: &DateTime) -> String {
        dt.to_string()
    }

    fn format_decimal128(&self, d: &Decimal128) -> String {
        d.to_string()
    }

    fn format_array(&self, arr: &[Bson]) -> String {
        format!("{:?}", arr)
    }

    fn format_document(&self, doc: &Document) -> String {
        serde_json::to_string(doc).unwrap_or_else(|_| "{}".to_string())
    }

    fn format_binary(&self, bin: &Binary) -> String {
        binary_to_hex(bin)
    }

    fn format_regex(&self, regex: &Regex) -> String {
        format!("/{}/{}", regex.pattern, regex.options)
    }

    fn format_timestamp(&self, ts: &Timestamp) -> String {
        format!("Timestamp({}, {})", ts.time, ts.increment)
    }

    fn format_undefined(&self) -> String {
        String::from("undefined")
    }

    fn format_min_key(&self) -> String {
        String::from("MinKey")
    }

    fn format_max_key(&self) -> String {
        String::from("MaxKey")
    }

    fn format_unknown(&self, value: &Bson) -> String {
        format!("{:?}", value)
    }
}

/// Shell-style converter for MongoDB shell-compatible output
///
/// Formats BSON values with type wrappers (ObjectId(), ISODate(), etc.)
/// and supports color highlighting
pub struct ShellStyleConverter {
    colorizer: Colorizer,
    indent: usize,
}

impl ShellStyleConverter {
    /// Create a new shell-style converter
    ///
    /// # Arguments
    /// * `use_colors` - Enable colored output
    pub fn new(use_colors: bool) -> Self {
        Self {
            colorizer: Colorizer::new(use_colors),
            indent: 2,
        }
    }

    /// Create a new shell-style converter with custom indent
    ///
    /// # Arguments
    /// * `use_colors` - Enable colored output
    /// * `indent` - Indentation spaces per level
    #[allow(dead_code)]
    pub fn with_indent(use_colors: bool, indent: usize) -> Self {
        Self {
            colorizer: Colorizer::new(use_colors),
            indent,
        }
    }

    /// Format array with indentation
    fn format_array_with_indent(&self, arr: &[Bson], indent_level: usize) -> String {
        if arr.is_empty() {
            return "[]".to_string();
        }

        let mut result = String::from("[\n");
        let indent = " ".repeat((indent_level + 1) * self.indent);

        for (i, value) in arr.iter().enumerate() {
            result.push_str(&indent);
            result.push_str(&self.convert_with_indent(value, indent_level + 1));

            if i < arr.len() - 1 {
                result.push(',');
            }
            result.push('\n');
        }

        result.push_str(&" ".repeat(indent_level * self.indent));
        result.push(']');
        result
    }

    /// Format document with indentation
    fn format_document_with_indent(&self, doc: &Document, indent_level: usize) -> String {
        if doc.is_empty() {
            return "{}".to_string();
        }

        let mut result = String::from("{\n");
        let indent = " ".repeat((indent_level + 1) * self.indent);
        let entries: Vec<_> = doc.iter().collect();

        for (i, (key, value)) in entries.iter().enumerate() {
            result.push_str(&indent);
            result.push_str(&self.colorizer.field_key(key));
            result.push_str(": ");
            result.push_str(&self.convert_with_indent(value, indent_level + 1));

            if i < entries.len() - 1 {
                result.push(',');
            }
            result.push('\n');
        }

        result.push_str(&" ".repeat(indent_level * self.indent));
        result.push('}');
        result
    }

    /// Convert with specific indent level
    pub fn convert_with_indent(&self, value: &Bson, indent_level: usize) -> String {
        match value {
            Bson::Array(arr) => self.format_array_with_indent(arr, indent_level),
            Bson::Document(doc) => self.format_document_with_indent(doc, indent_level),
            _ => self.convert_to_string(value),
        }
    }
}

impl BsonConverter for ShellStyleConverter {
    type Output = String;

    fn convert(&self, value: &Bson) -> String {
        self.convert_to_string(value)
    }
}

impl BsonStringConverter for ShellStyleConverter {
    fn format_string(&self, s: &str) -> String {
        self.colorizer.string(s)
    }

    fn format_int32(&self, n: i32) -> String {
        self.colorizer.number(&n.to_string())
    }

    fn format_int64(&self, n: i64) -> String {
        self.colorizer.type_wrapper("Long", &n.to_string())
    }

    fn format_double(&self, f: f64) -> String {
        self.colorizer.number(&f.to_string())
    }

    fn format_boolean(&self, b: bool) -> String {
        self.colorizer.number(&b.to_string())
    }

    fn format_null(&self) -> String {
        self.colorizer.null("null")
    }

    fn format_object_id(&self, oid: &ObjectId) -> String {
        self.colorizer.type_wrapper("ObjectId", &oid.to_string())
    }

    fn format_datetime(&self, dt: &DateTime) -> String {
        let iso = datetime_to_iso_string(dt);
        self.colorizer.iso_date(&iso)
    }

    fn format_decimal128(&self, d: &Decimal128) -> String {
        self.colorizer.type_wrapper("NumberDecimal", &d.to_string())
    }

    fn format_array(&self, arr: &[Bson]) -> String {
        self.format_array_with_indent(arr, 0)
    }

    fn format_document(&self, doc: &Document) -> String {
        self.format_document_with_indent(doc, 0)
    }

    fn format_binary(&self, bin: &Binary) -> String {
        let subtype_num = binary_subtype_to_u8(bin.subtype);
        self.colorizer.bin_data(subtype_num, &binary_to_hex(bin))
    }

    fn format_regex(&self, regex: &Regex) -> String {
        self.colorizer.regex(&regex.pattern, &regex.options)
    }

    fn format_timestamp(&self, ts: &Timestamp) -> String {
        self.colorizer.timestamp(ts.time, ts.increment)
    }

    fn format_undefined(&self) -> String {
        String::from("undefined")
    }

    fn format_min_key(&self) -> String {
        String::from("MinKey")
    }

    fn format_max_key(&self) -> String {
        String::from("MaxKey")
    }

    fn format_unknown(&self, value: &Bson) -> String {
        format!("{:?}", value)
    }
}

/// Compact converter for table cell display
///
/// Provides compact representation suitable for table cells:
/// - Shows array/document sizes instead of full content
/// - Truncates long values
/// - No color formatting
pub struct CompactConverter {
    max_inline_items: usize,
    max_inline_fields: usize,
}

impl CompactConverter {
    /// Create a new compact converter with default settings
    pub fn new() -> Self {
        Self {
            max_inline_items: 3,
            max_inline_fields: 2,
        }
    }

    /// Create a new compact converter with custom settings
    ///
    /// # Arguments
    /// * `max_inline_items` - Max array items to show inline
    /// * `max_inline_fields` - Max document fields to show inline
    #[allow(dead_code)]
    pub fn with_limits(max_inline_items: usize, max_inline_fields: usize) -> Self {
        Self {
            max_inline_items,
            max_inline_fields,
        }
    }
}

impl Default for CompactConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl BsonConverter for CompactConverter {
    type Output = String;

    fn convert(&self, value: &Bson) -> String {
        self.convert_to_string(value)
    }
}

impl BsonStringConverter for CompactConverter {
    fn format_string(&self, s: &str) -> String {
        s.to_string()
    }

    fn format_int32(&self, n: i32) -> String {
        n.to_string()
    }

    fn format_int64(&self, n: i64) -> String {
        format!("Long('{}')", n)
    }

    fn format_double(&self, f: f64) -> String {
        format_double_smart(f)
    }

    fn format_boolean(&self, b: bool) -> String {
        b.to_string()
    }

    fn format_null(&self) -> String {
        String::from("null")
    }

    fn format_object_id(&self, oid: &ObjectId) -> String {
        format!("ObjectId('{}')", oid)
    }

    fn format_datetime(&self, dt: &DateTime) -> String {
        let iso = datetime_to_iso_string(dt);
        format!("ISODate('{}')", iso)
    }

    fn format_decimal128(&self, d: &Decimal128) -> String {
        format!("NumberDecimal('{}')", d)
    }

    fn format_array(&self, arr: &[Bson]) -> String {
        if arr.is_empty() {
            String::from("[]")
        } else if should_inline_array(arr, self.max_inline_items) {
            let items: Vec<String> = arr.iter().map(|v| self.convert_to_string(v)).collect();
            format!("[{}]", items.join(", "))
        } else {
            format!("[Array({})]", arr.len())
        }
    }

    fn format_document(&self, doc: &Document) -> String {
        if doc.is_empty() {
            String::from("{}")
        } else if should_inline_document(doc, self.max_inline_fields) {
            let fields: Vec<String> = doc
                .iter()
                .map(|(k, v)| format!("{}: {}", k, self.convert_to_string(v)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        } else {
            format!("{{Object({})}}", doc.len())
        }
    }

    fn format_binary(&self, bin: &Binary) -> String {
        let hex = binary_to_hex(bin);
        if hex.len() > 16 {
            format!("Binary({}...)", &hex[..16])
        } else {
            format!("Binary({})", hex)
        }
    }

    fn format_regex(&self, regex: &Regex) -> String {
        format!("/{}/{}", regex.pattern, regex.options)
    }

    fn format_timestamp(&self, ts: &Timestamp) -> String {
        format!("Timestamp({}, {})", ts.time, ts.increment)
    }

    fn format_undefined(&self) -> String {
        String::from("undefined")
    }

    fn format_min_key(&self) -> String {
        String::from("MinKey")
    }

    fn format_max_key(&self) -> String {
        String::from("MaxKey")
    }

    fn format_unknown(&self, value: &Bson) -> String {
        format!("{:?}", value)
    }
}

/// JSON value converter
///
/// Converts BSON values to standard JSON (serde_json::Value)
/// Handles BSON-specific types appropriately
pub struct JsonConverter {
    /// Whether to simplify BSON types (true) or preserve extended JSON (false)
    simplify: bool,
}

impl JsonConverter {
    /// Create a new JSON converter
    ///
    /// # Arguments
    /// * `simplify` - If true, convert BSON types to simple JSON types
    pub fn new(simplify: bool) -> Self {
        Self { simplify }
    }

    /// Create a simplified JSON converter (default)
    pub fn simplified() -> Self {
        Self::new(true)
    }

    /// Create an extended JSON converter
    #[allow(dead_code)]
    pub fn extended() -> Self {
        Self::new(false)
    }
}

impl Default for JsonConverter {
    fn default() -> Self {
        Self::simplified()
    }
}

impl BsonConverter for JsonConverter {
    type Output = JsonValue;

    fn convert(&self, value: &Bson) -> JsonValue {
        self.convert_to_json(value)
    }
}

impl BsonJsonConverter for JsonConverter {
    fn convert_object_id(&self, oid: &ObjectId) -> JsonValue {
        JsonValue::String(oid.to_string())
    }

    fn convert_datetime(&self, dt: &DateTime) -> JsonValue {
        let iso = datetime_to_iso_string(dt);
        JsonValue::String(iso)
    }

    fn convert_decimal128(&self, d: &Decimal128) -> JsonValue {
        let s = d.to_string();
        if self.simplify {
            // Try to convert to number
            s.parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(JsonValue::Number)
                .unwrap_or_else(|| JsonValue::String(s))
        } else {
            JsonValue::String(s)
        }
    }

    fn convert_array(&self, arr: &[Bson]) -> JsonValue {
        let json_arr: Vec<JsonValue> = arr.iter().map(|v| self.convert_to_json(v)).collect();
        JsonValue::Array(json_arr)
    }

    fn convert_document_to_json(&self, doc: &Document) -> JsonValue {
        let mut map = serde_json::Map::new();
        for (key, value) in doc.iter() {
            map.insert(key.clone(), self.convert_to_json(value));
        }
        JsonValue::Object(map)
    }

    fn convert_binary(&self, bin: &Binary) -> JsonValue {
        JsonValue::String(binary_to_base64(bin))
    }

    fn convert_regex(&self, regex: &Regex) -> JsonValue {
        JsonValue::String(format!("/{}/{}", regex.pattern, regex.options))
    }

    fn convert_timestamp(&self, ts: &Timestamp) -> JsonValue {
        let millis = (ts.time as i64) * 1000 + (ts.increment as i64);
        JsonValue::Number(millis.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::{Bson, doc, oid::ObjectId};

    #[test]
    fn test_plain_text_converter() {
        let converter = PlainTextConverter::new();
        assert_eq!(converter.convert(&Bson::String("test".to_string())), "test");
        assert_eq!(converter.convert(&Bson::Int32(42)), "42");
        assert_eq!(converter.convert(&Bson::Int64(100)), "100");
        assert_eq!(converter.convert(&Bson::Boolean(true)), "true");
        assert_eq!(converter.convert(&Bson::Null), "");
    }

    #[test]
    fn test_shell_style_converter() {
        let converter = ShellStyleConverter::new(false);
        let result = converter.convert(&Bson::Int64(42));
        assert!(result.contains("Long"));
        assert!(result.contains("42"));
    }

    #[test]
    fn test_shell_style_object_id() {
        let converter = ShellStyleConverter::new(false);
        let oid = ObjectId::new();
        let result = converter.convert(&Bson::ObjectId(oid));
        assert!(result.contains("ObjectId"));
    }

    #[test]
    fn test_compact_converter() {
        let converter = CompactConverter::new();

        // Small array should be inline
        let small_arr = Bson::Array(vec![Bson::Int32(1), Bson::Int32(2)]);
        let result = converter.convert(&small_arr);
        assert!(result.contains("["));
        assert!(result.contains("1"));

        // Large array should show count
        let large_arr = Bson::Array(vec![
            Bson::Int32(1),
            Bson::Int32(2),
            Bson::Int32(3),
            Bson::Int32(4),
            Bson::Int32(5),
        ]);
        let result = converter.convert(&large_arr);
        assert!(result.contains("Array(5)"));
    }

    #[test]
    fn test_compact_document() {
        let converter = CompactConverter::new();

        // Small document should be inline
        let small_doc = doc! { "a": 1 };
        let result = converter.convert(&Bson::Document(small_doc));
        assert!(result.contains("a"));

        // Large document should show count
        let large_doc = doc! { "a": 1, "b": 2, "c": 3, "d": 4 };
        let result = converter.convert(&Bson::Document(large_doc));
        assert!(result.contains("Object(4)"));
    }

    #[test]
    fn test_json_converter() {
        let converter = JsonConverter::new(true);

        let result = converter.convert(&Bson::String("test".to_string()));
        assert_eq!(result, JsonValue::String("test".to_string()));

        let result = converter.convert(&Bson::Int32(42));
        assert_eq!(result, JsonValue::Number(42.into()));

        let result = converter.convert(&Bson::Boolean(true));
        assert_eq!(result, JsonValue::Bool(true));

        let result = converter.convert(&Bson::Null);
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn test_json_converter_array() {
        let converter = JsonConverter::new(true);
        let arr = vec![Bson::Int32(1), Bson::Int32(2), Bson::Int32(3)];
        let result = converter.convert(&Bson::Array(arr));

        if let JsonValue::Array(arr) = result {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected JSON array");
        }
    }

    #[test]
    fn test_json_converter_document() {
        let converter = JsonConverter::new(true);
        let doc = doc! { "name": "test", "value": 42 };
        let result = converter.convert(&Bson::Document(doc));

        if let JsonValue::Object(obj) = result {
            assert_eq!(obj.len(), 2);
            assert!(obj.contains_key("name"));
            assert!(obj.contains_key("value"));
        } else {
            panic!("Expected JSON object");
        }
    }
}
