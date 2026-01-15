//! Comprehensive tests for BSON utilities module

use super::helpers::*;
use super::*;
use mongodb::bson::{
    Binary, Bson, DateTime, Decimal128, Regex, Timestamp, doc, oid::ObjectId, spec::BinarySubtype,
};
use serde_json::Value as JsonValue;

// ===== Helper Function Tests =====

#[test]
fn test_datetime_to_iso_string() {
    let dt = DateTime::now();
    let iso = datetime_to_iso_string(&dt);
    assert!(!iso.is_empty());
    // ISO string should contain date separators
    assert!(iso.contains('-') || iso.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn test_binary_conversions() {
    let bin = Binary {
        subtype: BinarySubtype::Generic,
        bytes: vec![0x01, 0x02, 0x03, 0xff],
    };

    // Hex conversion
    let hex = binary_to_hex(&bin);
    assert_eq!(hex, "010203ff");

    // Base64 conversion
    let base64 = binary_to_base64(&bin);
    assert!(!base64.is_empty());
}

#[test]
fn test_binary_subtype_conversion() {
    assert_eq!(binary_subtype_to_u8(BinarySubtype::Generic), 0);
    assert_eq!(binary_subtype_to_u8(BinarySubtype::Function), 1);
    assert_eq!(binary_subtype_to_u8(BinarySubtype::BinaryOld), 2);
    assert_eq!(binary_subtype_to_u8(BinarySubtype::UuidOld), 3);
    assert_eq!(binary_subtype_to_u8(BinarySubtype::Uuid), 4);
    assert_eq!(binary_subtype_to_u8(BinarySubtype::Md5), 5);
    assert_eq!(binary_subtype_to_u8(BinarySubtype::UserDefined(42)), 42);
    assert_eq!(binary_subtype_to_u8(BinarySubtype::UserDefined(255)), 255);
}

#[test]
fn test_should_inline_array() {
    let empty = vec![];
    let small = vec![Bson::Int32(1), Bson::Int32(2)];
    let large = vec![
        Bson::Int32(1),
        Bson::Int32(2),
        Bson::Int32(3),
        Bson::Int32(4),
    ];

    assert!(should_inline_array(&empty, 3));
    assert!(should_inline_array(&small, 3));
    assert!(should_inline_array(&small, 2));
    assert!(!should_inline_array(&small, 1));
    assert!(!should_inline_array(&large, 3));
    assert!(should_inline_array(&large, 5));
}

#[test]
fn test_should_inline_document() {
    let empty = doc! {};
    let small = doc! { "a": 1, "b": 2 };
    let large = doc! { "a": 1, "b": 2, "c": 3, "d": 4 };

    assert!(should_inline_document(&empty, 3));
    assert!(should_inline_document(&small, 3));
    assert!(should_inline_document(&small, 2));
    assert!(!should_inline_document(&small, 1));
    assert!(!should_inline_document(&large, 3));
    assert!(should_inline_document(&large, 5));
}

#[test]
fn test_format_double_smart() {
    assert_eq!(format_double_smart(42.0), "42");
    assert_eq!(format_double_smart(0.0), "0");
    assert_eq!(format_double_smart(-42.0), "-42");
    assert_eq!(format_double_smart(42.5), "42.5");
    assert_eq!(format_double_smart(3.14159), "3.14159");
    assert_eq!(format_double_smart(-3.14159), "-3.14159");
}

#[test]
fn test_truncate_string() {
    assert_eq!(truncate_string("hello", 10), "hello");
    assert_eq!(truncate_string("hello world", 8), "hello...");
    assert_eq!(truncate_string("hi", 5), "hi");
    assert_eq!(truncate_string("", 5), "");
    assert_eq!(truncate_string("abcdefghij", 7), "abcd...");
}

// ===== PlainTextConverter Tests =====

#[test]
fn test_plain_text_basic_types() {
    let converter = PlainTextConverter::new();

    assert_eq!(converter.convert(&Bson::String("test".to_string())), "test");
    assert_eq!(converter.convert(&Bson::Int32(42)), "42");
    assert_eq!(converter.convert(&Bson::Int64(100)), "100");
    assert_eq!(converter.convert(&Bson::Double(3.14)), "3.14");
    assert_eq!(converter.convert(&Bson::Boolean(true)), "true");
    assert_eq!(converter.convert(&Bson::Boolean(false)), "false");
    assert_eq!(converter.convert(&Bson::Null), "");
}

#[test]
fn test_plain_text_object_id() {
    let converter = PlainTextConverter::new();
    let oid = ObjectId::new();
    let result = converter.convert(&Bson::ObjectId(oid));
    assert_eq!(result, oid.to_string());
}

#[test]
fn test_plain_text_datetime() {
    let converter = PlainTextConverter::new();
    let dt = DateTime::now();
    let result = converter.convert(&Bson::DateTime(dt));
    assert!(!result.is_empty());
}

#[test]
fn test_plain_text_decimal() {
    let converter = PlainTextConverter::new();
    let decimal = Decimal128::from_bytes([0u8; 16]);
    let result = converter.convert(&Bson::Decimal128(decimal));
    assert!(!result.is_empty());
}

#[test]
fn test_plain_text_binary() {
    let converter = PlainTextConverter::new();
    let bin = Binary {
        subtype: BinarySubtype::Generic,
        bytes: vec![0xde, 0xad, 0xbe, 0xef],
    };
    let result = converter.convert(&Bson::Binary(bin));
    assert_eq!(result, "deadbeef");
}

#[test]
fn test_plain_text_regex() {
    let converter = PlainTextConverter::new();
    let regex = Regex {
        pattern: "test".to_string(),
        options: "i".to_string(),
    };
    let result = converter.convert(&Bson::RegularExpression(regex));
    assert_eq!(result, "/test/i");
}

#[test]
fn test_plain_text_timestamp() {
    let converter = PlainTextConverter::new();
    let ts = Timestamp {
        time: 12345,
        increment: 67890,
    };
    let result = converter.convert(&Bson::Timestamp(ts));
    assert!(result.contains("12345"));
    assert!(result.contains("67890"));
}

#[test]
fn test_plain_text_optional() {
    let converter = PlainTextConverter::new();
    assert_eq!(converter.convert_optional(Some(&Bson::Int32(42))), "42");
    assert_eq!(converter.convert_optional(None), "");
}

// ===== ShellStyleConverter Tests =====

#[test]
fn test_shell_style_basic_types() {
    let converter = ShellStyleConverter::new(false);

    let result = converter.convert(&Bson::String("test".to_string()));
    assert!(result.contains("test"));

    let result = converter.convert(&Bson::Int32(42));
    assert!(result.contains("42"));

    let result = converter.convert(&Bson::Boolean(true));
    assert!(result.contains("true"));

    let result = converter.convert(&Bson::Null);
    assert!(result.contains("null"));
}

#[test]
fn test_shell_style_int64() {
    let converter = ShellStyleConverter::new(false);
    let result = converter.convert(&Bson::Int64(9876543210));
    assert!(result.contains("Long"));
    assert!(result.contains("9876543210"));
}

#[test]
fn test_shell_style_object_id() {
    let converter = ShellStyleConverter::new(false);
    let oid = ObjectId::new();
    let result = converter.convert(&Bson::ObjectId(oid));
    assert!(result.contains("ObjectId"));
    assert!(result.contains(&oid.to_string()));
}

#[test]
fn test_shell_style_datetime() {
    let converter = ShellStyleConverter::new(false);
    let dt = DateTime::now();
    let result = converter.convert(&Bson::DateTime(dt));
    assert!(!result.is_empty());
}

#[test]
fn test_shell_style_decimal() {
    let converter = ShellStyleConverter::new(false);
    let decimal = Decimal128::from_bytes([0u8; 16]);
    let result = converter.convert(&Bson::Decimal128(decimal));
    assert!(result.contains("NumberDecimal"));
}

#[test]
fn test_shell_style_array() {
    let converter = ShellStyleConverter::new(false);
    let arr = vec![Bson::Int32(1), Bson::Int32(2), Bson::Int32(3)];
    let result = converter.convert(&Bson::Array(arr));
    assert!(result.contains("["));
    assert!(result.contains("]"));
    assert!(result.contains("1"));
    assert!(result.contains("2"));
    assert!(result.contains("3"));
}

#[test]
fn test_shell_style_document() {
    let converter = ShellStyleConverter::new(false);
    let doc = doc! { "name": "test", "value": 42 };
    let result = converter.convert(&Bson::Document(doc));
    assert!(result.contains("{"));
    assert!(result.contains("}"));
    assert!(result.contains("name"));
    assert!(result.contains("test"));
    assert!(result.contains("value"));
    assert!(result.contains("42"));
}

#[test]
fn test_shell_style_nested_document() {
    let converter = ShellStyleConverter::new(false);
    let doc = doc! {
        "outer": {
            "inner": "value"
        }
    };
    let result = converter.convert(&Bson::Document(doc));
    assert!(result.contains("outer"));
    assert!(result.contains("inner"));
    assert!(result.contains("value"));
}

#[test]
fn test_shell_style_with_indent() {
    let converter = ShellStyleConverter::with_indent(false, 4);
    let doc = doc! { "a": 1 };
    let result = converter.convert_with_indent(&Bson::Document(doc), 1);
    assert!(result.contains("{"));
}

// ===== CompactConverter Tests =====

#[test]
fn test_compact_basic_types() {
    let converter = CompactConverter::new();

    assert_eq!(converter.convert(&Bson::String("test".to_string())), "test");
    assert_eq!(converter.convert(&Bson::Int32(42)), "42");
    assert_eq!(converter.convert(&Bson::Boolean(true)), "true");
    assert_eq!(converter.convert(&Bson::Null), "null");
}

#[test]
fn test_compact_int64() {
    let converter = CompactConverter::new();
    let result = converter.convert(&Bson::Int64(42));
    assert!(result.contains("Long"));
    assert!(result.contains("42"));
}

#[test]
fn test_compact_object_id() {
    let converter = CompactConverter::new();
    let oid = ObjectId::new();
    let result = converter.convert(&Bson::ObjectId(oid));
    assert!(result.contains("ObjectId"));
}

#[test]
fn test_compact_datetime() {
    let converter = CompactConverter::new();
    let dt = DateTime::now();
    let result = converter.convert(&Bson::DateTime(dt));
    assert!(result.contains("ISODate"));
}

#[test]
fn test_compact_decimal() {
    let converter = CompactConverter::new();
    let decimal = Decimal128::from_bytes([0u8; 16]);
    let result = converter.convert(&Bson::Decimal128(decimal));
    assert!(result.contains("NumberDecimal"));
}

#[test]
fn test_compact_small_array() {
    let converter = CompactConverter::new();
    let arr = vec![Bson::Int32(1), Bson::Int32(2)];
    let result = converter.convert(&Bson::Array(arr));
    assert!(result.contains("["));
    assert!(result.contains("1"));
    assert!(result.contains("2"));
    assert!(!result.contains("Array"));
}

#[test]
fn test_compact_large_array() {
    let converter = CompactConverter::new();
    let arr = vec![
        Bson::Int32(1),
        Bson::Int32(2),
        Bson::Int32(3),
        Bson::Int32(4),
        Bson::Int32(5),
    ];
    let result = converter.convert(&Bson::Array(arr));
    assert!(result.contains("Array(5)"));
    assert!(!result.contains("1")); // Should not show actual values
}

#[test]
fn test_compact_empty_array() {
    let converter = CompactConverter::new();
    let arr = vec![];
    let result = converter.convert(&Bson::Array(arr));
    assert_eq!(result, "[]");
}

#[test]
fn test_compact_small_document() {
    let converter = CompactConverter::new();
    let doc = doc! { "a": 1 };
    let result = converter.convert(&Bson::Document(doc));
    assert!(result.contains("a"));
    assert!(result.contains("1"));
    assert!(!result.contains("Object"));
}

#[test]
fn test_compact_large_document() {
    let converter = CompactConverter::new();
    let doc = doc! { "a": 1, "b": 2, "c": 3, "d": 4 };
    let result = converter.convert(&Bson::Document(doc));
    assert!(result.contains("Object(4)"));
    assert!(!result.contains("a")); // Should not show actual fields
}

#[test]
fn test_compact_empty_document() {
    let converter = CompactConverter::new();
    let doc = doc! {};
    let result = converter.convert(&Bson::Document(doc));
    assert_eq!(result, "{}");
}

#[test]
fn test_compact_binary() {
    let converter = CompactConverter::new();

    // Short binary
    let short_bin = Binary {
        subtype: BinarySubtype::Generic,
        bytes: vec![0x01, 0x02],
    };
    let result = converter.convert(&Bson::Binary(short_bin));
    assert!(result.contains("Binary"));
    assert!(result.contains("0102"));

    // Long binary (should truncate)
    let long_bin = Binary {
        subtype: BinarySubtype::Generic,
        bytes: vec![0xff; 20],
    };
    let result = converter.convert(&Bson::Binary(long_bin));
    assert!(result.contains("Binary"));
    assert!(result.contains("..."));
}

#[test]
fn test_compact_with_custom_limits() {
    let converter = CompactConverter::with_limits(1, 1);

    // Array with 2 items should show count
    let arr = vec![Bson::Int32(1), Bson::Int32(2)];
    let result = converter.convert(&Bson::Array(arr));
    assert!(result.contains("Array(2)"));

    // Document with 2 fields should show count
    let doc = doc! { "a": 1, "b": 2 };
    let result = converter.convert(&Bson::Document(doc));
    assert!(result.contains("Object(2)"));
}

// ===== JsonConverter Tests =====

#[test]
fn test_json_basic_types() {
    let converter = JsonConverter::new(true);

    assert_eq!(
        converter.convert(&Bson::String("test".to_string())),
        JsonValue::String("test".to_string())
    );
    assert_eq!(
        converter.convert(&Bson::Int32(42)),
        JsonValue::Number(42.into())
    );
    assert_eq!(
        converter.convert(&Bson::Int64(100)),
        JsonValue::Number(100.into())
    );
    assert_eq!(
        converter.convert(&Bson::Boolean(true)),
        JsonValue::Bool(true)
    );
    assert_eq!(converter.convert(&Bson::Null), JsonValue::Null);
}

#[test]
fn test_json_double() {
    let converter = JsonConverter::new(true);
    let result = converter.convert(&Bson::Double(3.14));

    match result {
        JsonValue::Number(n) => {
            assert!((n.as_f64().unwrap() - 3.14).abs() < 0.001);
        }
        _ => panic!("Expected number"),
    }
}

#[test]
fn test_json_object_id() {
    let converter = JsonConverter::new(true);
    let oid = ObjectId::new();
    let result = converter.convert(&Bson::ObjectId(oid));

    match result {
        JsonValue::String(s) => assert_eq!(s, oid.to_string()),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_json_datetime() {
    let converter = JsonConverter::new(true);
    let dt = DateTime::now();
    let result = converter.convert(&Bson::DateTime(dt));

    match result {
        JsonValue::String(s) => assert!(!s.is_empty()),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_json_array() {
    let converter = JsonConverter::new(true);
    let arr = vec![Bson::Int32(1), Bson::Int32(2), Bson::Int32(3)];
    let result = converter.convert(&Bson::Array(arr));

    match result {
        JsonValue::Array(a) => {
            assert_eq!(a.len(), 3);
            assert_eq!(a[0], JsonValue::Number(1.into()));
            assert_eq!(a[1], JsonValue::Number(2.into()));
            assert_eq!(a[2], JsonValue::Number(3.into()));
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_json_document() {
    let converter = JsonConverter::new(true);
    let doc = doc! { "name": "test", "value": 42 };
    let result = converter.convert(&Bson::Document(doc));

    match result {
        JsonValue::Object(obj) => {
            assert_eq!(obj.len(), 2);
            assert_eq!(
                obj.get("name"),
                Some(&JsonValue::String("test".to_string()))
            );
            assert_eq!(obj.get("value"), Some(&JsonValue::Number(42.into())));
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_json_nested_document() {
    let converter = JsonConverter::new(true);
    let doc = doc! {
        "outer": {
            "inner": "value",
            "number": 42
        }
    };
    let result = converter.convert(&Bson::Document(doc));

    match result {
        JsonValue::Object(obj) => {
            assert!(obj.contains_key("outer"));
            if let Some(JsonValue::Object(inner)) = obj.get("outer") {
                assert_eq!(
                    inner.get("inner"),
                    Some(&JsonValue::String("value".to_string()))
                );
                assert_eq!(inner.get("number"), Some(&JsonValue::Number(42.into())));
            } else {
                panic!("Expected nested object");
            }
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_json_binary() {
    let converter = JsonConverter::new(true);
    let bin = Binary {
        subtype: BinarySubtype::Generic,
        bytes: vec![0x01, 0x02, 0x03],
    };
    let result = converter.convert(&Bson::Binary(bin));

    match result {
        JsonValue::String(s) => assert!(!s.is_empty()),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_json_regex() {
    let converter = JsonConverter::new(true);
    let regex = Regex {
        pattern: "test".to_string(),
        options: "i".to_string(),
    };
    let result = converter.convert(&Bson::RegularExpression(regex));

    match result {
        JsonValue::String(s) => assert_eq!(s, "/test/i"),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_json_timestamp() {
    let converter = JsonConverter::new(true);
    let ts = Timestamp {
        time: 12345,
        increment: 67890,
    };
    let result = converter.convert(&Bson::Timestamp(ts));

    match result {
        JsonValue::Number(_) => {} // Success
        _ => panic!("Expected number"),
    }
}

#[test]
fn test_json_simplified_vs_extended() {
    let simplified = JsonConverter::simplified();
    let extended = JsonConverter::extended();

    let decimal = Decimal128::from_bytes([0u8; 16]);
    let bson = Bson::Decimal128(decimal);

    let _result_simplified = simplified.convert(&bson);
    let _result_extended = extended.convert(&bson);

    // Both should produce valid JSON
}

#[test]
fn test_json_special_values() {
    let converter = JsonConverter::new(true);

    assert_eq!(converter.convert(&Bson::Undefined), JsonValue::Null);

    let min_key = converter.convert(&Bson::MinKey);
    match min_key {
        JsonValue::String(s) => assert_eq!(s, "MinKey"),
        _ => panic!("Expected string"),
    }

    let max_key = converter.convert(&Bson::MaxKey);
    match max_key {
        JsonValue::String(s) => assert_eq!(s, "MaxKey"),
        _ => panic!("Expected string"),
    }
}
