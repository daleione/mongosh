//! SQL context and AST definitions
//!
//! This module defines the SQL AST (Abstract Syntax Tree) and context
//! models used for parsing, autocomplete, and error reporting.

#![allow(dead_code)]

use std::ops::Range;

/// Field path representation supporting nested fields and array access
#[derive(Debug, Clone, PartialEq)]
pub enum FieldPath {
    /// Simple field: `name`
    Simple(String),

    /// Nested field: `address.city`
    Nested { base: Box<FieldPath>, field: String },

    /// Array index access: `tags[0]` or `tags[-1]`
    ArrayIndex {
        base: Box<FieldPath>,
        index: ArrayIndex,
    },

    /// Array slice: `tags[0:5]` or `tags[:10:2]`
    ArraySlice {
        base: Box<FieldPath>,
        slice: ArraySlice,
    },
}

impl FieldPath {
    /// Create a simple field path
    pub fn simple(name: String) -> Self {
        FieldPath::Simple(name)
    }

    /// Create a nested field path
    pub fn nested(base: FieldPath, field: String) -> Self {
        FieldPath::Nested {
            base: Box::new(base),
            field,
        }
    }

    /// Create an array index path
    pub fn index(base: FieldPath, index: ArrayIndex) -> Self {
        FieldPath::ArrayIndex {
            base: Box::new(base),
            index,
        }
    }

    /// Create an array slice path
    pub fn slice(base: FieldPath, slice: ArraySlice) -> Self {
        FieldPath::ArraySlice {
            base: Box::new(base),
            slice,
        }
    }

    /// Convert to MongoDB dot notation path (for simple cases)
    pub fn to_mongodb_path(&self) -> Option<String> {
        match self {
            FieldPath::Simple(name) => Some(name.clone()),
            FieldPath::Nested { base, field } => {
                base.to_mongodb_path().map(|b| format!("{}.{}", b, field))
            }
            // Array access requires aggregation pipeline
            FieldPath::ArrayIndex { .. } | FieldPath::ArraySlice { .. } => None,
        }
    }

    /// Check if this path requires aggregation pipeline
    pub fn requires_aggregation(&self) -> bool {
        match self {
            FieldPath::Simple(_) => false,
            FieldPath::Nested { base, .. } => base.requires_aggregation(),
            FieldPath::ArrayIndex { .. } | FieldPath::ArraySlice { .. } => true,
        }
    }

    /// Get the base field name (for simple optimization)
    pub fn base_field(&self) -> String {
        match self {
            FieldPath::Simple(name) => name.clone(),
            FieldPath::Nested { base, .. }
            | FieldPath::ArrayIndex { base, .. }
            | FieldPath::ArraySlice { base, .. } => base.base_field(),
        }
    }
}

/// Array index (positive or negative)
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayIndex {
    /// Positive index: `arr[0]`, `arr[5]`
    Positive(i64),

    /// Negative index: `arr[-1]`, `arr[-2]`
    Negative(i64),
}

impl ArrayIndex {
    /// Create a positive index
    pub fn positive(index: i64) -> Self {
        ArrayIndex::Positive(index)
    }

    /// Create a negative index
    pub fn negative(index: i64) -> Self {
        ArrayIndex::Negative(index.abs())
    }

    /// Resolve to MongoDB array index
    pub fn resolve(&self, array_len: Option<usize>) -> Option<usize> {
        match (self, array_len) {
            (ArrayIndex::Positive(idx), _) if *idx >= 0 => Some(*idx as usize),
            (ArrayIndex::Negative(idx), Some(len)) if *idx <= len as i64 => {
                Some(len - (*idx as usize))
            }
            _ => None,
        }
    }
}

/// Array slice specification
#[derive(Debug, Clone, PartialEq)]
pub struct ArraySlice {
    /// Start index (inclusive, optional)
    pub start: Option<SliceIndex>,

    /// End index (exclusive, optional)
    pub end: Option<SliceIndex>,

    /// Step size (default 1)
    pub step: Option<i64>,
}

impl ArraySlice {
    /// Create a new slice
    pub fn new(start: Option<SliceIndex>, end: Option<SliceIndex>, step: Option<i64>) -> Self {
        Self { start, end, step }
    }

    /// Create a full slice `[:]`
    pub fn full() -> Self {
        Self::new(None, None, None)
    }

    /// Create a slice to end `[:n]`
    pub fn to(end: SliceIndex) -> Self {
        Self::new(None, Some(end), None)
    }

    /// Create a slice from start `[n:]`
    pub fn from(start: SliceIndex) -> Self {
        Self::new(Some(start), None, None)
    }
}

/// Slice index (can be positive or negative)
#[derive(Debug, Clone, PartialEq)]
pub enum SliceIndex {
    Positive(i64),
    Negative(i64),
}

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
            || self.columns.iter().any(|c| match c {
                SqlColumn::Aggregate { .. } => true,
                SqlColumn::Field { alias, .. } => alias.is_some(),
                _ => false,
            })
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
    Field {
        path: FieldPath,
        alias: Option<String>,
    },

    /// SELECT COUNT(*), SUM(col), COUNT(DISTINCT col), etc.
    Aggregate {
        func: String,
        field: Option<FieldPath>,
        alias: Option<String>,
        distinct: bool,
    },
}

impl SqlColumn {
    /// Create a simple field column
    pub fn field(path: FieldPath) -> Self {
        SqlColumn::Field { path, alias: None }
    }

    /// Create an aggregate column
    pub fn aggregate(func: String, field: Option<FieldPath>) -> Self {
        SqlColumn::Aggregate {
            func,
            field,
            alias: None,
            distinct: false,
        }
    }
}

/// SQL expression
#[derive(Debug, Clone, PartialEq)]
pub enum SqlExpr {
    /// Literal value
    Literal(SqlLiteral),

    /// Field path reference (supports nested fields and array access)
    FieldPath(FieldPath),

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
    /// Field path
    pub path: FieldPath,

    /// Ascending (true) or descending (false)
    pub asc: bool,
}

impl SqlOrderBy {
    /// Create a new ORDER BY clause
    pub fn new(path: FieldPath, asc: bool) -> Self {
        Self { path, asc }
    }
}

/// Array access error types
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayAccessError {
    /// Empty index brackets
    EmptyIndex,

    /// Invalid index type (not a number)
    InvalidIndexType(String),

    /// Missing closing bracket
    MissingCloseBracket,

    /// Invalid slice syntax
    InvalidSliceSyntax(String),

    /// Zero step size in slice
    ZeroStepSize,

    /// Unsupported feature
    UnsupportedFeature(String),
}

impl ArrayAccessError {
    /// Convert to user-friendly error message
    pub fn to_user_message(&self) -> String {
        match self {
            ArrayAccessError::EmptyIndex => {
                "Empty array index. Use arr[0] for first element or arr[-1] for last element."
                    .to_string()
            }
            ArrayAccessError::InvalidIndexType(val) => {
                format!("Invalid array index '{}'. Index must be a number.", val)
            }
            ArrayAccessError::MissingCloseBracket => {
                "Missing closing bracket ']' for array access.".to_string()
            }
            ArrayAccessError::InvalidSliceSyntax(msg) => {
                format!("Invalid array slice syntax: {}", msg)
            }
            ArrayAccessError::ZeroStepSize => "Array slice step cannot be zero.".to_string(),
            ArrayAccessError::UnsupportedFeature(feature) => {
                format!("Unsupported feature: {}", feature)
            }
        }
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
        select2.columns.push(SqlColumn::Aggregate {
            func: "COUNT".to_string(),
            field: None,
            alias: None,
            distinct: false,
        });
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
