//! Database operation parser using AST
//!
//! This module parses MongoDB database operations using Oxc AST parser.
//! It handles syntax like:
//! - db.collection.find({ query })
//! - db.collection.insertOne({ doc })
//! - db.collection.aggregate([{ $match: {} }])
//! etc.

use mongodb::bson::Document;
use oxc::allocator::Allocator;
use oxc::ast::ast::*;
use oxc::parser::Parser as OxcParser;
use oxc::span::SourceType;

use crate::error::{ParseError, Result};
use crate::parser::command::{
    AggregateOptions, Command, FindAndModifyOptions, FindOptions, QueryCommand, UpdateOptions,
};
use crate::parser::expr_converter::ExpressionConverter;

/// Represents a chained method call
#[derive(Debug)]
struct ChainMethod<'a> {
    name: String,
    args: &'a [Argument<'a>],
}

/// Parser for database operations
pub struct DbOperationParser;

impl DbOperationParser {
    /// Parse a database operation from input
    pub fn parse(input: &str) -> Result<Command> {
        let allocator = Allocator::default();
        let source_type = SourceType::default();
        let parser = OxcParser::new(&allocator, input, source_type);

        let ret = parser.parse();

        if !ret.errors.is_empty() {
            let error_msgs: Vec<String> = ret.errors.iter().map(|e| e.to_string()).collect();
            return Err(ParseError::SyntaxError(format!(
                "JavaScript syntax error: {}",
                error_msgs.join("; ")
            ))
            .into());
        }

        // Get the first statement
        let stmt = ret
            .program
            .body
            .first()
            .ok_or_else(|| ParseError::InvalidCommand("Empty input".to_string()))?;

        // Must be an expression statement
        if let Statement::ExpressionStatement(expr_stmt) = stmt {
            return Self::parse_expression(&expr_stmt.expression);
        }

        Err(ParseError::InvalidCommand("Expected expression statement".to_string()).into())
    }

    /// Parse an expression
    fn parse_expression(expr: &Expression) -> Result<Command> {
        match expr {
            Expression::CallExpression(call) => Self::parse_call_expression(call),
            _ => Err(ParseError::InvalidCommand(
                "Expected call expression (e.g., db.collection.find())".to_string(),
            )
            .into()),
        }
    }

    /// Parse a call expression: db.collection.operation(...) or chained calls
    fn parse_call_expression(call: &CallExpression) -> Result<Command> {
        // Check if this is a chained call (e.g., db.users.find().limit(10))
        if let Some((base_cmd, chain_methods)) = Self::try_parse_chained_call(call)? {
            // Apply chained methods to the base command
            return Self::apply_chain_methods(base_cmd, chain_methods);
        }

        // Not a chained call, parse as regular db.collection.operation()
        let (collection, operation) = Self::extract_db_call_target(&call.callee)?;
        let args = &call.arguments;

        // Route to specific operation parser based on operation name
        match operation.as_str() {
            "find" => Self::parse_find(&collection, args),
            "findOne" => Self::parse_find_one(&collection, args),
            "insertOne" => Self::parse_insert_one(&collection, args),
            "insertMany" => Self::parse_insert_many(&collection, args),
            "updateOne" => Self::parse_update_one(&collection, args),
            "updateMany" => Self::parse_update_many(&collection, args),
            "replaceOne" => Self::parse_replace_one(&collection, args),
            "deleteOne" => Self::parse_delete_one(&collection, args),
            "deleteMany" => Self::parse_delete_many(&collection, args),
            "aggregate" => Self::parse_aggregate(&collection, args),
            "count" => Self::parse_count_documents(&collection, args),
            "countDocuments" => Self::parse_count_documents(&collection, args),
            "estimatedDocumentCount" => Self::parse_estimated_document_count(&collection, args),
            "findOneAndDelete" => Self::parse_find_one_and_delete(&collection, args),
            "findOneAndUpdate" => Self::parse_find_one_and_update(&collection, args),
            "findOneAndReplace" => Self::parse_find_one_and_replace(&collection, args),
            "distinct" => Self::parse_distinct(&collection, args),
            "bulkWrite" => Self::parse_bulk_write(&collection, args),
            _ => Err(
                ParseError::InvalidCommand(format!("Unsupported operation: {}", operation)).into(),
            ),
        }
    }

