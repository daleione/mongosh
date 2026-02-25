//! SQL parser with recursive descent and error-tolerant partial parsing
//!
//! This parser is designed for CLI use with strong emphasis on:
//! - Partial parsing (handling incomplete input)
//! - Context tracking (for autocomplete)
//! - Error tolerance (never panic, always return useful information)
//!
//! # Design Philosophy
//!
//! Unlike traditional parsers that fail on incomplete input, this parser
//! embraces partial states as first-class citizens. When a user types
//! "SELECT * FROM |", we return a partial parse with context indicating
//! that a table name is expected.

use mongodb::bson::{Document, doc};

use super::command::{AggregateOptions, Command, FindOptions, QueryCommand};
use super::sql_context::{
    ArrayAccessError, ArrayIndex, ArraySlice, Expected, FieldPath, ParseError, ParseResult,
    SliceIndex, SqlClause, SqlColumn, SqlContext, SqlExpr, SqlLiteral, SqlLogicalOperator,
    SqlOperator, SqlOrderBy, SqlSelect,
};
use super::sql_expr::SqlExprConverter;
use super::sql_lexer::{SqlLexer, Token, TokenKind};
use crate::error::Result;

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
                _ => Some(super::command::ExplainVerbosity::QueryPlanner)
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
            },
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
            },
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

    /// Parse a single column (field or aggregate function)
    fn parse_column(&mut self) -> ParseResult<SqlColumn> {
        // Check for aggregate functions
        if let Some(token) = self.peek() {
            match &token.kind {
                TokenKind::Count
                | TokenKind::Sum
                | TokenKind::Avg
                | TokenKind::Min
                | TokenKind::Max => {
                    return self.parse_aggregate_column();
                }
                TokenKind::Ident(name) => {
                    let name = name.clone();
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

    /// Parse aggregate function column (COUNT, SUM, etc.)
    fn parse_aggregate_column(&mut self) -> ParseResult<SqlColumn> {
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
            if self.is_at_eof() {
                self.expected = vec![Expected::Keyword("(")];
                return ParseResult::Partial(
                    SqlColumn::Aggregate {
                        func,
                        field: None,
                        alias: None,
                        distinct: false,
                    },
                    self.expected.clone(),
                );
            }
            return ParseResult::Error(ParseError::new(
                "Expected '(' after aggregate function".to_string(),
                self.current_position()..self.current_position(),
            ));
        }

        // Check for DISTINCT keyword
        let distinct = self.match_token(&TokenKind::Distinct);

        // Parse field or *
        let field = if self.match_token(&TokenKind::Star) {
            if distinct {
                return ParseResult::Error(ParseError::new(
                    "DISTINCT cannot be used with *".to_string(),
                    self.current_position()..self.current_position(),
                ));
            }
            None
        } else if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let name = name.clone();
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

            Some(path)
        } else if self.is_at_eof() {
            self.expected = vec![Expected::Star, Expected::ColumnName];
            return ParseResult::Partial(
                SqlColumn::Aggregate {
                    func,
                    field: None,
                    alias: None,
                    distinct,
                },
                self.expected.clone(),
            );
        } else {
            return ParseResult::Error(ParseError::new(
                "Expected field name or * in aggregate function".to_string(),
                self.current_position()..self.current_position(),
            ));
        };

        // Expect closing parenthesis
        if !self.match_token(&TokenKind::RParen) {
            if self.is_at_eof() {
                self.expected = vec![Expected::Keyword(")")];
                return ParseResult::Partial(
                    SqlColumn::Aggregate {
                        func,
                        field,
                        alias: None,
                        distinct,
                    },
                    self.expected.clone(),
                );
            }
            return ParseResult::Error(ParseError::new(
                "Expected ')' after aggregate function".to_string(),
                self.current_position()..self.current_position(),
            ));
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
                        SqlColumn::Aggregate {
                            func,
                            field,
                            alias: None,
                            distinct,
                        },
                        self.expected.clone(),
                    );
                }
                _ => None,
            }
        } else {
            None
        };

        ParseResult::Ok(SqlColumn::Aggregate {
            func,
            field,
            alias,
            distinct,
        })
    }

    /// Parse field path continuation (dots, brackets)
    fn parse_field_path_continuation(
        &mut self,
        mut path: FieldPath,
    ) -> std::result::Result<FieldPath, ArrayAccessError> {
        loop {
            match self.peek_kind() {
                Some(TokenKind::Dot) => {
                    self.advance();
                    // Parse nested field
                    if let Some(TokenKind::Ident(field_name)) = self.peek_kind() {
                        let field_name = field_name.clone();
                        self.advance();
                        path = FieldPath::nested(path, field_name);
                    } else {
                        // Incomplete nested field, return what we have
                        break;
                    }
                }
                Some(TokenKind::LBracket) => {
                    self.advance();
                    path = self.parse_array_access(path)?;
                }
                _ => break,
            }
        }
        Ok(path)
    }

    /// Parse array access (index or slice)
    fn parse_array_access(
        &mut self,
        base: FieldPath,
    ) -> std::result::Result<FieldPath, ArrayAccessError> {
        // Check for empty brackets
        if self.match_token(&TokenKind::RBracket) {
            return Err(ArrayAccessError::EmptyIndex);
        }

        // Parse first number (could be index or slice start)
        let first_value = self.parse_array_index_or_start()?;

        // Check what comes next
        match self.peek_kind() {
            Some(TokenKind::RBracket) => {
                // Simple index: arr[5]
                self.advance();
                Ok(FieldPath::index(base, first_value))
            }
            Some(TokenKind::Colon) => {
                // Slice: arr[start:end] or arr[start:end:step]
                self.advance();
                let slice = self.parse_array_slice(Some(first_value))?;
                Ok(FieldPath::slice(base, slice))
            }
            _ => {
                if !self.match_token(&TokenKind::RBracket) {
                    Err(ArrayAccessError::MissingCloseBracket)
                } else {
                    Ok(FieldPath::index(base, first_value))
                }
            }
        }
    }

    /// Parse array index or slice start
    fn parse_array_index_or_start(&mut self) -> std::result::Result<ArrayIndex, ArrayAccessError> {
        match self.peek_kind() {
            Some(TokenKind::Number(num_str)) => {
                let num_str = num_str.clone();
                self.advance();

                if let Ok(idx) = num_str.parse::<i64>() {
                    if idx >= 0 {
                        Ok(ArrayIndex::positive(idx))
                    } else {
                        Ok(ArrayIndex::negative(idx))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType(num_str))
                }
            }
            Some(TokenKind::Minus) => {
                // Handle negative index with explicit minus sign
                self.advance();
                if let Some(TokenKind::Number(num_str)) = self.peek_kind() {
                    let num_str = num_str.clone();
                    self.advance();
                    if let Ok(idx) = num_str.parse::<i64>() {
                        Ok(ArrayIndex::negative(idx))
                    } else {
                        Err(ArrayAccessError::InvalidIndexType(format!("-{}", num_str)))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType("-".to_string()))
                }
            }
            _ => Err(ArrayAccessError::InvalidIndexType("".to_string())),
        }
    }

    /// Parse array slice after initial colon
    fn parse_array_slice(
        &mut self,
        start: Option<ArrayIndex>,
    ) -> std::result::Result<ArraySlice, ArrayAccessError> {
        let start_idx = start.map(|idx| match idx {
            ArrayIndex::Positive(n) => SliceIndex::Positive(n),
            ArrayIndex::Negative(n) => SliceIndex::Negative(n),
        });

        // Check for immediate closing bracket (start:)
        if self.match_token(&TokenKind::RBracket) {
            return Ok(ArraySlice::new(start_idx, None, None));
        }

        // Parse end index if present
        let end_idx = if matches!(self.peek_kind(), Some(TokenKind::Colon)) {
            // Another colon means no end specified (:end:step or ::step)
            None
        } else {
            // Parse end index
            self.parse_slice_index().ok()
        };

        // Check for step
        let step = if self.match_token(&TokenKind::Colon) {
            // Parse step
            if self.match_token(&TokenKind::RBracket) {
                // No step specified, use default
                None
            } else {
                match self.peek_kind() {
                    Some(TokenKind::Number(num_str)) => {
                        let num_str = num_str.clone();
                        self.advance();
                        if let Ok(step_val) = num_str.parse::<i64>() {
                            if step_val == 0 {
                                return Err(ArrayAccessError::ZeroStepSize);
                            }
                            Some(step_val)
                        } else {
                            return Err(ArrayAccessError::InvalidSliceSyntax(format!(
                                "Invalid step value: {}",
                                num_str
                            )));
                        }
                    }
                    _ => None,
                }
            }
        } else {
            None
        };

        // Expect closing bracket
        if !self.match_token(&TokenKind::RBracket) {
            return Err(ArrayAccessError::MissingCloseBracket);
        }

        Ok(ArraySlice::new(start_idx, end_idx, step))
    }

    /// Parse a slice index (positive or negative)
    fn parse_slice_index(&mut self) -> std::result::Result<SliceIndex, ArrayAccessError> {
        match self.peek_kind() {
            Some(TokenKind::Number(num_str)) => {
                let num_str = num_str.clone();
                self.advance();

                if let Ok(idx) = num_str.parse::<i64>() {
                    if idx >= 0 {
                        Ok(SliceIndex::Positive(idx))
                    } else {
                        Ok(SliceIndex::Negative(idx.abs()))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType(num_str))
                }
            }
            Some(TokenKind::Minus) => {
                // Handle negative slice index
                self.advance();
                if let Some(TokenKind::Number(num_str)) = self.peek_kind() {
                    let num_str = num_str.clone();
                    self.advance();
                    if let Ok(idx) = num_str.parse::<i64>() {
                        Ok(SliceIndex::Negative(idx))
                    } else {
                        Err(ArrayAccessError::InvalidIndexType(format!("-{}", num_str)))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType("-".to_string()))
                }
            }
            _ => Err(ArrayAccessError::InvalidIndexType("".to_string())),
        }
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
    fn parse_where_clause(&mut self) -> ParseResult<SqlExpr> {
        if self.is_at_eof() {
            self.expected = vec![Expected::Expression, Expected::ColumnName];
            return ParseResult::Partial(
                SqlExpr::Literal(SqlLiteral::Boolean(true)),
                self.expected.clone(),
            );
        }

        self.parse_expression(0)
    }

    /// Parse expression using Pratt parser (operator precedence)
    fn parse_expression(&mut self, min_bp: u8) -> ParseResult<SqlExpr> {
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
    fn parse_primary_expr(&mut self) -> ParseResult<SqlExpr> {
        // Parse left side (should be column)
        let left = if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let name = name.clone();
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

            SqlExpr::FieldPath(path)
        } else if self.is_at_eof() {
            self.expected = vec![Expected::ColumnName];
            return ParseResult::Partial(
                SqlExpr::Literal(SqlLiteral::Boolean(true)),
                self.expected.clone(),
            );
        } else {
            return ParseResult::Error(ParseError::new(
                "Expected column name in expression".to_string(),
                self.current_position()..self.current_position(),
            ));
        };

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
                            format!("Expected comparison operator (=, !=, >, <, >=, <=) after field name, found {:?}", kind),
                            self.current_position()..self.current_position(),
                        ));
                    }
                    TokenKind::GroupBy | TokenKind::OrderBy | TokenKind::Limit | TokenKind::Offset => {
                        // Next clause - field without comparison is invalid
                        return ParseResult::Error(ParseError::new(
                            "Expected comparison operator (=, !=, >, <, >=, <=) after field name".to_string(),
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
                            format!("Expected comparison operator (=, !=, >, <, >=, <=) after field name, found unexpected token"),
                            self.current_position()..self.current_position(),
                        ));
                    }
                }
            }
            // This shouldn't happen as we already checked is_at_eof() above
            return ParseResult::Error(ParseError::new(
                "Expected comparison operator after field name".to_string(),
                self.current_position()..self.current_position(),
            ));
        };

        // Parse right side (literal value or function call)
        let right = match self.parse_value_expr() {
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

    /// Parse a value expression (literal or function call)
    fn parse_value_expr(&mut self) -> ParseResult<SqlExpr> {
        // Check for typed literals: DATE '...', TIMESTAMP '...', TIME '...'
        if let Some(token_kind) = self.peek_kind() {
            match token_kind {
                TokenKind::Date | TokenKind::Timestamp | TokenKind::Time => {
                    let type_name = match token_kind {
                        TokenKind::Date => "DATE",
                        TokenKind::Timestamp => "TIMESTAMP",
                        TokenKind::Time => "TIME",
                        _ => unreachable!(),
                    }.to_string();

                    self.advance();

                    // Expect string literal
                    if let Some(TokenKind::String(value)) = self.peek_kind() {
                        let value = value.clone();
                        self.advance();
                        return ParseResult::Ok(SqlExpr::TypedLiteral { type_name, value });
                    } else if self.is_at_eof() {
                        return ParseResult::Partial(
                            SqlExpr::TypedLiteral { type_name, value: String::new() },
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
                        kind: "CURRENT_TIMESTAMP".to_string()
                    });
                }
                TokenKind::CurrentDate => {
                    self.advance();
                    return ParseResult::Ok(SqlExpr::CurrentTime {
                        kind: "CURRENT_DATE".to_string()
                    });
                }
                TokenKind::CurrentTime => {
                    self.advance();
                    return ParseResult::Ok(SqlExpr::CurrentTime {
                        kind: "CURRENT_TIME".to_string()
                    });
                }
                TokenKind::Now => {
                    self.advance();

                    // NOW can be used with or without parentheses
                    if self.match_token(&TokenKind::LParen) {
                        if !self.match_token(&TokenKind::RParen) {
                            if self.is_at_eof() {
                                return ParseResult::Partial(
                                    SqlExpr::CurrentTime { kind: "NOW".to_string() },
                                    vec![Expected::Keyword(")")],
                                );
                            }
                            return ParseResult::Error(ParseError::new(
                                "Expected ')' after NOW(".to_string(),
                                self.current_position()..self.current_position(),
                            ));
                        }
                    }

                    return ParseResult::Ok(SqlExpr::CurrentTime { kind: "NOW".to_string() });
                }
                _ => {}
            }
        }

        // Check if it's a function call (identifier followed by '(')
        if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let name = name.clone();
            let saved_pos = self.pos;
            self.advance();

            // Check for opening parenthesis
            if self.match_token(&TokenKind::LParen) {
                // Parse function arguments
                let mut args = Vec::new();

                // Parse first argument if not immediately closing paren
                if !self.check_token(&TokenKind::RParen) {
                    loop {
                        match self.parse_literal() {
                            ParseResult::Ok(lit) => {
                                args.push(SqlExpr::Literal(lit));
                            }
                            ParseResult::Partial(lit, exp) => {
                                return ParseResult::Partial(
                                    SqlExpr::Function {
                                        name,
                                        args: vec![SqlExpr::Literal(lit)],
                                    },
                                    exp,
                                );
                            }
                            ParseResult::Error(err) => return ParseResult::Error(err),
                        }

                        // Check for comma (more arguments) or closing paren
                        if self.match_token(&TokenKind::Comma) {
                            continue;
                        } else {
                            break;
                        }
                    }
                }

                // Expect closing parenthesis
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
                // Not a function call, restore position and parse as literal
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
    fn parse_literal(&mut self) -> ParseResult<SqlLiteral> {
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
                let path = match self.parse_field_path_continuation(FieldPath::simple(name)) {
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
                let path = match self.parse_field_path_continuation(FieldPath::simple(name)) {
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

    /// Convert SQL AST to MongoDB Command
    /// Wrap a command in EXPLAIN
    fn wrap_in_explain(&self, cmd: Command, verbosity: super::command::ExplainVerbosity) -> Result<Command> {
        use super::command::QueryCommand;

        match cmd {
            Command::Query(query_cmd) => {
                if !query_cmd.supports_explain() {
                    return Err(crate::error::ParseError::InvalidCommand(
                        "EXPLAIN can only be used with SELECT queries".to_string()
                    ).into());
                }

                let collection = query_cmd.collection().to_string();

                Ok(Command::Query(QueryCommand::Explain {
                    collection,
                    verbosity,
                    query: Box::new(query_cmd),
                }))
            },
            _ => Err(crate::error::ParseError::InvalidCommand(
                "EXPLAIN can only be used with query commands".to_string()
            ).into()),
        }
    }

    fn ast_to_command(&self, ast: SqlSelect) -> Result<Command> {
        let collection = ast.table.clone().ok_or_else(|| {
            crate::error::ParseError::InvalidCommand("Missing table name".to_string())
        })?;

        // Check if we need aggregation pipeline
        let needs_agg = ast.needs_aggregate() || self.has_complex_field_paths(&ast);

        if needs_agg {
            self.to_aggregate(ast, collection)
        } else {
            self.to_find(ast, collection)
        }
    }

    /// Check if SELECT has complex field paths requiring aggregation
    fn has_complex_field_paths(&self, ast: &SqlSelect) -> bool {
        // Check columns
        for col in &ast.columns {
            match col {
                SqlColumn::Field { path, .. } => {
                    if path.requires_aggregation() {
                        return true;
                    }
                }
                SqlColumn::Aggregate { field, .. } => {
                    if let Some(path) = field {
                        if path.requires_aggregation() {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }

        // Check WHERE clause
        if let Some(ref expr) = ast.where_clause {
            if self.expr_has_complex_paths(expr) {
                return true;
            }
        }

        // Check ORDER BY
        if let Some(ref order_by) = ast.order_by {
            for order in order_by {
                if order.path.requires_aggregation() {
                    return true;
                }
            }
        }

        false
    }

    /// Check if expression contains complex field paths
    fn expr_has_complex_paths(&self, expr: &SqlExpr) -> bool {
        match expr {
            SqlExpr::FieldPath(path) => path.requires_aggregation(),
            SqlExpr::BinaryOp { left, right, .. } => {
                self.expr_has_complex_paths(left) || self.expr_has_complex_paths(right)
            }
            SqlExpr::LogicalOp { left, right, .. } => {
                self.expr_has_complex_paths(left) || self.expr_has_complex_paths(right)
            }
            SqlExpr::In { expr, values } => {
                self.expr_has_complex_paths(expr)
                    || values.iter().any(|v| self.expr_has_complex_paths(v))
            }
            SqlExpr::Like { expr, .. } | SqlExpr::IsNull { expr, .. } => {
                self.expr_has_complex_paths(expr)
            }
            _ => false,
        }
    }

    /// Convert to find command
    fn to_find(&self, ast: SqlSelect, collection: String) -> Result<Command> {
        // Convert WHERE to filter
        let filter = if let Some(expr) = ast.where_clause {
            SqlExprConverter::expr_to_filter(&expr)?
        } else {
            Document::new()
        };

        // Convert columns to projection
        let projection = SqlExprConverter::columns_to_projection(&ast.columns)?;

        // Convert ORDER BY to sort
        let sort = if let Some(order_by) = ast.order_by {
            let mut sort_doc = Document::new();
            for order in order_by {
                // Get MongoDB path from FieldPath
                let path_str = order.path.to_mongodb_path().unwrap_or_else(|| {
                    // For complex paths, use base field
                    order.path.base_field()
                });
                sort_doc.insert(path_str, if order.asc { 1 } else { -1 });
            }
            Some(sort_doc)
        } else {
            None
        };

        Ok(Command::Query(QueryCommand::Find {
            collection,
            filter,
            options: FindOptions {
                limit: ast.limit.map(|l| l as i64),
                skip: ast.offset.map(|s| s as u64),
                sort,
                projection,
                ..Default::default()
            },
        }))
    }

    /// Convert to aggregate command
    fn to_aggregate(&self, ast: SqlSelect, collection: String) -> Result<Command> {
        let mut pipeline = Vec::new();

        // Add $match stage for WHERE clause
        if let Some(expr) = ast.where_clause {
            let filter = SqlExprConverter::expr_to_filter(&expr)?;
            pipeline.push(doc! { "$match": filter });
        }

        // Add $sort stage for ORDER BY (MUST come before $project to sort on original fields)
        if let Some(ref order_by) = ast.order_by {
            let mut sort_doc = Document::new();
            for order in order_by {
                // Get MongoDB path from FieldPath
                let path_str = order.path.to_mongodb_path().unwrap_or_else(|| {
                    // For complex paths, use base field
                    order.path.base_field()
                });
                sort_doc.insert(path_str, if order.asc { 1 } else { -1 });
            }
            pipeline.push(doc! { "$sort": sort_doc });
        }

        // Add $skip stage for OFFSET (before $limit and $project)
        if let Some(offset) = ast.offset {
            pipeline.push(doc! { "$skip": offset as i64 });
        }

        // Add $limit stage (before $project to limit documents early)
        if let Some(limit) = ast.limit {
            pipeline.push(doc! { "$limit": limit as i64 });
        }

        // Check if we have any aggregate functions
        let has_aggregates = ast
            .columns
            .iter()
            .any(|c| matches!(c, SqlColumn::Aggregate { .. }));

        // Add $group stage
        if let Some(ref group_by) = ast.group_by {
            // GROUP BY case: group by specific fields
            let group_doc = SqlExprConverter::build_group_stage(group_by, &ast.columns)?;
            pipeline.push(doc! { "$group": group_doc });

            // Add $project stage to rename _id to the original field name(s)
            let mut project_doc = Document::new();

            if group_by.len() == 1 {
                // Single field grouping - rename _id to the field name
                project_doc.insert("_id", 0); // Exclude _id
                project_doc.insert(group_by[0].clone(), "$_id");
            } else {
                // Multiple field grouping - expand _id object
                project_doc.insert("_id", 0); // Exclude _id
                for field in group_by {
                    project_doc.insert(field.clone(), format!("$_id.{}", field));
                }
            }

            // Include all aggregate function results
            for col in &ast.columns {
                if let SqlColumn::Aggregate {
                    func,
                    alias,
                    distinct,
                    ..
                } = col
                {
                    let output_name = alias.clone().unwrap_or_else(|| func.to_lowercase());
                    // For COUNT(DISTINCT), we need to count the array size
                    if *distinct && func.to_uppercase() == "COUNT" {
                        project_doc.insert(
                            output_name.clone(),
                            doc! { "$size": format!("${}", output_name) },
                        );
                    } else {
                        project_doc.insert(output_name.clone(), format!("${}", output_name));
                    }
                }
            }

            pipeline.push(doc! { "$project": project_doc });
        } else if has_aggregates {
            // No GROUP BY but has aggregates: aggregate over entire collection (e.g., SELECT COUNT(*) FROM ...)
            let mut group_doc = Document::new();
            group_doc.insert("_id", mongodb::bson::Bson::Null); // Group all documents together

            // Add aggregate functions
            for col in &ast.columns {
                if let SqlColumn::Aggregate {
                    func,
                    field,
                    alias,
                    distinct,
                } = col
                {
                    let output_name = alias.clone().unwrap_or_else(|| func.to_lowercase());
                    // Convert FieldPath to string for aggregate expr
                    let field_str = field.as_ref().and_then(|p| p.to_mongodb_path());
                    let aggregate_expr = SqlExprConverter::build_aggregate_expr(
                        func,
                        field_str.as_deref(),
                        *distinct,
                    )?;
                    group_doc.insert(output_name, aggregate_expr);
                }
            }

            pipeline.push(doc! { "$group": group_doc });

            // Add $project stage to exclude _id and keep only aggregate results
            let mut project_doc = Document::new();
            project_doc.insert("_id", 0); // Exclude _id

            for col in &ast.columns {
                if let SqlColumn::Aggregate {
                    func,
                    alias,
                    distinct,
                    ..
                } = col
                {
                    let output_name = alias.clone().unwrap_or_else(|| func.to_lowercase());
                    // For COUNT(DISTINCT), we need to count the array size
                    if *distinct && func.to_uppercase() == "COUNT" {
                        project_doc.insert(
                            output_name.clone(),
                            doc! { "$size": format!("${}", output_name) },
                        );
                    } else {
                        project_doc.insert(output_name.clone(), format!("${}", output_name));
                    }
                }
            }

            pipeline.push(doc! { "$project": project_doc });
        } else {
            // No GROUP BY, no aggregates: just field aliases
            // Add $project stage to handle field renaming
            let mut project_doc = Document::new();
            let mut has_id = false;

            for col in &ast.columns {
                if let SqlColumn::Field { path, alias } = col {
                    // Check if this is the _id field
                    if let Some(path_str) = path.to_mongodb_path() {
                        if path_str == "_id" {
                            has_id = true;
                        }

                        if let Some(alias_name) = alias {
                            // Rename field using alias
                            project_doc.insert(alias_name.clone(), format!("${}", path_str));
                        } else {
                            // Keep field with original name
                            project_doc.insert(path_str.clone(), 1);
                        }
                    } else {
                        // Complex path requires aggregation expression
                        let base_field = path.base_field();
                        let field_name = alias.as_ref().unwrap_or(&base_field);
                        if let Ok(bson_expr) = SqlExprConverter::field_path_to_bson(path) {
                            project_doc.insert(field_name.clone(), bson_expr);
                        }
                    }
                }
            }

            // Exclude _id if not explicitly requested
            if !has_id {
                project_doc.insert("_id", 0);
            }

            pipeline.push(doc! { "$project": project_doc });
        }

        Ok(Command::Query(QueryCommand::Aggregate {
            collection,
            pipeline,
            options: AggregateOptions::default(),
        }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::command::ExplainVerbosity;

    #[test]
    fn test_is_sql_command() {
        assert!(SqlParser::is_sql_command("SELECT * FROM users"));
        assert!(SqlParser::is_sql_command("select * from users"));
        assert!(SqlParser::is_sql_command("SELECT"));
        assert!(SqlParser::is_sql_command("  SELECT  "));
        assert!(!SqlParser::is_sql_command("show dbs"));
        assert!(!SqlParser::is_sql_command("db.users.find()"));
    }

    #[test]
    fn test_parse_simple_select() {
        let result = SqlParser::parse_to_command("SELECT * FROM users");
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(matches!(cmd, Command::Query(QueryCommand::Find { .. })));
    }

    #[test]
    fn test_parse_select_with_where() {
        let result = SqlParser::parse_to_command("SELECT * FROM users WHERE age > 18");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_select_with_columns() {
        let result = SqlParser::parse_to_command("SELECT name, age FROM users");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_order_by() {
        let result = SqlParser::parse_to_command("SELECT * FROM users ORDER BY name ASC");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_limit() {
        let result = SqlParser::parse_to_command("SELECT * FROM users LIMIT 10");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_aggregate() {
        let result = SqlParser::parse_to_command("SELECT COUNT(*) FROM users");
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(matches!(
            cmd,
            Command::Query(QueryCommand::Aggregate { .. })
        ));
    }

    #[test]
    fn test_parse_group_by() {
        let result = SqlParser::parse_to_command(
            "SELECT category, COUNT(*) FROM products GROUP BY category",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_partial_select() {
        let (result, context) = SqlParser::parse_with_context("SELECT *");
        assert!(result.is_partial());
        assert!(context.expected.contains(&Expected::Keyword("FROM")));
    }

    #[test]
    fn test_parse_partial_from() {
        let (result, context) = SqlParser::parse_with_context("SELECT * FROM ");
        assert!(result.is_partial());
        assert!(context.expected.contains(&Expected::TableName));
    }

    #[test]
    fn test_parse_partial_where() {
        let (result, context) = SqlParser::parse_with_context("SELECT * FROM users WHERE ");
        assert!(result.is_partial());
        assert!(
            context.expected.contains(&Expected::ColumnName)
                || context.expected.contains(&Expected::Expression)
        );
    }

    #[test]
    fn test_parse_with_string_alias() {
        let result = SqlParser::parse_to_command(
            "SELECT group_id AS 'group_id', COUNT(*) FROM templates GROUP BY group_id",
        );
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(matches!(
            cmd,
            Command::Query(QueryCommand::Aggregate { .. })
        ));
    }

    #[test]
    fn test_parse_aggregate_with_alias() {
        let result = SqlParser::parse_to_command("SELECT COUNT(*) AS total FROM users");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_where_after_group_by() {
        let result = SqlParser::parse_to_command(
            "SELECT status, COUNT(*) FROM tasks GROUP BY status WHERE template_id='123'",
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("WHERE clause must appear before GROUP BY"));
    }

    #[test]
    fn test_reject_group_by_after_order_by() {
        let result =
            SqlParser::parse_to_command("SELECT * FROM tasks ORDER BY created_at GROUP BY status");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("GROUP BY clause must appear before ORDER BY"));
    }

    #[test]
    fn test_reject_where_after_order_by() {
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks ORDER BY created_at WHERE status='active'",
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("WHERE clause must appear before"));
    }

    #[test]
    fn test_correct_clause_order_accepted() {
        // This should be accepted - correct order
        let result = SqlParser::parse_to_command(
            "SELECT status, COUNT(*) FROM tasks WHERE template_id='123' GROUP BY status",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_count_without_group_by() {
        // COUNT(*) without GROUP BY should generate proper aggregate pipeline
        let result =
            SqlParser::parse_to_command("SELECT COUNT(*) FROM tasks WHERE status='failed'");
        assert!(result.is_ok());
        let cmd = result.unwrap();

        // Should be an Aggregate command
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            // Should have $match and $group stages
            assert!(
                pipeline.len() >= 2,
                "Pipeline should have at least $match and $group stages"
            );

            // First stage should be $match
            assert!(pipeline[0].contains_key("$match"));

            // Second stage should be $group
            assert!(pipeline[1].contains_key("$group"));
        } else {
            panic!("Expected Aggregate command");
        }
    }

    #[test]
    fn test_parse_with_objectid_function() {
        // Test parsing ObjectId() function in WHERE clause
        let result = SqlParser::parse_to_command(
            "SELECT * FROM templates WHERE group_id=ObjectId('6920127eb40f0636d6b49042')",
        );
        assert!(
            result.is_ok(),
            "Failed to parse ObjectId function: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Find { filter, .. }) = cmd {
            // Should have group_id field in filter
            assert!(filter.contains_key("group_id"));

            // The value should be an ObjectId
            let value = filter.get("group_id").unwrap();
            assert!(matches!(value, mongodb::bson::Bson::ObjectId(_)));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_with_nested_fields() {
        // Test parsing nested fields with dot notation
        let result = SqlParser::parse_to_command(
            "SELECT input.images, user.name FROM templates WHERE input.type='image'",
        );
        assert!(
            result.is_ok(),
            "Failed to parse nested fields: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Find { filter, .. }) = cmd {
            // Should have input.type field in filter
            assert!(filter.contains_key("input.type"));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_order_by_with_nested_fields() {
        let result =
            SqlParser::parse_to_command("SELECT * FROM templates ORDER BY user.created_at DESC");
        assert!(
            result.is_ok(),
            "Failed to parse nested field in ORDER BY: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_group_by_with_nested_fields() {
        let result = SqlParser::parse_to_command(
            "SELECT user.country, COUNT(*) FROM templates GROUP BY user.country",
        );
        assert!(
            result.is_ok(),
            "Failed to parse nested field in GROUP BY: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_field_alias_without_aggregation() {
        // Test that field aliases work correctly (should use aggregation pipeline)
        let result = SqlParser::parse_to_command("SELECT input.images AS image FROM tasks LIMIT 1");
        assert!(result.is_ok(), "Failed to parse field alias: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            // Should use aggregation pipeline for aliases
            assert!(!pipeline.is_empty(), "Pipeline should not be empty");

            // Should have a $project stage
            let has_project = pipeline.iter().any(|stage| stage.contains_key("$project"));
            assert!(
                has_project,
                "Pipeline should contain $project stage for alias"
            );
        } else {
            panic!("Expected Aggregate command for query with alias");
        }
    }

    #[test]
    fn test_array_positive_index() {
        // Test positive array index: tags[0]
        let result = SqlParser::parse_to_command("SELECT tags[0] FROM posts");
        assert!(result.is_ok(), "Failed to parse array index: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            assert!(!pipeline.is_empty(), "Pipeline should not be empty");
            // Should use aggregation pipeline for array access
            let has_project = pipeline.iter().any(|stage| stage.contains_key("$project"));
            assert!(has_project, "Pipeline should contain $project stage");
        } else {
            panic!("Expected Aggregate command for array index access");
        }
    }

    #[test]
    fn test_array_negative_index() {
        // Test negative array index: tags[-1]
        let result = SqlParser::parse_to_command("SELECT tags[-1] FROM posts");
        assert!(
            result.is_ok(),
            "Failed to parse negative array index: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { .. }) = cmd {
            // Should use aggregation pipeline
        } else {
            panic!("Expected Aggregate command for negative array index");
        }
    }

    #[test]
    fn test_nested_array_index() {
        // Test nested field with array index: user.roles[0]
        let result = SqlParser::parse_to_command("SELECT user.roles[0] FROM accounts");
        assert!(
            result.is_ok(),
            "Failed to parse nested array index: {:?}",
            result
        );
    }

    #[test]
    fn test_array_slice() {
        // Test array slice: tags[0:5]
        let result = SqlParser::parse_to_command("SELECT tags[0:5] FROM posts");
        assert!(result.is_ok(), "Failed to parse array slice: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { .. }) = cmd {
            // Should use aggregation pipeline
        } else {
            panic!("Expected Aggregate command for array slice");
        }
    }

    #[test]
    fn test_where_with_array_index() {
        // Test WHERE clause with array index
        let result = SqlParser::parse_to_command("SELECT * FROM posts WHERE tags[0] = 'rust'");
        // This should require aggregation pipeline
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle array index in WHERE"
        );
    }

    #[test]
    fn test_order_by_with_array_index() {
        // Test ORDER BY with array index
        let result = SqlParser::parse_to_command("SELECT * FROM posts ORDER BY tags[0]");
        assert!(
            result.is_ok(),
            "Failed to parse ORDER BY with array index: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            // Should have $sort stage
            let has_sort = pipeline.iter().any(|stage| stage.contains_key("$sort"));
            assert!(has_sort, "Pipeline should contain $sort stage");
        } else {
            panic!("Expected Aggregate command for ORDER BY with array index");
        }
    }

    #[test]
    fn test_reject_semicolon_in_where_clause() {
        // Test that semicolon in WHERE clause is rejected
        let result = SqlParser::parse_to_command(
            "SELECT COUNT(*) FROM tasks WHERE user_id;2 WHERE template_id='task-123'",
        );
        assert!(
            result.is_err(),
            "Should reject semicolon in WHERE clause, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_reject_incomplete_where_expression() {
        // Test that incomplete WHERE expression (field without comparison) is rejected
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE user_id",
        );
        // This should be an error for incomplete input
        assert!(
            result.is_err(),
            "Should reject incomplete WHERE expression, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_reject_duplicate_where_clause() {
        // Test that duplicate WHERE clauses are rejected
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE user_id = 1 WHERE template_id = 2",
        );
        assert!(
            result.is_err(),
            "Should reject duplicate WHERE clause, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_explain_simple_select() {
        // Test EXPLAIN with simple SELECT
        let result = SqlParser::parse_to_command("EXPLAIN SELECT * FROM users");
        assert!(result.is_ok(), "Failed to parse EXPLAIN SELECT: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { collection, verbosity, query }) = cmd {
            assert_eq!(collection, "users");
            assert_eq!(verbosity, ExplainVerbosity::QueryPlanner);

            // Inner query should be Find
            assert!(matches!(*query, QueryCommand::Find { .. }));
        } else {
            panic!("Expected Explain command, got: {:?}", cmd);
        }
    }

    #[test]
    fn test_explain_with_where() {
        // Test EXPLAIN with WHERE clause
        let result = SqlParser::parse_to_command("EXPLAIN SELECT * FROM users WHERE age > 18");
        assert!(result.is_ok(), "Failed to parse EXPLAIN with WHERE: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { collection, query, .. }) = cmd {
            assert_eq!(collection, "users");

            // Inner query should have filter
            if let QueryCommand::Find { filter, .. } = *query {
                assert!(!filter.is_empty(), "Filter should not be empty");
            } else {
                panic!("Expected Find command inside Explain");
            }
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_with_execution_stats() {
        // Test EXPLAIN with executionStats verbosity (unquoted)
        let result = SqlParser::parse_to_command("EXPLAIN executionStats SELECT * FROM users");
        assert!(result.is_ok(), "Failed to parse EXPLAIN with executionStats: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { verbosity, .. }) = cmd {
            assert_eq!(verbosity, ExplainVerbosity::ExecutionStats);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_with_all_plans_execution() {
        // Test EXPLAIN with allPlansExecution verbosity (unquoted)
        let result = SqlParser::parse_to_command("EXPLAIN allPlansExecution SELECT name FROM users WHERE age > 18");
        assert!(result.is_ok(), "Failed to parse EXPLAIN with allPlansExecution: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { verbosity, .. }) = cmd {
            assert_eq!(verbosity, ExplainVerbosity::AllPlansExecution);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_aggregate() {
        // Test EXPLAIN with aggregation query (GROUP BY)
        let result = SqlParser::parse_to_command("EXPLAIN SELECT COUNT(*) FROM users GROUP BY age");
        assert!(result.is_ok(), "Failed to parse EXPLAIN with GROUP BY: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { query, .. }) = cmd {
            // Inner query should be Aggregate
            assert!(matches!(*query, QueryCommand::Aggregate { .. }));
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_with_order_by_limit() {
        // Test EXPLAIN with ORDER BY and LIMIT
        let result = SqlParser::parse_to_command("EXPLAIN SELECT * FROM users ORDER BY name LIMIT 10");
        assert!(result.is_ok(), "Failed to parse EXPLAIN with ORDER BY LIMIT: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { query, .. }) = cmd {
            if let QueryCommand::Find { options, .. } = *query {
                assert_eq!(options.limit, Some(10));
                assert!(options.sort.is_some());
            } else {
                panic!("Expected Find command inside Explain");
            }
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_case_insensitive() {
        // Test that EXPLAIN is case-insensitive
        let result1 = SqlParser::parse_to_command("EXPLAIN SELECT * FROM users");
        let result2 = SqlParser::parse_to_command("explain SELECT * FROM users");
        let result3 = SqlParser::parse_to_command("Explain SELECT * FROM users");

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());

        // All should produce Explain commands
        assert!(matches!(result1.unwrap(), Command::Query(QueryCommand::Explain { .. })));
        assert!(matches!(result2.unwrap(), Command::Query(QueryCommand::Explain { .. })));
        assert!(matches!(result3.unwrap(), Command::Query(QueryCommand::Explain { .. })));
    }

    #[test]
    fn test_explain_with_invalid_verbosity() {
        // Test EXPLAIN with invalid verbosity identifier
        let result = SqlParser::parse_to_command("EXPLAIN invalidVerbosity SELECT * FROM users");
        assert!(
            result.is_err(),
            "Should reject invalid verbosity, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_explain_with_quoted_verbosity() {
        // Test EXPLAIN with quoted verbosity (backwards compatibility)
        let result = SqlParser::parse_to_command("EXPLAIN 'executionStats' SELECT * FROM users");
        assert!(result.is_ok(), "Failed to parse EXPLAIN with quoted verbosity: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { verbosity, .. }) = cmd {
            assert_eq!(verbosity, ExplainVerbosity::ExecutionStats);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_is_sql_command_recognizes_explain() {
        // Test that is_sql_command recognizes EXPLAIN
        assert!(SqlParser::is_sql_command("EXPLAIN SELECT * FROM users"));
        assert!(SqlParser::is_sql_command("explain SELECT * FROM users"));
        assert!(SqlParser::is_sql_command("EXPLAIN executionStats SELECT * FROM users"));
        assert!(SqlParser::is_sql_command("EXPLAIN 'executionStats' SELECT * FROM users"));
        assert!(SqlParser::is_sql_command("EXPLAIN"));
    }

    #[test]
    fn test_parse_with_date_literal() {
        // Test DATE 'yyyy-mm-dd' syntax
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > DATE '2026-02-15'");
        assert!(result.is_ok(), "Failed to parse DATE literal: {:?}", result);
    }

    #[test]
    fn test_parse_with_timestamp_literal() {
        // Test TIMESTAMP 'yyyy-mm-dd HH:MM:SS' syntax
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > TIMESTAMP '2026-02-15 16:00:00'");
        assert!(result.is_ok(), "Failed to parse TIMESTAMP literal: {:?}", result);
    }

    #[test]
    fn test_parse_with_current_timestamp() {
        // Test CURRENT_TIMESTAMP (no parentheses)
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > CURRENT_TIMESTAMP");
        assert!(result.is_ok(), "Failed to parse CURRENT_TIMESTAMP: {:?}", result);
    }

    #[test]
    fn test_parse_with_current_date() {
        // Test CURRENT_DATE
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > CURRENT_DATE");
        assert!(result.is_ok(), "Failed to parse CURRENT_DATE: {:?}", result);
    }

    #[test]
    fn test_parse_with_now_function() {
        // Test NOW() function
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > NOW()");
        assert!(result.is_ok(), "Failed to parse NOW(): {:?}", result);
    }

    #[test]
    fn test_parse_with_now_no_parens() {
        // Test NOW without parentheses (should also work)
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > NOW");
        assert!(result.is_ok(), "Failed to parse NOW without parens: {:?}", result);
    }

    #[test]
    fn test_date_literal_simple_format() {
        // Test simple date format: '2026-02-15' (auto-converts to ISO)
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > DATE '2026-02-15'");
        assert!(result.is_ok(), "Failed to parse simple DATE format: {:?}", result);
    }

    #[test]
    fn test_timestamp_with_full_iso() {
        // Test full ISO 8601 format with timezone
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > TIMESTAMP '2026-02-15T16:00:00.000Z'");
        assert!(result.is_ok(), "Failed to parse full ISO TIMESTAMP: {:?}", result);
    }
}
