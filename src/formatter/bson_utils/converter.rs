//! Core converter traits for BSON value conversion
//!
//! This module defines the traits that all BSON converters must implement.

use mongodb::bson::{Bson, Document};
use serde_json::Value as JsonValue;

/// Core trait for BSON value conversion
///
/// This trait allows different conversion strategies to be implemented
/// for various output formats (string, JSON, etc.)
pub trait BsonConverter {
    /// Output type of the conversion
    type Output;

    /// Convert a BSON value to the output type
    ///
    /// # Arguments
    /// * `value` - BSON value to convert
    ///
    /// # Returns
    /// Converted value in the target format
    fn convert(&self, value: &Bson) -> Self::Output;

    /// Convert an optional BSON value
    ///
    /// # Arguments
    /// * `value` - Optional BSON value to convert
    ///
    /// # Returns
    /// Converted value or default for None
    fn convert_optional(&self, value: Option<&Bson>) -> Self::Output
    where
        Self::Output: Default,
    {
        value.map(|v| self.convert(v)).unwrap_or_default()
    }

    /// Convert a BSON document
    ///
    /// # Arguments
    /// * `doc` - BSON document to convert
    ///
    /// # Returns
    /// Converted document representation
    #[allow(dead_code)]
    fn convert_document(&self, doc: &Document) -> Self::Output {
        self.convert(&Bson::Document(doc.clone()))
    }
}

/// Extended trait for string-based BSON converters
///
/// This trait provides a default implementation using pattern matching
/// and delegates to specialized methods for each BSON type.
pub trait BsonStringConverter {
    // Required methods for each BSON type
    fn format_string(&self, s: &str) -> String;
    fn format_int32(&self, n: i32) -> String;
    fn format_int64(&self, n: i64) -> String;
    fn format_double(&self, f: f64) -> String;
    fn format_boolean(&self, b: bool) -> String;
    fn format_null(&self) -> String;
    fn format_object_id(&self, oid: &mongodb::bson::oid::ObjectId) -> String;
    fn format_datetime(&self, dt: &mongodb::bson::DateTime) -> String;
    fn format_decimal128(&self, d: &mongodb::bson::Decimal128) -> String;
    fn format_array(&self, arr: &[Bson]) -> String;
    fn format_document(&self, doc: &Document) -> String;
    fn format_binary(&self, bin: &mongodb::bson::Binary) -> String;
    fn format_regex(&self, regex: &mongodb::bson::Regex) -> String;
    fn format_timestamp(&self, ts: &mongodb::bson::Timestamp) -> String;
    fn format_undefined(&self) -> String;
    fn format_min_key(&self) -> String;
    fn format_max_key(&self) -> String;
    fn format_unknown(&self, value: &Bson) -> String;

    /// Convert BSON value to string (provided implementation)
    fn convert_to_string(&self, value: &Bson) -> String {
        match value {
            Bson::String(s) => self.format_string(s),
            Bson::Int32(n) => self.format_int32(*n),
            Bson::Int64(n) => self.format_int64(*n),
            Bson::Double(f) => self.format_double(*f),
            Bson::Boolean(b) => self.format_boolean(*b),
            Bson::Null => self.format_null(),
            Bson::ObjectId(oid) => self.format_object_id(oid),
            Bson::DateTime(dt) => self.format_datetime(dt),
            Bson::Decimal128(d) => self.format_decimal128(d),
            Bson::Array(arr) => self.format_array(arr),
            Bson::Document(doc) => self.format_document(doc),
            Bson::Binary(bin) => self.format_binary(bin),
            Bson::RegularExpression(regex) => self.format_regex(regex),
            Bson::Timestamp(ts) => self.format_timestamp(ts),
            Bson::Undefined => self.format_undefined(),
            Bson::MinKey => self.format_min_key(),
            Bson::MaxKey => self.format_max_key(),
            _ => self.format_unknown(value),
        }
    }
}

/// Trait for JSON conversion
pub trait BsonJsonConverter {
    // Methods for JSON-specific conversions
    fn convert_object_id(&self, oid: &mongodb::bson::oid::ObjectId) -> JsonValue;
    fn convert_datetime(&self, dt: &mongodb::bson::DateTime) -> JsonValue;
    fn convert_decimal128(&self, d: &mongodb::bson::Decimal128) -> JsonValue;
    fn convert_array(&self, arr: &[Bson]) -> JsonValue;
    fn convert_document_to_json(&self, doc: &Document) -> JsonValue;
    fn convert_binary(&self, bin: &mongodb::bson::Binary) -> JsonValue;
    fn convert_regex(&self, regex: &mongodb::bson::Regex) -> JsonValue;
    fn convert_timestamp(&self, ts: &mongodb::bson::Timestamp) -> JsonValue;

    /// Convert BSON value to JSON (provided implementation)
    fn convert_to_json(&self, value: &Bson) -> JsonValue {
        match value {
            Bson::String(s) => JsonValue::String(s.clone()),
            Bson::Int32(n) => JsonValue::Number((*n).into()),
            Bson::Int64(n) => JsonValue::Number((*n).into()),
            Bson::Double(f) => serde_json::Number::from_f64(*f)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            Bson::Boolean(b) => JsonValue::Bool(*b),
            Bson::Null => JsonValue::Null,
            Bson::ObjectId(oid) => self.convert_object_id(oid),
            Bson::DateTime(dt) => self.convert_datetime(dt),
            Bson::Decimal128(d) => self.convert_decimal128(d),
            Bson::Array(arr) => self.convert_array(arr),
            Bson::Document(doc) => self.convert_document_to_json(doc),
            Bson::Binary(bin) => self.convert_binary(bin),
            Bson::RegularExpression(regex) => self.convert_regex(regex),
            Bson::Timestamp(ts) => self.convert_timestamp(ts),
            Bson::Undefined => JsonValue::Null,
            Bson::MinKey => JsonValue::String("MinKey".to_string()),
            Bson::MaxKey => JsonValue::String("MaxKey".to_string()),
            _ => JsonValue::String(format!("{:?}", value)),
        }
    }
}
