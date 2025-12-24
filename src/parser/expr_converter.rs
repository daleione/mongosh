//! JavaScript expression to BSON converter
//!
//! This module converts Oxc AST expressions into MongoDB BSON documents.
//! It handles object literals, arrays, primitives, and MongoDB operators.

use mongodb::bson::{Bson, Document};
use oxc::ast::ast::*;

use crate::error::{ParseError, Result};

/// Converter for JavaScript expressions to BSON
pub struct ExpressionConverter;

impl ExpressionConverter {
    /// Convert a JavaScript expression to a BSON value
    pub fn expr_to_bson(expr: &Expression) -> Result<Bson> {
        match expr {
            // Object literal: { key: value, ... }
            Expression::ObjectExpression(obj) => Self::object_to_bson(obj).map(Bson::Document),

            // Array literal: [1, 2, 3]
            Expression::ArrayExpression(arr) => Self::array_to_bson(arr).map(Bson::Array),

            // String literal: "hello" or 'hello'
            Expression::StringLiteral(s) => Ok(Bson::String(s.value.to_string())),

            // Number literal: 42, 3.14
            Expression::NumericLiteral(n) => {
                if n.value.fract() == 0.0 {
                    Ok(Bson::Int64(n.value as i64))
                } else {
                    Ok(Bson::Double(n.value))
                }
            }

            // Boolean literal: true, false
            Expression::BooleanLiteral(b) => Ok(Bson::Boolean(b.value)),

            // Null literal
            Expression::NullLiteral(_) => Ok(Bson::Null),

            // Identifier (e.g., undefined, special MongoDB values)
            Expression::Identifier(id) => Self::identifier_to_bson(id),

            // Unary expression: -5, !true
            Expression::UnaryExpression(unary) => Self::unary_to_bson(unary),

            // Binary expression (limited support)
            Expression::BinaryExpression(binary) => Self::binary_to_bson(binary),

            // Template literal: `hello ${world}`
            Expression::TemplateLiteral(tmpl) => Self::template_to_bson(tmpl),

            // New expression: new Date(), new ObjectId()
            Expression::NewExpression(new_expr) => Self::new_expression_to_bson(new_expr),

            // Call expression: ObjectId("..."), ISODate("...")
            Expression::CallExpression(call) => Self::call_expression_to_bson(call),

            // Member expression (limited support)
            Expression::StaticMemberExpression(member) => Self::member_to_bson(member),

            // Parenthesized expression: (expr)
            Expression::ParenthesizedExpression(paren) => Self::expr_to_bson(&paren.expression),

            _ => Err(
                ParseError::InvalidQuery(format!("Unsupported expression type: {:?}", expr)).into(),
            ),
        }
    }

    /// Convert a JavaScript object to a BSON document
    pub fn object_to_bson(obj: &ObjectExpression) -> Result<Document> {
        let mut doc = Document::new();

        for prop in &obj.properties {
            match prop {
                ObjectPropertyKind::ObjectProperty(prop) => {
                    let key = Self::get_property_key(&prop.key)?;
                    let value = Self::expr_to_bson(&prop.value)?;
                    doc.insert(key, value);
                }
                ObjectPropertyKind::SpreadProperty(_) => {
                    return Err(ParseError::InvalidQuery(
                        "Spread properties not supported in BSON".to_string(),
                    )
                    .into());
                }
            }
        }

        Ok(doc)
    }

