//! MongoDB expression to BSON converter
//!
//! This module converts our custom MongoDB shell AST into MongoDB BSON documents.
//! It replaces the previous oxc-based implementation with a lightweight converter
//! that works directly with our purpose-built AST.

use mongodb::bson::{Bson, Document};

use super::mongo_ast::*;
use crate::error::{ParseError, Result};

/// Converter for MongoDB expressions to BSON
pub struct ExpressionConverter;

impl ExpressionConverter {
    /// Convert an expression to a BSON value
    pub fn expr_to_bson(expr: &Expr) -> Result<Bson> {
        match expr {
            // Object literal: { key: value, ... }
            Expr::Object(obj) => Self::object_to_bson(obj).map(Bson::Document),

            // Array literal: [1, 2, 3]
            Expr::Array(arr) => Self::array_to_bson(arr).map(Bson::Array),

            // String literal: "hello" or 'hello'
            Expr::String(s) => Ok(Bson::String(s.clone())),

            // Number literal: 42, 3.14
            Expr::Number(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    Ok(Bson::Int64(*n as i64))
                } else {
                    Ok(Bson::Double(*n))
                }
            }

            // Boolean literal: true, false
            Expr::Boolean(b) => Ok(Bson::Boolean(*b)),

            // Null literal
            Expr::Null => Ok(Bson::Null),

            // Identifier (e.g., undefined, Infinity, NaN)
            Expr::Ident(name) => Self::identifier_to_bson(name),

            // Unary expression: -5, !true
            Expr::Unary(unary) => Self::unary_to_bson(unary),

            // New expression: new Date(), new ObjectId()
            Expr::New(new_expr) => Self::new_expression_to_bson(new_expr),

            // Call expression: ObjectId("..."), ISODate("...")
            Expr::Call(call) => Self::call_expression_to_bson(call),

            // Member expression (not supported in BSON literals)
            Expr::Member(_) => Err(ParseError::InvalidQuery(
                "Member expressions not supported in BSON literals".to_string(),
            )
            .into()),
        }
    }

    /// Convert an object to a BSON document
    pub fn object_to_bson(obj: &ObjectExpr) -> Result<Document> {
        let mut doc = Document::new();

        for prop in &obj.properties {
            let key = prop.key.as_string();
            let value = Self::expr_to_bson(&prop.value)?;
            doc.insert(key, value);
        }

        Ok(doc)
    }

    /// Convert an array to a BSON array
    pub fn array_to_bson(arr: &ArrayExpr) -> Result<Vec<Bson>> {
        let mut result = Vec::new();

        for element in &arr.elements {
            let value = Self::expr_to_bson(element)?;
            result.push(value);
        }

        Ok(result)
    }

    /// Convert identifier to BSON (e.g., undefined, Infinity)
    fn identifier_to_bson(name: &str) -> Result<Bson> {
        match name {
            "undefined" => Ok(Bson::Null),
            "null" => Ok(Bson::Null),
            "true" => Ok(Bson::Boolean(true)),
            "false" => Ok(Bson::Boolean(false)),
            "Infinity" => Ok(Bson::Double(f64::INFINITY)),
            "NaN" => Ok(Bson::Double(f64::NAN)),
            _ => Err(ParseError::InvalidQuery(format!("Unknown identifier: {}", name)).into()),
        }
    }

    /// Convert unary expression to BSON (e.g., -5, +3)
    fn unary_to_bson(unary: &UnaryExpr) -> Result<Bson> {
        match unary.operator {
            UnaryOperator::Minus => {
                // Handle -number
                if let Expr::Number(n) = unary.argument.as_ref() {
                    let value = -n;
                    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64
                    {
                        Ok(Bson::Int64(value as i64))
                    } else {
                        Ok(Bson::Double(value))
                    }
                } else {
                    Err(ParseError::InvalidQuery(
                        "Unary negation only supported for numeric literals".to_string(),
                    )
                    .into())
                }
            }
            UnaryOperator::Plus => {
                // Handle +number
                if let Expr::Number(n) = unary.argument.as_ref() {
                    if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                        Ok(Bson::Int64(*n as i64))
                    } else {
                        Ok(Bson::Double(*n))
                    }
                } else {
                    Err(ParseError::InvalidQuery(
                        "Unary plus only supported for numeric literals".to_string(),
                    )
                    .into())
                }
            }
            UnaryOperator::Not => {
                // Handle !boolean - convert to boolean first
                let value = Self::expr_to_bson(unary.argument.as_ref())?;
                match value {
                    Bson::Boolean(b) => Ok(Bson::Boolean(!b)),
                    _ => Err(ParseError::InvalidQuery(
                        "Logical NOT requires boolean value".to_string(),
                    )
                    .into()),
                }
            }
        }
    }

    /// Convert new expression: new Date(), new ObjectId()
    fn new_expression_to_bson(new_expr: &NewExpr) -> Result<Bson> {
        // Get constructor name
        let ctor_name = if let Expr::Ident(name) = new_expr.callee.as_ref() {
            name.as_str()
        } else {
            return Err(ParseError::InvalidQuery(
                "new expression must have identifier callee".to_string(),
            )
            .into());
        };

        match ctor_name {
            "Date" => {
                // new Date() or new Date(timestamp) or new Date(dateString)
                if new_expr.arguments.is_empty() {
                    // Current time
                    Ok(Bson::DateTime(mongodb::bson::DateTime::now()))
                } else if let Some(arg) = new_expr.arguments.first() {
                    Self::parse_date_argument(arg)
                } else {
                    Err(ParseError::InvalidQuery("Invalid Date constructor".to_string()).into())
                }
            }
            "ObjectId" => {
                // new ObjectId() or new ObjectId("hexstring")
                if new_expr.arguments.is_empty() {
                    Ok(Bson::ObjectId(mongodb::bson::oid::ObjectId::new()))
                } else if let Some(arg) = new_expr.arguments.first() {
                    Self::parse_objectid_argument(arg)
                } else {
                    Err(ParseError::InvalidQuery("Invalid ObjectId constructor".to_string()).into())
                }
            }
            _ => Err(
                ParseError::InvalidQuery(format!("Unsupported constructor: {}", ctor_name)).into(),
            ),
        }
    }

    /// Convert call expression: ObjectId("..."), ISODate("...")
    fn call_expression_to_bson(call: &CallExpr) -> Result<Bson> {
        // Get function name
        let fn_name = if let Expr::Ident(name) = call.callee.as_ref() {
            name.as_str()
        } else {
            return Err(ParseError::InvalidQuery(
                "Call expression must have identifier callee".to_string(),
            )
            .into());
        };

        match fn_name {
            "ObjectId" => {
                if call.arguments.is_empty() {
                    Ok(Bson::ObjectId(mongodb::bson::oid::ObjectId::new()))
                } else if let Some(arg) = call.arguments.first() {
                    Self::parse_objectid_argument(arg)
                } else {
                    Err(ParseError::InvalidQuery("Invalid ObjectId call".to_string()).into())
                }
            }
            "ISODate" | "Date" => {
                if let Some(arg) = call.arguments.first() {
                    Self::parse_date_argument(arg)
                } else {
                    Ok(Bson::DateTime(mongodb::bson::DateTime::now()))
                }
            }
            "NumberInt" => {
                if let Some(arg) = call.arguments.first() {
                    Self::parse_int_argument(arg)
                } else {
                    Err(ParseError::InvalidQuery("NumberInt requires argument".to_string()).into())
                }
            }
            "NumberLong" | "Long" => {
                if let Some(arg) = call.arguments.first() {
                    Self::parse_long_argument(arg)
                } else {
                    Err(ParseError::InvalidQuery("NumberLong requires argument".to_string()).into())
                }
            }
            "NumberDecimal" | "Decimal128" => {
                if let Some(arg) = call.arguments.first() {
                    Self::parse_decimal_argument(arg)
                } else {
                    Err(
                        ParseError::InvalidQuery("NumberDecimal requires argument".to_string())
                            .into(),
                    )
                }
            }
            _ => Err(ParseError::InvalidQuery(format!("Unsupported function: {}", fn_name)).into()),
        }
    }

    /// Parse Date argument
    fn parse_date_argument(expr: &Expr) -> Result<Bson> {
        match expr {
            Expr::String(s) => {
                // Parse ISO date string
                let datetime = mongodb::bson::DateTime::parse_rfc3339_str(s)
                    .map_err(|e| ParseError::InvalidQuery(format!("Invalid date string: {}", e)))?;
                Ok(Bson::DateTime(datetime))
            }
            Expr::Number(n) => {
                // Timestamp in milliseconds
                let millis = *n as i64;
                Ok(Bson::DateTime(mongodb::bson::DateTime::from_millis(millis)))
            }
            _ => Err(ParseError::InvalidQuery(
                "Date argument must be string or number".to_string(),
            )
            .into()),
        }
    }

    /// Parse ObjectId argument
    fn parse_objectid_argument(expr: &Expr) -> Result<Bson> {
        if let Expr::String(s) = expr {
            let oid = mongodb::bson::oid::ObjectId::parse_str(s)
                .map_err(|e| ParseError::InvalidQuery(format!("Invalid ObjectId: {}", e)))?;
            Ok(Bson::ObjectId(oid))
        } else {
            Err(ParseError::InvalidQuery("ObjectId argument must be string".to_string()).into())
        }
    }

    /// Parse NumberInt argument
    fn parse_int_argument(expr: &Expr) -> Result<Bson> {
        match expr {
            Expr::Number(n) => Ok(Bson::Int32(*n as i32)),
            Expr::String(s) => {
                let val = s
                    .parse::<i32>()
                    .map_err(|e| ParseError::InvalidQuery(format!("Invalid int: {}", e)))?;
                Ok(Bson::Int32(val))
            }
            _ => Err(ParseError::InvalidQuery(
                "NumberInt argument must be number or string".to_string(),
            )
            .into()),
        }
    }

    /// Parse NumberLong argument
    fn parse_long_argument(expr: &Expr) -> Result<Bson> {
        match expr {
            Expr::Number(n) => Ok(Bson::Int64(*n as i64)),
            Expr::String(s) => {
                let val = s
                    .parse::<i64>()
                    .map_err(|e| ParseError::InvalidQuery(format!("Invalid long: {}", e)))?;
                Ok(Bson::Int64(val))
            }
            _ => Err(ParseError::InvalidQuery(
                "NumberLong argument must be number or string".to_string(),
            )
            .into()),
        }
    }

    /// Parse NumberDecimal argument (using string representation)
    fn parse_decimal_argument(expr: &Expr) -> Result<Bson> {
        match expr {
            Expr::String(s) => {
                // Store as string for now (BSON doesn't have native decimal in Rust driver)
                Ok(Bson::String(s.clone()))
            }
            Expr::Number(n) => Ok(Bson::Double(*n)),
            _ => Err(ParseError::InvalidQuery(
                "NumberDecimal argument must be number or string".to_string(),
            )
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mongo_parser::MongoParser;

    fn parse_and_convert(code: &str) -> Bson {
        let expr = MongoParser::parse(code).unwrap();
        ExpressionConverter::expr_to_bson(&expr).unwrap()
    }

    #[test]
    fn test_simple_object() {
        let bson = parse_and_convert("{name: 'John', age: 30}");
        if let Bson::Document(doc) = bson {
            assert_eq!(doc.get_str("name").unwrap(), "John");
            assert_eq!(doc.get_i64("age").unwrap(), 30);
        } else {
            panic!("Expected document");
        }
    }

    #[test]
    fn test_nested_object() {
        let bson = parse_and_convert("{user: {name: 'John', age: 30}}");
        if let Bson::Document(doc) = bson {
            let user = doc.get_document("user").unwrap();
            assert_eq!(user.get_str("name").unwrap(), "John");
            assert_eq!(user.get_i64("age").unwrap(), 30);
        } else {
            panic!("Expected document");
        }
    }

    #[test]
    fn test_array() {
        let bson = parse_and_convert("[1, 2, 3, 'four']");
        if let Bson::Array(arr) = bson {
            assert_eq!(arr.len(), 4);
            assert_eq!(arr[0].as_i64().unwrap(), 1);
            assert_eq!(arr[1].as_i64().unwrap(), 2);
            assert_eq!(arr[2].as_i64().unwrap(), 3);
            assert_eq!(arr[3].as_str().unwrap(), "four");
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_negative_number() {
        let expr = MongoParser::parse("-5").unwrap();
        let bson = ExpressionConverter::expr_to_bson(&expr).unwrap();
        assert_eq!(bson.as_i64().unwrap(), -5);
    }

    #[test]
    fn test_boolean_literals() {
        let bson = parse_and_convert("true");
        assert_eq!(bson.as_bool().unwrap(), true);

        let bson = parse_and_convert("false");
        assert_eq!(bson.as_bool().unwrap(), false);
    }

    #[test]
    fn test_null_literal() {
        let bson = parse_and_convert("null");
        assert!(matches!(bson, Bson::Null));
    }

    #[test]
    fn test_objectid_call() {
        let bson = parse_and_convert("ObjectId('507f1f77bcf86cd799439011')");
        assert!(matches!(bson, Bson::ObjectId(_)));
    }

    #[test]
    fn test_new_date() {
        let bson = parse_and_convert("new Date()");
        assert!(matches!(bson, Bson::DateTime(_)));
    }

    #[test]
    fn test_new_objectid() {
        let bson = parse_and_convert("new ObjectId()");
        assert!(matches!(bson, Bson::ObjectId(_)));
    }

    #[test]
    fn test_number_int() {
        let bson = parse_and_convert("NumberInt(42)");
        assert_eq!(bson.as_i32().unwrap(), 42);
    }

    #[test]
    fn test_number_long() {
        let bson = parse_and_convert("NumberLong(123456789)");
        assert_eq!(bson.as_i64().unwrap(), 123456789);
    }

    #[test]
    fn test_mongo_operators() {
        let bson = parse_and_convert("{age: {$gt: 18, $lt: 65}}");
        if let Bson::Document(doc) = bson {
            let age = doc.get_document("age").unwrap();
            assert_eq!(age.get_i64("$gt").unwrap(), 18);
            assert_eq!(age.get_i64("$lt").unwrap(), 65);
        } else {
            panic!("Expected document");
        }
    }

    #[test]
    fn test_array_of_objects() {
        let bson = parse_and_convert("[{name: 'Alice'}, {name: 'Bob'}]");
        if let Bson::Array(arr) = bson {
            assert_eq!(arr.len(), 2);
            let doc1 = arr[0].as_document().unwrap();
            let doc2 = arr[1].as_document().unwrap();
            assert_eq!(doc1.get_str("name").unwrap(), "Alice");
            assert_eq!(doc2.get_str("name").unwrap(), "Bob");
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_infinity() {
        let bson = parse_and_convert("Infinity");
        assert!(bson.as_f64().unwrap().is_infinite());
    }

    #[test]
    fn test_string_with_special_chars() {
        let bson = parse_and_convert("{message: 'Hello\\nWorld'}");
        if let Bson::Document(doc) = bson {
            assert!(doc.get_str("message").unwrap().contains('\n'));
        } else {
            panic!("Expected document");
        }
    }
}
