//! Helper functions for BSON value conversion
//!
//! This module provides common utility functions used by various BSON converters.

use mongodb::bson::{Binary, Bson, DateTime, Document, spec::BinarySubtype};

/// Convert DateTime to ISO 8601 string
///
/// # Arguments
/// * `dt` - BSON DateTime value
///
/// # Returns
/// ISO 8601 formatted string or timestamp fallback
pub fn datetime_to_iso_string(dt: &DateTime) -> String {
    dt.try_to_rfc3339_string()
        .unwrap_or_else(|_| format!("{}", dt.timestamp_millis()))
}

/// Convert Binary data to hexadecimal string
///
/// # Arguments
/// * `bin` - BSON Binary value
///
/// # Returns
/// Hexadecimal string representation
pub fn binary_to_hex(bin: &Binary) -> String {
    hex::encode(&bin.bytes)
}

/// Convert Binary data to Base64 string
///
/// # Arguments
/// * `bin` - BSON Binary value
///
/// # Returns
/// Base64 encoded string
pub fn binary_to_base64(bin: &Binary) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(&bin.bytes)
}

/// Convert BinarySubtype to u8 number
///
/// # Arguments
/// * `subtype` - BSON Binary subtype
///
/// # Returns
/// Numeric representation of the subtype
pub fn binary_subtype_to_u8(subtype: BinarySubtype) -> u8 {
    match subtype {
        BinarySubtype::Generic => 0,
        BinarySubtype::Function => 1,
        BinarySubtype::BinaryOld => 2,
        BinarySubtype::UuidOld => 3,
        BinarySubtype::Uuid => 4,
        BinarySubtype::Md5 => 5,
        BinarySubtype::Encrypted => 6,
        BinarySubtype::Column => 7,
        BinarySubtype::Sensitive => 8,
        BinarySubtype::UserDefined(n) => n,
        _ => 0, // Default to generic for unknown subtypes
    }
}

/// Check if array should be displayed inline (not too large)
///
/// # Arguments
/// * `arr` - BSON array
/// * `max_items` - Maximum items for inline display
///
/// # Returns
/// True if array should be displayed inline
pub fn should_inline_array(arr: &[Bson], max_items: usize) -> bool {
    arr.len() <= max_items
}

/// Check if document should be displayed inline (not too large)
///
/// # Arguments
/// * `doc` - BSON document
/// * `max_fields` - Maximum fields for inline display
///
/// # Returns
/// True if document should be displayed inline
pub fn should_inline_document(doc: &Document, max_fields: usize) -> bool {
    doc.len() <= max_fields
}

/// Format double with reasonable precision
///
/// # Arguments
/// * `f` - Double value
///
/// # Returns
/// Formatted string
pub fn format_double_smart(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e10 {
        format!("{:.0}", f)
    } else {
        format!("{}", f)
    }
}

/// Truncate string with ellipsis if too long
///
/// # Arguments
/// * `s` - Input string
/// * `max_len` - Maximum length
///
/// # Returns
/// Truncated string with "..." if needed
#[allow(dead_code)]
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::{DateTime as BsonDateTime, doc};

    #[test]
    fn test_datetime_to_iso_string() {
        let dt = BsonDateTime::now();
        let iso = datetime_to_iso_string(&dt);
        assert!(!iso.is_empty());
    }

    #[test]
    fn test_binary_to_hex() {
        let bin = Binary {
            subtype: BinarySubtype::Generic,
            bytes: vec![0x01, 0x02, 0x03, 0xff],
        };
        assert_eq!(binary_to_hex(&bin), "010203ff");
    }

    #[test]
    fn test_binary_to_base64() {
        let bin = Binary {
            subtype: BinarySubtype::Generic,
            bytes: vec![0x01, 0x02, 0x03],
        };
        let base64 = binary_to_base64(&bin);
        assert!(!base64.is_empty());
    }

    #[test]
    fn test_binary_subtype_to_u8() {
        assert_eq!(binary_subtype_to_u8(BinarySubtype::Generic), 0);
        assert_eq!(binary_subtype_to_u8(BinarySubtype::Function), 1);
        assert_eq!(binary_subtype_to_u8(BinarySubtype::Uuid), 4);
        assert_eq!(binary_subtype_to_u8(BinarySubtype::UserDefined(42)), 42);
    }

    #[test]
    fn test_should_inline_array() {
        let arr = vec![Bson::Int32(1), Bson::Int32(2)];
        assert!(should_inline_array(&arr, 3));
        assert!(!should_inline_array(&arr, 1));
    }

    #[test]
    fn test_should_inline_document() {
        let doc = doc! { "a": 1, "b": 2 };
        assert!(should_inline_document(&doc, 3));
        assert!(!should_inline_document(&doc, 1));
    }

    #[test]
    fn test_format_double_smart() {
        assert_eq!(format_double_smart(42.0), "42");
        assert_eq!(format_double_smart(42.5), "42.5");
        assert!(format_double_smart(3.14159).starts_with("3.14"));
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 5), "hi");
    }
}
