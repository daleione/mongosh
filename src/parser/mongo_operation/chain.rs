//! Chain method handling for MongoDB operations
//!
//! This module handles chained method calls like:
//! - db.collection.find().limit(10).skip(5)
//! - db.collection.aggregate([...]).batchSize(100)
//! - db.collection.explain().find()

use mongodb::bson::Document;

use crate::error::{ParseError, Result};
use crate::parser::command::{
    AggregateOptions, Command, ExplainVerbosity, FindOptions, QueryCommand,
};
use crate::parser::mongo_ast::*;

use super::args::ArgParser;

/// Represents a chained method call
#[derive(Debug, Clone)]
pub struct ChainMethod {
    pub name: String,
    pub args: Vec<Expr>,
}

/// Chain method handler
pub struct ChainHandler;

impl ChainHandler {
    /// Try to parse a chained call expression
    /// Returns Some((base_command, chain_methods)) if it's a chained call, None otherwise
    pub fn try_parse_chained_call(call: &CallExpr) -> Result<Option<(Command, Vec<ChainMethod>)>> {
        // Early return: Check if this is a chained call structure
        let member = match call.callee.as_ref() {
            Expr::Member(m) => m,
            _ => return Ok(None),
        };

        // Early return: The object must be a call (indicates chaining)
        if !matches!(member.object.as_ref(), Expr::Call(_)) {
            return Ok(None);
        }

        // Check if this chain contains an explain call
        if Self::contains_explain_in_chain(call)? {
            return Self::handle_explain_chain(call, member);
        }

        // Regular chained call like: db.users.find().limit(10)
        Self::parse_regular_chain(call)
    }

    /// Handle chains that contain an explain call
    fn handle_explain_chain(
        call: &CallExpr,
        member: &MemberExpr,
    ) -> Result<Option<(Command, Vec<ChainMethod>)>> {
        // Check if explain is at the end of the chain
        if let MemberProperty::Ident(name) = &member.property {
            if name == "explain" {
                // Explain is at the END: db.collection.find().explain()
                // Treat it as a regular chain method
                return Self::parse_regular_chain(call);
            }
        }

        // Explain is in the MIDDLE/BEGINNING: db.collection.explain().find()...
        Self::try_parse_explain_chain(call)
    }

