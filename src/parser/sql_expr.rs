//! SQL expression to BSON converter
//!
//! This module converts SQL expressions and AST nodes into MongoDB BSON
//! documents for use in queries and aggregation pipelines.

use mongodb::bson::{Document, doc};

use super::sql_context::{
    ArithmeticOperator, ArrayIndex, ArraySlice, FieldPath, SliceIndex, SqlColumn, SqlExpr,
    SqlLiteral, SqlLogicalOperator, SqlOperator,
};
use crate::error::{ParseError, Result};

/// SQL expression converter
pub struct SqlExprConverter;

impl SqlExprConverter {
    /// Convert SQL columns to MongoDB projection document
    pub fn columns_to_projection(columns: &[SqlColumn]) -> Result<Option<Document>> {
        // SELECT * means no projection (return all fields)
        if columns.is_empty() || columns.len() == 1 && matches!(columns[0], SqlColumn::Star) {
            return Ok(None);
        }

        let mut projection = Document::new();
        let mut has_id = false;

        for col in columns {
            match col {
                SqlColumn::Star => {
                    // If we have * mixed with other columns, just return None (all fields)
                    return Ok(None);
                }
                SqlColumn::Field { path, alias } => {
                    // Get the MongoDB path representation
                    if let Some(path_str) = path.to_mongodb_path() {
                        let field_name = alias.as_ref().unwrap_or(&path_str);
                        projection.insert(field_name.clone(), 1);

                        // Check if _id is explicitly requested
                        if path_str == "_id" {
                            has_id = true;
                        }
                    } else {
                        // Complex path requires aggregation - handle in pipeline
                        if let Some(alias_name) = alias {
                            projection.insert(alias_name.clone(), 1);
                        }
                    }
                }
                SqlColumn::Aggregate { alias, .. } => {
                    // For aggregates, we need aggregation pipeline
                    // The projection will be built in the pipeline
                    if let Some(alias_name) = alias {
                        projection.insert(alias_name.clone(), 1);
                    }
                }
                SqlColumn::Expression { alias, .. } => {
                    // Expression columns require aggregation pipeline
                    // The projection will be built in the pipeline with $project stage
                    if let Some(alias_name) = alias {
                        projection.insert(alias_name.clone(), 1);
                    }
                }
            }
        }

        if projection.is_empty() {
            Ok(None)
        } else {
            // If _id was not explicitly requested, exclude it
            if !has_id {
                projection.insert("_id", 0);
            }
            Ok(Some(projection))
        }
    }

    /// Convert SQL expression to MongoDB filter document
    pub fn expr_to_filter(expr: &SqlExpr) -> Result<Document> {
        match expr {
            SqlExpr::Literal(_lit) => {
                Err(ParseError::InvalidCommand("Cannot use literal as filter".to_string()).into())
            }

            SqlExpr::FieldPath(path) => {
                // Field path reference - check if exists
                if let Some(path_str) = path.to_mongodb_path() {
                    Ok(doc! { path_str: { "$exists": true } })
                } else {
                    Err(ParseError::InvalidCommand(
                        "Complex field paths in WHERE require aggregation pipeline".to_string(),
                    )
                    .into())
                }
            }

            SqlExpr::BinaryOp { left, op, right } => Self::binary_op_to_filter(left, op, right),

            SqlExpr::LogicalOp { left, op, right } => Self::logical_op_to_filter(left, op, right),

            SqlExpr::Function { name, args: _ } => Err(ParseError::InvalidCommand(format!(
                "Function {} not supported in WHERE clause",
                name
            ))
            .into()),

            SqlExpr::In { expr, values } => Self::in_to_filter(expr, values),

            SqlExpr::Like { expr, pattern } => Self::like_to_filter(expr, pattern),

            SqlExpr::IsNull { expr, negated } => Self::is_null_to_filter(expr, *negated),

            SqlExpr::TypedLiteral { .. } => {
                Err(ParseError::InvalidCommand("Cannot use typed literal as filter".to_string()).into())
            }

            SqlExpr::CurrentTime { .. } => {
                Err(ParseError::InvalidCommand("Cannot use current time function as filter".to_string()).into())
            }

            SqlExpr::Interval { .. } => {
                Err(ParseError::InvalidCommand("Cannot use interval as filter".to_string()).into())
            }

            SqlExpr::ArithmeticOp { left, op, right } => {
                // Arithmetic operations in WHERE need $expr with aggregation operators
                Self::arithmetic_comparison_to_filter(left, op, right)
            }
        }
    }

    /// Convert arithmetic expression to MongoDB aggregation expression
    pub fn arithmetic_to_aggregate(
        left: &SqlExpr,
        op: &ArithmeticOperator,
        right: &SqlExpr,
    ) -> Result<mongodb::bson::Bson> {
        let left_expr = Self::expr_to_aggregate_value(left)?;
        let right_expr = Self::expr_to_aggregate_value(right)?;

        Ok(mongodb::bson::Bson::Document(doc! {
            op.to_mongo_operator(): [left_expr, right_expr]
        }))
    }

