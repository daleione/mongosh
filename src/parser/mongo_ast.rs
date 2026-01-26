//! MongoDB Shell AST (Abstract Syntax Tree)
//!
//! This module defines AST structures for MongoDB shell expressions.
//! It replaces the oxc dependency with a lightweight, purpose-built AST
//! specifically designed for MongoDB shell syntax parsing.

use std::ops::Range;

/// Span information for source locations
pub type Span = Range<usize>;

/// Root expression type
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Object literal: { key: value, ... }
    Object(ObjectExpr),
    /// Array literal: [1, 2, 3]
    Array(ArrayExpr),
    /// String literal: "hello" or 'world'
    String(String),
    /// Number literal: 42 or 3.14
    Number(f64),
    /// Boolean literal: true or false
    Boolean(bool),
    /// Null literal
    Null,
    /// Identifier: variable name
    Ident(String),
    /// Member expression: obj.prop
    Member(Box<MemberExpr>),
    /// Call expression: fn(args)
    Call(Box<CallExpr>),
    /// New expression: new Ctor(args)
    New(Box<NewExpr>),
    /// Unary expression: -x, +x, !x
    Unary(Box<UnaryExpr>),
}

/// Object expression: { key: value, ... }
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectExpr {
    pub properties: Vec<Property>,
    pub span: Span,
}

impl ObjectExpr {
    pub fn new(properties: Vec<Property>, span: Span) -> Self {
        Self { properties, span }
    }
}

/// Object property: key: value
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub key: PropertyKey,
    pub value: Expr,
    pub span: Span,
}

impl Property {
    pub fn new(key: PropertyKey, value: Expr, span: Span) -> Self {
        Self { key, value, span }
    }
}

/// Property key (can be identifier, string, or number)
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKey {
    Ident(String),
    String(String),
    Number(String),
}

impl PropertyKey {
    pub fn as_string(&self) -> String {
        match self {
            PropertyKey::Ident(s) => s.clone(),
            PropertyKey::String(s) => s.clone(),
            PropertyKey::Number(s) => s.clone(),
        }
    }
}

/// Array expression: [1, 2, 3]
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

impl ArrayExpr {
    pub fn new(elements: Vec<Expr>, span: Span) -> Self {
        Self { elements, span }
    }
}

/// Member expression: obj.prop or obj[expr]
#[derive(Debug, Clone, PartialEq)]
pub struct MemberExpr {
    pub object: Box<Expr>,
    pub property: MemberProperty,
    pub span: Span,
}

impl MemberExpr {
    pub fn new(object: Expr, property: MemberProperty, span: Span) -> Self {
        Self {
            object: Box::new(object),
            property,
            span,
        }
    }
}

/// Member property (static or computed)
#[derive(Debug, Clone, PartialEq)]
pub enum MemberProperty {
    /// Static: obj.prop
    Ident(String),
    /// Computed: obj[expr]
    Computed(Expr),
}

/// Call expression: fn(arg1, arg2, ...)
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub arguments: Vec<Expr>,
    pub span: Span,
}

impl CallExpr {
    pub fn new(callee: Expr, arguments: Vec<Expr>, span: Span) -> Self {
        Self {
            callee: Box::new(callee),
            arguments,
            span,
        }
    }
}

/// New expression: new Ctor(arg1, arg2, ...)
#[derive(Debug, Clone, PartialEq)]
pub struct NewExpr {
    pub callee: Box<Expr>,
    pub arguments: Vec<Expr>,
    pub span: Span,
}

impl NewExpr {
    pub fn new(callee: Expr, arguments: Vec<Expr>, span: Span) -> Self {
        Self {
            callee: Box::new(callee),
            arguments,
            span,
        }
    }
}

/// Unary expression: -x, +x, !x
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub operator: UnaryOperator,
    pub argument: Box<Expr>,
    pub span: Span,
}

impl UnaryExpr {
    pub fn new(operator: UnaryOperator, argument: Expr, span: Span) -> Self {
        Self {
            operator,
            argument: Box::new(argument),
            span,
        }
    }
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// Negation: -x
    Minus,
    /// Plus: +x
    Plus,
    /// Logical NOT: !x
    Not,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_constructors() {
        let s = Expr::String("hello".to_string());
        assert!(matches!(s, Expr::String(_)));

        let n = Expr::Number(42.0);
        assert!(matches!(n, Expr::Number(_)));

        let b = Expr::Boolean(true);
        assert!(matches!(b, Expr::Boolean(true)));

        let null = Expr::Null;
        assert!(matches!(null, Expr::Null));

        let id = Expr::Ident("test".to_string());
        assert!(matches!(id, Expr::Ident(_)));
    }

    #[test]
    fn test_property_key_as_string() {
        let key1 = PropertyKey::Ident("name".to_string());
        assert_eq!(key1.as_string(), "name");

        let key2 = PropertyKey::String("age".to_string());
        assert_eq!(key2.as_string(), "age");

        let key3 = PropertyKey::Number("123".to_string());
        assert_eq!(key3.as_string(), "123");
    }

    #[test]
    fn test_object_expr() {
        let prop = Property::new(
            PropertyKey::Ident("name".to_string()),
            Expr::String("John".to_string()),
            0..10,
        );
        let obj = ObjectExpr::new(vec![prop], 0..15);
        assert_eq!(obj.properties.len(), 1);
    }

    #[test]
    fn test_array_expr() {
        let arr = ArrayExpr::new(
            vec![Expr::Number(1.0), Expr::Number(2.0), Expr::Number(3.0)],
            0..10,
        );
        assert_eq!(arr.elements.len(), 3);
    }

    #[test]
    fn test_call_expr() {
        let callee = Expr::Ident("find".to_string());
        let args = vec![Expr::Object(ObjectExpr::new(vec![], 0..2))];
        let call = CallExpr::new(callee, args, 0..10);
        assert_eq!(call.arguments.len(), 1);
    }

    #[test]
    fn test_unary_expr() {
        let unary = UnaryExpr::new(UnaryOperator::Minus, Expr::Number(5.0), 0..2);
        assert_eq!(unary.operator, UnaryOperator::Minus);
    }
}