    /// Try to parse a chained call like db.users.find().limit(10).skip(5)
    /// Returns (base_command, chain_methods) if this is a chained call
    fn try_parse_chained_call<'a>(
        call: &'a CallExpression<'a>,
    ) -> Result<Option<(Command, Vec<ChainMethod<'a>>)>> {
        // Check if the callee is itself a CallExpression (indicating a chain)
        if let Expression::StaticMemberExpression(member) = &call.callee
            && let Expression::CallExpression(_base_call) = &member.object
        {
            // This is a chained call!
            // Recursively parse the base call and collect chain methods
            let mut chain_methods = Vec::new();
            let base_cmd = Self::collect_chain_methods(call, &mut chain_methods)?;
            return Ok(Some((base_cmd, chain_methods)));
        }
        Ok(None)
    }

    /// Recursively collect all chained methods from innermost to outermost
    fn collect_chain_methods<'a>(
        call: &'a CallExpression<'a>,
        methods: &mut Vec<ChainMethod<'a>>,
    ) -> Result<Command> {
        // Check if callee is a member expression with a call as object
        if let Expression::StaticMemberExpression(member) = &call.callee {
            let method_name = member.property.name.to_string();

            if let Expression::CallExpression(inner_call) = &member.object {
                // Recursively process the inner call
                let base_cmd = Self::collect_chain_methods(inner_call, methods)?;

                // Add current method to the chain
                methods.push(ChainMethod {
                    name: method_name,
                    args: &call.arguments,
                });

                return Ok(base_cmd);
            }
        }

        // Base case: this should be db.collection.operation()
        Self::parse_call_expression_simple(call)
    }

    /// Parse a simple (non-chained) call expression
    fn parse_call_expression_simple(call: &CallExpression) -> Result<Command> {
        let (collection, operation) = Self::extract_db_call_target(&call.callee)?;
        let args = &call.arguments;

        match operation.as_str() {
            "find" => Self::parse_find(&collection, args),
            "findOne" => Self::parse_find_one(&collection, args),
            "aggregate" => Self::parse_aggregate(&collection, args),
            "countDocuments" => Self::parse_count_documents(&collection, args),
            _ => Err(ParseError::InvalidCommand(format!(
                "Operation '{}' does not support chaining",
                operation
            ))
            .into()),
        }
    }

    /// Apply chained methods to a base command
    fn apply_chain_methods<'a>(mut cmd: Command, methods: Vec<ChainMethod<'a>>) -> Result<Command> {
        for method in methods {
            cmd = Self::apply_single_chain_method(cmd, method)?;
        }
        Ok(cmd)
    }

    /// Apply a single chained method to a command
    fn apply_single_chain_method<'a>(cmd: Command, method: ChainMethod<'a>) -> Result<Command> {
        match cmd {
            Command::Query(query_cmd) => {
                let updated = Self::apply_chain_to_query(query_cmd, method)?;
                Ok(Command::Query(updated))
            }
            _ => Err(ParseError::InvalidCommand(
                "Chained methods only supported on query commands".to_string(),
            )
            .into()),
        }
    }

    /// Apply a chained method to a query command
    fn apply_chain_to_query<'a>(
        query_cmd: QueryCommand,
        method: ChainMethod<'a>,
    ) -> Result<QueryCommand> {
        match query_cmd {
            QueryCommand::Find {
                collection,
                filter,
                mut options,
            } => {
                Self::apply_find_chain_method(&mut options, &method)?;
                Ok(QueryCommand::Find {
                    collection,
                    filter,
                    options,
                })
            }
            QueryCommand::FindOne {
                collection,
                filter,
                mut options,
            } => {
                Self::apply_find_chain_method(&mut options, &method)?;
                Ok(QueryCommand::FindOne {
                    collection,
                    filter,
                    options,
                })
            }
            QueryCommand::Aggregate {
                collection,
                pipeline,
                mut options,
            } => {
                Self::apply_aggregate_chain_method(&mut options, &method)?;
                Ok(QueryCommand::Aggregate {
                    collection,
                    pipeline,
                    options,
                })
            }
            _ => Err(ParseError::InvalidCommand(
                "Chained methods not supported for this operation".to_string(),
            )
            .into()),
        }
    }

    /// Apply a chained method to FindOptions
    fn apply_find_chain_method<'a>(
        options: &mut FindOptions,
        method: &ChainMethod<'a>,
    ) -> Result<()> {
        match method.name.as_str() {
            "limit" => {
                if let Some(arg) = method.args.first()
                    && let Some(expr) = arg.as_expression()
                {
                    if let Expression::NumericLiteral(n) = expr {
                        options.limit = Some(n.value as i64);
                    } else {
                        return Err(ParseError::InvalidQuery(
                            "limit() requires a number argument".to_string(),
                        )
                        .into());
                    }
                }
            }
            "skip" => {
                if let Some(arg) = method.args.first()
                    && let Some(expr) = arg.as_expression()
                {
                    if let Expression::NumericLiteral(n) = expr {
                        options.skip = Some(n.value as u64);
                    } else {
                        return Err(ParseError::InvalidQuery(
                            "skip() requires a number argument".to_string(),
                        )
                        .into());
                    }
                }
            }
            "sort" => {
                if let Some(arg) = method.args.first()
                    && let Some(expr) = arg.as_expression()
                {
                    let bson = ExpressionConverter::expr_to_bson(expr)?;
                    if let mongodb::bson::Bson::Document(doc) = bson {
                        options.sort = Some(doc);
                    } else {
                        return Err(ParseError::InvalidQuery(
                            "sort() requires an object argument".to_string(),
                        )
                        .into());
                    }
                }
            }
            "projection" => {
                if let Some(arg) = method.args.first()
                    && let Some(expr) = arg.as_expression()
                {
                    let bson = ExpressionConverter::expr_to_bson(expr)?;
                    if let mongodb::bson::Bson::Document(doc) = bson {
                        options.projection = Some(doc);
                    } else {
                        return Err(ParseError::InvalidQuery(
                            "projection() requires an object argument".to_string(),
                        )
                        .into());
                    }
                }
            }
            "batchSize" => {
                if let Some(arg) = method.args.first()
                    && let Some(expr) = arg.as_expression()
                {
                    if let Expression::NumericLiteral(n) = expr {
                        options.batch_size = Some(n.value as u32);
                    } else {
                        return Err(ParseError::InvalidQuery(
                            "batchSize() requires a number argument".to_string(),
                        )
                        .into());
                    }
                }
            }
            _ => {
                return Err(ParseError::InvalidCommand(format!(
                    "Unknown chained method: {}",
                    method.name
                ))
                .into());
            }
        }
        Ok(())
    }

    /// Apply a chained method to AggregateOptions
    fn apply_aggregate_chain_method<'a>(
        options: &mut AggregateOptions,
        method: &ChainMethod<'a>,
    ) -> Result<()> {
        match method.name.as_str() {
            "allowDiskUse" => {
                options.allow_disk_use = true;
            }
            "batchSize" => {
                if let Some(arg) = method.args.first()
                    && let Some(expr) = arg.as_expression()
                {
                    if let Expression::NumericLiteral(n) = expr {
                        options.batch_size = Some(n.value as u32);
                    } else {
                        return Err(ParseError::InvalidQuery(
                            "batchSize() requires a number argument".to_string(),
                        )
                        .into());
                    }
                }
            }
            _ => {
                return Err(ParseError::InvalidCommand(format!(
                    "Unknown chained method for aggregate: {}",
                    method.name
                ))
                .into());
            }
        }
        Ok(())
    }

    /// Extract db.collection.operation from callee
    /// Returns (collection_name, operation_name)
    fn extract_db_call_target(callee: &Expression) -> Result<(String, String)> {
        // Must be a member expression: db.collection.operation
        if let Expression::StaticMemberExpression(member) = callee {
            let operation = member.property.name.to_string();

            // The object should be db.collection
            if let Expression::StaticMemberExpression(inner_member) = &member.object {
                let collection = inner_member.property.name.to_string();

                // The inner object should be 'db'
                if let Expression::Identifier(id) = &inner_member.object
                    && id.name == "db"
                {
                    return Ok((collection, operation));
                }
            }
        } else if let Expression::ComputedMemberExpression(member) = callee {
            // Handle db["collection"]["operation"] or db.collection["operation"]
            return Self::extract_computed_member_target(member);
        }

        Err(
            ParseError::InvalidCommand("Expected db.collection.operation() syntax".to_string())
                .into(),
        )
    }

    /// Extract from computed member expression
    fn extract_computed_member_target(
        member: &ComputedMemberExpression,
    ) -> Result<(String, String)> {
        // Get operation from expression (should be string literal)
        let operation = if let Expression::StringLiteral(s) = &member.expression {
            s.value.to_string()
        } else {
            return Err(ParseError::InvalidCommand(
                "Computed member expression must use string literal".to_string(),
            )
            .into());
        };

        // Check if object is db.collection
        if let Expression::StaticMemberExpression(inner) = &member.object {
            let collection = inner.property.name.to_string();
            if let Expression::Identifier(id) = &inner.object
                && id.name == "db"
            {
                return Ok((collection, operation));
            }
        }

        Err(ParseError::InvalidCommand(
            "Expected db.collection[\"operation\"]() syntax".to_string(),
        )
        .into())
    }

    /// Get argument at index as BSON document
    fn get_doc_arg(args: &[Argument], index: usize) -> Result<Document> {
        if let Some(arg) = args.get(index) {
            // Use as_expression() to get Expression from Argument
            if let Some(expr) = arg.as_expression() {
                let bson = ExpressionConverter::expr_to_bson(expr)?;
                if let mongodb::bson::Bson::Document(doc) = bson {
                    Ok(doc)
                } else {
                    Err(
                        ParseError::InvalidQuery(format!("Argument {} must be an object", index))
                            .into(),
                    )
                }
            } else {
                Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
            }
        } else {
            Ok(Document::new())
        }
    }

    /// Get argument at index as BSON array of documents
    fn get_doc_array_arg(args: &[Argument], index: usize) -> Result<Vec<Document>> {
        if let Some(arg) = args.get(index) {
            // Use as_expression() to get Expression from Argument
            if let Some(expr) = arg.as_expression() {
                let bson = ExpressionConverter::expr_to_bson(expr)?;
                if let mongodb::bson::Bson::Array(arr) = bson {
                    let mut docs = Vec::new();
                    for item in arr {
                        if let mongodb::bson::Bson::Document(doc) = item {
                            docs.push(doc);
                        } else {
                            return Err(ParseError::InvalidQuery(
                                "Array must contain only documents".to_string(),
                            )
                            .into());
                        }
                    }
                    Ok(docs)
                } else {
                    Err(
                        ParseError::InvalidQuery(format!("Argument {} must be an array", index))
                            .into(),
                    )
                }
            } else {
                Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Get argument at index as string
    fn get_string_arg(args: &[Argument], index: usize) -> Result<String> {
        if let Some(arg) = args.get(index) {
            // Use as_expression() to get Expression from Argument
            if let Some(expr) = arg.as_expression() {
                if let Expression::StringLiteral(s) = expr {
                    Ok(s.value.to_string())
                } else {
                    Err(
                        ParseError::InvalidQuery(format!("Argument {} must be a string", index))
                            .into(),
                    )
                }
            } else {
                Err(ParseError::InvalidQuery("Invalid argument type".to_string()).into())
            }
        } else {
            Err(ParseError::InvalidQuery(format!("Missing required argument {}", index)).into())
        }
    }

    // === CRUD Operation Parsers ===

    /// Parse find operation: db.collection.find(filter, options)
    fn parse_find(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let options = FindOptions::default(); // TODO: parse options from second arg

        Ok(Command::Query(QueryCommand::Find {
            collection: collection.to_string(),
            filter,
            options,
        }))
    }

    /// Parse findOne operation: db.collection.findOne(filter, options)
    fn parse_find_one(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let options = FindOptions::default();

        Ok(Command::Query(QueryCommand::FindOne {
            collection: collection.to_string(),
            filter,
            options,
        }))
    }

    /// Parse insertOne operation: db.collection.insertOne(document)
    fn parse_insert_one(collection: &str, args: &[Argument]) -> Result<Command> {
        let document = Self::get_doc_arg(args, 0)?;

        if document.is_empty() {
            return Err(ParseError::InvalidQuery(
                "insertOne requires a non-empty document".to_string(),
            )
            .into());
        }

        Ok(Command::Query(QueryCommand::InsertOne {
            collection: collection.to_string(),
            document,
        }))
    }

    /// Parse insertMany operation: db.collection.insertMany([documents])
    fn parse_insert_many(collection: &str, args: &[Argument]) -> Result<Command> {
        let documents = Self::get_doc_array_arg(args, 0)?;

        if documents.is_empty() {
            return Err(ParseError::InvalidQuery(
                "insertMany requires at least one document".to_string(),
            )
            .into());
        }

        Ok(Command::Query(QueryCommand::InsertMany {
            collection: collection.to_string(),
            documents,
        }))
    }

    /// Parse updateOne operation: db.collection.updateOne(filter, update, options)
    fn parse_update_one(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let update = Self::get_doc_arg(args, 1)?;
        let options = UpdateOptions::default();

        Ok(Command::Query(QueryCommand::UpdateOne {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse updateMany operation: db.collection.updateMany(filter, update, options)
    fn parse_update_many(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let update = Self::get_doc_arg(args, 1)?;
        let options = UpdateOptions::default();

        Ok(Command::Query(QueryCommand::UpdateMany {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse replaceOne operation: db.collection.replaceOne(filter, replacement, options)
    fn parse_replace_one(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let replacement = Self::get_doc_arg(args, 1)?;
        let options = UpdateOptions::default();

        Ok(Command::Query(QueryCommand::ReplaceOne {
            collection: collection.to_string(),
            filter,
            replacement,
            options,
        }))
    }

    /// Parse deleteOne operation: db.collection.deleteOne(filter)
    fn parse_delete_one(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::DeleteOne {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse deleteMany operation: db.collection.deleteMany(filter)
    fn parse_delete_many(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::DeleteMany {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse aggregate operation: db.collection.aggregate(pipeline, options)
    fn parse_aggregate(collection: &str, args: &[Argument]) -> Result<Command> {
        let pipeline = Self::get_doc_array_arg(args, 0)?;
        let options = AggregateOptions::default();

        Ok(Command::Query(QueryCommand::Aggregate {
            collection: collection.to_string(),
            pipeline,
            options,
        }))
    }

    /// Parse countDocuments operation: db.collection.countDocuments(filter)
    fn parse_count_documents(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::CountDocuments {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse estimatedDocumentCount operation
    fn parse_estimated_document_count(collection: &str, _args: &[Argument]) -> Result<Command> {
        Ok(Command::Query(QueryCommand::EstimatedDocumentCount {
            collection: collection.to_string(),
        }))
    }

    /// Parse findOneAndDelete operation
    fn parse_find_one_and_delete(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let options = FindAndModifyOptions::default();

        Ok(Command::Query(QueryCommand::FindOneAndDelete {
            collection: collection.to_string(),
            filter,
            options,
        }))
    }

    /// Parse findOneAndUpdate operation
    fn parse_find_one_and_update(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let update = Self::get_doc_arg(args, 1)?;
        let options = FindAndModifyOptions::default();

        Ok(Command::Query(QueryCommand::FindOneAndUpdate {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse findOneAndReplace operation
    fn parse_find_one_and_replace(collection: &str, args: &[Argument]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let replacement = Self::get_doc_arg(args, 1)?;
        let options = FindAndModifyOptions::default();

        Ok(Command::Query(QueryCommand::FindOneAndReplace {
            collection: collection.to_string(),
            filter,
            replacement,
            options,
        }))
    }

    /// Parse distinct operation: db.collection.distinct(field, filter)
    fn parse_distinct(collection: &str, args: &[Argument]) -> Result<Command> {
        let field = Self::get_string_arg(args, 0)?;
        let filter = if args.len() > 1 {
            Some(Self::get_doc_arg(args, 1)?)
        } else {
            None
        };

        Ok(Command::Query(QueryCommand::Distinct {
            collection: collection.to_string(),
            field,
            filter,
        }))
    }

    /// Parse bulkWrite operation: db.collection.bulkWrite(operations, options)
    fn parse_bulk_write(collection: &str, args: &[Argument]) -> Result<Command> {
        let operations = Self::get_doc_array_arg(args, 0)?;
        let ordered = true; // Default to ordered

        Ok(Command::Query(QueryCommand::BulkWrite {
            collection: collection.to_string(),
            operations,
            ordered,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_find_empty() {
        let result = DbOperationParser::parse("db.users.find()").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert!(filter.is_empty());
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_find_with_filter() {
        let result = DbOperationParser::parse("db.users.find({ age: 25 })").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_i64("age").unwrap(), 25);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_find_with_operators() {
        let result = DbOperationParser::parse("db.users.find({ age: { $gt: 18 } })").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = result
        {
            assert_eq!(collection, "users");
            let age_cond = filter.get_document("age").unwrap();
            assert_eq!(age_cond.get_i64("$gt").unwrap(), 18);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_insert_one() {
        let result =
            DbOperationParser::parse("db.users.insertOne({ name: 'Alice', age: 30 })").unwrap();
        if let Command::Query(QueryCommand::InsertOne {
            collection,
            document,
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(document.get_str("name").unwrap(), "Alice");
            assert_eq!(document.get_i64("age").unwrap(), 30);
        } else {
            panic!("Expected InsertOne command");
        }
    }

    #[test]
    fn test_parse_insert_many() {
        let result =
            DbOperationParser::parse("db.users.insertMany([{ name: 'Alice' }, { name: 'Bob' }])")
                .unwrap();
        if let Command::Query(QueryCommand::InsertMany {
            collection,
            documents,
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(documents.len(), 2);
            assert_eq!(documents[0].get_str("name").unwrap(), "Alice");
            assert_eq!(documents[1].get_str("name").unwrap(), "Bob");
        } else {
            panic!("Expected InsertMany command");
        }
    }

    #[test]
    fn test_parse_update_one() {
        let result = DbOperationParser::parse(
            "db.users.updateOne({ name: 'Alice' }, { $set: { age: 31 } })",
        )
        .unwrap();
        if let Command::Query(QueryCommand::UpdateOne {
            collection,
            filter,
            update,
            ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
            let set_doc = update.get_document("$set").unwrap();
            assert_eq!(set_doc.get_i64("age").unwrap(), 31);
        } else {
            panic!("Expected UpdateOne command");
        }
    }

    #[test]
    fn test_parse_delete_one() {
        let result = DbOperationParser::parse("db.users.deleteOne({ name: 'Alice' })").unwrap();
        if let Command::Query(QueryCommand::DeleteOne {
            collection, filter, ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
        } else {
            panic!("Expected DeleteOne command");
        }
    }

    #[test]
    fn test_parse_aggregate() {
        let result = DbOperationParser::parse(
            "db.users.aggregate([{ $match: { age: { $gt: 18 } } }, { $group: { _id: '$city' } }])",
        )
        .unwrap();
        if let Command::Query(QueryCommand::Aggregate {
            collection,
            pipeline,
            ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(pipeline.len(), 2);
        } else {
            panic!("Expected Aggregate command");
        }
    }

    #[test]
    fn test_parse_count_documents() {
        let result =
            DbOperationParser::parse("db.users.countDocuments({ age: { $gte: 18 } })").unwrap();
        if let Command::Query(QueryCommand::CountDocuments {
            collection, filter, ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert!(!filter.is_empty());
        } else {
            panic!("Expected CountDocuments command");
        }
    }

    #[test]
    fn test_parse_count() {
        // Test count() as alias for countDocuments()
        let result = DbOperationParser::parse("db.users.count({ age: { $gte: 18 } })").unwrap();
        if let Command::Query(QueryCommand::CountDocuments {
            collection, filter, ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert!(!filter.is_empty());
        } else {
            panic!("Expected CountDocuments command");
        }

        // Test count() with empty filter
        let result = DbOperationParser::parse("db.users.count({})").unwrap();
        if let Command::Query(QueryCommand::CountDocuments {
            collection, filter, ..
        }) = result
        {
            assert_eq!(collection, "users");
            assert!(filter.is_empty());
        } else {
            panic!("Expected CountDocuments command");
        }
    }

    #[test]
    fn test_parse_chained_limit() {
        let result = DbOperationParser::parse("db.users.find().limit(10)").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection,
            filter,
            options,
        }) = result
        {
            assert_eq!(collection, "users");
            assert!(filter.is_empty());
            assert_eq!(options.limit, Some(10));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_chained_skip() {
        let result = DbOperationParser::parse("db.users.find().skip(5)").unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = result {
            assert_eq!(options.skip, Some(5));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_chained_sort() {
        let result = DbOperationParser::parse("db.users.find().sort({ name: 1 })").unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = result {
            assert!(options.sort.is_some());
            let sort = options.sort.unwrap();
            assert_eq!(sort.get_i64("name").unwrap(), 1);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_multiple_chained_methods() {
        let result =
            DbOperationParser::parse("db.users.find({ age: { $gt: 18 } }).limit(10).skip(5)")
                .unwrap();
        if let Command::Query(QueryCommand::Find {
            collection,
            filter,
            options,
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(options.limit, Some(10));
            assert_eq!(options.skip, Some(5));
            let age_cond = filter.get_document("age").unwrap();
            assert_eq!(age_cond.get_i64("$gt").unwrap(), 18);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_complex_chain() {
        let result = DbOperationParser::parse(
            "db.users.find({ status: 'active' }).sort({ createdAt: -1 }).limit(20).skip(10)",
        )
        .unwrap();
        if let Command::Query(QueryCommand::Find {
            collection,
            filter,
            options,
        }) = result
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("status").unwrap(), "active");
            assert_eq!(options.limit, Some(20));
            assert_eq!(options.skip, Some(10));
            assert!(options.sort.is_some());
        } else {
            panic!("Expected Find command");
        }
    }
}
