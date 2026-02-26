//! Column parsing for SQL parser
//!
//! This module handles parsing of:
//! - Column specifications (field names, expressions, aggregates)
//! - Aggregate functions (COUNT, SUM, AVG, MIN, MAX)
//! - Column aliases (AS alias)
//! - Arithmetic expressions in SELECT clause

use super::super::sql_context::{
    Expected, FieldPath, ParseError, ParseResult, SqlColumn, SqlExpr,
};
use super::super::sql_lexer::TokenKind;

impl super::SqlParser {
    /// Parse a single column (field, aggregate function, or arithmetic expression)
    pub(super) fn parse_column(&mut self) -> ParseResult<SqlColumn> {
        // Check for aggregate functions
        if let Some(token) = self.peek() {
            match &token.kind {
                TokenKind::Count
                | TokenKind::Sum
                | TokenKind::Avg
                | TokenKind::Min
                | TokenKind::Max => {
                    // Parse aggregate function, but check if it's followed by arithmetic
                    return self.parse_aggregate_or_expression();
                }
                TokenKind::Ident(name) => {
                    let name = name.clone();
                    let saved_pos = self.pos;
                    self.advance();

                    // Parse field path (supports nested fields and array access)
                    let path = match self.parse_field_path_continuation(FieldPath::simple(name)) {
                        Ok(p) => p,
                        Err(err) => {
                            return ParseResult::Error(ParseError::new(
                                err.to_user_message(),
                                self.current_position()..self.current_position(),
                            ));
                        }
                    };

                    // Check if next token is an arithmetic operator
                    if let Some(kind) = self.peek_kind() {
                        match kind {
                            TokenKind::Plus | TokenKind::Minus | TokenKind::Star
                            | TokenKind::Slash | TokenKind::Percent => {
                                // This is an arithmetic expression, reparse from beginning
                                self.pos = saved_pos;
                                return self.parse_expression_column();
                            }
                            _ => {}
                        }
                    }

                    // Check for AS alias
                    let alias = if self.match_keyword(&TokenKind::As) {
                        match self.peek_kind() {
                            Some(TokenKind::Ident(alias_name)) => {
                                let alias = alias_name.clone();
                                self.advance();
                                Some(alias)
                            }
                            Some(TokenKind::String(alias_name)) => {
                                let alias = alias_name.clone();
                                self.advance();
                                Some(alias)
                            }
                            _ if self.is_at_eof() => {
                                self.expected = vec![Expected::ColumnName];
                                return ParseResult::Partial(
                                    SqlColumn::Field { path, alias: None },
                                    self.expected.clone(),
                                );
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    return ParseResult::Ok(SqlColumn::Field { path, alias });
                }
                TokenKind::LParen => {
                    // Parenthesized expression
                    return self.parse_expression_column();
                }
                _ => {}
            }
        }

        if self.is_at_eof() {
            self.expected = vec![Expected::ColumnName, Expected::AggregateFunction];
            return ParseResult::Partial(
                SqlColumn::Field {
                    path: FieldPath::simple(String::new()),
                    alias: None,
                },
                self.expected.clone(),
            );
        }

        ParseResult::Error(ParseError::new(
            "Expected column name or aggregate function".to_string(),
            self.current_position()..self.current_position(),
        ))
    }

    /// Parse an expression column (arithmetic expression with optional alias)
    pub(super) fn parse_expression_column(&mut self) -> ParseResult<SqlColumn> {
        // Parse arithmetic expression
        let expr = match self.parse_arithmetic_expr(0) {
            ParseResult::Ok(expr) => expr,
            ParseResult::Partial(expr, exp) => {
                return ParseResult::Partial(
                    SqlColumn::Expression {
                        expr: Box::new(expr),
                        alias: None,
                    },
                    exp,
                );
            }
            ParseResult::Error(err) => return ParseResult::Error(err),
        };

        // Check for AS alias
        let alias = if self.match_keyword(&TokenKind::As) {
            match self.peek_kind() {
                Some(TokenKind::Ident(alias_name)) => {
                    let alias = alias_name.clone();
                    self.advance();
                    Some(alias)
                }
                Some(TokenKind::String(alias_name)) => {
                    let alias = alias_name.clone();
                    self.advance();
                    Some(alias)
                }
                _ if self.is_at_eof() => {
                    self.expected = vec![Expected::ColumnName];
                    return ParseResult::Partial(
                        SqlColumn::Expression {
                            expr: Box::new(expr),
                            alias: None,
                        },
                        self.expected.clone(),
                    );
                }
                _ => None,
            }
        } else {
            None
        };

        ParseResult::Ok(SqlColumn::Expression {
            expr: Box::new(expr),
            alias,
        })
    }

    /// Parse aggregate function, checking if it's part of an arithmetic expression
    pub(super) fn parse_aggregate_or_expression(&mut self) -> ParseResult<SqlColumn> {
        // First parse the aggregate function as an expression
        let agg_expr = match self.parse_aggregate_as_expr() {
            ParseResult::Ok(expr) => expr,
            ParseResult::Partial(expr, exp) => {
                return ParseResult::Partial(
                    SqlColumn::Expression {
                        expr: Box::new(expr),
                        alias: None,
                    },
                    exp,
                )
            }
            ParseResult::Error(err) => return ParseResult::Error(err),
        };

        // Check if followed by arithmetic operator
        if let Some(kind) = self.peek_kind() {
            match kind {
                TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent => {
                    // This is an arithmetic expression starting with aggregate
                    // We need to continue parsing the arithmetic expression
                    let full_expr = match self.continue_arithmetic_expr(agg_expr, 0) {
                        Ok(expr) => expr,
                        Err(err) => return ParseResult::Error(err),
                    };

                    // Check for AS alias
                    let alias = if self.match_keyword(&TokenKind::As) {
                        match self.peek_kind() {
                            Some(TokenKind::Ident(alias_name)) => {
                                let alias = alias_name.clone();
                                self.advance();
                                Some(alias)
                            }
                            Some(TokenKind::String(alias_name)) => {
                                let alias = alias_name.clone();
                                self.advance();
                                Some(alias)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    return ParseResult::Ok(SqlColumn::Expression {
                        expr: Box::new(full_expr),
                        alias,
                    });
                }
                _ => {}
            }
        }

        // Not followed by arithmetic - convert back to SqlColumn::Aggregate
        match agg_expr {
            SqlExpr::Function { name, args } => {
                // Extract field from args if present
                let (field, distinct) = if args.is_empty() {
                    (None, false)
                } else if let Some(SqlExpr::FieldPath(path)) = args.first() {
                    (Some(path.clone()), false)
                } else {
                    (None, false)
                };

                // Check for AS alias
                let alias = if self.match_keyword(&TokenKind::As) {
                    match self.peek_kind() {
                        Some(TokenKind::Ident(alias_name)) => {
                            let alias = alias_name.clone();
                            self.advance();
                            Some(alias)
                        }
                        Some(TokenKind::String(alias_name)) => {
                            let alias = alias_name.clone();
                            self.advance();
                            Some(alias)
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                ParseResult::Ok(SqlColumn::Aggregate {
                    func: name,
                    field,
                    alias,
                    distinct,
                })
            }
            _ => ParseResult::Ok(SqlColumn::Expression {
                expr: Box::new(agg_expr),
                alias: None,
            }),
        }
    }

    /// Parse aggregate function as SqlExpr::Function
    pub(super) fn parse_aggregate_as_expr(&mut self) -> ParseResult<SqlExpr> {
        let func = match self.peek_kind() {
            Some(TokenKind::Count) => "COUNT".to_string(),
            Some(TokenKind::Sum) => "SUM".to_string(),
            Some(TokenKind::Avg) => "AVG".to_string(),
            Some(TokenKind::Min) => "MIN".to_string(),
            Some(TokenKind::Max) => "MAX".to_string(),
            _ => {
                return ParseResult::Error(ParseError::new(
                    "Expected aggregate function".to_string(),
                    self.current_position()..self.current_position(),
                ));
            }
        };

        self.advance();

        // Expect opening parenthesis
        if !self.match_token(&TokenKind::LParen) {
            return ParseResult::Error(ParseError::new(
                "Expected '(' after aggregate function".to_string(),
                self.current_position()..self.current_position(),
            ));
        }

        // Check for DISTINCT keyword (skip for now in expression context)
        self.match_token(&TokenKind::Distinct);

        // Parse field or *
        let args = if self.match_token(&TokenKind::Star) {
            vec![] // COUNT(*) has no args
        } else if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let name = name.clone();
            self.advance();

            let path = match self.parse_field_path_continuation(FieldPath::simple(name)) {
                Ok(p) => p,
                Err(err) => {
                    return ParseResult::Error(ParseError::new(
                        err.to_user_message(),
                        self.current_position()..self.current_position(),
                    ));
                }
            };

            vec![SqlExpr::FieldPath(path)]
        } else {
            vec![]
        };

        // Expect closing parenthesis
        if !self.match_token(&TokenKind::RParen) {
            return ParseResult::Error(ParseError::new(
                "Expected ')' after aggregate function".to_string(),
                self.current_position()..self.current_position(),
            ));
        }

        ParseResult::Ok(SqlExpr::Function { name: func, args })
    }
}
