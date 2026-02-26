//! SQL parser module
//!
//! This module provides SQL query parsing with support for:
//! - SELECT queries with WHERE, GROUP BY, ORDER BY, LIMIT, OFFSET
//! - Aggregate functions (COUNT, SUM, AVG, MIN, MAX)
//! - Arithmetic expressions
//! - Nested fields and array access
//! - EXPLAIN queries
//! - Partial parsing for autocomplete
//!
//! # Architecture
//!
//! The parser is split into focused submodules:
//! - `field`: Field path and array access parsing
//! - `expr`: Expression parsing (arithmetic, comparison, logical)
//! - `column`: Column specification parsing
//! - `converter`: AST to MongoDB command conversion
//! - `tests`: Comprehensive test suite

use super::command::Command;
use super::sql_context::{
    Expected, ParseError, ParseResult, SqlClause, SqlColumn, SqlContext, SqlOrderBy, SqlSelect,
};
use super::sql_lexer::{SqlLexer, Token, TokenKind};
use crate::error::Result;

// Submodules
mod column;
mod converter;
mod expr;
mod field;

#[cfg(test)]
mod tests;

/// SQL Parser with error-tolerant partial parsing
pub struct SqlParser {
    tokens: Vec<Token>,
    pos: usize,
    current_clause: SqlClause,
    expected: Vec<Expected>,
}

