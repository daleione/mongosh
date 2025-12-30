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
    Expected, ParseError, ParseResult, SqlClause, SqlColumn, SqlContext, SqlExpr, SqlLiteral,
    SqlLogicalOperator, SqlOperator, SqlOrderBy, SqlSelect,
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
    }

    /// Parse SQL and convert to Command
    pub fn parse_to_command(input: &str) -> Result<Command> {
        let tokens = SqlLexer::tokenize(input);
        let mut parser = Self::new(tokens);
        let result = parser.parse_select_statement();

        match result {
            ParseResult::Ok(select) => parser.ast_to_command(select),
            ParseResult::Partial(select, expected) => {
                // Try to convert partial parse if it has enough information
                if select.table.is_some() {
                    parser.ast_to_command(select)
                } else {
                    Err(crate::error::ParseError::InvalidCommand(format!(
                        "Incomplete SQL query. Expected: {}",
                        expected
                            .iter()
                            .map(|e| e.description())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .into())
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
                                    SqlColumn::Field { name, alias: None },
                                    self.expected.clone(),
                                );
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    return ParseResult::Ok(SqlColumn::Field { name, alias });
                }
                _ => {}
            }
        }

        if self.is_at_eof() {
            self.expected = vec![Expected::ColumnName, Expected::AggregateFunction];
            return ParseResult::Partial(
                SqlColumn::Field {
                    name: String::new(),
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
                    },
                    self.expected.clone(),
                );
            }
            return ParseResult::Error(ParseError::new(
                "Expected '(' after aggregate function".to_string(),
                self.current_position()..self.current_position(),
            ));
        }

        // Parse field or *
        let field = if self.match_token(&TokenKind::Star) {
            None
        } else if let Some(TokenKind::Ident(name)) = self.peek_kind() {
            let name = name.clone();
            self.advance();
            Some(name)
        } else if self.is_at_eof() {
            self.expected = vec![Expected::Star, Expected::ColumnName];
            return ParseResult::Partial(
                SqlColumn::Aggregate {
                    func,
                    field: None,
                    alias: None,
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
                        },
                        self.expected.clone(),
                    );
                }
                _ => None,
            }
        } else {
            None
        };

        ParseResult::Ok(SqlColumn::Aggregate { func, field, alias })
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
            SqlExpr::Column(name)
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
            return ParseResult::Ok(left);
        };

        // Parse right side (literal value)
        let right = match self.parse_literal() {
            ParseResult::Ok(lit) => SqlExpr::Literal(lit),
            ParseResult::Partial(lit, exp) => {
                return ParseResult::Partial(
                    SqlExpr::BinaryOp {
                        left: Box::new(left),
                        op,
                        right: Box::new(SqlExpr::Literal(lit)),
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
                columns.push(name);
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
                let column = name.clone();
                self.advance();

                // Check for ASC/DESC
                let asc = if self.match_keyword(&TokenKind::Desc) {
                    false
                } else {
                    self.match_keyword(&TokenKind::Asc);
                    true
                };

                orders.push(SqlOrderBy::new(column, asc));
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
    fn ast_to_command(&self, ast: SqlSelect) -> Result<Command> {
        let collection = ast.table.clone().ok_or_else(|| {
            crate::error::ParseError::InvalidCommand("Missing table name".to_string())
        })?;

        if ast.needs_aggregate() {
            self.to_aggregate(ast, collection)
        } else {
            self.to_find(ast, collection)
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
                sort_doc.insert(order.column, if order.asc { 1 } else { -1 });
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

        // Add $group stage for GROUP BY
        if let Some(ref group_by) = ast.group_by {
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
                if let SqlColumn::Aggregate { func, field, alias } = col {
                    let output_name =
                        SqlExprConverter::get_aggregate_output_name(func, field, alias);
                    project_doc.insert(output_name.clone(), format!("${}", output_name));
                }
            }

            pipeline.push(doc! { "$project": project_doc });
        }

        // Add $sort stage for ORDER BY
        if let Some(order_by) = ast.order_by {
            let mut sort_doc = Document::new();
            for order in order_by {
                sort_doc.insert(order.column, if order.asc { 1 } else { -1 });
            }
            pipeline.push(doc! { "$sort": sort_doc });
        }

        // Add $skip stage for OFFSET
        if let Some(offset) = ast.offset {
            pipeline.push(doc! { "$skip": offset as i64 });
        }

        // Add $limit stage
        if let Some(limit) = ast.limit {
            pipeline.push(doc! { "$limit": limit as i64 });
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

    /// Get current position
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
}
