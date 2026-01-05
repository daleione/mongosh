//! SQL expression to BSON converter
//!
//! This module converts SQL expressions and AST nodes into MongoDB BSON
//! documents for use in queries and aggregation pipelines.

use mongodb::bson::{Document, doc};

use super::sql_context::{SqlColumn, SqlExpr, SqlLiteral, SqlLogicalOperator, SqlOperator};
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

        for col in columns {
            match col {
                SqlColumn::Star => {
                    // If we have * mixed with other columns, just return None (all fields)
                    return Ok(None);
                }
                SqlColumn::Field { name, alias } => {
                    let field_name = alias.as_ref().unwrap_or(name);
                    projection.insert(field_name.clone(), 1);
                }
                SqlColumn::Aggregate { alias, .. } => {
                    // For aggregates, we need aggregation pipeline
                    // The projection will be built in the pipeline
                    if let Some(alias_name) = alias {
                        projection.insert(alias_name.clone(), 1);
                    }
                }
            }
        }

        if projection.is_empty() {
            Ok(None)
        } else {
            Ok(Some(projection))
        }
    }

    /// Convert SQL expression to MongoDB filter document
    pub fn expr_to_filter(expr: &SqlExpr) -> Result<Document> {
        match expr {
            SqlExpr::Literal(_lit) => {
                Err(ParseError::InvalidCommand("Cannot use literal as filter".to_string()).into())
            }

            SqlExpr::Column(name) => {
                // Column reference alone - check if truthy
                Ok(doc! { name: { "$exists": true } })
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
        }
    }

    /// Convert binary operation to filter
    fn binary_op_to_filter(left: &SqlExpr, op: &SqlOperator, right: &SqlExpr) -> Result<Document> {
        // Left side should be a column name
        let column = match left {
            SqlExpr::Column(name) => name.clone(),
            _ => {
                return Err(ParseError::InvalidCommand(
                    "Left side of comparison must be a column name".to_string(),
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
            SqlExpr::Column(name) => name.clone(),
            _ => {
                return Err(ParseError::InvalidCommand(
                    "IN expression must have column on left side".to_string(),
                )
                .into());
            }
        };

        let bson_values: Result<Vec<mongodb::bson::Bson>> =
            values.iter().map(|v| Self::expr_to_bson_value(v)).collect();

        Ok(doc! { column: { "$in": bson_values? } })
    }

    /// Convert LIKE expression to filter (using regex)
    fn like_to_filter(expr: &SqlExpr, pattern: &str) -> Result<Document> {
        let column = match expr {
            SqlExpr::Column(name) => name.clone(),
            _ => {
                return Err(ParseError::InvalidCommand(
                    "LIKE expression must have column on left side".to_string(),
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
            SqlExpr::Column(name) => name.clone(),
            _ => {
                return Err(ParseError::InvalidCommand(
                    "IS NULL expression must have column on left side".to_string(),
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
            SqlExpr::Column(name) => {
                // Column reference as value - use field path syntax
                Ok(mongodb::bson::Bson::String(format!("${}", name)))
            }
            _ => Err(ParseError::InvalidCommand(
                "Complex expressions not supported as values".to_string(),
            )
            .into()),
        }
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
        field: &Option<String>,
        distinct: bool,
    ) -> Result<Document> {
        let func_upper = func.to_uppercase();

        let agg_expr = match func_upper.as_str() {
            "COUNT" => {
                if distinct {
                    // COUNT(DISTINCT field) - use $addToSet to collect unique values
                    let field_name = field.as_ref().ok_or_else(|| {
                        ParseError::InvalidCommand("COUNT(DISTINCT) requires a field".to_string())
                    })?;
                    doc! { "$addToSet": format!("${}", field_name) }
                } else if field.is_none() {
                    doc! { "$sum": 1 }
                } else {
                    doc! { "$sum": { "$cond": [{ "$ifNull": [format!("${}", field.as_ref().unwrap()), false] }, 1, 0] } }
                }
            }
            "SUM" => {
                let field_name = field.as_ref().ok_or_else(|| {
                    ParseError::InvalidCommand("SUM requires a field".to_string())
                })?;
                doc! { "$sum": format!("${}", field_name) }
            }
            "AVG" => {
                let field_name = field.as_ref().ok_or_else(|| {
                    ParseError::InvalidCommand("AVG requires a field".to_string())
                })?;
                doc! { "$avg": format!("${}", field_name) }
            }
            "MIN" => {
                let field_name = field.as_ref().ok_or_else(|| {
                    ParseError::InvalidCommand("MIN requires a field".to_string())
                })?;
                doc! { "$min": format!("${}", field_name) }
            }
            "MAX" => {
                let field_name = field.as_ref().ok_or_else(|| {
                    ParseError::InvalidCommand("MAX requires a field".to_string())
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
                let agg_expr = Self::build_aggregate_expr(func, field, *distinct)?;
                group_doc.insert(output_name, agg_expr);
            }
        }

        Ok(group_doc)
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
            SqlColumn::field("name".to_string()),
            SqlColumn::field("age".to_string()),
        ];
        let projection = SqlExprConverter::columns_to_projection(&columns).unwrap();
        assert!(projection.is_some());
        let proj = projection.unwrap();
        assert_eq!(proj.get("name"), Some(&mongodb::bson::Bson::Int32(1)));
        assert_eq!(proj.get("age"), Some(&mongodb::bson::Bson::Int32(1)));
    }

    #[test]
    fn test_binary_op_eq() {
        let left = SqlExpr::Column("age".to_string());
        let right = SqlExpr::Literal(SqlLiteral::Number(18.0));
        let filter =
            SqlExprConverter::binary_op_to_filter(&left, &SqlOperator::Eq, &right).unwrap();
        assert_eq!(filter.get("age"), Some(&mongodb::bson::Bson::Int64(18)));
    }

    #[test]
    fn test_binary_op_gt() {
        let left = SqlExpr::Column("age".to_string());
        let right = SqlExpr::Literal(SqlLiteral::Number(18.0));
        let filter =
            SqlExprConverter::binary_op_to_filter(&left, &SqlOperator::Gt, &right).unwrap();
        let age_doc = filter.get_document("age").unwrap();
        assert_eq!(age_doc.get("$gt"), Some(&mongodb::bson::Bson::Int64(18)));
    }

    #[test]
    fn test_logical_and() {
        let left = SqlExpr::BinaryOp {
            left: Box::new(SqlExpr::Column("age".to_string())),
            op: SqlOperator::Gt,
            right: Box::new(SqlExpr::Literal(SqlLiteral::Number(18.0))),
        };
        let right = SqlExpr::BinaryOp {
            left: Box::new(SqlExpr::Column("status".to_string())),
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
        let expr = SqlExpr::Column("name".to_string());
        let filter = SqlExprConverter::like_to_filter(&expr, "John%").unwrap();
        let name_doc = filter.get_document("name").unwrap();
        assert!(name_doc.contains_key("$regex"));
        let regex = name_doc.get_str("$regex").unwrap();
        assert!(regex.starts_with("^John"));
    }

    #[test]
    fn test_is_null_to_filter() {
        let expr = SqlExpr::Column("deleted_at".to_string());
        let filter = SqlExprConverter::is_null_to_filter(&expr, false).unwrap();
        assert_eq!(filter.get("deleted_at"), Some(&mongodb::bson::Bson::Null));
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
}