    /// Convert a JavaScript array to a BSON array
    pub fn array_to_bson(arr: &ArrayExpression) -> Result<Vec<Bson>> {
        let mut result = Vec::new();

        for element in &arr.elements {
            match element {
                ArrayExpressionElement::SpreadElement(_) => {
                    return Err(ParseError::InvalidQuery(
                        "Spread elements not supported in BSON arrays".to_string(),
                    )
                    .into());
                }
                ArrayExpressionElement::Elision(_) => {
                    result.push(Bson::Null);
                }
                ArrayExpressionElement::BooleanLiteral(b) => {
                    result.push(Bson::Boolean(b.value));
                }
                ArrayExpressionElement::NullLiteral(_) => {
                    result.push(Bson::Null);
                }
                ArrayExpressionElement::NumericLiteral(n) => {
                    if n.value.fract() == 0.0 {
                        result.push(Bson::Int64(n.value as i64));
                    } else {
                        result.push(Bson::Double(n.value));
                    }
                }
                ArrayExpressionElement::StringLiteral(s) => {
                    result.push(Bson::String(s.value.to_string()));
                }
                ArrayExpressionElement::Identifier(id) => {
                    result.push(Self::identifier_to_bson(id)?);
                }
                ArrayExpressionElement::ObjectExpression(obj) => {
                    result.push(Bson::Document(Self::object_to_bson(obj)?));
                }
                ArrayExpressionElement::ArrayExpression(arr) => {
                    result.push(Bson::Array(Self::array_to_bson(arr)?));
                }
                ArrayExpressionElement::UnaryExpression(unary) => {
                    result.push(Self::unary_to_bson(unary)?);
                }
                ArrayExpressionElement::NewExpression(new_expr) => {
                    result.push(Self::new_expression_to_bson(new_expr)?);
                }
                ArrayExpressionElement::CallExpression(call) => {
                    result.push(Self::call_expression_to_bson(call)?);
                }
                _ => {
                    return Err(ParseError::InvalidQuery(format!(
                        "Unsupported array element type: {:?}",
                        element
                    ))
                    .into());
                }
            }
        }

        Ok(result)
    }

    /// Get property key from PropertyKey
    fn get_property_key(key: &PropertyKey) -> Result<String> {
        match key {
            PropertyKey::StaticIdentifier(id) => Ok(id.name.to_string()),
            PropertyKey::StringLiteral(s) => Ok(s.value.to_string()),
            PropertyKey::NumericLiteral(n) => Ok(n.value.to_string()),
            PropertyKey::Identifier(id) => Ok(id.name.to_string()),
            _ => Err(ParseError::InvalidQuery("Unsupported property key type".to_string()).into()),
        }
    }

    /// Convert identifier to BSON (e.g., undefined, Infinity)
    fn identifier_to_bson(id: &IdentifierReference) -> Result<Bson> {
        match id.name.as_str() {
            "undefined" => Ok(Bson::Null),
            "null" => Ok(Bson::Null),
            "true" => Ok(Bson::Boolean(true)),
            "false" => Ok(Bson::Boolean(false)),
            "Infinity" => Ok(Bson::Double(f64::INFINITY)),
            "NaN" => Ok(Bson::Double(f64::NAN)),
            _ => Err(ParseError::InvalidQuery(format!("Unknown identifier: {}", id.name)).into()),
        }
    }

