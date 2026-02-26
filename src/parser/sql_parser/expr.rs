//! Expression parsing for SQL parser
//!
//! This module handles parsing of:
//! - Logical expressions (AND, OR)
//! - Comparison expressions (=, !=, >, <, >=, <=)
//! - Arithmetic expressions (+, -, *, /, %)
//! - Literals (numbers, strings, booleans, null)
//! - Function calls
//! - Typed literals (DATE, TIMESTAMP, etc.)

use super::super::sql_context::{
    ArithmeticOperator, Expected, FieldPath, ParseError, ParseResult, SqlExpr, SqlLiteral,
    SqlLogicalOperator, SqlOperator,
};
use super::super::sql_lexer::TokenKind;

impl super::SqlParser {
    /// Parse expression using Pratt parser (operator precedence)
    pub(super) fn parse_expression(&mut self, min_bp: u8) -> ParseResult<SqlExpr> {
        let mut left = match self.parse_primary_expr() {
            ParseResult::Ok(expr) => expr,
            result => return result,
        };

        loop {
            if self.is_at_eof() {
                self.expected = vec![Expected::Operator, Expected::EndOfStatement];
                return ParseResult::Ok(left);
            }

            // Check for logical operators
            let (op, l_bp, r_bp) = if self.check_keyword(&TokenKind::And) {
                (SqlLogicalOperator::And, 3, 4)
            } else if self.check_keyword(&TokenKind::Or) {
                (SqlLogicalOperator::Or, 1, 2)
            } else {
                break;
            };

            if l_bp < min_bp {
                break;
            }

            self.advance(); // Consume operator

            let right = match self.parse_expression(r_bp) {
                ParseResult::Ok(expr) => expr,
                ParseResult::Partial(expr, exp) => {
                    return ParseResult::Partial(
                        SqlExpr::LogicalOp {
                            left: Box::new(left),
                            op,
                            right: Box::new(expr),
                        },
                        exp,
                    );
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            };

            left = SqlExpr::LogicalOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        ParseResult::Ok(left)
    }

    /// Parse primary expression (comparison, literal, column)
    /// Now supports arithmetic expressions on both sides of comparison
    pub(super) fn parse_primary_expr(&mut self) -> ParseResult<SqlExpr> {
        // Handle parenthesized expressions at the top level
        if self.check_token(&TokenKind::LParen) {
            // Could be a parenthesized arithmetic expression
            let left = match self.parse_comparison_operand() {
                ParseResult::Ok(expr) => expr,
                result => return result,
            };
            return self.parse_comparison_with_left(left);
        }

        // Parse left side as an arithmetic expression (field, literal, or arithmetic)
        let left = match self.parse_comparison_operand() {
            ParseResult::Ok(expr) => expr,
            ParseResult::Partial(expr, exp) => return ParseResult::Partial(expr, exp),
            ParseResult::Error(err) => return ParseResult::Error(err),
        };

        self.parse_comparison_with_left(left)
    }

    /// Parse a comparison operand (arithmetic expression)
    pub(super) fn parse_comparison_operand(&mut self) -> ParseResult<SqlExpr> {
        self.parse_arithmetic_expr(0)
    }

    /// Parse comparison operator and right side, given the left side
    pub(super) fn parse_comparison_with_left(&mut self, left: SqlExpr) -> ParseResult<SqlExpr> {
        // Check for comparison operator
        let op = if self.match_token(&TokenKind::Eq) {
            SqlOperator::Eq
        } else if self.match_token(&TokenKind::Ne) {
            SqlOperator::Ne
        } else if self.match_token(&TokenKind::Gt) {
            SqlOperator::Gt
        } else if self.match_token(&TokenKind::Lt) {
            SqlOperator::Lt
        } else if self.match_token(&TokenKind::Ge) {
            SqlOperator::Ge
        } else if self.match_token(&TokenKind::Le) {
            SqlOperator::Le
        } else if self.is_at_eof() {
            self.expected = vec![Expected::Operator];
            return ParseResult::Partial(left, self.expected.clone());
        } else {
            // Check if the next token is a valid token that can follow a WHERE expression
            // Valid tokens: AND, OR, GROUP, ORDER, LIMIT, OFFSET, EOF
            if let Some(kind) = self.peek_kind() {
                match kind {
                    TokenKind::And | TokenKind::Or => {
                        // Logical operators - this is actually invalid SQL (field without comparison)
                        // but we'll let the expression parser handle it for better error messages
                        return ParseResult::Error(ParseError::new(
                            format!("Expected comparison operator (=, !=, >, <, >=, <=) after expression, found {:?}", kind),
                            self.current_position()..self.current_position(),
                        ));
                    }
                    TokenKind::GroupBy | TokenKind::OrderBy | TokenKind::Limit | TokenKind::Offset => {
                        // Next clause - field without comparison is invalid
                        return ParseResult::Error(ParseError::new(
                            "Expected comparison operator (=, !=, >, <, >=, <=) after expression".to_string(),
                            self.current_position()..self.current_position(),
                        ));
                    }
                    TokenKind::Semicolon => {
                        // Semicolon in the middle of WHERE clause is invalid
                        return ParseResult::Error(ParseError::new(
                            "Unexpected semicolon in WHERE clause. Expected comparison operator (=, !=, >, <, >=, <=)".to_string(),
                            self.current_position()..self.current_position(),
                        ));
                    }
                    _ => {
                        // Any other token is unexpected
                        return ParseResult::Error(ParseError::new(
                            "Expected comparison operator (=, !=, >, <, >=, <=) after expression".to_string(),
                            self.current_position()..self.current_position(),
                        ));
                    }
                }
            }
            // This shouldn't happen as we already checked is_at_eof() above
            return ParseResult::Error(ParseError::new(
                "Expected comparison operator after expression".to_string(),
                self.current_position()..self.current_position(),
            ));
        };

        // Parse right side as arithmetic expression too
        let right = match self.parse_comparison_operand() {
            ParseResult::Ok(expr) => expr,
            ParseResult::Partial(expr, exp) => {
                return ParseResult::Partial(
                    SqlExpr::BinaryOp {
                        left: Box::new(left),
                        op,
                        right: Box::new(expr),
                    },
                    exp,
                );
            }
            ParseResult::Error(err) => return ParseResult::Error(err),
        };

        ParseResult::Ok(SqlExpr::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        })
    }

    /// Parse an arithmetic expression with operator precedence (Pratt parser)
    pub(super) fn parse_arithmetic_expr(&mut self, min_bp: u8) -> ParseResult<SqlExpr> {
        // Parse left-hand side (atom: literal, field, or parenthesized expression)
        let mut left = match self.parse_arithmetic_atom() {
            ParseResult::Ok(expr) => expr,
            result => return result,
        };

        // Parse operators with precedence
        loop {
            if self.is_at_eof() {
                return ParseResult::Ok(left);
            }

            // Check for arithmetic operator
            let (op, l_bp, r_bp) = match self.peek_kind() {
                Some(TokenKind::Plus) => (ArithmeticOperator::Add, 9, 10),
                Some(TokenKind::Minus) => (ArithmeticOperator::Subtract, 9, 10),
                Some(TokenKind::Star) => (ArithmeticOperator::Multiply, 11, 12),
                Some(TokenKind::Slash) => (ArithmeticOperator::Divide, 11, 12),
                Some(TokenKind::Percent) => (ArithmeticOperator::Modulo, 11, 12),
                _ => break, // Not an arithmetic operator
            };

            if l_bp < min_bp {
                break;
            }

            self.advance(); // Consume operator

            let right = match self.parse_arithmetic_expr(r_bp) {
                ParseResult::Ok(expr) => expr,
                ParseResult::Partial(expr, exp) => {
                    return ParseResult::Partial(
                        SqlExpr::ArithmeticOp {
                            left: Box::new(left),
                            op,
                            right: Box::new(expr),
                        },
                        exp,
                    );
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            };

            left = SqlExpr::ArithmeticOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        ParseResult::Ok(left)
    }

    /// Parse an arithmetic atom (literal, field reference, function, or parenthesized expression)
    pub(super) fn parse_arithmetic_atom(&mut self) -> ParseResult<SqlExpr> {
        // Handle parenthesized expressions
        if self.match_token(&TokenKind::LParen) {
            let inner = match self.parse_arithmetic_expr(0) {
                ParseResult::Ok(expr) => expr,
                result => return result,
            };

            if !self.match_token(&TokenKind::RParen) {
                if self.is_at_eof() {
                    return ParseResult::Partial(inner, vec![Expected::Keyword(")")]);
                }
                return ParseResult::Error(ParseError::new(
                    "Expected ')' after expression".to_string(),
                    self.current_position()..self.current_position(),
                ));
            }

            return ParseResult::Ok(inner);
        }

        // Check for field reference (identifier)
        if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let name = name.clone();
            let saved_pos = self.pos;
            self.advance();

            // Check if it's a function call
            if self.check_token(&TokenKind::LParen) {
                self.pos = saved_pos; // Restore position and parse as function below
            } else {
                // Parse field path continuation (nested fields, array access)
                let path = match self.parse_field_path_continuation(FieldPath::simple(name)) {
                    Ok(p) => p,
                    Err(err) => {
                        return ParseResult::Error(ParseError::new(
                            err.to_user_message(),
                            self.current_position()..self.current_position(),
                        ));
                    }
                };
                return ParseResult::Ok(SqlExpr::FieldPath(path));
            }
        }

        // Parse as literal (number, string, etc.) or other value expressions
        // Delegate to the existing logic for typed literals, functions, etc.
        self.parse_value_atom()
    }

    /// Parse a value atom (literal, function call, typed literal, etc.)
    pub(super) fn parse_value_atom(&mut self) -> ParseResult<SqlExpr> {
        // Check for typed literals: DATE '...', TIMESTAMP '...', TIME '...'
        if let Some(token_kind) = self.peek_kind() {
            match token_kind {
                TokenKind::Date | TokenKind::Timestamp | TokenKind::Time => {
                    let type_name = match token_kind {
                        TokenKind::Date => "DATE",
                        TokenKind::Timestamp => "TIMESTAMP",
                        TokenKind::Time => "TIME",
                        _ => unreachable!(),
                    }
                    .to_string();

                    self.advance();

                    if let Some(TokenKind::String(value)) = self.peek_kind() {
                        let value = value.clone();
                        self.advance();
                        return ParseResult::Ok(SqlExpr::TypedLiteral { type_name, value });
                    } else if self.is_at_eof() {
                        return ParseResult::Partial(
                            SqlExpr::TypedLiteral {
                                type_name,
                                value: String::new(),
                            },
                            vec![Expected::String],
                        );
                    } else {
                        return ParseResult::Error(ParseError::new(
                            format!("Expected string literal after {}", type_name),
                            self.current_position()..self.current_position(),
                        ));
                    }
                }
                TokenKind::CurrentTimestamp => {
                    self.advance();
                    return ParseResult::Ok(SqlExpr::CurrentTime {
                        kind: "CURRENT_TIMESTAMP".to_string(),
                    });
                }
                TokenKind::CurrentDate => {
                    self.advance();
                    return ParseResult::Ok(SqlExpr::CurrentTime {
                        kind: "CURRENT_DATE".to_string(),
                    });
                }
                TokenKind::CurrentTime => {
                    self.advance();
                    return ParseResult::Ok(SqlExpr::CurrentTime {
                        kind: "CURRENT_TIME".to_string(),
                    });
                }
                TokenKind::Now => {
                    self.advance();
                    if self.match_token(&TokenKind::LParen) {
                        if !self.match_token(&TokenKind::RParen) {
                            if self.is_at_eof() {
                                return ParseResult::Partial(
                                    SqlExpr::CurrentTime {
                                        kind: "NOW".to_string(),
                                    },
                                    vec![Expected::Keyword(")")],
                                );
                            }
                            return ParseResult::Error(ParseError::new(
                                "Expected ')' after NOW(".to_string(),
                                self.current_position()..self.current_position(),
                            ));
                        }
                    }
                    return ParseResult::Ok(SqlExpr::CurrentTime {
                        kind: "NOW".to_string(),
                    });
                }
                _ => {}
            }
        }

        // Check for function call
        if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let name = name.clone();
            let saved_pos = self.pos;
            self.advance();

            if self.match_token(&TokenKind::LParen) {
                let mut args = Vec::new();

                if !self.check_token(&TokenKind::RParen) {
                    loop {
                        // Parse argument as arithmetic expression to support: ROUND(price * 1.13, 2)
                        match self.parse_arithmetic_expr(0) {
                            ParseResult::Ok(expr) => {
                                args.push(expr);
                            }
                            ParseResult::Partial(expr, exp) => {
                                return ParseResult::Partial(
                                    SqlExpr::Function {
                                        name,
                                        args: vec![expr],
                                    },
                                    exp,
                                );
                            }
                            ParseResult::Error(err) => return ParseResult::Error(err),
                        }

                        if self.match_token(&TokenKind::Comma) {
                            continue;
                        } else {
                            break;
                        }
                    }
                }

                if !self.match_token(&TokenKind::RParen) {
                    if self.is_at_eof() {
                        return ParseResult::Partial(
                            SqlExpr::Function { name, args },
                            vec![Expected::Keyword(")")],
                        );
                    }
                    return ParseResult::Error(ParseError::new(
                        "Expected ')' after function arguments".to_string(),
                        self.current_position()..self.current_position(),
                    ));
                }

                return ParseResult::Ok(SqlExpr::Function { name, args });
            } else {
                self.pos = saved_pos;
            }
        }

        // Parse as literal
        match self.parse_literal() {
            ParseResult::Ok(lit) => ParseResult::Ok(SqlExpr::Literal(lit)),
            ParseResult::Partial(lit, exp) => ParseResult::Partial(SqlExpr::Literal(lit), exp),
            ParseResult::Error(err) => ParseResult::Error(err),
        }
    }

    /// Parse literal value
    pub(super) fn parse_literal(&mut self) -> ParseResult<SqlLiteral> {
        if self.is_at_eof() {
            self.expected = vec![Expected::Number, Expected::String];
            return ParseResult::Partial(SqlLiteral::Null, self.expected.clone());
        }

        match self.peek_kind() {
            Some(TokenKind::Number(n)) => {
                let num = n.clone();
                self.advance();
                let value = num.parse::<f64>().unwrap_or(0.0);
                ParseResult::Ok(SqlLiteral::Number(value))
            }
            Some(TokenKind::String(s)) => {
                let str_val = s.clone();
                self.advance();
                ParseResult::Ok(SqlLiteral::String(str_val))
            }
            Some(TokenKind::True) => {
                self.advance();
                ParseResult::Ok(SqlLiteral::Boolean(true))
            }
            Some(TokenKind::False) => {
                self.advance();
                ParseResult::Ok(SqlLiteral::Boolean(false))
            }
            Some(TokenKind::Null) => {
                self.advance();
                ParseResult::Ok(SqlLiteral::Null)
            }
            _ => ParseResult::Error(ParseError::new(
                "Expected literal value".to_string(),
                self.current_position()..self.current_position(),
            )),
        }
    }

    /// Continue parsing arithmetic expression with given left side
    pub(super) fn continue_arithmetic_expr(
        &mut self,
        left: SqlExpr,
        min_bp: u8,
    ) -> std::result::Result<SqlExpr, ParseError> {
        let mut left = left;

        loop {
            if self.is_at_eof() {
                return Ok(left);
            }

            // Check for arithmetic operator
            let (op, l_bp, r_bp) = match self.peek_kind() {
                Some(TokenKind::Plus) => (ArithmeticOperator::Add, 9, 10),
                Some(TokenKind::Minus) => (ArithmeticOperator::Subtract, 9, 10),
                Some(TokenKind::Star) => (ArithmeticOperator::Multiply, 11, 12),
                Some(TokenKind::Slash) => (ArithmeticOperator::Divide, 11, 12),
                Some(TokenKind::Percent) => (ArithmeticOperator::Modulo, 11, 12),
                _ => break,
            };

            if l_bp < min_bp {
                break;
            }

            self.advance(); // Consume operator

            let right = match self.parse_arithmetic_expr(r_bp) {
                ParseResult::Ok(expr) => expr,
                ParseResult::Error(err) => return Err(err),
                ParseResult::Partial(expr, _) => expr,
            };

            left = SqlExpr::ArithmeticOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }
}
