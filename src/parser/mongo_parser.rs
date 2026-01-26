//! MongoDB Shell Parser
//!
//! This parser handles MongoDB shell syntax like `db.collection.operation(args)`.
//! It replaces the oxc dependency with a lightweight, purpose-built parser
//! specifically designed for MongoDB shell expressions.
//!
//! # Design Principles
//!
//! - **Purpose-built** - only handles MongoDB shell syntax subset
//! - **Error-tolerant** - provides helpful error messages
//! - **No dependencies** - uses only the mongo_lexer and mongo_ast modules
//! - **Recursive descent** - simple and maintainable parsing strategy

use super::mongo_ast::*;
use super::mongo_lexer::{MongoLexer, MongoToken, MongoTokenKind};
use crate::error::{ParseError, Result};

/// MongoDB Shell Parser
pub struct MongoParser {
    tokens: Vec<MongoToken>,
    pos: usize,
}

impl MongoParser {
    /// Create a new parser from input string
    pub fn new(input: &str) -> Self {
        let tokens = MongoLexer::tokenize(input);
        Self { tokens, pos: 0 }
    }

    /// Parse the input as an expression
    pub fn parse(input: &str) -> Result<Expr> {
        let mut parser = Self::new(input);
        parser.parse_expression()
    }

    /// Parse an expression
    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_unary()
    }

    /// Parse unary expression: -x, +x, !x
    fn parse_unary(&mut self) -> Result<Expr> {
        let start = self.current_pos();

        // Check for unary operators
        if self.match_token(&MongoTokenKind::Minus) {
            let argument = self.parse_unary()?;
            let end = self.previous_pos();
            return Ok(Expr::Unary(Box::new(UnaryExpr::new(
                UnaryOperator::Minus,
                argument,
                start..end,
            ))));
        }

        if self.match_token(&MongoTokenKind::Plus) {
            let argument = self.parse_unary()?;
            let end = self.previous_pos();
            return Ok(Expr::Unary(Box::new(UnaryExpr::new(
                UnaryOperator::Plus,
                argument,
                start..end,
            ))));
        }

        if self.match_token(&MongoTokenKind::Bang) {
            let argument = self.parse_unary()?;
            let end = self.previous_pos();
            return Ok(Expr::Unary(Box::new(UnaryExpr::new(
                UnaryOperator::Not,
                argument,
                start..end,
            ))));
        }

        self.parse_member_or_call()
    }

    /// Parse member expression, call expression, or new expression
    fn parse_member_or_call(&mut self) -> Result<Expr> {
        let start = self.current_pos();

        // Check for 'new' keyword
        if let Some(MongoToken {
            kind: MongoTokenKind::Ident(name),
            ..
        }) = self.current()
        {
            if name == "new" {
                self.advance();
                return self.parse_new_expression(start);
            }
        }

        // Parse the base expression (primary)
        let mut expr = self.parse_primary()?;

        // Handle member access and function calls
        loop {
            if self.match_token(&MongoTokenKind::Dot) {
                // Member access: obj.prop
                let prop_name = self.expect_identifier("Expected property name after '.'")?;
                let end = self.previous_pos();
                expr = Expr::Member(Box::new(MemberExpr::new(
                    expr,
                    MemberProperty::Ident(prop_name),
                    start..end,
                )));
            } else if self.match_token(&MongoTokenKind::LBracket) {
                // Computed member access: obj[expr]
                let property = self.parse_expression()?;
                self.expect_token(
                    &MongoTokenKind::RBracket,
                    "Expected ']' after computed member",
                )?;
                let end = self.previous_pos();
                expr = Expr::Member(Box::new(MemberExpr::new(
                    expr,
                    MemberProperty::Computed(property),
                    start..end,
                )));
            } else if self.match_token(&MongoTokenKind::LParen) {
                // Function call: fn(args)
                let arguments = self.parse_arguments()?;
                self.expect_token(&MongoTokenKind::RParen, "Expected ')' after arguments")?;
                let end = self.previous_pos();
                expr = Expr::Call(Box::new(CallExpr::new(expr, arguments, start..end)));
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse new expression: new Ctor(args)
    fn parse_new_expression(&mut self, start: usize) -> Result<Expr> {
        let callee = self.parse_primary()?;

        // Parse arguments if present
        let arguments = if self.match_token(&MongoTokenKind::LParen) {
            let args = self.parse_arguments()?;
            self.expect_token(&MongoTokenKind::RParen, "Expected ')' after new arguments")?;
            args
        } else {
            vec![]
        };

        let end = self.previous_pos();
        Ok(Expr::New(Box::new(NewExpr::new(
            callee,
            arguments,
            start..end,
        ))))
    }

    /// Parse primary expression (literals, identifiers, objects, arrays)
    fn parse_primary(&mut self) -> Result<Expr> {
        let start = self.current_pos();

        match self.current() {
            Some(token) => match &token.kind {
                // String literal
                MongoTokenKind::String(s) => {
                    let value = s.clone();
                    self.advance();
                    Ok(Expr::String(value))
                }
                // Number literal
                MongoTokenKind::Number(n) => {
                    let value = n
                        .parse::<f64>()
                        .map_err(|_| ParseError::SyntaxError(format!("Invalid number: {}", n)))?;
                    self.advance();
                    Ok(Expr::Number(value))
                }
                // Identifier or keyword
                MongoTokenKind::Ident(name) => {
                    let name = name.clone();
                    self.advance();

                    // Check for special identifiers
                    match name.as_str() {
                        "true" => Ok(Expr::Boolean(true)),
                        "false" => Ok(Expr::Boolean(false)),
                        "null" | "undefined" => Ok(Expr::Null),
                        "Infinity" => Ok(Expr::Number(f64::INFINITY)),
                        "NaN" => Ok(Expr::Number(f64::NAN)),
                        _ => Ok(Expr::Ident(name)),
                    }
                }
                // db keyword
                MongoTokenKind::Db => {
                    self.advance();
                    Ok(Expr::Ident("db".to_string()))
                }
                // Object literal: { ... }
                MongoTokenKind::LBrace => self.parse_object(start),
                // Array literal: [ ... ]
                MongoTokenKind::LBracket => self.parse_array(start),
                // Parenthesized expression: ( expr )
                MongoTokenKind::LParen => {
                    self.advance();
                    let expr = self.parse_expression()?;
                    self.expect_token(&MongoTokenKind::RParen, "Expected ')' after expression")?;
                    Ok(expr)
                }
                _ => Err(
                    ParseError::SyntaxError(format!("Unexpected token: {:?}", token.kind)).into(),
                ),
            },
            None => Err(ParseError::SyntaxError("Unexpected end of input".to_string()).into()),
        }
    }

    /// Parse object literal: { key: value, ... }
    fn parse_object(&mut self, start: usize) -> Result<Expr> {
        self.expect_token(&MongoTokenKind::LBrace, "Expected '{'")?;

        let mut properties = Vec::new();

        // Handle empty object
        if self.match_token(&MongoTokenKind::RBrace) {
            let end = self.previous_pos();
            return Ok(Expr::Object(ObjectExpr::new(properties, start..end)));
        }

        loop {
            let prop_start = self.current_pos();

            // Parse property key
            let key = self.parse_property_key()?;

            // Expect colon
            self.expect_token(&MongoTokenKind::Colon, "Expected ':' after property key")?;

            // Parse property value
            let value = self.parse_expression()?;

            let prop_end = self.previous_pos();
            properties.push(Property::new(key, value, prop_start..prop_end));

            // Check for comma or end of object
            if self.match_token(&MongoTokenKind::Comma) {
                // Allow trailing comma
                if self.check(&MongoTokenKind::RBrace) {
                    break;
                }
                continue;
            } else if self.check(&MongoTokenKind::RBrace) {
                break;
            } else {
                return Err(ParseError::SyntaxError(
                    "Expected ',' or '}' after property".to_string(),
                )
                .into());
            }
        }

        self.expect_token(&MongoTokenKind::RBrace, "Expected '}'")?;
        let end = self.previous_pos();

        Ok(Expr::Object(ObjectExpr::new(properties, start..end)))
    }

    /// Parse property key (identifier, string, or number)
    fn parse_property_key(&mut self) -> Result<PropertyKey> {
        match self.current() {
            Some(token) => match &token.kind {
                MongoTokenKind::Ident(name) => {
                    let key = PropertyKey::Ident(name.clone());
                    self.advance();
                    Ok(key)
                }
                MongoTokenKind::String(s) => {
                    let key = PropertyKey::String(s.clone());
                    self.advance();
                    Ok(key)
                }
                MongoTokenKind::Number(n) => {
                    let key = PropertyKey::Number(n.clone());
                    self.advance();
                    Ok(key)
                }
                _ => Err(ParseError::SyntaxError(
                    "Expected property key (identifier, string, or number)".to_string(),
                )
                .into()),
            },
            None => Err(ParseError::SyntaxError("Unexpected end of input".to_string()).into()),
        }
    }

    /// Parse array literal: [elem1, elem2, ...]
    fn parse_array(&mut self, start: usize) -> Result<Expr> {
        self.expect_token(&MongoTokenKind::LBracket, "Expected '['")?;

        let mut elements = Vec::new();

        // Handle empty array
        if self.match_token(&MongoTokenKind::RBracket) {
            let end = self.previous_pos();
            return Ok(Expr::Array(ArrayExpr::new(elements, start..end)));
        }

        loop {
            // Parse array element
            let element = self.parse_expression()?;
            elements.push(element);

            // Check for comma or end of array
            if self.match_token(&MongoTokenKind::Comma) {
                // Allow trailing comma
                if self.check(&MongoTokenKind::RBracket) {
                    break;
                }
                continue;
            } else if self.check(&MongoTokenKind::RBracket) {
                break;
            } else {
                return Err(ParseError::SyntaxError(
                    "Expected ',' or ']' after array element".to_string(),
                )
                .into());
            }
        }

        self.expect_token(&MongoTokenKind::RBracket, "Expected ']'")?;
        let end = self.previous_pos();

        Ok(Expr::Array(ArrayExpr::new(elements, start..end)))
    }

    /// Parse function arguments: arg1, arg2, ...
    fn parse_arguments(&mut self) -> Result<Vec<Expr>> {
        let mut arguments = Vec::new();

        // Handle empty arguments
        if self.check(&MongoTokenKind::RParen) {
            return Ok(arguments);
        }

        loop {
            let arg = self.parse_expression()?;
            arguments.push(arg);

            if self.match_token(&MongoTokenKind::Comma) {
                // Allow trailing comma
                if self.check(&MongoTokenKind::RParen) {
                    break;
                }
                continue;
            } else {
                break;
            }
        }

        Ok(arguments)
    }

    // Token manipulation methods

    /// Get current token
    fn current(&self) -> Option<&MongoToken> {
        self.tokens.get(self.pos)
    }

    /// Check if current token matches the given kind
    fn check(&self, kind: &MongoTokenKind) -> bool {
        if let Some(token) = self.current() {
            std::mem::discriminant(&token.kind) == std::mem::discriminant(kind)
        } else {
            false
        }
    }

    /// Match and consume token if it matches the given kind
    fn match_token(&mut self, kind: &MongoTokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Advance to next token
    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    /// Expect a specific token kind
    fn expect_token(&mut self, kind: &MongoTokenKind, message: &str) -> Result<()> {
        if self.check(kind) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::SyntaxError(message.to_string()).into())
        }
    }

    /// Expect an identifier and return its name
    fn expect_identifier(&mut self, message: &str) -> Result<String> {
        match self.current() {
            Some(token) => match &token.kind {
                MongoTokenKind::Ident(name) => {
                    let name = name.clone();
                    self.advance();
                    Ok(name)
                }
                _ => Err(ParseError::SyntaxError(message.to_string()).into()),
            },
            None => Err(ParseError::SyntaxError(message.to_string()).into()),
        }
    }

    /// Get current position
    fn current_pos(&self) -> usize {
        if let Some(token) = self.current() {
            token.span.start
        } else if let Some(last) = self.tokens.last() {
            last.span.end
        } else {
            0
        }
    }

    /// Get previous position
    fn previous_pos(&self) -> usize {
        if self.pos > 0 {
            if let Some(token) = self.tokens.get(self.pos - 1) {
                return token.span.end;
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string_literal() {
        let expr = MongoParser::parse("'hello'").unwrap();
        assert!(matches!(expr, Expr::String(s) if s == "hello"));
    }

    #[test]
    fn test_parse_number_literal() {
        let expr = MongoParser::parse("42").unwrap();
        assert!(matches!(expr, Expr::Number(n) if n == 42.0));

        let expr = MongoParser::parse("3.14").unwrap();
        assert!(matches!(expr, Expr::Number(n) if (n - 3.14).abs() < 0.001));
    }

    #[test]
    fn test_parse_boolean_literal() {
        let expr = MongoParser::parse("true").unwrap();
        assert!(matches!(expr, Expr::Boolean(true)));

        let expr = MongoParser::parse("false").unwrap();
        assert!(matches!(expr, Expr::Boolean(false)));
    }

    #[test]
    fn test_parse_null_literal() {
        let expr = MongoParser::parse("null").unwrap();
        assert!(matches!(expr, Expr::Null));

        let expr = MongoParser::parse("undefined").unwrap();
        assert!(matches!(expr, Expr::Null));
    }

    #[test]
    fn test_parse_identifier() {
        let expr = MongoParser::parse("db").unwrap();
        assert!(matches!(expr, Expr::Ident(s) if s == "db"));
    }

    #[test]
    fn test_parse_empty_object() {
        let expr = MongoParser::parse("{}").unwrap();
        match expr {
            Expr::Object(obj) => assert_eq!(obj.properties.len(), 0),
            _ => panic!("Expected object expression"),
        }
    }

    #[test]
    fn test_parse_simple_object() {
        let expr = MongoParser::parse("{name: 'John', age: 30}").unwrap();
        match expr {
            Expr::Object(obj) => {
                assert_eq!(obj.properties.len(), 2);
                assert_eq!(obj.properties[0].key.as_string(), "name");
                assert_eq!(obj.properties[1].key.as_string(), "age");
            }
            _ => panic!("Expected object expression"),
        }
    }

    #[test]
    fn test_parse_nested_object() {
        let expr = MongoParser::parse("{user: {name: 'John'}}").unwrap();
        match expr {
            Expr::Object(obj) => {
                assert_eq!(obj.properties.len(), 1);
                assert!(matches!(&obj.properties[0].value, Expr::Object(_)));
            }
            _ => panic!("Expected object expression"),
        }
    }

    #[test]
    fn test_parse_empty_array() {
        let expr = MongoParser::parse("[]").unwrap();
        match expr {
            Expr::Array(arr) => assert_eq!(arr.elements.len(), 0),
            _ => panic!("Expected array expression"),
        }
    }

    #[test]
    fn test_parse_simple_array() {
        let expr = MongoParser::parse("[1, 2, 3]").unwrap();
        match expr {
            Expr::Array(arr) => assert_eq!(arr.elements.len(), 3),
            _ => panic!("Expected array expression"),
        }
    }

    #[test]
    fn test_parse_member_expression() {
        let expr = MongoParser::parse("db.users").unwrap();
        match expr {
            Expr::Member(member) => {
                assert!(matches!(*member.object, Expr::Ident(ref s) if s == "db"));
                assert!(matches!(member.property, MemberProperty::Ident(ref s) if s == "users"));
            }
            _ => panic!("Expected member expression"),
        }
    }

    #[test]
    fn test_parse_chained_member_expression() {
        let expr = MongoParser::parse("db.users.find").unwrap();
        match expr {
            Expr::Member(outer) => {
                assert!(matches!(outer.property, MemberProperty::Ident(ref s) if s == "find"));
                match *outer.object {
                    Expr::Member(inner) => {
                        assert!(
                            matches!(inner.property, MemberProperty::Ident(ref s) if s == "users")
                        );
                    }
                    _ => panic!("Expected nested member expression"),
                }
            }
            _ => panic!("Expected member expression"),
        }
    }

    #[test]
    fn test_parse_call_expression() {
        let expr = MongoParser::parse("find()").unwrap();
        match expr {
            Expr::Call(call) => {
                assert!(matches!(*call.callee, Expr::Ident(ref s) if s == "find"));
                assert_eq!(call.arguments.len(), 0);
            }
            _ => panic!("Expected call expression"),
        }
    }

    #[test]
    fn test_parse_call_with_arguments() {
        let expr = MongoParser::parse("find({}, {_id: 0})").unwrap();
        match expr {
            Expr::Call(call) => {
                assert_eq!(call.arguments.len(), 2);
            }
            _ => panic!("Expected call expression"),
        }
    }

    #[test]
    fn test_parse_chained_call() {
        let expr = MongoParser::parse("db.users.find().limit(10)").unwrap();
        match expr {
            Expr::Call(outer_call) => {
                assert!(matches!(*outer_call.callee, Expr::Member(_)));
                match *outer_call.callee {
                    Expr::Member(member) => {
                        assert!(
                            matches!(member.property, MemberProperty::Ident(ref s) if s == "limit")
                        );
                        assert!(matches!(*member.object, Expr::Call(_)));
                    }
                    _ => panic!("Expected member expression in callee"),
                }
            }
            _ => panic!("Expected call expression"),
        }
    }

    #[test]
    fn test_parse_new_expression() {
        let expr = MongoParser::parse("new Date()").unwrap();
        match expr {
            Expr::New(new_expr) => {
                assert!(matches!(*new_expr.callee, Expr::Ident(ref s) if s == "Date"));
                assert_eq!(new_expr.arguments.len(), 0);
            }
            _ => panic!("Expected new expression"),
        }
    }

    #[test]
    fn test_parse_new_with_arguments() {
        let expr = MongoParser::parse("new ObjectId('507f1f77bcf86cd799439011')").unwrap();
        match expr {
            Expr::New(new_expr) => {
                assert!(matches!(*new_expr.callee, Expr::Ident(ref s) if s == "ObjectId"));
                assert_eq!(new_expr.arguments.len(), 1);
            }
            _ => panic!("Expected new expression"),
        }
    }

    #[test]
    fn test_parse_unary_minus() {
        let expr = MongoParser::parse("-5").unwrap();
        match expr {
            Expr::Unary(unary) => {
                assert_eq!(unary.operator, UnaryOperator::Minus);
                assert!(matches!(*unary.argument, Expr::Number(n) if n == 5.0));
            }
            _ => panic!("Expected unary expression"),
        }
    }

    #[test]
    fn test_parse_computed_member() {
        let expr = MongoParser::parse("obj['key']").unwrap();
        match expr {
            Expr::Member(member) => {
                assert!(matches!(member.property, MemberProperty::Computed(_)));
            }
            _ => panic!("Expected member expression"),
        }
    }

    #[test]
    fn test_parse_complex_expression() {
        let expr = MongoParser::parse("db.users.find({age: {$gt: 18}})").unwrap();
        assert!(matches!(expr, Expr::Call(_)));
    }
}