    /// Convert unary expression to BSON (e.g., -5, +3)
    fn unary_to_bson(unary: &UnaryExpression) -> Result<Bson> {
        match unary.operator {
            UnaryOperator::UnaryNegation => {
                // Handle -number
                if let Expression::NumericLiteral(n) = &unary.argument {
                    let value = -n.value;
                    if value.fract() == 0.0 {
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
            UnaryOperator::UnaryPlus => {
                // Handle +number
                if let Expression::NumericLiteral(n) = &unary.argument {
                    if n.value.fract() == 0.0 {
                        Ok(Bson::Int64(n.value as i64))
                    } else {
                        Ok(Bson::Double(n.value))
                    }
                } else {
                    Err(ParseError::InvalidQuery(
                        "Unary plus only supported for numeric literals".to_string(),
                    )
                    .into())
                }
            }
            UnaryOperator::LogicalNot => {
                // Handle !boolean - convert to boolean first
                let value = Self::expr_to_bson(&unary.argument)?;
                match value {
                    Bson::Boolean(b) => Ok(Bson::Boolean(!b)),
                    _ => Err(ParseError::InvalidQuery(
                        "Logical NOT requires boolean value".to_string(),
                    )
                    .into()),
                }
            }
            _ => Err(ParseError::InvalidQuery(format!(
                "Unsupported unary operator: {:?}",
                unary.operator
            ))
            .into()),
        }
    }

    /// Convert binary expression (very limited support)
    fn binary_to_bson(_binary: &BinaryExpression) -> Result<Bson> {
        Err(ParseError::InvalidQuery(
            "Binary expressions not supported in BSON literals".to_string(),
        )
        .into())
    }

    /// Convert template literal to string
    fn template_to_bson(tmpl: &TemplateLiteral) -> Result<Bson> {
        // Simple case: no expressions, just concatenate quasi strings
        if tmpl.expressions.is_empty() {
            let mut result = String::new();
            for quasi in &tmpl.quasis {
                result.push_str(&quasi.value.raw);
            }
            return Ok(Bson::String(result));
        }

        Err(ParseError::InvalidQuery(
            "Template literals with expressions not supported".to_string(),
        )
        .into())
    }

    /// Convert new expression: new Date(), new ObjectId()
    fn new_expression_to_bson(new_expr: &NewExpression) -> Result<Bson> {
        // Get constructor name
        let ctor_name = if let Expression::Identifier(id) = &new_expr.callee {
            id.name.as_str()
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
                    if let Some(expr) = arg.as_expression() {
                        Self::parse_date_argument(expr)
                    } else {
                        Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
                    }
                } else {
                    Err(ParseError::InvalidQuery("Invalid Date constructor".to_string()).into())
                }
            }
            "ObjectId" => {
                // new ObjectId() or new ObjectId("hexstring")
                if new_expr.arguments.is_empty() {
                    Ok(Bson::ObjectId(mongodb::bson::oid::ObjectId::new()))
                } else if let Some(arg) = new_expr.arguments.first() {
                    if let Some(expr) = arg.as_expression() {
                        Self::parse_objectid_argument(expr)
                    } else {
                        Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
                    }
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
    fn call_expression_to_bson(call: &CallExpression) -> Result<Bson> {
        // Get function name
        let fn_name = if let Expression::Identifier(id) = &call.callee {
            id.name.as_str()
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
                    if let Some(expr) = arg.as_expression() {
                        Self::parse_objectid_argument(expr)
                    } else {
                        Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
                    }
                } else {
                    Err(ParseError::InvalidQuery("Invalid ObjectId call".to_string()).into())
                }
            }
            "ISODate" | "Date" => {
                if let Some(arg) = call.arguments.first() {
                    if let Some(expr) = arg.as_expression() {
                        Self::parse_date_argument(expr)
                    } else {
                        Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
                    }
                } else {
                    Ok(Bson::DateTime(mongodb::bson::DateTime::now()))
                }
            }
            "NumberInt" => {
                if let Some(arg) = call.arguments.first() {
                    if let Some(expr) = arg.as_expression() {
                        Self::parse_int_argument(expr)
                    } else {
                        Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
                    }
                } else {
                    Err(ParseError::InvalidQuery("NumberInt requires argument".to_string()).into())
                }
            }
            "NumberLong" | "Long" => {
                if let Some(arg) = call.arguments.first() {
                    if let Some(expr) = arg.as_expression() {
                        Self::parse_long_argument(expr)
                    } else {
                        Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
                    }
                } else {
                    Err(ParseError::InvalidQuery("NumberLong requires argument".to_string()).into())
                }
            }
            "NumberDecimal" => {
                if let Some(arg) = call.arguments.first() {
                    if let Some(expr) = arg.as_expression() {
                        Self::parse_decimal_argument(expr)
                    } else {
                        Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
                    }
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

    /// Convert static member expression (limited)
    fn member_to_bson(_member: &StaticMemberExpression) -> Result<Bson> {
        Err(ParseError::InvalidQuery(
            "Member expressions not supported in BSON literals".to_string(),
        )
        .into())
    }

    /// Parse Date argument
    fn parse_date_argument(expr: &Expression) -> Result<Bson> {
        match expr {
            Expression::StringLiteral(s) => {
                // Parse ISO date string
                let date_str = s.value.as_str();
                let datetime = mongodb::bson::DateTime::parse_rfc3339_str(date_str)
                    .map_err(|e| ParseError::InvalidQuery(format!("Invalid date string: {}", e)))?;
                Ok(Bson::DateTime(datetime))
            }
            Expression::NumericLiteral(n) => {
                // Timestamp in milliseconds
                let millis = n.value as i64;
                Ok(Bson::DateTime(mongodb::bson::DateTime::from_millis(millis)))
            }
            _ => Err(ParseError::InvalidQuery(
                "Date argument must be string or number".to_string(),
            )
            .into()),
        }
    }

    /// Parse ObjectId argument
    fn parse_objectid_argument(expr: &Expression) -> Result<Bson> {
        if let Expression::StringLiteral(s) = expr {
            let oid = mongodb::bson::oid::ObjectId::parse_str(s.value.as_str())
                .map_err(|e| ParseError::InvalidQuery(format!("Invalid ObjectId: {}", e)))?;
            Ok(Bson::ObjectId(oid))
        } else {
            Err(ParseError::InvalidQuery("ObjectId argument must be string".to_string()).into())
        }
    }

    /// Parse NumberInt argument
    fn parse_int_argument(expr: &Expression) -> Result<Bson> {
        match expr {
            Expression::NumericLiteral(n) => Ok(Bson::Int32(n.value as i32)),
            Expression::StringLiteral(s) => {
                let val = s
                    .value
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
    fn parse_long_argument(expr: &Expression) -> Result<Bson> {
        match expr {
            Expression::NumericLiteral(n) => Ok(Bson::Int64(n.value as i64)),
            Expression::StringLiteral(s) => {
                let val = s
                    .value
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
    fn parse_decimal_argument(expr: &Expression) -> Result<Bson> {
        match expr {
            Expression::StringLiteral(s) => {
                // Store as string for now (BSON doesn't have native decimal in Rust driver)
                Ok(Bson::String(s.value.to_string()))
            }
            Expression::NumericLiteral(n) => Ok(Bson::Double(n.value)),
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

    use oxc::allocator::Allocator;
    use oxc::parser::Parser;
    use oxc::span::SourceType;

    fn parse_and_convert(code: &str) -> Bson {
        let allocator = Allocator::default();
        let source_type = SourceType::default();
        let ret = Parser::new(&allocator, code, source_type).parse();

        if let Some(Statement::ExpressionStatement(expr_stmt)) = ret.program.body.first() {
            ExpressionConverter::expr_to_bson(&expr_stmt.expression).unwrap()
        } else {
            panic!("Failed to parse expression");
        }
    }

    #[test]
    fn test_simple_object() {
        let result = parse_and_convert("({ name: 'John', age: 30 })");

        if let Bson::Document(doc) = result {
            assert_eq!(doc.get_str("name").unwrap(), "John");
            assert_eq!(doc.get_i64("age").unwrap(), 30);
        } else {
            panic!("Expected document");
        }
    }

    #[test]
    fn test_nested_object() {
        let result = parse_and_convert("({ user: { name: 'Alice', meta: { active: true } } })");

        if let Bson::Document(doc) = result {
            let user = doc.get_document("user").unwrap();
            assert_eq!(user.get_str("name").unwrap(), "Alice");
        } else {
            panic!("Expected document");
        }
    }

    #[test]
    fn test_array() {
        let result = parse_and_convert("[1, 'two', true, null]");

        if let Bson::Array(arr) = result {
            assert_eq!(arr.len(), 4);
            assert_eq!(arr[0], Bson::Int64(1));
            assert_eq!(arr[1], Bson::String("two".to_string()));
            assert_eq!(arr[2], Bson::Boolean(true));
            assert_eq!(arr[3], Bson::Null);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_negative_number() {
        let result = parse_and_convert("({ value: -42 })");

        if let Bson::Document(doc) = result {
            assert_eq!(doc.get_i64("value").unwrap(), -42);
        } else {
            panic!("Expected document");
        }
    }
}
