//! Query operation parsers for MongoDB
//!
//! This module contains parsers for all query-related MongoDB operations:
//! - find, findOne
//! - insert operations
//! - update operations
//! - delete operations
//! - aggregate, count, distinct
//! - findAndModify and its variants

use mongodb::bson::Document;

use crate::error::{ParseError, Result};
use crate::parser::command::{Command, ExplainVerbosity, FindOptions, QueryCommand};
use crate::parser::mongo_ast::*;

use super::args::ArgParser;

/// Query operation parsers
pub struct QueryOpsParser;

impl QueryOpsParser {
    /// Parse explain operation: db.collection.explain(verbosity).queryMethod()
    /// This expects a chained call where explain() is followed by a query method
    pub fn parse_explain(_collection: &str, args: &[Expr], _call: &CallExpr) -> Result<Command> {
        // Parse verbosity argument (optional, defaults to "queryPlanner")
        let _verbosity = if args.is_empty() {
            ExplainVerbosity::default()
        } else if let Some(Expr::String(verb_str)) = args.first() {
            ExplainVerbosity::from_str(verb_str)?
        } else if let Some(Expr::Boolean(b)) = args.first() {
            // Handle boolean for backwards compatibility
            if *b {
                ExplainVerbosity::AllPlansExecution
            } else {
                ExplainVerbosity::QueryPlanner
            }
        } else {
            return Err(ParseError::InvalidCommand(
                "explain() expects a string verbosity argument or no argument".to_string(),
            )
            .into());
        };

        // Now we need to look for the chained method call
        // The structure should be: db.collection.explain().find() or similar
        // We need to check if this explain() call has a chained method after it

        // This is tricky because we're currently inside the explain() call
        // We need to be called differently to handle the chain
        // Let's check if there's a parent call that has explain as its callee object

        // For now, return an error with helpful message
        Err(ParseError::InvalidCommand(
            "explain() must be followed by a query method like find(), aggregate(), etc.\nExample: db.collection.explain().find({})".to_string(),
        )
        .into())
    }

    /// Parse find operation: db.collection.find(filter, projection)
    pub fn parse_find(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let projection = ArgParser::get_projection(args, 1)?;

        Ok(Command::Query(QueryCommand::Find {
            collection: collection.to_string(),
            filter,
            options: FindOptions {
                projection,
                ..Default::default()
            },
        }))
    }

    /// Parse findOne operation: db.collection.findOne(filter, projection)
    pub fn parse_find_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let projection = ArgParser::get_projection(args, 1)?;

