//! Database operation parser using custom MongoDB AST
//!
//! This module parses MongoDB database operations using our custom parser.
//! It handles syntax like:
//! - db.collection.find({ query })
//! - db.collection.insertOne({ doc })
//! - db.collection.aggregate([{ $match: {} }])
//! etc.

mod admin_ops;
mod args;
mod chain;
mod options;
mod query_ops;

use crate::error::{ParseError, Result};
use crate::parser::command::Command;
use crate::parser::mongo_ast::*;
use crate::parser::mongo_parser::MongoParser;
use chain::ChainParseResult;

use admin_ops::AdminOpsParser;
use args::ArgParser;
use chain::ChainHandler;
use query_ops::QueryOpsParser;

/// Parser for database operations
pub struct DbOperationParser;

impl DbOperationParser {
    /// Parse a database operation from input
    pub fn parse(input: &str) -> Result<Command> {
        let expr = MongoParser::parse(input)?;
        Self::parse_expression(&expr)
    }

    /// Parse an expression
    fn parse_expression(expr: &Expr) -> Result<Command> {
        match expr {
            Expr::Call(call) => Self::parse_call_expression(call),
            _ => Err(ParseError::InvalidCommand(
                "Expected call expression (e.g., db.collection.find())".to_string(),
            )
            .into()),
        }
    }

    /// Parse a call expression: db.collection.operation(...) or chained calls
    fn parse_call_expression(call: &CallExpr) -> Result<Command> {
        // Check if this is a chained call (e.g., db.users.find().limit(10))
        match ChainHandler::try_parse_chained_call(call)? {
            ChainParseResult::Chained(base_cmd, chain_methods) => {
                // Apply chained methods to the base command
                return ChainHandler::apply_chain_methods(base_cmd, chain_methods);
            }
            ChainParseResult::NotChained => {
                // Fall through to parse as regular db.collection.operation()
            }
        }

        // Not a chained call, parse as regular db.collection.operation()
        let (collection, operation) = ArgParser::extract_db_call_target(call.callee.as_ref())?;
        let args = &call.arguments;

        // Route to specific operation parser based on operation name
        match operation.as_str() {
            "explain" => QueryOpsParser::parse_explain(&collection, args, call),
            "find" => QueryOpsParser::parse_find(&collection, args),
            "findOne" => QueryOpsParser::parse_find_one(&collection, args),
            "insertOne" => QueryOpsParser::parse_insert_one(&collection, args),
            "insertMany" => QueryOpsParser::parse_insert_many(&collection, args),
            "updateOne" => QueryOpsParser::parse_update_one(&collection, args),
            "updateMany" => QueryOpsParser::parse_update_many(&collection, args),
            "replaceOne" => QueryOpsParser::parse_replace_one(&collection, args),
            "deleteOne" => QueryOpsParser::parse_delete_one(&collection, args),
            "deleteMany" => QueryOpsParser::parse_delete_many(&collection, args),
            "aggregate" => QueryOpsParser::parse_aggregate(&collection, args),
            "countDocuments" => QueryOpsParser::parse_count_documents(&collection, args),
            "count" => QueryOpsParser::parse_count_documents(&collection, args),
            "estimatedDocumentCount" => QueryOpsParser::parse_estimated_document_count(&collection, args),
            "findOneAndDelete" => QueryOpsParser::parse_find_one_and_delete(&collection, args),
            "findOneAndUpdate" => QueryOpsParser::parse_find_one_and_update(&collection, args),
            "findOneAndReplace" => QueryOpsParser::parse_find_one_and_replace(&collection, args),
            "findAndModify" => QueryOpsParser::parse_find_and_modify(&collection, args),
            "distinct" => QueryOpsParser::parse_distinct(&collection, args),
            "bulkWrite" => QueryOpsParser::parse_bulk_write(&collection, args),
            "getIndexes" => AdminOpsParser::parse_get_indexes(&collection),
            "createIndex" => AdminOpsParser::parse_create_index(&collection, args),
            "createIndexes" => AdminOpsParser::parse_create_indexes(&collection, args),
            "dropIndex" => AdminOpsParser::parse_drop_index(&collection, args),
            "dropIndexes" => AdminOpsParser::parse_drop_indexes(&collection),
            "drop" => AdminOpsParser::parse_drop_collection(&collection),
            "renameCollection" => AdminOpsParser::parse_rename_collection(&collection, args),
            "stats" => AdminOpsParser::parse_collection_stats(&collection, args),
            _ => Err(
                ParseError::InvalidCommand(format!("Unknown operation '{}'", operation)).into(),
            ),
        }
    }
}

/// Parse a simple (non-chained) call expression
pub(crate) fn parse_call_expression_simple(call: &CallExpr) -> Result<Command> {
    let (collection, operation) = ArgParser::extract_db_call_target(call.callee.as_ref())?;
    let args = &call.arguments;

    match operation.as_str() {
        "find" => QueryOpsParser::parse_find(&collection, args),
        "findOne" => QueryOpsParser::parse_find_one(&collection, args),
        "aggregate" => QueryOpsParser::parse_aggregate(&collection, args),
        "count" | "countDocuments" => QueryOpsParser::parse_count_documents(&collection, args),
        "distinct" => QueryOpsParser::parse_distinct(&collection, args),
        _ => Err(ParseError::InvalidCommand(format!(
            "Operation '{}' cannot be chained",
            operation
        ))
        .into()),
    }
}