    /// Parse a regular chain
    fn parse_regular_chain(call: &CallExpr) -> Result<Option<(Command, Vec<ChainMethod>)>> {
        let (base_expr, chain_methods) = Self::collect_chain_methods(call)?;

        let base_call = match base_expr {
            Expr::Call(call) => call,
            _ => {
                return Err(ParseError::InvalidCommand(
                    "Expected base call expression".to_string(),
                )
                .into())
            }
        };

        let base_cmd = super::parse_call_expression_simple(&base_call)?;
        Ok(Some((base_cmd, chain_methods)))
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
                                    let filter = ArgParser::get_doc_arg(&base_call.arguments, 0)?;
                                    let projection = ArgParser::get_projection(&base_call.arguments, 1)?;
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
                                    let filter = ArgParser::get_doc_arg(&base_call.arguments, 0)?;
                                    let projection = ArgParser::get_projection(&base_call.arguments, 1)?;
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
                                    let pipeline = ArgParser::get_doc_array_arg(&base_call.arguments, 0)?;
                                    let options = ArgParser::get_aggregate_options(&base_call.arguments, 1)?;
                                    QueryCommand::Aggregate {
                                        collection: collection.clone(),
                                        pipeline,
                                        options,
                                    }
                                }
                                "count" | "countDocuments" => {
                                    let filter = ArgParser::get_doc_arg(&base_call.arguments, 0)?;
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
                                        Some(ArgParser::get_doc_arg(&base_call.arguments, 1)?)
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

    /// Apply chain methods to a base command
    pub fn apply_chain_methods(mut cmd: Command, chain_methods: Vec<ChainMethod>) -> Result<Command> {
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
                let limit_val = ArgParser::get_number_arg(&method.args, 0)?;
                if limit_val < 0 {
                    return Err(ParseError::InvalidQuery(
                        "limit() value must be non-negative".to_string(),
                    )
                    .into());
                }
                options.limit = Some(limit_val);
            }
            "skip" => {
                let skip_val = ArgParser::get_number_arg(&method.args, 0)?;
                if skip_val < 0 {
                    return Err(ParseError::InvalidQuery(
                        "skip() value must be non-negative".to_string(),
                    )
                    .into());
                }
                options.skip = Some(skip_val as u64);
            }
            "sort" => {
                options.sort = Some(ArgParser::get_doc_arg(&method.args, 0)?);
            }
            "projection" => {
                options.projection = Some(ArgParser::get_doc_arg(&method.args, 0)?);
            }
            "batchSize" => {
                let batch_size = ArgParser::get_number_arg(&method.args, 0)?;
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
                            options.hint = Some(ArgParser::get_doc_arg(&method.args, 0)?);
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
                let batch_size = ArgParser::get_number_arg(&method.args, 0)?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mongo_operation::DbOperationParser;
    use crate::parser::command::{ExplainVerbosity, QueryCommand};

    #[test]
    fn test_parse_chained_limit() {
        let result = DbOperationParser::parse("db.users.find().limit(10)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_chained_skip() {
        let result = DbOperationParser::parse("db.users.find().skip(5)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_chained_sort() {
        let result = DbOperationParser::parse("db.users.find().sort({ age: -1 })");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_multiple_chained_methods() {
        let result = DbOperationParser::parse("db.users.find().limit(10).skip(5)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_complex_chain() {
        let result = DbOperationParser::parse(
            "db.users.find({ age: { $gte: 18 } }).sort({ name: 1 }).limit(10).skip(5)"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            if let QueryCommand::Find { options, .. } = query {
                assert_eq!(options.limit, Some(10));
                assert_eq!(options.skip, Some(5));
                assert!(options.sort.is_some());
            }
        }
    }

    #[test]
    fn test_parse_aggregate_with_invalid_batch_size() {
        let result = DbOperationParser::parse("db.users.aggregate([]).batchSize(0)");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_aggregate_with_negative_batch_size() {
        let result = DbOperationParser::parse("db.users.aggregate([]).batchSize(-1)");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_aggregate_with_valid_batch_size() {
        let result = DbOperationParser::parse("db.users.aggregate([]).batchSize(100)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_find_with_explain_after() {
        let result = DbOperationParser::parse("db.users.find({ age: { $gt: 18 } }).explain()");
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::Explain { .. }));
        }
    }

    #[test]
    fn test_parse_find_with_explain_after_with_verbosity() {
        let result = DbOperationParser::parse("db.users.find({ age: { $gt: 18 } }).explain('executionStats')");
        assert!(result.is_ok());
        if let Ok(Command::Query(QueryCommand::Explain { verbosity, .. })) = result {
            assert!(matches!(verbosity, ExplainVerbosity::ExecutionStats));
        }
    }

    #[test]
    fn test_parse_find_with_explain_and_chain_methods() {
        let result = DbOperationParser::parse("db.users.find({ age: { $gt: 18 } }).limit(10).explain()");
        assert!(result.is_ok());
        if let Ok(Command::Query(QueryCommand::Explain { query, .. })) = result {
            if let QueryCommand::Find { options, .. } = *query {
                assert_eq!(options.limit, Some(10));
            }
        }
    }

    #[test]
    fn test_parse_aggregate_with_explain_after() {
        let result = DbOperationParser::parse(
            "db.users.aggregate([{ $match: { status: 'active' } }]).explain()"
        );
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::Explain { .. }));
        }
    }

    #[test]
    fn test_parse_distinct_with_explain_after() {
        let result = DbOperationParser::parse("db.users.distinct('city').explain()");
        assert!(result.is_ok());
        if let Ok(Command::Query(query)) = result {
            assert!(matches!(query, QueryCommand::Explain { .. }));
        }
    }
}
