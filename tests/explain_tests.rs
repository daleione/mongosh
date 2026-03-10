//! Integration tests for explain functionality

use mongosh::parser::{Parser, Command, QueryCommand, ExplainVerbosity};

#[test]
fn test_parse_explain_find() {
    let input = r#"db.users.explain().find({age: {$gt: 25}})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { collection, verbosity, query })) = result {
        assert_eq!(collection, "users");
        assert_eq!(verbosity, ExplainVerbosity::QueryPlanner);

        // Check inner query is Find
        match *query {
            QueryCommand::Find { .. } => {},
            _ => panic!("Expected Find command inside Explain"),
        }
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_with_execution_stats() {
    let input = r#"db.users.explain("executionStats").find({age: {$gt: 25}})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { verbosity, .. })) = result {
        assert_eq!(verbosity, ExplainVerbosity::ExecutionStats);
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_with_all_plans() {
    let input = r#"db.users.explain("allPlansExecution").find({age: {$gt: 25}})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { verbosity, .. })) = result {
        assert_eq!(verbosity, ExplainVerbosity::AllPlansExecution);
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_with_boolean_true() {
    let input = r#"db.users.explain(true).find({age: {$gt: 25}})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { verbosity, .. })) = result {
        assert_eq!(verbosity, ExplainVerbosity::AllPlansExecution);
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_with_boolean_false() {
    let input = r#"db.users.explain(false).find({age: {$gt: 25}})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { verbosity, .. })) = result {
        assert_eq!(verbosity, ExplainVerbosity::QueryPlanner);
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_find_with_chain() {
    let input = r#"db.users.explain().find({age: {$gt: 25}}).limit(10)"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    if let Err(ref e) = result {
        eprintln!("Parse error: {:?}", e);
    }
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { query, .. })) = result {
        // Check that limit was applied to the inner Find
        match *query {
            QueryCommand::Find { options, .. } => {
                assert_eq!(options.limit, Some(10));
            },
            _ => panic!("Expected Find command inside Explain"),
        }
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_find_with_multiple_chains() {
    let input = r#"db.users.explain().find({age: {$gt: 25}}).limit(10).skip(5)"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    if let Err(ref e) = result {
        eprintln!("Parse error: {:?}", e);
    }
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { query, .. })) = result {
        match *query {
            QueryCommand::Find { options, .. } => {
                assert_eq!(options.limit, Some(10));
                assert_eq!(options.skip, Some(5));
            },
            _ => panic!("Expected Find command inside Explain"),
        }
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_find_one() {
    let input = r#"db.users.explain().findOne({name: "Alice"})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { query, .. })) = result {
        match *query {
            QueryCommand::FindOne { .. } => {},
            _ => panic!("Expected FindOne command inside Explain"),
        }
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_aggregate() {
    let input = r#"db.users.explain().aggregate([{$match: {age: {$gt: 25}}}])"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { query, .. })) = result {
        match *query {
            QueryCommand::Aggregate { .. } => {},
            _ => panic!("Expected Aggregate command inside Explain"),
        }
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_count() {
    let input = r#"db.users.explain().count({status: "active"})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { query, .. })) = result {
        match *query {
            QueryCommand::CountDocuments { .. } => {},
            _ => panic!("Expected CountDocuments command inside Explain"),
        }
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_distinct() {
    let input = r#"db.users.explain().distinct("city", {age: {$gt: 18}})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_ok());

    if let Ok(Command::Query(QueryCommand::Explain { query, .. })) = result {
        match *query {
            QueryCommand::Distinct { field, .. } => {
                assert_eq!(field, "city");
            },
            _ => panic!("Expected Distinct command inside Explain"),
        }
    } else {
        panic!("Expected Explain command");
    }
}

#[test]
fn test_parse_explain_without_method_fails() {
    let input = r#"db.users.explain()"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_err());

    if let Err(e) = result {
        let error_msg = format!("{}", e);
        assert!(error_msg.contains("must be followed by a query method"));
    }
}

#[test]
fn test_parse_explain_with_invalid_verbosity() {
    let input = r#"db.users.explain("invalid").find({})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_err());

    if let Err(e) = result {
        let error_msg = format!("{}", e);
        assert!(error_msg.contains("Invalid explain verbosity"));
    }
}

#[test]
fn test_parse_explain_with_too_many_args() {
    let input = r#"db.users.explain("queryPlanner", "extra").find({})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_err());

    if let Err(e) = result {
        let error_msg = format!("{}", e);
        assert!(error_msg.contains("at most 1 argument"));
    }
}

#[test]
fn test_parse_explain_with_unsupported_method() {
    let input = r#"db.users.explain().insertOne({name: "Alice"})"#;
    let mut parser = Parser::new();
    let result = parser.parse(input);
    assert!(result.is_err());

    if let Err(e) = result {
        let error_msg = format!("{}", e);
        assert!(error_msg.contains("does not support method"));
    }
}

#[test]
fn test_explain_verbosity_from_str() {
    assert_eq!(
        ExplainVerbosity::from_str("queryPlanner").unwrap(),
        ExplainVerbosity::QueryPlanner
    );
    assert_eq!(
        ExplainVerbosity::from_str("executionStats").unwrap(),
        ExplainVerbosity::ExecutionStats
    );
    assert_eq!(
        ExplainVerbosity::from_str("allPlansExecution").unwrap(),
        ExplainVerbosity::AllPlansExecution
    );

    // Backwards compatibility
    assert_eq!(
        ExplainVerbosity::from_str("true").unwrap(),
        ExplainVerbosity::AllPlansExecution
    );
    assert_eq!(
        ExplainVerbosity::from_str("false").unwrap(),
        ExplainVerbosity::QueryPlanner
    );

    // Invalid
    assert!(ExplainVerbosity::from_str("invalid").is_err());
}

#[test]
fn test_explain_verbosity_as_str() {
    assert_eq!(ExplainVerbosity::QueryPlanner.as_str(), "queryPlanner");
    assert_eq!(ExplainVerbosity::ExecutionStats.as_str(), "executionStats");
    assert_eq!(ExplainVerbosity::AllPlansExecution.as_str(), "allPlansExecution");
}

#[test]
fn test_explain_verbosity_from_bool() {
    assert_eq!(ExplainVerbosity::from_bool(true), ExplainVerbosity::AllPlansExecution);
    assert_eq!(ExplainVerbosity::from_bool(false), ExplainVerbosity::QueryPlanner);
}

#[test]
fn test_explain_verbosity_default() {
    assert_eq!(ExplainVerbosity::default(), ExplainVerbosity::QueryPlanner);
}