    /// Convert SQL expression to aggregation pipeline value
    pub fn expr_to_aggregate_value(expr: &SqlExpr) -> Result<mongodb::bson::Bson> {
        match expr {
            SqlExpr::Literal(lit) => Ok(Self::literal_to_bson(lit)),

            SqlExpr::FieldPath(path) => {
                if let Some(path_str) = path.to_mongodb_path() {
                    Ok(mongodb::bson::Bson::String(format!("${}", path_str)))
                } else {
                    Err(ParseError::InvalidCommand(
                        "Complex field paths require special handling".to_string(),
                    )
                    .into())
                }
            }

            SqlExpr::ArithmeticOp { left, op, right } => {
                Self::arithmetic_to_aggregate(left, op, right)
            }

            SqlExpr::Function { name, args } => {
                Self::function_to_aggregate(name, args)
            }

            SqlExpr::TypedLiteral { type_name, value } => {
                Self::typed_literal_to_bson(type_name, value)
            }

            SqlExpr::CurrentTime { kind } => {
                Self::current_time_to_bson(kind)
            }

            _ => Err(ParseError::InvalidCommand(
                "Expression not supported in aggregation context".to_string(),
            )
            .into()),
        }
    }

    /// Convert function call to aggregation expression
    fn function_to_aggregate(name: &str, args: &[SqlExpr]) -> Result<mongodb::bson::Bson> {
        let upper_name = name.to_uppercase();
        match upper_name.as_str() {
            "ROUND" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(ParseError::InvalidCommand(
                        "ROUND requires 1 or 2 arguments".to_string(),
                    )
                    .into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                let place = if args.len() == 2 {
                    Self::expr_to_aggregate_value(&args[1])?
                } else {
                    mongodb::bson::Bson::Int32(0)
                };
                Ok(mongodb::bson::Bson::Document(doc! {
                    "$round": [value, place]
                }))
            }
            "ABS" => {
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "ABS requires exactly 1 argument".to_string(),
                    )
                    .into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                Ok(mongodb::bson::Bson::Document(doc! { "$abs": value }))
            }
            "CEIL" | "CEILING" => {
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "CEIL requires exactly 1 argument".to_string(),
                    )
                    .into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                Ok(mongodb::bson::Bson::Document(doc! { "$ceil": value }))
            }
            "FLOOR" => {
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "FLOOR requires exactly 1 argument".to_string(),
                    )
                    .into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                Ok(mongodb::bson::Bson::Document(doc! { "$floor": value }))
            }
            "TRUNC" | "TRUNCATE" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(ParseError::InvalidCommand(
                        "TRUNC requires 1 or 2 arguments".to_string(),
                    )
                    .into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                let place = if args.len() == 2 {
                    Self::expr_to_aggregate_value(&args[1])?
                } else {
                    mongodb::bson::Bson::Int32(0)
                };
                Ok(mongodb::bson::Bson::Document(doc! {
                    "$trunc": [value, place]
                }))
            }
            "OBJECTID" => {
                // ObjectId function - convert string argument to ObjectId literal
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "ObjectId requires exactly 1 argument".to_string(),
                    )
                    .into());
                }
                // Get the string value and convert to ObjectId
                match &args[0] {
                    SqlExpr::Literal(SqlLiteral::String(s)) => {
                        match mongodb::bson::oid::ObjectId::parse_str(s) {
                            Ok(oid) => Ok(mongodb::bson::Bson::ObjectId(oid)),
                            Err(e) => Err(ParseError::InvalidCommand(format!(
                                "Invalid ObjectId string '{}': {}", s, e
                            )).into()),
                        }
                    }
                    _ => Err(ParseError::InvalidCommand(
                        "ObjectId argument must be a string literal".to_string(),
                    ).into()),
                }
            }
            // Aggregate functions - convert to MongoDB aggregation operators
            "COUNT" => {
                // COUNT(*) or COUNT(field) - in expression context, return $sum: 1 or $sum with condition
                if args.is_empty() {
                    // COUNT(*) - just returns count, represented as literal in expression
                    Ok(mongodb::bson::Bson::Document(doc! { "$sum": 1 }))
                } else {
                    let field = Self::expr_to_aggregate_value(&args[0])?;
                    Ok(mongodb::bson::Bson::Document(doc! {
                        "$sum": { "$cond": [{ "$ne": [field, mongodb::bson::Bson::Null] }, 1, 0] }
                    }))
                }
            }
            "SUM" => {
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "SUM requires exactly 1 argument".to_string(),
                    ).into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                Ok(mongodb::bson::Bson::Document(doc! { "$sum": value }))
            }
            "AVG" => {
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "AVG requires exactly 1 argument".to_string(),
                    ).into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                Ok(mongodb::bson::Bson::Document(doc! { "$avg": value }))
            }
            "MIN" => {
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "MIN requires exactly 1 argument".to_string(),
                    ).into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                Ok(mongodb::bson::Bson::Document(doc! { "$min": value }))
            }
            "MAX" => {
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(
                        "MAX requires exactly 1 argument".to_string(),
                    ).into());
                }
                let value = Self::expr_to_aggregate_value(&args[0])?;
                Ok(mongodb::bson::Bson::Document(doc! { "$max": value }))
            }
            _ => Err(ParseError::InvalidCommand(format!(
                "Function {} not supported in aggregation",
                name
            ))
            .into()),
        }
    }

    /// Convert arithmetic comparison to $expr filter (for WHERE clause with arithmetic)
    fn arithmetic_comparison_to_filter(
        left: &SqlExpr,
        op: &ArithmeticOperator,
        right: &SqlExpr,
    ) -> Result<Document> {
        // For standalone arithmetic in WHERE, we need to check if it's part of a comparison
        // This handles cases like: WHERE price * quantity > 100
        // The arithmetic expression itself needs $expr wrapper
        let arith_expr = Self::arithmetic_to_aggregate(left, op, right)?;

        // Return as $expr for use in comparisons
        Ok(doc! {
            "$expr": {
                "$ne": [arith_expr, mongodb::bson::Bson::Null]
            }
        })
    }

    /// Convert binary operation to filter
    fn binary_op_to_filter(left: &SqlExpr, op: &SqlOperator, right: &SqlExpr) -> Result<Document> {
        // Check if either side contains complex expressions that need $expr
        let needs_expr = Self::expr_needs_aggregation(left) || Self::expr_needs_aggregation(right);

        if needs_expr {
            return Self::binary_op_to_expr_filter(left, op, right);
        }

        // Standard case: left side should be a field path
        let column = match left {
            SqlExpr::FieldPath(path) => {
                if let Some(path_str) = path.to_mongodb_path() {
                    path_str
                } else {
                    return Err(ParseError::InvalidCommand(
                        "Complex field paths in WHERE require aggregation pipeline".to_string(),
                    )
                    .into());
                }
            }
            _ => {
                return Err(ParseError::InvalidCommand(
                    "Left side of comparison must be a field path".to_string(),
                )
                .into());
            }
        };

        // Right side should be a literal value
        let value = Self::expr_to_bson_value(right)?;

        let filter = match op {
            SqlOperator::Eq => doc! { column: value },
            SqlOperator::Ne => doc! { column: { "$ne": value } },
            SqlOperator::Gt => doc! { column: { "$gt": value } },
            SqlOperator::Lt => doc! { column: { "$lt": value } },
            SqlOperator::Ge => doc! { column: { "$gte": value } },
            SqlOperator::Le => doc! { column: { "$lte": value } },
        };

        Ok(filter)
    }

    /// Check if an expression requires aggregation ($expr) context
    fn expr_needs_aggregation(expr: &SqlExpr) -> bool {
        match expr {
            SqlExpr::ArithmeticOp { .. } => true,
            SqlExpr::Function { name, .. } => {
                // Literal-value functions don't need aggregation context
                let upper_name = name.to_uppercase();
                !matches!(upper_name.as_str(), "OBJECTID" | "ISODATE" | "DATE" | "NUMBERLONG" | "NUMBERINT" | "NUMBERDECIMAL")
            }
            SqlExpr::BinaryOp { left, right, .. } => {
                Self::expr_needs_aggregation(left) || Self::expr_needs_aggregation(right)
            }
            _ => false,
        }
    }

    /// Convert binary operation with arithmetic to $expr filter
    /// Handles cases like: WHERE price * quantity > 100
    fn binary_op_to_expr_filter(
        left: &SqlExpr,
        op: &SqlOperator,
        right: &SqlExpr,
    ) -> Result<Document> {
        let left_expr = Self::expr_to_aggregate_value(left)?;
        let right_expr = Self::expr_to_aggregate_value(right)?;

        let mongo_op = match op {
            SqlOperator::Eq => "$eq",
            SqlOperator::Ne => "$ne",
            SqlOperator::Gt => "$gt",
            SqlOperator::Lt => "$lt",
            SqlOperator::Ge => "$gte",
            SqlOperator::Le => "$lte",
        };

        Ok(doc! {
            "$expr": {
                mongo_op: [left_expr, right_expr]
            }
        })
    }

    /// Convert logical operation to filter
    fn logical_op_to_filter(
        left: &SqlExpr,
        op: &SqlLogicalOperator,
        right: &SqlExpr,
    ) -> Result<Document> {
        let left_filter = Self::expr_to_filter(left)?;
        let right_filter = Self::expr_to_filter(right)?;

        let filter = match op {
            SqlLogicalOperator::And => {
                doc! { "$and": [left_filter, right_filter] }
            }
            SqlLogicalOperator::Or => {
                doc! { "$or": [left_filter, right_filter] }
            }
            SqlLogicalOperator::Not => {
                doc! { "$nor": [right_filter] }
            }
        };

        Ok(filter)
    }

    /// Convert IN expression to filter
    fn in_to_filter(expr: &SqlExpr, values: &[SqlExpr]) -> Result<Document> {
        let column = match expr {
            SqlExpr::FieldPath(path) => {
                if let Some(path_str) = path.to_mongodb_path() {
                    path_str
                } else {
                    return Err(ParseError::InvalidCommand(
                        "Complex field paths in IN require aggregation pipeline".to_string(),
                    )
                    .into());
                }
            }
            _ => {
                return Err(ParseError::InvalidCommand(
                    "IN expression must have field path on left side".to_string(),
                )
                .into());
            }
        };

        let bson_values: Result<Vec<mongodb::bson::Bson>> =
            values.iter().map(|v| Self::expr_to_bson_value(v)).collect();

        Ok(doc! { column: { "$in": bson_values? } })
    }

    /// Convert LIKE expression to filter (using regex)
    /// Convert LIKE expression to filter
    fn like_to_filter(expr: &SqlExpr, pattern: &str) -> Result<Document> {
        let column = match expr {
            SqlExpr::FieldPath(path) => {
                if let Some(path_str) = path.to_mongodb_path() {
                    path_str
                } else {
                    return Err(ParseError::InvalidCommand(
                        "Complex field paths in LIKE require aggregation pipeline".to_string(),
                    )
                    .into());
                }
            }
            _ => {
                return Err(ParseError::InvalidCommand(
                    "LIKE expression must have field path on left side".to_string(),
                )
                .into());
            }
        };

        // Convert SQL LIKE pattern to regex
        // % -> .* (any characters)
        // _ -> . (single character)
        let mut regex = String::new();
        let mut chars = pattern.chars().peekable();

        regex.push('^'); // Anchor at start

        while let Some(ch) = chars.next() {
            match ch {
                '%' => regex.push_str(".*"),
                '_' => regex.push('.'),
                '\\' => {
                    if let Some(next) = chars.next() {
                        regex.push('\\');
                        regex.push(next);
                    }
                }
                '.' | '*' | '+' | '?' | '|' | '[' | ']' | '(' | ')' | '{' | '}' | '^' | '$' => {
                    regex.push('\\');
                    regex.push(ch);
                }
                _ => regex.push(ch),
            }
        }

        regex.push('$'); // Anchor at end

        Ok(doc! {
            column: {
                "$regex": regex,
                "$options": "i"
            }
        })
    }

    /// Convert IS NULL expression to filter
    fn is_null_to_filter(expr: &SqlExpr, negated: bool) -> Result<Document> {
        let column = match expr {
            SqlExpr::FieldPath(path) => {
                if let Some(path_str) = path.to_mongodb_path() {
                    path_str
                } else {
                    return Err(ParseError::InvalidCommand(
                        "Complex field paths in IS NULL require aggregation pipeline".to_string(),
                    )
                    .into());
                }
            }
            _ => {
                return Err(ParseError::InvalidCommand(
                    "IS NULL expression must have field path on left side".to_string(),
                )
                .into());
            }
        };

        if negated {
            Ok(doc! { column: { "$ne": null } })
        } else {
            Ok(doc! { column: null })
        }
    }

    /// Convert SQL expression to BSON value
    fn expr_to_bson_value(expr: &SqlExpr) -> Result<mongodb::bson::Bson> {
        match expr {
            SqlExpr::Literal(lit) => Ok(Self::literal_to_bson(lit)),
            SqlExpr::FieldPath(path) => {
                // Field path reference as value - use MongoDB path syntax
                if let Some(path_str) = path.to_mongodb_path() {
                    Ok(mongodb::bson::Bson::String(format!("${}", path_str)))
                } else {
                    // Complex path requires aggregation expression
                    Self::field_path_to_bson(path)
                }
            }
            SqlExpr::Function { name, args } => Self::function_to_bson(name, args),
            SqlExpr::TypedLiteral { type_name, value } => Self::typed_literal_to_bson(type_name, value),
            SqlExpr::CurrentTime { kind } => Self::current_time_to_bson(kind),
            SqlExpr::Interval { .. } => Err(ParseError::InvalidCommand(
                "INTERVAL not yet supported as a standalone value".to_string(),
            )
            .into()),
            _ => Err(ParseError::InvalidCommand(
                "Complex expressions not supported as values".to_string(),
            )
            .into()),
        }
    }

    /// Convert SQL function call to BSON value
    fn function_to_bson(name: &str, args: &[SqlExpr]) -> Result<mongodb::bson::Bson> {
        match name.to_uppercase().as_str() {
            "OBJECTID" => {
                // ObjectId expects a single string argument
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(format!(
                        "ObjectId() expects 1 argument, got {}",
                        args.len()
                    ))
                    .into());
                }

                let id_str = match &args[0] {
                    SqlExpr::Literal(SqlLiteral::String(s)) => s.clone(),
                    _ => {
                        return Err(ParseError::InvalidCommand(
                            "ObjectId() expects a string argument".to_string(),
                        )
                        .into());
                    }
                };

                // Parse the hex string into an ObjectId
                match mongodb::bson::oid::ObjectId::parse_str(&id_str) {
                    Ok(oid) => Ok(mongodb::bson::Bson::ObjectId(oid)),
                    Err(e) => Err(ParseError::InvalidCommand(format!(
                        "Invalid ObjectId string '{}': {}",
                        id_str, e
                    ))
                    .into()),
                }
            }
            "ISODATE" | "DATE" => {
                // ISODate/Date expects a single string argument with ISO 8601 format
                if args.len() != 1 {
                    return Err(ParseError::InvalidCommand(format!(
                        "{}() expects 1 argument, got {}",
                        name,
                        args.len()
                    ))
                    .into());
                }

                let date_str = match &args[0] {
                    SqlExpr::Literal(SqlLiteral::String(s)) => s.clone(),
                    _ => {
                        return Err(ParseError::InvalidCommand(
                            format!("{}() expects a string argument", name),
                        )
                        .into());
                    }
                };

                // Parse ISO 8601 date string using MongoDB's built-in parser
                mongodb::bson::DateTime::parse_rfc3339_str(&date_str)
                    .map(mongodb::bson::Bson::DateTime)
                    .map_err(|e| ParseError::InvalidCommand(format!(
                        "Invalid date string '{}': {}. Expected ISO 8601 format (e.g., '2026-02-15T16:00:00.000Z')",
                        date_str, e
                    )).into())
            }
            _ => Err(ParseError::InvalidCommand(format!("Unsupported function: {}", name)).into()),
        }
    }

    /// Convert typed literal to BSON (DATE '...', TIMESTAMP '...')
    fn typed_literal_to_bson(type_name: &str, value: &str) -> Result<mongodb::bson::Bson> {
        match type_name.to_uppercase().as_str() {
            "DATE" | "TIMESTAMP" => Self::parse_datetime_string(value),
            "TIME" => Err(ParseError::InvalidCommand(
                "TIME type not yet supported. Use TIMESTAMP instead.".to_string(),
            ).into()),
            _ => Err(ParseError::InvalidCommand(
                format!("Unsupported type: {}", type_name)
            ).into()),
        }
    }

    /// Parse datetime string with multiple format support
    fn parse_datetime_string(value: &str) -> Result<mongodb::bson::Bson> {
        use mongodb::bson::DateTime as BsonDateTime;

        // Fast path: try RFC 3339 directly (most common case)
        if let Ok(dt) = BsonDateTime::parse_rfc3339_str(value) {
            return Ok(mongodb::bson::Bson::DateTime(dt));
        }

        // Analyze string characteristics once
        let has_t = value.contains('T');
        let has_space = value.contains(' ');
        let has_tz = value.ends_with('Z')
            || value.contains('+')
            || Self::has_negative_tz_offset(value);

        // Build normalized ISO 8601 string based on input format
        let iso_str = match (has_t, has_space) {
            // Space-separated: '2026-02-15 16:00:00' → '2026-02-15T16:00:00Z'
            (false, true) => {
                let base = value.replace(' ', "T");
                if has_tz { base } else { format!("{}Z", base) }
            }
            // Has 'T' but no timezone: '2026-02-15T16:00:00' → '2026-02-15T16:00:00Z'
            (true, _) if !has_tz => format!("{}Z", value),
            // Date only: '2026-02-15' or '2026/02/15'
            (false, false) => {
                let normalized = value.replace('/', "-");
                format!("{}T00:00:00Z", normalized)
            }
            // Already handled by fast path or invalid
            _ => return Err(Self::datetime_parse_error(value)),
        };

        BsonDateTime::parse_rfc3339_str(&iso_str)
            .map(mongodb::bson::Bson::DateTime)
            .map_err(|_| Self::datetime_parse_error(value))
    }

    /// Check if string has negative timezone offset (e.g., '-08:00')
    fn has_negative_tz_offset(value: &str) -> bool {
        // Look for pattern like '-HH:MM' at end, distinguishing from date dashes
        if let Some(pos) = value.rfind('-') {
            // Timezone offset appears after 'T' and has ':' following
            pos > 10 && value[pos..].contains(':')
        } else {
            false
        }
    }

    /// Build datetime parse error with supported formats
    #[inline]
    fn datetime_parse_error(value: &str) -> crate::error::MongoshError {
        ParseError::InvalidCommand(format!(
            "Invalid date string '{}'. Supported formats:\n  \
             - ISO 8601: '2026-02-15T16:00:00Z'\n  \
             - Date only: '2026-02-15'\n  \
             - Space-separated: '2026-02-15 16:00:00'\n  \
             - Slash-separated: '2026/02/15'",
            value
        )).into()
    }

    /// Convert current time function to BSON (CURRENT_TIMESTAMP, NOW())
    fn current_time_to_bson(kind: &str) -> Result<mongodb::bson::Bson> {
        match kind.to_uppercase().as_str() {
            "CURRENT_TIMESTAMP" | "CURRENT_DATE" | "CURRENT_TIME" | "NOW" => {
                Ok(mongodb::bson::Bson::DateTime(mongodb::bson::DateTime::now()))
            }
            _ => Err(ParseError::InvalidCommand(format!(
                "Unsupported current time function: {}",
                kind
            ))
            .into()),
        }
    }

    /// Convert SQL literal to BSON value (public version for use outside this module)
    pub fn literal_to_bson_public(lit: &SqlLiteral) -> mongodb::bson::Bson {
        Self::literal_to_bson(lit)
    }

    /// Convert SQL literal to BSON value
    fn literal_to_bson(lit: &SqlLiteral) -> mongodb::bson::Bson {
        match lit {
            SqlLiteral::String(s) => mongodb::bson::Bson::String(s.clone()),
            SqlLiteral::Number(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    mongodb::bson::Bson::Int64(*n as i64)
                } else {
                    mongodb::bson::Bson::Double(*n)
                }
            }
            SqlLiteral::Boolean(b) => mongodb::bson::Bson::Boolean(*b),
            SqlLiteral::Null => mongodb::bson::Bson::Null,
        }
    }

    /// Build a MongoDB aggregate expression for a SQL aggregate function
    pub fn build_aggregate_expr(
        func: &str,
        field: Option<&str>,
        distinct: bool,
    ) -> Result<Document> {
        let func_upper = func.to_uppercase();

        let agg_expr = match func_upper.as_str() {
            "COUNT" => {
                if let Some(field_name) = field {
                    if distinct {
                        // COUNT(DISTINCT field) -> collect unique values into set
                        doc! { "$addToSet": format!("${}", field_name) }
                    } else {
                        // COUNT(field) -> count non-null values
                        doc! { "$sum": doc! { "$cond": [{ "$ifNull": [format!("${}", field_name), false] }, 1, 0] } }
                    }
                } else {
                    // COUNT(*) -> count all documents
                    doc! { "$sum": 1 }
                }
            }
            "SUM" => {
                let field_name = field.ok_or_else(|| {
                    ParseError::InvalidCommand("SUM requires a field name".to_string())
                })?;
                doc! { "$sum": format!("${}", field_name) }
            }
            "AVG" => {
                let field_name = field.ok_or_else(|| {
                    ParseError::InvalidCommand("AVG requires a field name".to_string())
                })?;
                doc! { "$avg": format!("${}", field_name) }
            }
            "MIN" => {
                let field_name = field.ok_or_else(|| {
                    ParseError::InvalidCommand("MIN requires a field name".to_string())
                })?;
                doc! { "$min": format!("${}", field_name) }
            }
            "MAX" => {
                let field_name = field.ok_or_else(|| {
                    ParseError::InvalidCommand("MAX requires a field name".to_string())
                })?;
                doc! { "$max": format!("${}", field_name) }
            }
            _ => {
                return Err(ParseError::InvalidCommand(format!(
                    "Unsupported aggregate function: {}",
                    func
                ))
                .into());
            }
        };

        Ok(agg_expr)
    }

    /// Build aggregation $group stage from GROUP BY and aggregate functions
    pub fn build_group_stage(group_by: &[String], columns: &[SqlColumn]) -> Result<Document> {
        let mut group_doc = Document::new();

        // Build _id field from GROUP BY columns
        if group_by.len() == 1 {
            // Single field grouping
            group_doc.insert("_id", format!("${}", group_by[0]));
        } else {
            // Multiple field grouping
            let mut id_doc = Document::new();
            for field in group_by {
                id_doc.insert(field.clone(), format!("${}", field));
            }
            group_doc.insert("_id", id_doc);
        }

        // Add aggregate functions
        for col in columns {
            if let SqlColumn::Aggregate {
                func,
                field,
                alias,
                distinct,
            } = col
            {
                let output_name = alias.clone().unwrap_or_else(|| func.to_lowercase());
                let field_str = field.as_ref().and_then(|p| p.to_mongodb_path());
                let aggregate_expr =
                    SqlExprConverter::build_aggregate_expr(func, field_str.as_deref(), *distinct)?;
                group_doc.insert(output_name, aggregate_expr);
            }
        }

        Ok(group_doc)
    }

    /// Convert FieldPath to BSON for aggregation expressions
    pub fn field_path_to_bson(path: &FieldPath) -> Result<mongodb::bson::Bson> {
        match path {
            FieldPath::Simple(name) => Ok(mongodb::bson::Bson::String(format!("${}", name))),
            FieldPath::Nested { base, field } => {
                if let Some(base_str) = base.to_mongodb_path() {
                    Ok(mongodb::bson::Bson::String(format!(
                        "${}.{}",
                        base_str, field
                    )))
                } else {
                    // Complex nested path requires aggregation expression
                    Err(ParseError::InvalidCommand(
                        "Complex nested paths not yet fully supported".to_string(),
                    )
                    .into())
                }
            }
            FieldPath::ArrayIndex { base, index } => {
                // Use $arrayElemAt aggregation operator
                let base_path = if let Some(p) = base.to_mongodb_path() {
                    format!("${}", p)
                } else {
                    return Err(ParseError::InvalidCommand(
                        "Complex array base paths not yet supported".to_string(),
                    )
                    .into());
                };

                let index_value = match index {
                    ArrayIndex::Positive(idx) => *idx,
                    ArrayIndex::Negative(idx) => -*idx,
                };

                Ok(mongodb::bson::Bson::Document(doc! {
                    "$arrayElemAt": [base_path, index_value]
                }))
            }
            FieldPath::ArraySlice { base, slice } => {
                // Use $slice aggregation operator
                let base_path = if let Some(p) = base.to_mongodb_path() {
                    format!("${}", p)
                } else {
                    return Err(ParseError::InvalidCommand(
                        "Complex array base paths not yet supported".to_string(),
                    )
                    .into());
                };

                Ok(Self::build_slice_expr(&base_path, slice))
            }
        }
    }

    /// Build $slice expression for array slicing
    fn build_slice_expr(base_path: &str, slice: &ArraySlice) -> mongodb::bson::Bson {
        let start = match &slice.start {
            Some(SliceIndex::Positive(n)) => *n,
            Some(SliceIndex::Negative(n)) => -*n,
            None => 0,
        };

        let count = match (&slice.start, &slice.end) {
            (None, Some(SliceIndex::Positive(end))) => *end,
            (Some(SliceIndex::Positive(s)), Some(SliceIndex::Positive(e))) => e - s,
            (None, None) => {
                // Full slice - return the array as-is
                return mongodb::bson::Bson::String(base_path.to_string());
            }
            _ => {
                // Complex slice with negative indices - use conditional logic
                // For now, use a simple approach
                100000 // Large number to get rest of array
            }
        };

        if slice.step.is_some() && slice.step != Some(1) {
            // Step not equal to 1 requires more complex aggregation
            // For now, just do basic slice
            mongodb::bson::Bson::Document(doc! {
                "$slice": [base_path, start, count]
            })
        } else {
            mongodb::bson::Bson::Document(doc! {
                "$slice": [base_path, start, count]
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_columns_to_projection_star() {
        let columns = vec![SqlColumn::Star];
        let projection = SqlExprConverter::columns_to_projection(&columns).unwrap();
        assert!(projection.is_none());
    }

    #[test]
    fn test_columns_to_projection_fields() {
        let columns = vec![
            SqlColumn::field(FieldPath::simple("name".to_string())),
            SqlColumn::field(FieldPath::simple("age".to_string())),
        ];
        let projection = SqlExprConverter::columns_to_projection(&columns).unwrap();
        assert!(projection.is_some());
        let proj = projection.unwrap();
        assert_eq!(proj.get("name"), Some(&mongodb::bson::Bson::Int32(1)));
        assert_eq!(proj.get("age"), Some(&mongodb::bson::Bson::Int32(1)));
        // _id should be excluded when not explicitly requested
        assert_eq!(proj.get("_id"), Some(&mongodb::bson::Bson::Int32(0)));
    }

    #[test]
    fn test_columns_to_projection_with_id() {
        let columns = vec![
            SqlColumn::field(FieldPath::simple("_id".to_string())),
            SqlColumn::field(FieldPath::simple("name".to_string())),
        ];
        let projection = SqlExprConverter::columns_to_projection(&columns).unwrap();
        assert!(projection.is_some());
        let proj = projection.unwrap();
        assert_eq!(proj.get("_id"), Some(&mongodb::bson::Bson::Int32(1)));
        assert_eq!(proj.get("name"), Some(&mongodb::bson::Bson::Int32(1)));
    }

    #[test]
    fn test_binary_op_eq() {
        let left = SqlExpr::FieldPath(FieldPath::simple("age".to_string()));
        let right = SqlExpr::Literal(SqlLiteral::Number(18.0));
        let filter =
            SqlExprConverter::binary_op_to_filter(&left, &SqlOperator::Eq, &right).unwrap();
        assert_eq!(filter.get("age"), Some(&mongodb::bson::Bson::Int64(18)));
    }

    #[test]
    fn test_binary_op_gt() {
        let left = SqlExpr::FieldPath(FieldPath::simple("age".to_string()));
        let right = SqlExpr::Literal(SqlLiteral::Number(18.0));
        let filter =
            SqlExprConverter::binary_op_to_filter(&left, &SqlOperator::Gt, &right).unwrap();
        let age_doc = filter.get_document("age").unwrap();
        assert_eq!(age_doc.get("$gt"), Some(&mongodb::bson::Bson::Int64(18)));
    }

    #[test]
    fn test_logical_and() {
        let left = SqlExpr::BinaryOp {
            left: Box::new(SqlExpr::FieldPath(FieldPath::simple("age".to_string()))),
            op: SqlOperator::Gt,
            right: Box::new(SqlExpr::Literal(SqlLiteral::Number(18.0))),
        };
        let right = SqlExpr::BinaryOp {
            left: Box::new(SqlExpr::FieldPath(FieldPath::simple("status".to_string()))),
            op: SqlOperator::Eq,
            right: Box::new(SqlExpr::Literal(SqlLiteral::String("active".to_string()))),
        };
        let filter =
            SqlExprConverter::logical_op_to_filter(&left, &SqlLogicalOperator::And, &right)
                .unwrap();
        assert!(filter.contains_key("$and"));
    }

    #[test]
    fn test_like_to_filter() {
        let expr = SqlExpr::FieldPath(FieldPath::simple("name".to_string()));
        let filter = SqlExprConverter::like_to_filter(&expr, "John%").unwrap();
        let name_doc = filter.get_document("name").unwrap();
        assert!(name_doc.contains_key("$regex"));
        let regex = name_doc.get_str("$regex").unwrap();
        assert!(regex.starts_with("^John"));
    }

    #[test]
    fn test_is_null_to_filter() {
        let expr = SqlExpr::FieldPath(FieldPath::simple("name".to_string()));
        let filter = SqlExprConverter::is_null_to_filter(&expr, false).unwrap();
        assert_eq!(filter.get("name"), Some(&mongodb::bson::Bson::Null));
    }

    #[test]
    fn test_literal_to_bson_string() {
        let lit = SqlLiteral::String("hello".to_string());
        let bson = SqlExprConverter::literal_to_bson(&lit);
        assert_eq!(bson, mongodb::bson::Bson::String("hello".to_string()));
    }

    #[test]
    fn test_literal_to_bson_number_int() {
        let lit = SqlLiteral::Number(42.0);
        let bson = SqlExprConverter::literal_to_bson(&lit);
        assert_eq!(bson, mongodb::bson::Bson::Int64(42));
    }

    #[test]
    fn test_literal_to_bson_number_float() {
        let lit = SqlLiteral::Number(3.14);
        let bson = SqlExprConverter::literal_to_bson(&lit);
        assert_eq!(bson, mongodb::bson::Bson::Double(3.14));
    }

    #[test]
    fn test_literal_to_bson_bool() {
        let lit = SqlLiteral::Boolean(true);
        let bson = SqlExprConverter::literal_to_bson(&lit);
        assert_eq!(bson, mongodb::bson::Bson::Boolean(true));
    }

    #[test]
    fn test_build_group_stage_with_count() {
        let group_by = vec!["category".to_string()];
        let columns = vec![SqlColumn::Aggregate {
            func: "COUNT".to_string(),
            field: None,
            alias: None,
            distinct: false,
        }];
        let result = SqlExprConverter::build_group_stage(&group_by, &columns);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert!(doc.contains_key("_id"));
        assert!(doc.contains_key("count"));
    }

    #[test]
    fn test_isodate_function() {
        let args = vec![SqlExpr::Literal(SqlLiteral::String(
            "2026-02-15T16:00:00.000Z".to_string(),
        ))];
        let result = SqlExprConverter::function_to_bson("ISODate", &args);
        assert!(result.is_ok());
        match result.unwrap() {
            mongodb::bson::Bson::DateTime(_) => {} // Success
            _ => panic!("Expected DateTime type"),
        }
    }

    #[test]
    fn test_date_function() {
        let args = vec![SqlExpr::Literal(SqlLiteral::String(
            "2024-01-01T00:00:00Z".to_string(),
        ))];
        let result = SqlExprConverter::function_to_bson("DATE", &args);
        assert!(result.is_ok());
        match result.unwrap() {
            mongodb::bson::Bson::DateTime(_) => {} // Success
            _ => panic!("Expected DateTime type"),
        }
    }

    #[test]
    fn test_isodate_with_milliseconds() {
        let args = vec![SqlExpr::Literal(SqlLiteral::String(
            "2024-06-15T10:30:00.500Z".to_string(),
        ))];
        let result = SqlExprConverter::function_to_bson("ISODate", &args);
        assert!(result.is_ok());
        match result.unwrap() {
            mongodb::bson::Bson::DateTime(_) => {} // Success
            _ => panic!("Expected DateTime type"),
        }
    }

    #[test]
    fn test_isodate_invalid_format() {
        let args = vec![SqlExpr::Literal(SqlLiteral::String(
            "invalid-date".to_string(),
        ))];
        let result = SqlExprConverter::function_to_bson("ISODate", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_isodate_wrong_arg_count() {
        let args = vec![];
        let result = SqlExprConverter::function_to_bson("ISODate", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_isodate_non_string_arg() {
        let args = vec![SqlExpr::Literal(SqlLiteral::Number(123.0))];
        let result = SqlExprConverter::function_to_bson("ISODate", &args);
        assert!(result.is_err());
    }
}
