//! AST to MongoDB command conversion
//!
//! This module handles conversion of SQL AST to MongoDB commands:
//! - Converting SELECT to find() or aggregate()
//! - Building aggregation pipelines
//! - Converting WHERE clauses to MongoDB filters
//! - Handling GROUP BY, ORDER BY, LIMIT, OFFSET
//! - EXPLAIN query wrapping

use mongodb::bson::{doc, Document};

use super::super::command::{AggregateOptions, Command, FindOptions, QueryCommand};
use super::super::sql_context::{SqlColumn, SqlExpr, SqlSelect};
use super::super::sql_expr::SqlExprConverter;
use crate::error::Result;

impl super::SqlParser {
    /// Convert SQL AST to MongoDB Command
    pub(super) fn ast_to_command(&self, ast: SqlSelect) -> Result<Command> {
        let collection = ast
            .table
            .clone()
            .ok_or_else(|| crate::error::ParseError::InvalidCommand("Missing table name".to_string()))?;

        // Check if we need aggregation pipeline
        let needs_agg = ast.needs_aggregate() || self.has_complex_field_paths(&ast);

        if needs_agg {
            self.to_aggregate(ast, collection)
        } else {
            self.to_find(ast, collection)
        }
    }

    /// Wrap a command in EXPLAIN
    pub(super) fn wrap_in_explain(
        &self,
        cmd: Command,
        verbosity: super::super::command::ExplainVerbosity,
    ) -> Result<Command> {
        use super::super::command::QueryCommand;

        match cmd {
            Command::Query(query_cmd) => {
                if !query_cmd.supports_explain() {
                    return Err(crate::error::ParseError::InvalidCommand(
                        "EXPLAIN can only be used with SELECT queries".to_string(),
                    )
                    .into());
                }

                let collection = query_cmd.collection().to_string();

                Ok(Command::Query(QueryCommand::Explain {
                    collection,
                    verbosity,
                    query: Box::new(query_cmd),
                }))
            }
            _ => Err(crate::error::ParseError::InvalidCommand(
                "EXPLAIN can only be used with query commands".to_string(),
            )
            .into()),
        }
    }