impl SqlParser {
    /// Create a new parser from tokens
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            current_clause: SqlClause::Select,
            expected: Vec::new(),
        }
    }

    /// Check if input looks like a SQL command
    pub fn is_sql_command(input: &str) -> bool {
        let trimmed = input.trim().to_uppercase();
        trimmed.starts_with("SELECT ")
            || trimmed == "SELECT"
            || trimmed.starts_with("SELECT\t")
            || trimmed.starts_with("SELECT\n")
            || trimmed.starts_with("EXPLAIN ")
            || trimmed == "EXPLAIN"
            || trimmed.starts_with("EXPLAIN\t")
            || trimmed.starts_with("EXPLAIN\n")
    }

    /// Parse SQL and convert to Command
    pub fn parse_to_command(input: &str) -> Result<Command> {
        let tokens = SqlLexer::tokenize(input);
        let mut parser = Self::new(tokens);

        // Check if this is an EXPLAIN statement
        let is_explain = parser.peek_kind() == Some(&TokenKind::Explain);
        let verbosity = if is_explain {
            parser.advance(); // consume EXPLAIN

            // Check for optional verbosity parameter
            // EXPLAIN SELECT ... (default queryPlanner)
            // EXPLAIN queryPlanner SELECT ...
            // EXPLAIN executionStats SELECT ...
            // EXPLAIN allPlansExecution SELECT ...
            match parser.peek_kind() {
                Some(TokenKind::Ident(verb_str)) => {
                    let verb = super::command::ExplainVerbosity::from_str(&verb_str)?;
                    parser.advance(); // consume verbosity identifier
                    Some(verb)
                }
                Some(TokenKind::String(verb_str)) => {
                    // Also support quoted strings for backwards compatibility
                    let verb = super::command::ExplainVerbosity::from_str(&verb_str)?;
                    parser.advance(); // consume verbosity string
                    Some(verb)
                }
                _ => Some(super::command::ExplainVerbosity::QueryPlanner),
            }
        } else {
            None
        };

        let result = parser.parse_select_statement();

        match result {
            ParseResult::Ok(select) => {
                let cmd = parser.ast_to_command(select)?;

                // Wrap in EXPLAIN if needed
                if let Some(verbosity) = verbosity {
                    parser.wrap_in_explain(cmd, verbosity)
                } else {
                    Ok(cmd)
                }
            }
            ParseResult::Partial(select, expected) => {
                // For partial parse, reject execution if:
                // 1. No table name
                // 2. Expected operators (incomplete WHERE expression)
                if select.table.is_none() {
                    return Err(crate::error::ParseError::InvalidCommand(format!(
                        "Incomplete SQL query. Expected: {}",
                        expected
                            .iter()
                            .map(|e| e.description())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .into());
                }

                // Check if we have an incomplete WHERE clause (expecting operator)
                if expected.contains(&Expected::Operator) {
                    return Err(crate::error::ParseError::InvalidCommand(
                        "Incomplete WHERE clause. Expected comparison operator (=, !=, >, <, >=, <=)".to_string()
                    )
                    .into());
                }

                // Try to convert partial parse if it has enough information
                let cmd = parser.ast_to_command(select)?;

                // Wrap in EXPLAIN if needed
                if let Some(verbosity) = verbosity {
                    parser.wrap_in_explain(cmd, verbosity)
                } else {
                    Ok(cmd)
                }
            }
            ParseResult::Error(err) => Err(crate::error::ParseError::InvalidCommand(format!(
                "SQL parse error: {}",
                err.message
            ))
            .into()),
        }
    }

    /// Parse with context for autocomplete
    #[allow(dead_code)]
    pub fn parse_with_context(input: &str) -> (ParseResult<SqlSelect>, SqlContext) {
        let tokens = SqlLexer::tokenize(input);
        let mut parser = Self::new(tokens);
        let result = parser.parse_select_statement();

        let context = SqlContext {
            clause: parser.current_clause.clone(),
            position: parser.pos,
            expected: parser.expected.clone(),
            partial_input: input.to_string(),
        };

        (result, context)
    }

    /// Parse a SELECT statement
    fn parse_select_statement(&mut self) -> ParseResult<SqlSelect> {
        // Expect SELECT keyword
        if !self.match_keyword(&TokenKind::Select) {
            self.expected = vec![Expected::Keyword("SELECT")];
            return ParseResult::Error(ParseError::new(
                "Expected SELECT keyword".to_string(),
                0..0,
            ));
        }

        self.current_clause = SqlClause::Select;

        // Parse select list
        let columns = match self.parse_select_list() {
            ParseResult::Ok(cols) => cols,
            ParseResult::Partial(cols, exp) => {
                self.expected = exp.clone();
                let mut select = SqlSelect::new();
                select.columns = cols;
                return ParseResult::Partial(select, exp);
            }
            ParseResult::Error(err) => return ParseResult::Error(err),
        };

        // Parse FROM clause (optional for partial)
        self.current_clause = SqlClause::From;
        let table = if self.match_keyword(&TokenKind::From) {
            match self.parse_from_clause() {
                ParseResult::Ok(tbl) => Some(tbl),
                ParseResult::Partial(tbl, exp) => {
                    self.expected = exp.clone();
                    let mut select = SqlSelect::new();
                    select.columns = columns;
                    select.table = if tbl.is_empty() { None } else { Some(tbl) };
                    return ParseResult::Partial(select, exp);
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            }
        } else if self.is_at_eof() {
            // Partial: "SELECT * |"
            self.expected = vec![Expected::Keyword("FROM")];
            let mut select = SqlSelect::new();
            select.columns = columns;
            return ParseResult::Partial(select, self.expected.clone());
        } else {
            None
        };

        // Parse WHERE clause (optional)
        self.current_clause = SqlClause::Where;
        let where_clause = if self.match_keyword(&TokenKind::Where) {
            match self.parse_where_clause() {
                ParseResult::Ok(expr) => Some(expr),
                ParseResult::Partial(expr, exp) => {
                    self.expected = exp.clone();
                    let mut select = SqlSelect::new();
                    select.columns = columns;
                    select.table = table;
                    select.where_clause = Some(expr);
                    return ParseResult::Partial(select, exp);
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            }
        } else {
            None
        };

        // Parse GROUP BY clause (optional)
        self.current_clause = SqlClause::GroupBy;
        let group_by = if self.match_keyword(&TokenKind::GroupBy) {
            match self.parse_group_by_clause() {
                ParseResult::Ok(cols) => Some(cols),
                ParseResult::Partial(cols, exp) => {
                    self.expected = exp.clone();
                    let mut select = SqlSelect::new();
                    select.columns = columns;
                    select.table = table;
                    select.where_clause = where_clause;
                    select.group_by = if cols.is_empty() { None } else { Some(cols) };
                    return ParseResult::Partial(select, exp);
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            }
        } else {
            None
        };

        // After GROUP BY, check for out-of-order clauses
        if let Some(err) = self.validate_clause_order(&[TokenKind::Where]) {
            return ParseResult::Error(err);
        }

        // Parse ORDER BY clause (optional)

        self.current_clause = SqlClause::OrderBy;
        let order_by = if self.match_keyword(&TokenKind::OrderBy) {
            match self.parse_order_by_clause() {
                ParseResult::Ok(orders) => Some(orders),
                ParseResult::Partial(orders, exp) => {
                    self.expected = exp.clone();
                    let mut select = SqlSelect::new();
                    select.columns = columns;
                    select.table = table;
                    select.where_clause = where_clause;
                    select.group_by = group_by;
                    select.order_by = if orders.is_empty() {
                        None
                    } else {
                        Some(orders)
                    };
                    return ParseResult::Partial(select, exp);
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            }
        } else {
            None
        };

        // After ORDER BY, check for out-of-order clauses
        if let Some(err) = self.validate_clause_order(&[TokenKind::Where, TokenKind::GroupBy]) {
            return ParseResult::Error(err);
        }

        // Parse LIMIT clause (optional)

        self.current_clause = SqlClause::Limit;
        let limit = if self.match_keyword(&TokenKind::Limit) {
            match self.parse_limit_clause() {
                ParseResult::Ok(n) => n,
                ParseResult::Partial(n, exp) => {
                    self.expected = exp.clone();
                    let mut select = SqlSelect::new();
                    select.columns = columns;
                    select.table = table;
                    select.where_clause = where_clause;
                    select.group_by = group_by;
                    select.order_by = order_by;
                    select.limit = n;
                    return ParseResult::Partial(select, exp);
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            }
        } else {
            None
        };

        // Parse OFFSET clause (optional)
        self.current_clause = SqlClause::Offset;
        let offset = if self.match_keyword(&TokenKind::Offset) {
            match self.parse_offset_clause() {
                ParseResult::Ok(n) => n,
                ParseResult::Partial(n, exp) => {
                    self.expected = exp.clone();
                    let mut select = SqlSelect::new();
                    select.columns = columns.clone();
                    select.table = table.clone();
                    select.where_clause = where_clause.clone();
                    select.group_by = group_by.clone();
                    select.order_by = order_by.clone();
                    select.limit = limit;
                    select.offset = n;
                    return ParseResult::Partial(select, exp);
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            }
        } else {
            None
        };

        // Complete parse
        ParseResult::Ok(SqlSelect {
            columns,
            table,
            where_clause,
            group_by,
            order_by,
            limit,
            offset,
        })
    }

    /// Parse SELECT column list
    fn parse_select_list(&mut self) -> ParseResult<Vec<SqlColumn>> {
        let mut columns = Vec::new();

        // Check for SELECT *
        if self.match_token(&TokenKind::Star) {
            columns.push(SqlColumn::Star);
            return ParseResult::Ok(columns);
        }

        // Check for EOF after SELECT
        if self.is_at_eof() {
            self.expected = vec![
                Expected::Star,
                Expected::ColumnName,
                Expected::AggregateFunction,
            ];
            return ParseResult::Partial(columns, self.expected.clone());
        }

        loop {
            match self.parse_column() {
                ParseResult::Ok(col) => columns.push(col),
                ParseResult::Partial(col, exp) => {
                    columns.push(col);
                    return ParseResult::Partial(columns, exp);
                }
                ParseResult::Error(err) => return ParseResult::Error(err),
            }

            // Check for comma
            if !self.match_token(&TokenKind::Comma) {
                break;
            }

            // EOF after comma - expect another column
            if self.is_at_eof() {
                self.expected = vec![Expected::ColumnName, Expected::AggregateFunction];
                return ParseResult::Partial(columns, self.expected.clone());
            }
        }

        ParseResult::Ok(columns)
    }

    /// Parse FROM clause
    fn parse_from_clause(&mut self) -> ParseResult<String> {
        if self.is_at_eof() {
            self.expected = vec![Expected::TableName];
            return ParseResult::Partial(String::new(), self.expected.clone());
        }

        if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let table = name.clone();
            self.advance();
            ParseResult::Ok(table)
        } else {
            ParseResult::Error(ParseError::new(
                "Expected table name".to_string(),
                self.current_position()..self.current_position(),
            ))
        }
    }

    /// Parse WHERE clause
    fn parse_where_clause(&mut self) -> ParseResult<super::sql_context::SqlExpr> {
        if self.is_at_eof() {
            self.expected = vec![Expected::Expression, Expected::ColumnName];
            return ParseResult::Partial(
                super::sql_context::SqlExpr::Literal(super::sql_context::SqlLiteral::Boolean(
                    true,
                )),
                self.expected.clone(),
            );
        }

        self.parse_expression(0)
    }

    /// Parse GROUP BY clause
    fn parse_group_by_clause(&mut self) -> ParseResult<Vec<String>> {
        let mut columns = Vec::new();

        if self.is_at_eof() {
            self.expected = vec![Expected::ColumnName];
            return ParseResult::Partial(columns, self.expected.clone());
        }

        loop {
            if let Some(TokenKind::Ident(name)) = self.peek_kind() {
                let name = name.clone();
                self.advance();

                // Parse field path (supports nested fields and array access)
                let path = match self.parse_field_path_continuation(
                    super::sql_context::FieldPath::simple(name),
                ) {
                    Ok(p) => p,
                    Err(err) => {
                        return ParseResult::Error(ParseError::new(
                            err.to_user_message(),
                            self.current_position()..self.current_position(),
                        ));
                    }
                };

                // For GROUP BY, we need to convert path to string for now
                // TODO: Update GROUP BY to use FieldPath directly
                if let Some(path_str) = path.to_mongodb_path() {
                    columns.push(path_str);
                } else {
                    return ParseResult::Error(ParseError::new(
                        "Array access in GROUP BY requires aggregation pipeline".to_string(),
                        self.current_position()..self.current_position(),
                    ));
                }
            } else if self.is_at_eof() {
                self.expected = vec![Expected::ColumnName];
                return ParseResult::Partial(columns, self.expected.clone());
            } else {
                break;
            }

            if !self.match_token(&TokenKind::Comma) {
                break;
            }

            if self.is_at_eof() {
                self.expected = vec![Expected::ColumnName];
                return ParseResult::Partial(columns, self.expected.clone());
            }
        }

        if columns.is_empty() {
            return ParseResult::Error(ParseError::new(
                "Expected column name in GROUP BY".to_string(),
                self.current_position()..self.current_position(),
            ));
        }

        ParseResult::Ok(columns)
    }

    /// Parse ORDER BY clause
    fn parse_order_by_clause(&mut self) -> ParseResult<Vec<SqlOrderBy>> {
        let mut orders = Vec::new();

        if self.is_at_eof() {
            self.expected = vec![Expected::ColumnName];
            return ParseResult::Partial(orders, self.expected.clone());
        }

        loop {
            if let Some(TokenKind::Ident(name)) = self.peek_kind() {
                let name = name.clone();
                self.advance();

                // Parse field path (supports nested fields and array access)
                let path = match self.parse_field_path_continuation(
                    super::sql_context::FieldPath::simple(name),
                ) {
                    Ok(p) => p,
                    Err(err) => {
                        return ParseResult::Error(ParseError::new(
                            err.to_user_message(),
                            self.current_position()..self.current_position(),
                        ));
                    }
                };

                // Check for ASC/DESC
                let asc = if self.match_keyword(&TokenKind::Desc) {
                    false
                } else {
                    self.match_keyword(&TokenKind::Asc);
                    true
                };

                orders.push(SqlOrderBy::new(path, asc));
            } else if self.is_at_eof() {
                self.expected = vec![Expected::ColumnName];
                return ParseResult::Partial(orders, self.expected.clone());
            } else {
                break;
            }

            if !self.match_token(&TokenKind::Comma) {
                break;
            }

            if self.is_at_eof() {
                self.expected = vec![Expected::ColumnName];
                return ParseResult::Partial(orders, self.expected.clone());
            }
        }

        if orders.is_empty() {
            return ParseResult::Error(ParseError::new(
                "Expected column name in ORDER BY".to_string(),
                self.current_position()..self.current_position(),
            ));
        }

        ParseResult::Ok(orders)
    }

    /// Parse LIMIT clause
    fn parse_limit_clause(&mut self) -> ParseResult<Option<usize>> {
        if self.is_at_eof() {
            self.expected = vec![Expected::Number];
            return ParseResult::Partial(None, self.expected.clone());
        }

        if let Some(TokenKind::Number(n)) = self.peek_kind() {
            let num = n.clone();
            self.advance();
            let value = num.parse::<usize>().unwrap_or(0);
            ParseResult::Ok(Some(value))
        } else {
            ParseResult::Error(ParseError::new(
                "Expected number in LIMIT clause".to_string(),
                self.current_position()..self.current_position(),
            ))
        }
    }

    /// Parse OFFSET clause
    fn parse_offset_clause(&mut self) -> ParseResult<Option<usize>> {
        if self.is_at_eof() {
            self.expected = vec![Expected::Number];
            return ParseResult::Partial(None, self.expected.clone());
        }

        if let Some(TokenKind::Number(n)) = self.peek_kind() {
            let num = n.clone();
            self.advance();
            let value = num.parse::<usize>().unwrap_or(0);
            ParseResult::Ok(Some(value))
        } else {
            ParseResult::Error(ParseError::new(
                "Expected number in OFFSET clause".to_string(),
                self.current_position()..self.current_position(),
            ))
        }
    }

    // Helper methods for token manipulation

    /// Check if at end of token stream
    fn is_at_eof(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.peek_kind(), Some(TokenKind::EOF))
    }

    /// Peek at current token
    fn peek(&self) -> Option<&Token> {
        if self.pos < self.tokens.len() {
            Some(&self.tokens[self.pos])
        } else {
            None
        }
    }

    /// Peek at current token kind
    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    /// Check if current token matches kind
    fn check_token(&self, kind: &TokenKind) -> bool {
        if let Some(current) = self.peek_kind() {
            std::mem::discriminant(current) == std::mem::discriminant(kind)
        } else {
            false
        }
    }

    /// Check if current token matches keyword
    fn check_keyword(&self, kind: &TokenKind) -> bool {
        self.check_token(kind)
    }

    /// Match and consume token if it matches
    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check_token(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Match and consume keyword
    fn match_keyword(&mut self, kind: &TokenKind) -> bool {
        self.match_token(kind)
    }

    /// Advance to next token
    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    /// Validate SQL clause order after parsing the current clause
    /// Returns an error if any of the specified clauses appear out of order
    fn validate_clause_order(&self, invalid_clauses: &[TokenKind]) -> Option<ParseError> {
        for clause_kind in invalid_clauses {
            if self.check_token(clause_kind) {
                let clause_name = Self::clause_name(clause_kind);
                let current_name = Self::current_clause_name(&self.current_clause);
                let position = self.current_position();

                let error_msg = format!(
                    "{} clause must appear before {}.",
                    clause_name, current_name
                );

                return Some(
                    ParseError::new(error_msg, position..position + 1).with_hint(format!(
                        "Move the {} clause before {}",
                        clause_name, current_name
                    )),
                );
            }
        }
        None
    }

    /// Get the name of a clause token kind
    fn clause_name(kind: &TokenKind) -> &'static str {
        match kind {
            TokenKind::Where => "WHERE",
            TokenKind::GroupBy => "GROUP BY",
            TokenKind::OrderBy => "ORDER BY",
            TokenKind::Limit => "LIMIT",
            TokenKind::Offset => "OFFSET",
            _ => "clause",
        }
    }

    /// Get the name of the current SQL clause
    fn current_clause_name(clause: &SqlClause) -> &'static str {
        match clause {
            SqlClause::Select => "SELECT",
            SqlClause::From => "FROM",
            SqlClause::Where => "WHERE",
            SqlClause::GroupBy => "GROUP BY",
            SqlClause::OrderBy => "ORDER BY",
            SqlClause::Limit => "LIMIT",
            SqlClause::Offset => "OFFSET",
        }
    }

    fn current_position(&self) -> usize {
        if let Some(token) = self.peek() {
            token.span.start
        } else {
            0
        }
    }
}
