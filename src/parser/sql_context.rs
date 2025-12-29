//! SQL context and AST definitions
//!
//! This module defines the SQL AST (Abstract Syntax Tree) and context
//! models used for parsing, autocomplete, and error reporting.

use std::ops::Range;

/// SQL parsing context - tracks where we are in the parse tree
#[derive(Debug, Clone, PartialEq)]
pub struct SqlContext {
    /// Current SQL clause being parsed
    pub clause: SqlClause,

    /// Current position in the input
    pub position: usize,

    /// What tokens/constructs are expected next
    pub expected: Vec<Expected>,

    /// Partial input being parsed
    pub partial_input: String,
}

impl SqlContext {
    /// Create a new SQL context
    pub fn new(clause: SqlClause, position: usize, expected: Vec<Expected>) -> Self {
        Self {
            clause,
            position,
            expected,
            partial_input: String::new(),
        }
    }
}

/// SQL clause types
#[derive(Debug, Clone, PartialEq)]
pub enum SqlClause {
    /// In SELECT clause
    Select,

    /// In FROM clause
    From,

    /// In WHERE clause
    Where,

    /// In GROUP BY clause
    GroupBy,

    /// In ORDER BY clause
    OrderBy,

    /// In LIMIT clause
    Limit,

    /// In OFFSET clause
    Offset,
}

/// Expected token or construct (for autocomplete and error messages)
#[derive(Debug, Clone, PartialEq)]
pub enum Expected {
    /// Specific keyword
    Keyword(&'static str),

    /// Table/collection name
    TableName,

    /// Column/field name
    ColumnName,

    /// Expression
    Expression,

    /// Operator
    Operator,

    /// Aggregate function (COUNT, SUM, etc.)
    AggregateFunction,

    /// Order direction (ASC/DESC)
    OrderDirection,

    /// Number literal
    Number,

    /// String literal
    String,

    /// Star (*)
    Star,

    /// End of statement
    EndOfStatement,
}

impl Expected {
    /// Convert to human-readable description
    pub fn description(&self) -> &str {
        match self {
            Expected::Keyword(kw) => kw,
            Expected::TableName => "table name",
            Expected::ColumnName => "column name",
            Expected::Expression => "expression",
            Expected::Operator => "operator",
            Expected::AggregateFunction => "aggregate function",
            Expected::OrderDirection => "ASC or DESC",
            Expected::Number => "number",
            Expected::String => "string",
            Expected::Star => "*",
            Expected::EndOfStatement => "end of statement",
        }
    }
}

/// Parse result - can be complete, partial, or error
#[derive(Debug, Clone, PartialEq)]
pub enum ParseResult<T> {
    /// Successful complete parse
    Ok(T),

    /// Partial parse with expectations for completion
    Partial(T, Vec<Expected>),

    /// Parse error
    Error(ParseError),
}

impl<T> ParseResult<T> {
    /// Check if result is ok
    pub fn is_ok(&self) -> bool {
        matches!(self, ParseResult::Ok(_))
    }

    /// Check if result is partial
    pub fn is_partial(&self) -> bool {
        matches!(self, ParseResult::Partial(_, _))
    }

    /// Check if result is error
    pub fn is_error(&self) -> bool {
        matches!(self, ParseResult::Error(_))
    }

    /// Get the value if ok or partial
    pub fn value(self) -> Option<T> {
        match self {
            ParseResult::Ok(v) | ParseResult::Partial(v, _) => Some(v),
            ParseResult::Error(_) => None,
        }
    }
}

/// Parse error with context
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Error message
    pub message: String,

    /// Position in input where error occurred
    pub span: Range<usize>,

    /// What was expected at this position
    pub expected: Vec<Expected>,

    /// Optional hint for fixing the error
    pub hint: Option<String>,
}

impl ParseError {
    /// Create a new parse error
    pub fn new(message: String, span: Range<usize>) -> Self {
        Self {
            message,
            span,
            expected: Vec::new(),
            hint: None,
        }
    }

    /// Create an error with expected tokens
    pub fn with_expected(message: String, span: Range<usize>, expected: Vec<Expected>) -> Self {
        Self {
            message,
            span,
            expected,
            hint: None,
        }
    }

    /// Add a hint to the error
    pub fn with_hint(mut self, hint: String) -> Self {
        self.hint = Some(hint);
        self
    }
}

/// SQL SELECT statement AST
#[derive(Debug, Clone, PartialEq)]
pub struct SqlSelect {
    /// Selected columns
    pub columns: Vec<SqlColumn>,

    /// Table name (optional for partial parses)
    pub table: Option<String>,

    /// WHERE clause filter
    pub where_clause: Option<SqlExpr>,

    /// GROUP BY columns
    pub group_by: Option<Vec<String>>,

    /// ORDER BY clauses
    pub order_by: Option<Vec<SqlOrderBy>>,

    /// LIMIT count
    pub limit: Option<usize>,

    /// OFFSET count
    pub offset: Option<usize>,
}

impl SqlSelect {
    /// Create a new empty SELECT statement
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            table: None,
            where_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            offset: None,
        }
    }

    /// Check if this select needs aggregation pipeline
    pub fn needs_aggregate(&self) -> bool {
        self.group_by.is_some()
            || self
                .columns
                .iter()
                .any(|c| matches!(c, SqlColumn::Aggregate { .. }))
    }
}