    /// Check if SELECT has complex field paths requiring aggregation
    pub(super) fn has_complex_field_paths(&self, ast: &SqlSelect) -> bool {
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

    /// Check if expression contains complex field paths or arithmetic operations
    pub(super) fn expr_has_complex_paths(&self, expr: &SqlExpr) -> bool {
        match expr {
            SqlExpr::FieldPath(path) => path.requires_aggregation(),
            SqlExpr::BinaryOp { left, right, .. } => {
                self.expr_has_complex_paths(left) || self.expr_has_complex_paths(right)
            }
            SqlExpr::LogicalOp { left, right, .. } => {
                self.expr_has_complex_paths(left) || self.expr_has_complex_paths(right)
            }
            SqlExpr::ArithmeticOp { .. } => {
                // Arithmetic operations always require aggregation pipeline
                true
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
    pub(super) fn to_find(&self, ast: SqlSelect, collection: String) -> Result<Command> {
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
    pub(super) fn to_aggregate(&self, ast: SqlSelect, collection: String) -> Result<Command> {
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

        // Check if we have any aggregate functions (either as SqlColumn::Aggregate or inside Expression)
        let has_aggregates = ast.columns.iter().any(|c| match c {
            SqlColumn::Aggregate { .. } => true,
            SqlColumn::Expression { expr, .. } => Self::expr_contains_aggregate(expr),
            _ => false,
        });

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

            // Add aggregate functions - collect intermediate results for expressions
            let mut expr_columns: Vec<(&SqlExpr, Option<&String>)> = Vec::new();

            for col in &ast.columns {
                match col {
                    SqlColumn::Aggregate {
                        func,
                        field,
                        alias,
                        distinct,
                    } => {
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
                    SqlColumn::Expression { expr, alias } => {
                        // For expressions containing aggregates, we need to:
                        // 1. Add intermediate aggregate results to $group
                        // 2. Compute the final expression in $project
                        Self::add_aggregate_to_group(expr, &mut group_doc)?;
                        expr_columns.push((expr, alias.as_ref()));
                    }
                    _ => {}
                }
            }

            pipeline.push(doc! { "$group": group_doc });

            // Add $project stage to exclude _id and compute final results
            let mut project_doc = Document::new();
            project_doc.insert("_id", 0); // Exclude _id

            for col in &ast.columns {
                match col {
                    SqlColumn::Aggregate {
                        func,
                        alias,
                        distinct,
                        ..
                    } => {
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
                    SqlColumn::Expression { expr, alias } => {
                        // Build expression using $group results
                        let field_name = alias
                            .clone()
                            .unwrap_or_else(|| expr.to_display_string());
                        if let Ok(bson_expr) = Self::build_post_group_expr(expr) {
                            project_doc.insert(field_name, bson_expr);
                        }
                    }
                    _ => {}
                }
            }

            pipeline.push(doc! { "$project": project_doc });
        } else {
            // No GROUP BY, no aggregates: just field aliases or expressions
            // Add $project stage to handle field renaming and expressions
            let mut project_doc = Document::new();
            let mut has_id = false;

            for col in &ast.columns {
                match col {
                    SqlColumn::Field { path, alias } => {
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
                    SqlColumn::Expression { expr, alias } => {
                        // Convert expression to aggregation expression
                        if let Ok(bson_expr) = SqlExprConverter::expr_to_aggregate_value(expr) {
                            // Use alias if provided, otherwise use the expression string
                            let field_name = alias
                                .clone()
                                .unwrap_or_else(|| expr.to_display_string());
                            project_doc.insert(field_name, bson_expr);
                        }
                    }
                    _ => {}
                }
            }

            // Exclude _id if not explicitly requested
            if !has_id {
                project_doc.insert("_id", 0);
            }

            pipeline.push(doc! { "$project": project_doc });
        }

        // Add $skip and $limit AFTER $group/$project
        // This ensures LIMIT applies to final results, not documents before aggregation
        if let Some(offset) = ast.offset {
            pipeline.push(doc! { "$skip": offset as i64 });
        }
        if let Some(limit) = ast.limit {
            pipeline.push(doc! { "$limit": limit as i64 });
        }

        Ok(Command::Query(QueryCommand::Aggregate {
            collection,
            pipeline,
            options: AggregateOptions::default(),
        }))
    }

    /// Check if an expression contains aggregate functions
    pub(super) fn expr_contains_aggregate(expr: &SqlExpr) -> bool {
        match expr {
            SqlExpr::Function { name, .. } => {
                let upper = name.to_uppercase();
                matches!(upper.as_str(), "COUNT" | "SUM" | "AVG" | "MIN" | "MAX")
            }
            SqlExpr::ArithmeticOp { left, right, .. } => {
                Self::expr_contains_aggregate(left) || Self::expr_contains_aggregate(right)
            }
            _ => false,
        }
    }

    /// Add aggregate functions from expression to $group document
    pub(super) fn add_aggregate_to_group(
        expr: &SqlExpr,
        group_doc: &mut Document,
    ) -> Result<()> {
        match expr {
            SqlExpr::Function { name, args } => {
                let upper = name.to_uppercase();
                if matches!(upper.as_str(), "COUNT" | "SUM" | "AVG" | "MIN" | "MAX") {
                    // Use function name as intermediate field name
                    let field_name = if args.is_empty() {
                        format!("_agg_{}", upper.to_lowercase())
                    } else {
                        format!("_agg_{}_{}", upper.to_lowercase(), args.len())
                    };

                    if !group_doc.contains_key(&field_name) {
                        let agg_expr = match upper.as_str() {
                            "COUNT" => doc! { "$sum": 1 },
                            "SUM" => {
                                if let Some(SqlExpr::FieldPath(path)) = args.first() {
                                    let field = path
                                        .to_mongodb_path()
                                        .unwrap_or_else(|| path.base_field());
                                    doc! { "$sum": format!("${}", field) }
                                } else {
                                    doc! { "$sum": 1 }
                                }
                            }
                            "AVG" => {
                                if let Some(SqlExpr::FieldPath(path)) = args.first() {
                                    let field = path
                                        .to_mongodb_path()
                                        .unwrap_or_else(|| path.base_field());
                                    doc! { "$avg": format!("${}", field) }
                                } else {
                                    doc! { "$avg": 0 }
                                }
                            }
                            "MIN" => {
                                if let Some(SqlExpr::FieldPath(path)) = args.first() {
                                    let field = path
                                        .to_mongodb_path()
                                        .unwrap_or_else(|| path.base_field());
                                    doc! { "$min": format!("${}", field) }
                                } else {
                                    doc! { "$min": 0 }
                                }
                            }
                            "MAX" => {
                                if let Some(SqlExpr::FieldPath(path)) = args.first() {
                                    let field = path
                                        .to_mongodb_path()
                                        .unwrap_or_else(|| path.base_field());
                                    doc! { "$max": format!("${}", field) }
                                } else {
                                    doc! { "$max": 0 }
                                }
                            }
                            _ => doc! { "$sum": 1 },
                        };
                        group_doc.insert(field_name, agg_expr);
                    }
                }
            }
            SqlExpr::ArithmeticOp { left, right, .. } => {
                Self::add_aggregate_to_group(left, group_doc)?;
                Self::add_aggregate_to_group(right, group_doc)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Build expression for $project stage that references $group results
    pub(super) fn build_post_group_expr(
        expr: &SqlExpr,
    ) -> Result<mongodb::bson::Bson> {
        match expr {
            SqlExpr::Function { name, args } => {
                let upper = name.to_uppercase();
                if matches!(upper.as_str(), "COUNT" | "SUM" | "AVG" | "MIN" | "MAX") {
                    // Reference the intermediate field from $group
                    let field_name = if args.is_empty() {
                        format!("_agg_{}", upper.to_lowercase())
                    } else {
                        format!("_agg_{}_{}", upper.to_lowercase(), args.len())
                    };
                    Ok(mongodb::bson::Bson::String(format!("${}", field_name)))
                } else {
                    SqlExprConverter::expr_to_aggregate_value(expr)
                }
            }
            SqlExpr::ArithmeticOp { left, op, right } => {
                let left_expr = Self::build_post_group_expr(left)?;
                let right_expr = Self::build_post_group_expr(right)?;
                Ok(mongodb::bson::Bson::Document(doc! {
                    op.to_mongo_operator(): [left_expr, right_expr]
                }))
            }
            SqlExpr::Literal(lit) => Ok(SqlExprConverter::literal_to_bson_public(lit)),
            _ => SqlExprConverter::expr_to_aggregate_value(expr),
        }
    }
}