        Ok(Command::Query(QueryCommand::Find {
            collection: collection.to_string(),
            filter,
            options: FindOptions {
                projection,
                limit: Some(1),
                ..Default::default()
            },
        }))
    }

    /// Parse insertOne operation: db.collection.insertOne(document)
    pub fn parse_insert_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let document = ArgParser::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::InsertOne {
            collection: collection.to_string(),
            document,
        }))
    }

    /// Parse insertMany operation: db.collection.insertMany(documents)
    pub fn parse_insert_many(collection: &str, args: &[Expr]) -> Result<Command> {
        let documents = ArgParser::get_doc_array_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::InsertMany {
            collection: collection.to_string(),
            documents,
        }))
    }

    /// Parse updateOne operation: db.collection.updateOne(filter, update, options)
    pub fn parse_update_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let update = ArgParser::get_doc_arg(args, 1)?;
        let options = ArgParser::get_update_options(args, 2)?;

        Ok(Command::Query(QueryCommand::UpdateOne {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse updateMany operation: db.collection.updateMany(filter, update, options)
    pub fn parse_update_many(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let update = ArgParser::get_doc_arg(args, 1)?;
        let options = ArgParser::get_update_options(args, 2)?;

        Ok(Command::Query(QueryCommand::UpdateMany {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse replaceOne operation: db.collection.replaceOne(filter, replacement, options)
    pub fn parse_replace_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let replacement = ArgParser::get_doc_arg(args, 1)?;
        let options = ArgParser::get_update_options(args, 2)?;

        Ok(Command::Query(QueryCommand::ReplaceOne {
            collection: collection.to_string(),
            filter,
            replacement,
            options,
        }))
    }

    /// Parse deleteOne operation: db.collection.deleteOne(filter)
    pub fn parse_delete_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::DeleteOne {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse deleteMany operation: db.collection.deleteMany(filter)
    pub fn parse_delete_many(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::DeleteMany {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse aggregate operation: db.collection.aggregate(pipeline, options)
    pub fn parse_aggregate(collection: &str, args: &[Expr]) -> Result<Command> {
        let pipeline = ArgParser::get_doc_array_arg(args, 0)?;
        let options = ArgParser::get_aggregate_options(args, 1)?;

        Ok(Command::Query(QueryCommand::Aggregate {
            collection: collection.to_string(),
            pipeline,
            options,
        }))
    }

    /// Parse countDocuments operation: db.collection.countDocuments(filter)
    pub fn parse_count_documents(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::CountDocuments {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse estimatedDocumentCount operation
    pub fn parse_estimated_document_count(collection: &str, _args: &[Expr]) -> Result<Command> {
        Ok(Command::Query(QueryCommand::EstimatedDocumentCount {
            collection: collection.to_string(),
        }))
    }

    /// Parse findOneAndDelete operation
    pub fn parse_find_one_and_delete(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let options = ArgParser::get_find_and_modify_options(args, 1)?;

        Ok(Command::Query(QueryCommand::FindOneAndDelete {
            collection: collection.to_string(),
            filter,
            options,
        }))
    }

    /// Parse findOneAndUpdate operation
    pub fn parse_find_one_and_update(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let update = ArgParser::get_doc_arg(args, 1)?;
        let options = ArgParser::get_find_and_modify_options(args, 2)?;

        Ok(Command::Query(QueryCommand::FindOneAndUpdate {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse findOneAndReplace operation
    pub fn parse_find_one_and_replace(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = ArgParser::get_doc_arg(args, 0)?;
        let replacement = ArgParser::get_doc_arg(args, 1)?;
        let options = ArgParser::get_find_and_modify_options(args, 2)?;

        Ok(Command::Query(QueryCommand::FindOneAndReplace {
            collection: collection.to_string(),
            filter,
            replacement,
            options,
        }))
    }

    /// Parse distinct operation
    pub fn parse_distinct(collection: &str, args: &[Expr]) -> Result<Command> {
        let field = ArgParser::get_string_arg(args, 0)?;
        let filter = if args.len() > 1 {
            Some(ArgParser::get_doc_arg(args, 1)?)
        } else {
            None
        };

        Ok(Command::Query(QueryCommand::Distinct {
            collection: collection.to_string(),
            field,
            filter,
        }))
    }

    /// Parse bulkWrite operation
    pub fn parse_bulk_write(collection: &str, args: &[Expr]) -> Result<Command> {
        let operations = ArgParser::get_doc_array_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::BulkWrite {
            collection: collection.to_string(),
            operations,
            ordered: true,
        }))
    }

    /// Parse findAndModify operation
    pub fn parse_find_and_modify(collection: &str, args: &[Expr]) -> Result<Command> {
        // findAndModify takes a single document with all options
        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "findAndModify() requires an options document".to_string(),
            )
            .into());
        }

        if args.len() > 1 {
            return Err(ParseError::InvalidCommand(
                format!("findAndModify() expects 1 argument, got {}", args.len()),
            )
            .into());
        }

        // Parse the options document
        let options_doc = ArgParser::get_doc_arg(args, 0)?;

        // Extract query (defaults to empty document)
        let query = options_doc
            .get_document("query")
            .unwrap_or(&Document::new())
            .clone();

        // Extract sort (optional)
        let sort = options_doc.get_document("sort").ok().cloned();

        // Extract remove flag (defaults to false)
        let remove = options_doc
            .get_bool("remove")
            .unwrap_or(false);

        // Extract update (optional, but required if remove is false)
        let update = options_doc.get_document("update").ok().cloned();

        // Must specify either remove or update
        if !remove && update.is_none() {
            return Err(ParseError::InvalidCommand(
                "findAndModify() requires either 'remove: true' or an 'update' document".to_string(),
            )
            .into());
        }

        if remove && update.is_some() {
            return Err(ParseError::InvalidCommand(
                "findAndModify() cannot specify both 'remove' and 'update'".to_string(),
            )
            .into());
        }

        // Extract new flag (defaults to false)
        let new = options_doc
            .get_bool("new")
            .unwrap_or(false);

        // Extract fields/projection (optional)
        let fields = options_doc.get_document("fields").ok().cloned();

        // Extract upsert (defaults to false)
        let upsert = options_doc
            .get_bool("upsert")
            .unwrap_or(false);

        // Extract arrayFilters (optional)
        let array_filters = options_doc
            .get_array("arrayFilters")
            .ok()
            .and_then(|arr| {
                let docs: std::result::Result<Vec<Document>, ParseError> = arr
                    .iter()
                    .map(|v| v.as_document().ok_or_else(|| {
                        ParseError::InvalidCommand("arrayFilters must be an array of documents".to_string())
                    }).map(|d| d.clone()))
                    .collect();
                docs.ok()
            });

        // Extract maxTimeMS (optional)
        let max_time_ms = options_doc
            .get_i64("maxTimeMS")
            .ok()
            .map(|v| v as u64)
            .or_else(|| options_doc.get_i32("maxTimeMS").ok().map(|v| v as u64));

        // Extract collation (optional)
        let collation = options_doc.get_document("collation").ok().cloned();

        Ok(Command::Query(QueryCommand::FindAndModify {
            collection: collection.to_string(),
            query,
            sort,
            remove,
            update,
            new,
            fields,
            upsert,
            array_filters,
            max_time_ms,
            collation,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mongo_operation::DbOperationParser;

    #[test]
    fn test_parse_find_empty() {
        let result = DbOperationParser::parse("db.users.find()");
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::Find { .. }));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_find_with_filter() {
        let result = DbOperationParser::parse("db.users.find({ age: 25 })");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_find_with_operators() {
        let result = DbOperationParser::parse("db.users.find({ age: { $gt: 18 } })");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_insert_one() {
        let result = DbOperationParser::parse("db.users.insertOne({ name: 'Alice', age: 30 })");
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::InsertOne { .. }));
        }
    }

    #[test]
    fn test_parse_insert_many() {
        let result = DbOperationParser::parse("db.users.insertMany([{ name: 'Bob' }, { name: 'Charlie' }])");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_update_one() {
        let result = DbOperationParser::parse(
            "db.users.updateOne({ name: 'Alice' }, { $set: { age: 31 } })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::UpdateOne { .. }));
        }
    }

    #[test]
    fn test_parse_delete_one() {
        let result = DbOperationParser::parse("db.users.deleteOne({ name: 'Alice' })");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_aggregate() {
        let result = DbOperationParser::parse("db.orders.aggregate([{ $match: { status: 'pending' } }])");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_count_documents() {
        let result = DbOperationParser::parse("db.users.countDocuments({ age: { $gte: 18 } })");
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::CountDocuments { .. }));
        }
    }

    #[test]
    fn test_parse_count() {
        let result = DbOperationParser::parse("db.users.count({ active: true })");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_estimated_document_count() {
        let result = DbOperationParser::parse("db.users.estimatedDocumentCount()");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_distinct() {
        let result = DbOperationParser::parse("db.users.distinct('city')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_distinct_with_filter() {
        let result = DbOperationParser::parse(
            "db.users.distinct('city', { age: { $gte: 18 } })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(QueryCommand::Distinct { field, filter, .. })) = result {
            assert_eq!(field, "city");
            assert!(filter.is_some());
        }
    }

    #[test]
    fn test_parse_replace_one() {
        let result = DbOperationParser::parse(
            "db.users.replaceOne({ name: 'Alice' }, { name: 'Alice', age: 30 })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::ReplaceOne { .. }));
        }
    }

    #[test]
    fn test_parse_find_one_and_delete() {
        let result = DbOperationParser::parse("db.users.findOneAndDelete({ name: 'Bob' })");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_find_one_and_update() {
        let result = DbOperationParser::parse(
            "db.users.findOneAndUpdate({ name: 'Alice' }, { $set: { age: 31 } })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::FindOneAndUpdate { .. }));
        }
    }

    #[test]
    fn test_parse_find_one_and_replace() {
        let result = DbOperationParser::parse(
            "db.users.findOneAndReplace({ name: 'Alice' }, { name: 'Alice', age: 30 })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::FindOneAndReplace { .. }));
        }
    }

    #[test]
    fn test_parse_find_and_modify_update() {
        let result = DbOperationParser::parse(
            "db.users.findAndModify({ query: { name: 'Alice' }, update: { $set: { age: 31 } } })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::FindAndModify { .. }));
        }
    }

    #[test]
    fn test_parse_find_and_modify_remove() {
        let result = DbOperationParser::parse(
            "db.users.findAndModify({ query: { name: 'Bob' }, remove: true })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(QueryCommand::FindAndModify { remove, .. })) = result {
            assert!(remove);
        }
    }

    #[test]
    fn test_parse_find_and_modify_with_options() {
        let result = DbOperationParser::parse(
            "db.users.findAndModify({ query: { name: 'Alice' }, update: { $inc: { score: 1 } }, new: true, upsert: true })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(QueryCommand::FindAndModify { new, upsert, .. })) = result {
            assert!(new);
            assert!(upsert);
        }
    }
}