impl Default for SqlSelect {
    fn default() -> Self {
        Self::new()
    }
}

/// SQL column specification
#[derive(Debug, Clone, PartialEq)]
pub enum SqlColumn {
    /// SELECT *
    Star,

    /// SELECT column_name [AS alias]
    Field { name: String, alias: Option<String> },

    /// SELECT COUNT(*), SUM(col), etc.
    Aggregate {
        func: String,
        field: Option<String>,
        alias: Option<String>,
    },
}

impl SqlColumn {
    /// Create a simple field column
    pub fn field(name: String) -> Self {
        SqlColumn::Field { name, alias: None }
    }

    /// Create an aggregate column
    pub fn aggregate(func: String, field: Option<String>) -> Self {
        SqlColumn::Aggregate {
            func,
            field,
            alias: None,
        }
    }
}

/// SQL expression
#[derive(Debug, Clone, PartialEq)]
pub enum SqlExpr {
    /// Literal value
    Literal(SqlLiteral),

    /// Column reference
    Column(String),

    /// Binary operation (comparison)
    BinaryOp {
        left: Box<SqlExpr>,
        op: SqlOperator,
        right: Box<SqlExpr>,
    },

    /// Logical operation (AND, OR, NOT)
    LogicalOp {
        left: Box<SqlExpr>,
        op: SqlLogicalOperator,
        right: Box<SqlExpr>,
    },

    /// Function call
    Function { name: String, args: Vec<SqlExpr> },

    /// IN operator
    In {
        expr: Box<SqlExpr>,
        values: Vec<SqlExpr>,
    },

    /// LIKE pattern matching
    Like { expr: Box<SqlExpr>, pattern: String },

    /// IS NULL / IS NOT NULL
    IsNull { expr: Box<SqlExpr>, negated: bool },
}

/// SQL literal value
#[derive(Debug, Clone, PartialEq)]
pub enum SqlLiteral {
    /// String literal
    String(String),

    /// Number literal (stored as f64)
    Number(f64),

    /// Boolean literal
    Boolean(bool),

    /// NULL literal
    Null,
}

/// SQL comparison operators
#[derive(Debug, Clone, PartialEq)]
pub enum SqlOperator {
    /// =
    Eq,

    /// != or <>
    Ne,

    /// >
    Gt,

    /// <
    Lt,

    /// >=
    Ge,

    /// <=
    Le,
}

impl SqlOperator {
    /// Get operator precedence for Pratt parsing
    pub fn binding_power(&self) -> (u8, u8) {
        match self {
            SqlOperator::Eq | SqlOperator::Ne => (3, 4),
            SqlOperator::Gt | SqlOperator::Lt | SqlOperator::Ge | SqlOperator::Le => (5, 6),
        }
    }
}

/// SQL logical operators
#[derive(Debug, Clone, PartialEq)]
pub enum SqlLogicalOperator {
    /// AND
    And,

    /// OR
    Or,

    /// NOT
    Not,
}

impl SqlLogicalOperator {
    /// Get operator precedence for Pratt parsing
    pub fn binding_power(&self) -> (u8, u8) {
        match self {
            SqlLogicalOperator::Or => (1, 2),
            SqlLogicalOperator::And => (3, 4),
            SqlLogicalOperator::Not => (7, 8),
        }
    }
}

/// ORDER BY clause
#[derive(Debug, Clone, PartialEq)]
pub struct SqlOrderBy {
    /// Column name
    pub column: String,

    /// Ascending (true) or descending (false)
    pub asc: bool,
}

impl SqlOrderBy {
    /// Create a new ORDER BY clause
    pub fn new(column: String, asc: bool) -> Self {
        Self { column, asc }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_select_needs_aggregate() {
        let mut select = SqlSelect::new();
        assert!(!select.needs_aggregate());

        select.group_by = Some(vec!["category".to_string()]);
        assert!(select.needs_aggregate());

        let mut select2 = SqlSelect::new();
        select2
            .columns
            .push(SqlColumn::aggregate("COUNT".to_string(), None));
        assert!(select2.needs_aggregate());
    }

    #[test]
    fn test_expected_description() {
        assert_eq!(Expected::Keyword("SELECT").description(), "SELECT");
        assert_eq!(Expected::TableName.description(), "table name");
        assert_eq!(Expected::ColumnName.description(), "column name");
    }

    #[test]
    fn test_parse_result_checks() {
        let ok_result: ParseResult<i32> = ParseResult::Ok(42);
        assert!(ok_result.is_ok());
        assert!(!ok_result.is_partial());
        assert!(!ok_result.is_error());

        let partial_result: ParseResult<i32> = ParseResult::Partial(42, vec![]);
        assert!(!partial_result.is_ok());
        assert!(partial_result.is_partial());
        assert!(!partial_result.is_error());

        let error_result: ParseResult<i32> =
            ParseResult::Error(ParseError::new("error".to_string(), 0..1));
        assert!(!error_result.is_ok());
        assert!(!error_result.is_partial());
        assert!(error_result.is_error());
    }
}
