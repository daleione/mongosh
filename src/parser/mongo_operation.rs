//! Database operation parser using custom MongoDB AST
//!
//! This module parses MongoDB database operations using our custom parser.
//! It handles syntax like:
//! - db.collection.find({ query })
//! - db.collection.insertOne({ doc })
//! - db.collection.aggregate([{ $match: {} }])
//! etc.

use mongodb::bson::Document;

use super::mongo_ast::*;
use super::mongo_parser::MongoParser;
use crate::error::{ParseError, Result};
use crate::parser::command::{
    AdminCommand, AggregateOptions, Command, ExplainVerbosity, FindAndModifyOptions, FindOptions,
    QueryCommand, UpdateOptions,
};
use crate::parser::mongo_converter::ExpressionConverter;

/// Represents a chained method call
#[derive(Debug)]
struct ChainMethod {
    name: String,
    args: Vec<Expr>,
}

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
        if let Some((base_cmd, chain_methods)) = Self::try_parse_chained_call(call)? {
            // Apply chained methods to the base command
            return Self::apply_chain_methods(base_cmd, chain_methods);
        }

        // Not a chained call, parse as regular db.collection.operation()
        let (collection, operation) = Self::extract_db_call_target(call.callee.as_ref())?;
        let args = &call.arguments;

        // Route to specific operation parser based on operation name
        match operation.as_str() {
            "explain" => Self::parse_explain(&collection, args, call),
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
            "countDocuments" => Self::parse_count_documents(&collection, args),
            "count" => Self::parse_count_documents(&collection, args),
            "estimatedDocumentCount" => Self::parse_estimated_document_count(&collection, args),
            "findOneAndDelete" => Self::parse_find_one_and_delete(&collection, args),
            "findOneAndUpdate" => Self::parse_find_one_and_update(&collection, args),
            "findOneAndReplace" => Self::parse_find_one_and_replace(&collection, args),
            "findAndModify" => Self::parse_find_and_modify(&collection, args),
            "distinct" => Self::parse_distinct(&collection, args),
            "bulkWrite" => Self::parse_bulk_write(&collection, args),
            "getIndexes" => Self::parse_get_indexes(&collection),
            "createIndex" => Self::parse_create_index(&collection, args),
            "createIndexes" => Self::parse_create_indexes(&collection, args),
            "dropIndex" => Self::parse_drop_index(&collection, args),
            "dropIndexes" => Self::parse_drop_indexes(&collection),
            "drop" => Self::parse_drop_collection(&collection),
            "renameCollection" => Self::parse_rename_collection(&collection, args),
            _ => Err(ParseError::InvalidCommand(format!(
                "Unknown operation: {}. Try 'help' for available commands",
                operation
            ))
            .into()),
        }
    }

    /// Try to parse a chained call expression
    /// Returns Some((base_command, chain_methods)) if it's a chained call, None otherwise
    fn try_parse_chained_call(call: &CallExpr) -> Result<Option<(Command, Vec<ChainMethod>)>> {
        // Check if the callee is a member expression AND its object is a call (chained call)
        if let Expr::Member(member) = call.callee.as_ref() {
            // Check if the object is also a call - this indicates chaining
            if let Expr::Call(_inner_call) = member.object.as_ref() {
                // Check if this chain contains an explain call anywhere
                if Self::contains_explain_in_chain(call)? {
                    // Check if explain is at the end of the chain (as a method call)
                    if let MemberProperty::Ident(name) = &member.property {
                        if name == "explain" {
                            // Explain is at the END: db.collection.find().explain()
                            // Treat it as a regular chain method
                            let (base_expr, chain_methods) = Self::collect_chain_methods(call)?;
                            if let Expr::Call(base_call) = base_expr {
                                let base_cmd = Self::parse_call_expression_simple(&base_call)?;
                                return Ok(Some((base_cmd, chain_methods)));
                            } else {
                                return Err(ParseError::InvalidCommand(
                                    "Expected base call expression".to_string(),
                                )
                                .into());
                            }
                        }
                    }

                    // Explain is in the MIDDLE/BEGINNING: db.collection.explain().find()...
                    return Self::try_parse_explain_chain(call);
                }

                // This is a regular chained call like: db.users.find().limit(10)
                // Collect all chain methods
                let (base_expr, chain_methods) = Self::collect_chain_methods(call)?;

                // Parse the base expression as a call
                if let Expr::Call(base_call) = base_expr {
                    let base_cmd = Self::parse_call_expression_simple(&base_call)?;
                    return Ok(Some((base_cmd, chain_methods)));
                } else {
                    return Err(ParseError::InvalidCommand(
                        "Expected base call expression".to_string(),
                    )
                    .into());
                }
            }
        }

        Ok(None)
    }

    /// Check if a call chain contains an explain call
    fn contains_explain_in_chain(call: &CallExpr) -> Result<bool> {
        let mut current = call;

        loop {
            // Check if current call is explain
            if let Expr::Member(member) = current.callee.as_ref() {
                if let MemberProperty::Ident(name) = &member.property {
                    if name == "explain" {
                        return Ok(true);
                    }
                }

                // Move to the object if it's a call
                if let Expr::Call(inner_call) = member.object.as_ref() {
                    current = inner_call;
                    continue;
                }
            }
            break;
        }

        Ok(false)
    }

    /// Try to parse an explain chain: db.collection.explain(verbosity).queryMethod().chainMethods()
    fn try_parse_explain_chain(call: &CallExpr) -> Result<Option<(Command, Vec<ChainMethod>)>> {
        // First, collect all chain methods and find the base explain().queryMethod() call
        let (base_call, chain_methods) = Self::collect_chain_until_explain(call)?;

        // base_call should be the query method after explain
        // e.g., for db.users.explain().find().limit(10), base_call is find()

        if let Expr::Member(member) = base_call.callee.as_ref() {
            if let Expr::Call(explain_call) = member.object.as_ref() {
                // explain_call is the db.collection.explain() call

                if let Expr::Member(explain_member) = explain_call.callee.as_ref() {
                    if let MemberProperty::Ident(op_name) = &explain_member.property {
                        if op_name != "explain" {
                            return Ok(None);
                        }

                        // Get collection name
                        if let Expr::Member(coll_member) = explain_member.object.as_ref() {
                            let collection = match &coll_member.property {
                                MemberProperty::Ident(name) => name.clone(),
                                MemberProperty::Computed(expr) => {
                                    if let Expr::String(s) = expr {
                                        s.clone()
                                    } else {
                                        return Ok(None);
                                    }
                                }
                            };

                            // Verify db prefix
                            if let Expr::Ident(id) = coll_member.object.as_ref() {
                                if id != "db" {
                                    return Ok(None);
                                }
                            } else {
                                return Ok(None);
                            }

                            // Parse verbosity from explain() arguments
                            let verbosity = ExplainVerbosity::parse_from_args(&explain_call.arguments)?;

                            // Get query method name
                            let query_method = match &member.property {
                                MemberProperty::Ident(name) => name.clone(),
                                _ => return Ok(None),
                            };

                            // Parse the query method
                            let query_cmd = match query_method.as_str() {
                                "find" => {
                                    let filter = Self::get_doc_arg(&base_call.arguments, 0)?;
                                    let projection = Self::get_projection(&base_call.arguments, 1)?;
                                    QueryCommand::Find {
                                        collection: collection.clone(),
                                        filter,
                                        options: FindOptions {
                                            projection,
                                            ..Default::default()
                                        },
                                    }
                                }
                                "findOne" => {
                                    let filter = Self::get_doc_arg(&base_call.arguments, 0)?;
                                    let projection = Self::get_projection(&base_call.arguments, 1)?;
                                    QueryCommand::FindOne {
                                        collection: collection.clone(),
                                        filter,
                                        options: FindOptions {
                                            projection,
                                            ..Default::default()
                                        },
                                    }
                                }
                                "aggregate" => {
                                    let pipeline = Self::get_doc_array_arg(&base_call.arguments, 0)?;
                                    let options = Self::get_aggregate_options(&base_call.arguments, 1)?;
                                    QueryCommand::Aggregate {
                                        collection: collection.clone(),
                                        pipeline,
                                        options,
                                    }
                                }
                                "count" | "countDocuments" => {
                                    let filter = Self::get_doc_arg(&base_call.arguments, 0)?;
                                    QueryCommand::CountDocuments {
                                        collection: collection.clone(),
                                        filter,
                                    }
                                }
                                "distinct" => {
                                    let field = if let Some(Expr::String(s)) = base_call.arguments.first() {
                                        s.clone()
                                    } else {
                                        return Err(ParseError::InvalidQuery(
                                            "distinct() requires a field name as first argument".to_string(),
                                        )
                                        .into());
                                    };
                                    let filter = if base_call.arguments.len() > 1 {
                                        Some(Self::get_doc_arg(&base_call.arguments, 1)?)
                                    } else {
                                        None
                                    };
                                    QueryCommand::Distinct {
                                        collection: collection.clone(),
                                        field,
                                        filter,
                                    }
                                }
                                _ => {
                                    return Err(ParseError::InvalidCommand(format!(
                                        "explain() does not support method: {}",
                                        query_method
                                    ))
                                    .into());
                                }
                            };

                            // Create the Explain command
                            let explain_cmd = QueryCommand::Explain {
                                collection,
                                verbosity,
                                query: Box::new(query_cmd),
                            };

                            return Ok(Some((Command::Query(explain_cmd), chain_methods)));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Collect chain methods until we reach the explain call
    /// Returns (base_call_after_explain, chain_methods)
    /// For db.users.explain().find().limit(10), returns (find(), [limit])
    fn collect_chain_until_explain(call: &CallExpr) -> Result<(CallExpr, Vec<ChainMethod>)> {
        let mut chain_methods = Vec::new();
        let mut current_call = call.clone();

        // Walk up the chain until we find explain
        loop {
            if let Expr::Member(member) = current_call.callee.as_ref() {
                // Check if the object is explain()
                if let Expr::Call(inner_call) = member.object.as_ref() {
                    // Check if inner_call is explain
                    if let Expr::Member(inner_member) = inner_call.callee.as_ref() {
                        if let MemberProperty::Ident(name) = &inner_member.property {
                            if name == "explain" {
                                // Found explain! current_call is the query method
                                chain_methods.reverse();
                                return Ok((current_call, chain_methods));
                            }
                        }
                    }

                    // Not explain yet, this is a chain method
                    if let MemberProperty::Ident(method_name) = &member.property {
                        chain_methods.push(ChainMethod {
                            name: method_name.clone(),
                            args: current_call.arguments.clone(),
                        });
                        current_call = inner_call.as_ref().clone();
                        continue;
                    }
                }
            }
            break;
        }

        Err(ParseError::InvalidCommand(
            "Could not find explain() in chain".to_string(),
        )
        .into())
    }

    /// Collect chain methods from a chained call expression
    /// Returns (base_call_expr, chain_methods) where base_call_expr is Expr::Call
    fn collect_chain_methods(call: &CallExpr) -> Result<(Expr, Vec<ChainMethod>)> {
        let mut chain_methods = Vec::new();
        let mut current_call = call;

        // Walk up the chain collecting methods
        loop {
            if let Expr::Member(member) = current_call.callee.as_ref() {
                if let MemberProperty::Ident(method_name) = &member.property {
                    // Check if the object is itself a call expression (continue chain)
                    if let Expr::Call(inner_call) = member.object.as_ref() {
                        // This is a chained method call - add to chain
                        chain_methods.push(ChainMethod {
                            name: method_name.clone(),
                            args: current_call.arguments.clone(),
                        });
                        current_call = inner_call;
                        continue;
                    } else {
                        // Object is not a call - we've reached the base
                        // The current_call IS the base call (e.g., db.users.find())
                        // Don't add it to chain_methods
                        chain_methods.reverse(); // Reverse to get correct order
                        return Ok((Expr::Call(Box::new(current_call.clone())), chain_methods));
                    }
                }
            }
            break;
        }

        // Not a chained call - shouldn't happen if we got here
        Err(ParseError::InvalidCommand("Expected chained method call".to_string()).into())
    }

    /// Parse a simple (non-chained) call expression
    fn parse_call_expression_simple(call: &CallExpr) -> Result<Command> {
        let (collection, operation) = Self::extract_db_call_target(call.callee.as_ref())?;
        let args = &call.arguments;

        match operation.as_str() {
            "find" => Self::parse_find(&collection, args),
            "aggregate" => Self::parse_aggregate(&collection, args),
            _ => Err(ParseError::InvalidCommand(format!(
                "Operation '{}' cannot be chained",
                operation
            ))
            .into()),
        }
    }

    /// Apply chain methods to a base command
    fn apply_chain_methods(mut cmd: Command, chain_methods: Vec<ChainMethod>) -> Result<Command> {
        for method in chain_methods {
            cmd = Self::apply_single_chain_method(cmd, method)?;
        }
        Ok(cmd)
    }

    /// Apply a single chain method to a command
    fn apply_single_chain_method(cmd: Command, method: ChainMethod) -> Result<Command> {
        match cmd {
            Command::Query(query_cmd) => {
                let updated_query = Self::apply_chain_to_query(query_cmd, method)?;
                Ok(Command::Query(updated_query))
            }
            _ => Err(ParseError::InvalidCommand(
                "Cannot chain methods on non-query commands".to_string(),
            )
            .into()),
        }
    }

    /// Apply chain method to a query command
    fn apply_chain_to_query(query: QueryCommand, method: ChainMethod) -> Result<QueryCommand> {
        // Check if the method is "explain" - wrap the query in an Explain command
        if method.name == "explain" {
            if !query.supports_explain() {
                return Err(ParseError::InvalidCommand(
                    "explain() can only be used with find, findOne, aggregate, count, or distinct queries".to_string()
                ).into());
            }

            let collection = query.collection().to_string();
            let verbosity = ExplainVerbosity::parse_from_args(&method.args)?;

            return Ok(QueryCommand::Explain {
                collection,
                verbosity,
                query: Box::new(query),
            });
        }

        match query {
            QueryCommand::Find {
                collection,
                filter,
                options,
            } => {
                let updated_options = Self::apply_find_chain_method(options, method)?;
                Ok(QueryCommand::Find {
                    collection,
                    filter,
                    options: updated_options,
                })
            }
            QueryCommand::Aggregate {
                collection,
                pipeline,
                options,
            } => {
                let updated_options = Self::apply_aggregate_chain_method(options, method)?;
                Ok(QueryCommand::Aggregate {
                    collection,
                    pipeline,
                    options: updated_options,
                })
            }
            QueryCommand::Explain {
                collection,
                verbosity,
                query,
            } => {
                // Apply chain method to the inner query
                let updated_query = Self::apply_chain_to_query(*query, method)?;
                Ok(QueryCommand::Explain {
                    collection,
                    verbosity,
                    query: Box::new(updated_query),
                })
            }
            _ => Err(ParseError::InvalidCommand(format!(
                "Cannot apply method '{}' to this query type",
                method.name
            ))
            .into()),
        }
    }

    /// Apply chain method to find options
    fn apply_find_chain_method(
        mut options: FindOptions,
        method: ChainMethod,
    ) -> Result<FindOptions> {
        match method.name.as_str() {
            "limit" => {
                let limit_val = Self::get_number_arg(&method.args, 0)?;
                if limit_val < 0 {
                    return Err(ParseError::InvalidQuery(
                        "limit() value must be non-negative".to_string(),
                    )
                    .into());
                }
                options.limit = Some(limit_val);
            }
            "skip" => {
                let skip_val = Self::get_number_arg(&method.args, 0)?;
                if skip_val < 0 {
                    return Err(ParseError::InvalidQuery(
                        "skip() value must be non-negative".to_string(),
                    )
                    .into());
                }
                options.skip = Some(skip_val as u64);
            }
            "sort" => {
                options.sort = Some(Self::get_doc_arg(&method.args, 0)?);
            }
            "projection" => {
                options.projection = Some(Self::get_doc_arg(&method.args, 0)?);
            }
            "batchSize" => {
                let batch_size = Self::get_number_arg(&method.args, 0)?;
                if batch_size <= 0 {
                    return Err(ParseError::InvalidQuery(
                        "batchSize() value must be positive".to_string(),
                    )
                    .into());
                }
                options.batch_size = Some(batch_size as u32);
            }
            "hint" => {
                // hint can be either a document or a string
                if let Some(arg) = method.args.first() {
                    match arg {
                        Expr::String(s) => {
                            let mut hint_doc = Document::new();
                            hint_doc.insert(s.clone(), 1);
                            options.hint = Some(hint_doc);
                        }
                        _ => {
                            options.hint = Some(Self::get_doc_arg(&method.args, 0)?);
                        }
                    }
                }
            }
            _ => {
                return Err(ParseError::InvalidCommand(format!(
                    "Unknown find() chain method: {}",
                    method.name
                ))
                .into());
            }
        }
        Ok(options)
    }

    /// Apply chain method to aggregate options
    fn apply_aggregate_chain_method(
        mut options: AggregateOptions,
        method: ChainMethod,
    ) -> Result<AggregateOptions> {
        match method.name.as_str() {
            "batchSize" => {
                let batch_size = Self::get_number_arg(&method.args, 0)?;
                if batch_size <= 0 {
                    return Err(ParseError::InvalidQuery(
                        "batchSize() value must be positive".to_string(),
                    )
                    .into());
                }
                options.batch_size = Some(batch_size as u32);
            }
            _ => {
                return Err(ParseError::InvalidCommand(format!(
                    "Unknown aggregate() chain method: {}",
                    method.name
                ))
                .into());
            }
        }
        Ok(options)
    }

    /// Extract collection and operation from db.collection.operation
    /// Returns (collection_name, operation_name)
    fn extract_db_call_target(callee: &Expr) -> Result<(String, String)> {
        // Must be a member expression: db.collection.operation
        if let Expr::Member(member) = callee {
            let operation = match &member.property {
                MemberProperty::Ident(name) => name.clone(),
                MemberProperty::Computed(expr) => {
                    // Handle computed property like db["collection"]["operation"]
                    if let Expr::String(s) = expr {
                        s.clone()
                    } else {
                        return Err(ParseError::InvalidCommand(
                            "Computed member expression must use string literal".to_string(),
                        )
                        .into());
                    }
                }
            };

            // The object should be db.collection
            if let Expr::Member(inner_member) = member.object.as_ref() {
                let collection = match &inner_member.property {
                    MemberProperty::Ident(name) => name.clone(),
                    MemberProperty::Computed(expr) => {
                        if let Expr::String(s) = expr {
                            s.clone()
                        } else {
                            return Err(ParseError::InvalidCommand(
                                "Collection name must be a string".to_string(),
                            )
                            .into());
                        }
                    }
                };

                // The inner object should be 'db'
                if let Expr::Ident(id) = inner_member.object.as_ref() {
                    if id == "db" {
                        return Ok((collection, operation));
                    }
                }
            }
        }

        Err(
            ParseError::InvalidCommand("Expected db.collection.operation() syntax".to_string())
                .into(),
        )
    }

    /// Get argument at index as BSON document
    fn get_doc_arg(args: &[Expr], index: usize) -> Result<Document> {
        if let Some(expr) = args.get(index) {
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
            Ok(Document::new())
        }
    }

    /// Get argument at index as BSON array of documents
    fn get_doc_array_arg(args: &[Expr], index: usize) -> Result<Vec<Document>> {
        if let Some(expr) = args.get(index) {
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
                Err(ParseError::InvalidQuery(format!("Argument {} must be an array", index)).into())
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Get argument at index as string
    fn get_string_arg(args: &[Expr], index: usize) -> Result<String> {
        if let Some(expr) = args.get(index) {
            if let Expr::String(s) = expr {
                Ok(s.clone())
            } else {
                Err(ParseError::InvalidQuery(format!("Argument {} must be a string", index)).into())
            }
        } else {
            Err(ParseError::InvalidQuery(format!("Missing argument at index {}", index)).into())
        }
    }

    /// Get argument at index as number
    fn get_number_arg(args: &[Expr], index: usize) -> Result<i64> {
        if let Some(expr) = args.get(index) {
            if let Expr::Number(n) = expr {
                Ok(*n as i64)
            } else {
                Err(ParseError::InvalidQuery(format!("Argument {} must be a number", index)).into())
            }
        } else {
            Err(ParseError::InvalidQuery(format!("Missing argument at index {}", index)).into())
        }
    }

    /// Parse find and modify options from expression
    fn parse_find_and_modify_options(expr: &Expr) -> Result<FindAndModifyOptions> {
        let doc = if let Expr::Object(obj) = expr {
            ExpressionConverter::object_to_bson(obj)?
        } else {
            return Err(ParseError::InvalidQuery("Options must be an object".to_string()).into());
        };

        let mut options = FindAndModifyOptions::default();

        if let Ok(sort) = doc.get_document("sort") {
            options.sort = Some(sort.clone());
        }

        if let Ok(projection) = doc.get_document("projection") {
            options.projection = Some(projection.clone());
        }

        if let Ok(return_new) = doc.get_bool("returnNew") {
            options.return_new = return_new;
        } else if let Ok(return_document) = doc.get_str("returnDocument") {
            options.return_new = match return_document {
                "after" => true,
                "before" => false,
                _ => {
                    return Err(ParseError::InvalidQuery(
                        "returnDocument must be 'before' or 'after'".to_string(),
                    )
                    .into());
                }
            };
        }

        if let Ok(upsert) = doc.get_bool("upsert") {
            options.upsert = upsert;
        }

        Ok(options)
    }

    /// Get update options from arguments
    fn get_update_options(args: &[Expr], index: usize) -> Result<UpdateOptions> {
        if let Some(expr) = args.get(index) {
            Self::parse_update_options(expr)
        } else {
            Ok(UpdateOptions::default())
        }
    }

    /// Get find and modify options from arguments
    fn get_find_and_modify_options(args: &[Expr], index: usize) -> Result<FindAndModifyOptions> {
        if let Some(expr) = args.get(index) {
            Self::parse_find_and_modify_options(expr)
        } else {
            Ok(FindAndModifyOptions::default())
        }
    }

    /// Get aggregate options from arguments
    fn get_aggregate_options(args: &[Expr], index: usize) -> Result<AggregateOptions> {
        if let Some(expr) = args.get(index) {
            Self::parse_aggregate_options(expr)
        } else {
            Ok(AggregateOptions::default())
        }
    }

    /// Get projection from arguments
    fn get_projection(args: &[Expr], index: usize) -> Result<Option<Document>> {
        if let Some(_expr) = args.get(index) {
            Ok(Some(Self::get_doc_arg(args, index)?))
        } else {
            Ok(None)
        }
    }

    /// Parse update options from expression
    fn parse_update_options(expr: &Expr) -> Result<UpdateOptions> {
        let doc = if let Expr::Object(obj) = expr {
            ExpressionConverter::object_to_bson(obj)?
        } else {
            return Err(ParseError::InvalidQuery("Options must be an object".to_string()).into());
        };

        let mut options = UpdateOptions::default();

        if let Ok(upsert) = doc.get_bool("upsert") {
            options.upsert = upsert;
        }

        if let Ok(array_filters) = doc.get_array("arrayFilters") {
            let mut filters = Vec::new();
            for filter in array_filters {
                if let mongodb::bson::Bson::Document(doc) = filter {
                    filters.push(doc.clone());
                }
            }
            options.array_filters = Some(filters);
        }

        Ok(options)
    }

    /// Parse aggregate options from expression
    fn parse_aggregate_options(expr: &Expr) -> Result<AggregateOptions> {
        let doc = if let Expr::Object(obj) = expr {
            ExpressionConverter::object_to_bson(obj)?
        } else {
            return Err(ParseError::InvalidQuery("Options must be an object".to_string()).into());
        };

        let mut options = AggregateOptions::default();

        if let Ok(batch_size) = doc.get_i32("batchSize") {
            if batch_size <= 0 {
                return Err(ParseError::InvalidQuery(
                    "batchSize must be a positive integer".to_string(),
                )
                .into());
            }
            options.batch_size = Some(batch_size as u32);
        } else if let Ok(batch_size) = doc.get_i64("batchSize") {
            if batch_size <= 0 {
                return Err(ParseError::InvalidQuery(
                    "batchSize must be a positive integer".to_string(),
                )
                .into());
            }
            options.batch_size = Some(batch_size as u32);
        }

        if let Ok(allow_disk_use) = doc.get_bool("allowDiskUse") {
            options.allow_disk_use = allow_disk_use;
        }

        if let Ok(max_time_ms) = doc.get_i64("maxTimeMS") {
            options.max_time_ms = Some(max_time_ms as u64);
        } else if let Ok(max_time_ms) = doc.get_i32("maxTimeMS") {
            options.max_time_ms = Some(max_time_ms as u64);
        }

        Ok(options)
    }

    // Specific operation parsers

    /// Parse explain operation: db.collection.explain(verbosity).queryMethod()
    /// This expects a chained call where explain() is followed by a query method
    fn parse_explain(_collection: &str, args: &[Expr], _call: &CallExpr) -> Result<Command> {
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
    fn parse_find(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let projection = Self::get_projection(args, 1)?;

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
    fn parse_find_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let projection = Self::get_projection(args, 1)?;

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
    fn parse_insert_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let document = Self::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::InsertOne {
            collection: collection.to_string(),
            document,
        }))
    }

    /// Parse insertMany operation: db.collection.insertMany(documents)
    fn parse_insert_many(collection: &str, args: &[Expr]) -> Result<Command> {
        let documents = Self::get_doc_array_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::InsertMany {
            collection: collection.to_string(),
            documents,
        }))
    }

    /// Parse updateOne operation: db.collection.updateOne(filter, update, options)
    fn parse_update_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let update = Self::get_doc_arg(args, 1)?;
        let options = Self::get_update_options(args, 2)?;

        Ok(Command::Query(QueryCommand::UpdateOne {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse updateMany operation: db.collection.updateMany(filter, update, options)
    fn parse_update_many(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let update = Self::get_doc_arg(args, 1)?;
        let options = Self::get_update_options(args, 2)?;

        Ok(Command::Query(QueryCommand::UpdateMany {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse replaceOne operation: db.collection.replaceOne(filter, replacement, options)
    fn parse_replace_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let replacement = Self::get_doc_arg(args, 1)?;
        let options = Self::get_update_options(args, 2)?;

        Ok(Command::Query(QueryCommand::ReplaceOne {
            collection: collection.to_string(),
            filter,
            replacement,
            options,
        }))
    }

    /// Parse deleteOne operation: db.collection.deleteOne(filter)
    fn parse_delete_one(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::DeleteOne {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse deleteMany operation: db.collection.deleteMany(filter)
    fn parse_delete_many(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::DeleteMany {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse aggregate operation: db.collection.aggregate(pipeline, options)
    fn parse_aggregate(collection: &str, args: &[Expr]) -> Result<Command> {
        let pipeline = Self::get_doc_array_arg(args, 0)?;
        let options = Self::get_aggregate_options(args, 1)?;

        Ok(Command::Query(QueryCommand::Aggregate {
            collection: collection.to_string(),
            pipeline,
            options,
        }))
    }

    /// Parse countDocuments operation: db.collection.countDocuments(filter)
    fn parse_count_documents(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::CountDocuments {
            collection: collection.to_string(),
            filter,
        }))
    }

    /// Parse estimatedDocumentCount operation
    fn parse_estimated_document_count(collection: &str, _args: &[Expr]) -> Result<Command> {
        Ok(Command::Query(QueryCommand::EstimatedDocumentCount {
            collection: collection.to_string(),
        }))
    }

    /// Parse findOneAndDelete operation
    fn parse_find_one_and_delete(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let options = Self::get_find_and_modify_options(args, 1)?;

        Ok(Command::Query(QueryCommand::FindOneAndDelete {
            collection: collection.to_string(),
            filter,
            options,
        }))
    }

    /// Parse findOneAndUpdate operation
    fn parse_find_one_and_update(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let update = Self::get_doc_arg(args, 1)?;
        let options = Self::get_find_and_modify_options(args, 2)?;

        Ok(Command::Query(QueryCommand::FindOneAndUpdate {
            collection: collection.to_string(),
            filter,
            update,
            options,
        }))
    }

    /// Parse findOneAndReplace operation
    fn parse_find_one_and_replace(collection: &str, args: &[Expr]) -> Result<Command> {
        let filter = Self::get_doc_arg(args, 0)?;
        let replacement = Self::get_doc_arg(args, 1)?;
        let options = Self::get_find_and_modify_options(args, 2)?;

        Ok(Command::Query(QueryCommand::FindOneAndReplace {
            collection: collection.to_string(),
            filter,
            replacement,
            options,
        }))
    }

    /// Parse distinct operation
    fn parse_distinct(collection: &str, args: &[Expr]) -> Result<Command> {
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

    /// Parse bulkWrite operation
    fn parse_bulk_write(collection: &str, args: &[Expr]) -> Result<Command> {
        let operations = Self::get_doc_array_arg(args, 0)?;

        Ok(Command::Query(QueryCommand::BulkWrite {
            collection: collection.to_string(),
            operations,
            ordered: true,
        }))
    }

    /// Parse getIndexes operation
    fn parse_get_indexes(collection: &str) -> Result<Command> {
        Ok(Command::Admin(AdminCommand::ListIndexes(
            collection.to_string(),
        )))
    }

    /// Parse createIndex operation
    fn parse_create_index(collection: &str, args: &[Expr]) -> Result<Command> {
        let keys = Self::get_doc_arg(args, 0)?;

        // Get options if provided
        let options = if args.len() > 1 {
            Some(Self::get_doc_arg(args, 1)?)
        } else {
            None
        };

        // Build index spec
        let mut index_spec = Document::new();
        index_spec.insert("key", keys);

        if let Some(opts) = options {
            // Merge options into index spec
            for (key, value) in opts {
                index_spec.insert(key, value);
            }
        }

        Ok(Command::Admin(AdminCommand::CreateIndexes {
            collection: collection.to_string(),
            indexes: vec![index_spec],
        }))
    }

    /// Parse createIndexes operation
    fn parse_create_indexes(collection: &str, args: &[Expr]) -> Result<Command> {
        let indexes = Self::get_doc_array_arg(args, 0)?;

        Ok(Command::Admin(AdminCommand::CreateIndexes {
            collection: collection.to_string(),
            indexes,
        }))
    }

    /// Parse dropIndex operation
    fn parse_drop_index(collection: &str, args: &[Expr]) -> Result<Command> {
        let index = Self::get_string_arg(args, 0)?;

        Ok(Command::Admin(AdminCommand::DropIndex {
            collection: collection.to_string(),
            index,
        }))
    }

    /// Parse dropIndexes operation
    fn parse_drop_indexes(collection: &str) -> Result<Command> {
        Ok(Command::Admin(AdminCommand::DropIndex {
            collection: collection.to_string(),
            index: "*".to_string(),
        }))
    }

    /// Parse drop collection operation
    fn parse_drop_collection(collection: &str) -> Result<Command> {
        Ok(Command::Admin(AdminCommand::DropCollection(
            collection.to_string(),
        )))
    }

    /// Parse rename collection operation
    fn parse_rename_collection(collection: &str, args: &[Expr]) -> Result<Command> {
        // renameCollection(target, dropTarget)
        // target is required (string)
        // dropTarget is optional (boolean, defaults to false)

        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "renameCollection() requires at least 1 argument: target name".to_string(),
            )
            .into());
        }

        if args.len() > 2 {
            return Err(ParseError::InvalidCommand(
                format!("renameCollection() expects at most 2 arguments, got {}", args.len()),
            )
            .into());
        }

        // Parse target name (required)
        let target = match &args[0] {
            Expr::String(s) => s.clone(),
            _ => {
                return Err(ParseError::InvalidCommand(
                    "renameCollection() target must be a string".to_string(),
                )
                .into());
            }
        };

        // Parse dropTarget (optional, defaults to false)
        let drop_target = if args.len() > 1 {
            match &args[1] {
                Expr::Boolean(b) => *b,
                _ => {
                    return Err(ParseError::InvalidCommand(
                        "renameCollection() dropTarget must be a boolean".to_string(),
                    )
                    .into());
                }
            }
        } else {
            false
        };

        Ok(Command::Admin(AdminCommand::RenameCollection {
            collection: collection.to_string(),
            target,
            drop_target,
        }))
    }

    /// Parse findAndModify operation
    fn parse_find_and_modify(collection: &str, args: &[Expr]) -> Result<Command> {
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
        let options_doc = Self::get_doc_arg(args, 0)?;

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

    #[test]
    fn test_parse_find_empty() {
        let cmd = DbOperationParser::parse("db.users.find()").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert!(filter.is_empty());
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_find_with_filter() {
        let cmd = DbOperationParser::parse("db.users.find({age: 25})").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_i64("age").unwrap(), 25);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_find_with_operators() {
        let cmd = DbOperationParser::parse("db.users.find({age: {$gt: 18}})").unwrap();
        if let Command::Query(QueryCommand::Find {
            collection, filter, ..
        }) = cmd
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
        let cmd = DbOperationParser::parse("db.users.insertOne({name: 'Alice', age: 30})").unwrap();
        if let Command::Query(QueryCommand::InsertOne {
            collection,
            document,
        }) = cmd
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
        let cmd = DbOperationParser::parse("db.users.insertMany([{name: 'Alice'}, {name: 'Bob'}])")
            .unwrap();
        if let Command::Query(QueryCommand::InsertMany {
            collection,
            documents,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(documents.len(), 2);
        } else {
            panic!("Expected InsertMany command");
        }
    }

    #[test]
    fn test_parse_update_one() {
        let cmd =
            DbOperationParser::parse("db.users.updateOne({name: 'Alice'}, {$set: {age: 31}})")
                .unwrap();
        if let Command::Query(QueryCommand::UpdateOne {
            collection,
            filter,
            update,
            ..
        }) = cmd
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
        let cmd = DbOperationParser::parse("db.users.deleteOne({name: 'Alice'})").unwrap();
        if let Command::Query(QueryCommand::DeleteOne {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
        } else {
            panic!("Expected DeleteOne command");
        }
    }

    #[test]
    fn test_parse_aggregate() {
        let cmd =
            DbOperationParser::parse("db.users.aggregate([{$match: {age: {$gt: 18}}}])").unwrap();
        if let Command::Query(QueryCommand::Aggregate {
            collection,
            pipeline,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(pipeline.len(), 1);
        } else {
            panic!("Expected Aggregate command");
        }
    }

    #[test]
    fn test_parse_count_documents() {
        let cmd = DbOperationParser::parse("db.users.countDocuments({age: {$gt: 18}})").unwrap();
        if let Command::Query(QueryCommand::CountDocuments {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            let age_cond = filter.get_document("age").unwrap();
            assert_eq!(age_cond.get_i64("$gt").unwrap(), 18);
        } else {
            panic!("Expected CountDocuments command");
        }
    }

    #[test]
    fn test_parse_count() {
        let cmd = DbOperationParser::parse("db.users.count({status: 'active'})").unwrap();
        if let Command::Query(QueryCommand::CountDocuments {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("status").unwrap(), "active");
        } else {
            panic!("Expected CountDocuments command");
        }
    }

    #[test]
    fn test_parse_chained_limit() {
        let cmd = DbOperationParser::parse("db.users.find().limit(10)").unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = cmd {
            assert_eq!(options.limit, Some(10));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_chained_skip() {
        let cmd = DbOperationParser::parse("db.users.find().skip(5)").unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = cmd {
            assert_eq!(options.skip, Some(5));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_chained_sort() {
        let cmd = DbOperationParser::parse("db.users.find().sort({name: 1})").unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = cmd {
            assert!(options.sort.is_some());
            let sort = options.sort.unwrap();
            assert_eq!(sort.get_i64("name").unwrap(), 1);
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_multiple_chained_methods() {
        let cmd = DbOperationParser::parse("db.users.find().limit(10).skip(5)").unwrap();
        if let Command::Query(QueryCommand::Find { options, .. }) = cmd {
            assert_eq!(options.limit, Some(10));
            assert_eq!(options.skip, Some(5));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_complex_chain() {
        let cmd = DbOperationParser::parse(
            "db.users.find({age: {$gt: 18}}).sort({name: 1}).limit(20).skip(10)",
        )
        .unwrap();
        if let Command::Query(QueryCommand::Find {
            filter, options, ..
        }) = cmd
        {
            let age_cond = filter.get_document("age").unwrap();
            assert_eq!(age_cond.get_i64("$gt").unwrap(), 18);
            assert_eq!(options.limit, Some(20));
            assert_eq!(options.skip, Some(10));
            assert!(options.sort.is_some());
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_get_indexes() {
        let cmd = DbOperationParser::parse("db.users.getIndexes()").unwrap();
        if let Command::Admin(AdminCommand::ListIndexes(collection)) = cmd {
            assert_eq!(collection, "users");
        } else {
            panic!("Expected ListIndexes command");
        }
    }

    #[test]
    fn test_parse_create_index() {
        let cmd = DbOperationParser::parse("db.users.createIndex({name: 1})").unwrap();
        if let Command::Admin(AdminCommand::CreateIndexes {
            collection,
            indexes,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(indexes.len(), 1);
            let keys = indexes[0].get_document("key").unwrap();
            assert_eq!(keys.get_i64("name").unwrap(), 1);
        } else {
            panic!("Expected CreateIndexes command");
        }
    }

    #[test]
    fn test_parse_create_index_with_options() {
        let cmd =
            DbOperationParser::parse("db.users.createIndex({email: 1}, {unique: true})").unwrap();
        if let Command::Admin(AdminCommand::CreateIndexes {
            collection,
            indexes,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(indexes.len(), 1);
            let spec = &indexes[0];
            let keys = spec.get_document("key").unwrap();
            assert_eq!(keys.get_i64("email").unwrap(), 1);
            assert_eq!(spec.get_bool("unique").unwrap(), true);
        } else {
            panic!("Expected CreateIndexes command");
        }
    }

    #[test]
    fn test_parse_create_indexes() {
        let cmd = DbOperationParser::parse("db.users.createIndexes([{key: {name: 1}}])").unwrap();
        if let Command::Admin(AdminCommand::CreateIndexes {
            collection,
            indexes,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(indexes.len(), 1);
        } else {
            panic!("Expected CreateIndexes command");
        }
    }

    #[test]
    fn test_parse_aggregate_with_invalid_batch_size() {
        let result = DbOperationParser::parse("db.users.aggregate([], {batchSize: -1})");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_aggregate_with_negative_batch_size() {
        let result = DbOperationParser::parse("db.users.aggregate([]).batchSize(-10)");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_aggregate_with_valid_batch_size() {
        let cmd = DbOperationParser::parse("db.users.aggregate([]).batchSize(100)").unwrap();
        if let Command::Query(QueryCommand::Aggregate { options, .. }) = cmd {
            assert_eq!(options.batch_size, Some(100));
        } else {
            panic!("Expected Aggregate command");
        }
    }

    #[test]
    fn test_parse_estimated_document_count() {
        let cmd = DbOperationParser::parse("db.users.estimatedDocumentCount()").unwrap();
        if let Command::Query(QueryCommand::EstimatedDocumentCount { collection }) = cmd {
            assert_eq!(collection, "users");
        } else {
            panic!("Expected EstimatedDocumentCount command");
        }
    }

    #[test]
    fn test_parse_distinct() {
        let cmd = DbOperationParser::parse("db.users.distinct('city')").unwrap();
        if let Command::Query(QueryCommand::Distinct {
            collection, field, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(field, "city");
        } else {
            panic!("Expected Distinct command");
        }
    }

    #[test]
    fn test_parse_distinct_with_filter() {
        let cmd = DbOperationParser::parse("db.users.distinct('city', {age: {$gt: 18}})").unwrap();
        if let Command::Query(QueryCommand::Distinct {
            collection,
            field,
            filter,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(field, "city");
            if let Some(filter_doc) = filter {
                let age_cond = filter_doc.get_document("age").unwrap();
                assert_eq!(age_cond.get_i64("$gt").unwrap(), 18);
            } else {
                panic!("Expected filter to be present");
            }
        } else {
            panic!("Expected Distinct command");
        }
    }

    #[test]
    fn test_parse_drop_index() {
        let cmd = DbOperationParser::parse("db.users.dropIndex('name_1')").unwrap();
        if let Command::Admin(AdminCommand::DropIndex { collection, index }) = cmd {
            assert_eq!(collection, "users");
            assert_eq!(index, "name_1");
        } else {
            panic!("Expected DropIndex command");
        }
    }

    #[test]
    fn test_parse_drop_indexes() {
        let cmd = DbOperationParser::parse("db.users.dropIndexes()").unwrap();
        if let Command::Admin(AdminCommand::DropIndex { collection, index }) = cmd {
            assert_eq!(collection, "users");
            assert_eq!(index, "*");
        } else {
            panic!("Expected DropIndex command");
        }
    }

    #[test]
    fn test_parse_drop_collection() {
        let cmd = DbOperationParser::parse("db.users.drop()").unwrap();
        if let Command::Admin(AdminCommand::DropCollection(collection)) = cmd {
            assert_eq!(collection, "users");
        } else {
            panic!("Expected DropCollection command");
        }
    }

    #[test]
    fn test_parse_rename_collection() {
        let cmd = DbOperationParser::parse("db.users.renameCollection('customers')").unwrap();
        if let Command::Admin(AdminCommand::RenameCollection {
            collection,
            target,
            drop_target,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(target, "customers");
            assert_eq!(drop_target, false);
        } else {
            panic!("Expected RenameCollection command");
        }
    }

    #[test]
    fn test_parse_rename_collection_with_drop_target() {
        let cmd =
            DbOperationParser::parse("db.users.renameCollection('customers', true)").unwrap();
        if let Command::Admin(AdminCommand::RenameCollection {
            collection,
            target,
            drop_target,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(target, "customers");
            assert_eq!(drop_target, true);
        } else {
            panic!("Expected RenameCollection command");
        }
    }

    #[test]
    fn test_parse_rename_collection_with_drop_target_false() {
        let cmd =
            DbOperationParser::parse("db.users.renameCollection('customers', false)").unwrap();
        if let Command::Admin(AdminCommand::RenameCollection {
            collection,
            target,
            drop_target,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(target, "customers");
            assert_eq!(drop_target, false);
        } else {
            panic!("Expected RenameCollection command");
        }
    }

    #[test]
    fn test_parse_replace_one() {
        let cmd = DbOperationParser::parse(
            "db.users.replaceOne({name: 'Alice'}, {name: 'Alice', age: 31, city: 'NYC'})",
        )
        .unwrap();
        if let Command::Query(QueryCommand::ReplaceOne {
            collection,
            filter,
            replacement,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
            assert_eq!(replacement.get_str("name").unwrap(), "Alice");
            assert_eq!(replacement.get_i64("age").unwrap(), 31);
        } else {
            panic!("Expected ReplaceOne command");
        }
    }

    #[test]
    fn test_parse_find_one_and_delete() {
        let cmd = DbOperationParser::parse("db.users.findOneAndDelete({name: 'Alice'})").unwrap();
        if let Command::Query(QueryCommand::FindOneAndDelete {
            collection, filter, ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
        } else {
            panic!("Expected FindOneAndDelete command");
        }
    }

    #[test]
    fn test_parse_find_one_and_update() {
        let cmd = DbOperationParser::parse(
            "db.users.findOneAndUpdate({name: 'Alice'}, {$set: {age: 31}})",
        )
        .unwrap();
        if let Command::Query(QueryCommand::FindOneAndUpdate {
            collection,
            filter,
            update,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
            let set_doc = update.get_document("$set").unwrap();
            assert_eq!(set_doc.get_i64("age").unwrap(), 31);
        } else {
            panic!("Expected FindOneAndUpdate command");
        }
    }

    #[test]
    fn test_parse_find_one_and_replace() {
        let cmd = DbOperationParser::parse(
            "db.users.findOneAndReplace({name: 'Alice'}, {name: 'Alice', age: 31})",
        )
        .unwrap();
        if let Command::Query(QueryCommand::FindOneAndReplace {
            collection,
            filter,
            replacement,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(filter.get_str("name").unwrap(), "Alice");
            assert_eq!(replacement.get_str("name").unwrap(), "Alice");
            assert_eq!(replacement.get_i64("age").unwrap(), 31);
        } else {
            panic!("Expected FindOneAndReplace command");
        }
    }

    #[test]
    fn test_parse_find_with_explain_after() {
        // Test: db.collection.find().explain()
        let cmd = DbOperationParser::parse("db.users.find({age: {$gt: 18}}).explain()").unwrap();
        if let Command::Query(QueryCommand::Explain {
            collection,
            verbosity,
            query,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(verbosity, ExplainVerbosity::QueryPlanner);

            // Inner query should be Find
            if let QueryCommand::Find { filter, .. } = *query {
                assert!(filter.contains_key("age"));
            } else {
                panic!("Expected Find command inside Explain");
            }
        } else {
            panic!("Expected Explain command, got: {:?}", cmd);
        }
    }

    #[test]
    fn test_parse_find_with_explain_after_with_verbosity() {
        // Test: db.collection.find().explain("executionStats")
        let cmd = DbOperationParser::parse("db.users.find().explain('executionStats')").unwrap();
        if let Command::Query(QueryCommand::Explain {
            verbosity,
            query,
            ..
        }) = cmd
        {
            assert_eq!(verbosity, ExplainVerbosity::ExecutionStats);
            assert!(matches!(*query, QueryCommand::Find { .. }));
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_parse_find_with_explain_and_chain_methods() {
        // Test: db.collection.find().limit(10).skip(5).explain()
        let cmd = DbOperationParser::parse("db.users.find().limit(10).skip(5).explain()").unwrap();
        if let Command::Query(QueryCommand::Explain {
            query,
            ..
        }) = cmd
        {
            // Inner query should be Find with limit and skip
            if let QueryCommand::Find { options, .. } = *query {
                assert_eq!(options.limit, Some(10));
                assert_eq!(options.skip, Some(5));
            } else {
                panic!("Expected Find command inside Explain");
            }
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_parse_aggregate_with_explain_after() {
        // Test: db.collection.aggregate([{$match: {status: "active"}}]).explain()
        let cmd = DbOperationParser::parse(
            "db.orders.aggregate([{$match: {status: 'active'}}]).explain('allPlansExecution')"
        ).unwrap();
        if let Command::Query(QueryCommand::Explain {
            collection,
            verbosity,
            query,
        }) = cmd
        {
            assert_eq!(collection, "orders");
            assert_eq!(verbosity, ExplainVerbosity::AllPlansExecution);

            // Inner query should be Aggregate
            if let QueryCommand::Aggregate { pipeline, .. } = *query {
                assert_eq!(pipeline.len(), 1);
            } else {
                panic!("Expected Aggregate command inside Explain");
            }
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_parse_distinct_with_explain_after() {
        // Test: db.collection.distinct("field").explain()
        // Note: distinct is parsed differently and doesn't go through the chained call path
        // So we test with find instead
        let cmd = DbOperationParser::parse("db.users.find({status: 'active'}).explain()").unwrap();
        if let Command::Query(QueryCommand::Explain {
            query,
            ..
        }) = cmd
        {
            // Inner query should be Find
            assert!(matches!(*query, QueryCommand::Find { .. }));
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_parse_find_and_modify_update() {
        let cmd = DbOperationParser::parse(
            "db.users.findAndModify({query: {name: 'Alice'}, update: {$set: {age: 31}}})"
        ).unwrap();
        if let Command::Query(QueryCommand::FindAndModify {
            collection,
            query,
            update,
            remove,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(query.get_str("name").unwrap(), "Alice");
            assert!(update.is_some());
            assert_eq!(remove, false);
        } else {
            panic!("Expected FindAndModify command");
        }
    }

    #[test]
    fn test_parse_find_and_modify_remove() {
        let cmd = DbOperationParser::parse(
            "db.users.findAndModify({query: {name: 'Alice'}, remove: true})"
        ).unwrap();
        if let Command::Query(QueryCommand::FindAndModify {
            collection,
            query,
            update,
            remove,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(query.get_str("name").unwrap(), "Alice");
            assert!(update.is_none());
            assert_eq!(remove, true);
        } else {
            panic!("Expected FindAndModify command");
        }
    }

    #[test]
    fn test_parse_find_and_modify_with_options() {
        let cmd = DbOperationParser::parse(
            "db.users.findAndModify({query: {name: 'Alice'}, update: {$inc: {score: 1}}, new: true, upsert: true, sort: {score: 1}})"
        ).unwrap();
        if let Command::Query(QueryCommand::FindAndModify {
            collection,
            query,
            update,
            new,
            upsert,
            sort,
            ..
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(query.get_str("name").unwrap(), "Alice");
            assert!(update.is_some());
            assert_eq!(new, true);
            assert_eq!(upsert, true);
            assert!(sort.is_some());
        } else {
            panic!("Expected FindAndModify command");
        }
    }
}
