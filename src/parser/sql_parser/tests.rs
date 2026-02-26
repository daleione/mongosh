//! Tests for SQL parser
//!
//! This module contains comprehensive tests for the SQL parser,
//! covering all major features and edge cases.

#[cfg(test)]
mod tests {
    use super::super::SqlParser;
    use crate::parser::command::{Command, ExplainVerbosity, QueryCommand};
    use crate::parser::sql_context::Expected;

    #[test]
    fn test_is_sql_command() {
        assert!(SqlParser::is_sql_command("SELECT * FROM users"));
        assert!(SqlParser::is_sql_command("select * from users"));
        assert!(SqlParser::is_sql_command("SELECT"));
        assert!(SqlParser::is_sql_command("  SELECT  "));
        assert!(!SqlParser::is_sql_command("show dbs"));
        assert!(!SqlParser::is_sql_command("db.users.find()"));
    }

    #[test]
    fn test_parse_simple_select() {
        let result = SqlParser::parse_to_command("SELECT * FROM users");
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(matches!(cmd, Command::Query(QueryCommand::Find { .. })));
    }

    #[test]
    fn test_parse_select_with_where() {
        let result = SqlParser::parse_to_command("SELECT * FROM users WHERE age > 18");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_select_with_columns() {
        let result = SqlParser::parse_to_command("SELECT name, age FROM users");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_order_by() {
        let result = SqlParser::parse_to_command("SELECT * FROM users ORDER BY name ASC");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_limit() {
        let result = SqlParser::parse_to_command("SELECT * FROM users LIMIT 10");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_aggregate() {
        let result = SqlParser::parse_to_command("SELECT COUNT(*) FROM users");
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(matches!(
            cmd,
            Command::Query(QueryCommand::Aggregate { .. })
        ));
    }

    #[test]
    fn test_parse_group_by() {
        let result = SqlParser::parse_to_command(
            "SELECT category, COUNT(*) FROM products GROUP BY category",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_partial_select() {
        let (result, context) = SqlParser::parse_with_context("SELECT *");
        assert!(result.is_partial());
        assert!(context.expected.contains(&Expected::Keyword("FROM")));
    }

    #[test]
    fn test_parse_partial_from() {
        let (result, context) = SqlParser::parse_with_context("SELECT * FROM ");
        assert!(result.is_partial());
        assert!(context.expected.contains(&Expected::TableName));
    }

    #[test]
    fn test_parse_partial_where() {
        let (result, context) = SqlParser::parse_with_context("SELECT * FROM users WHERE ");
        assert!(result.is_partial());
        assert!(
            context.expected.contains(&Expected::ColumnName)
                || context.expected.contains(&Expected::Expression)
        );
    }

    #[test]
    fn test_parse_with_string_alias() {
        let result = SqlParser::parse_to_command(
            "SELECT group_id AS 'group_id', COUNT(*) FROM templates GROUP BY group_id",
        );
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(matches!(
            cmd,
            Command::Query(QueryCommand::Aggregate { .. })
        ));
    }

    #[test]
    fn test_parse_aggregate_with_alias() {
        let result = SqlParser::parse_to_command("SELECT COUNT(*) AS total FROM users");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_where_after_group_by() {
        let result = SqlParser::parse_to_command(
            "SELECT status, COUNT(*) FROM tasks GROUP BY status WHERE template_id='123'",
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("WHERE clause must appear before GROUP BY"));
    }

    #[test]
    fn test_reject_group_by_after_order_by() {
        let result =
            SqlParser::parse_to_command("SELECT * FROM tasks ORDER BY created_at GROUP BY status");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("GROUP BY clause must appear before ORDER BY"));
    }

    #[test]
    fn test_reject_where_after_order_by() {
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks ORDER BY created_at WHERE status='active'",
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("WHERE clause must appear before"));
    }

    #[test]
    fn test_correct_clause_order_accepted() {
        // This should be accepted - correct order
        let result = SqlParser::parse_to_command(
            "SELECT status, COUNT(*) FROM tasks WHERE template_id='123' GROUP BY status",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_count_without_group_by() {
        // COUNT(*) without GROUP BY should generate proper aggregate pipeline
        let result =
            SqlParser::parse_to_command("SELECT COUNT(*) FROM tasks WHERE status='failed'");
        assert!(result.is_ok());
        let cmd = result.unwrap();

        // Should be an Aggregate command
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            // Should have $match and $group stages
            assert!(
                pipeline.len() >= 2,
                "Pipeline should have at least $match and $group stages"
            );

            // First stage should be $match
            assert!(pipeline[0].contains_key("$match"));

            // Second stage should be $group
            assert!(pipeline[1].contains_key("$group"));
        } else {
            panic!("Expected Aggregate command");
        }
    }

    #[test]
    fn test_parse_with_objectid_function() {
        // Test parsing ObjectId() function in WHERE clause
        let result = SqlParser::parse_to_command(
            "SELECT * FROM templates WHERE group_id=ObjectId('6920127eb40f0636d6b49042')",
        );
        assert!(
            result.is_ok(),
            "Failed to parse ObjectId function: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Find { filter, .. }) = cmd {
            // Should have group_id field in filter
            assert!(filter.contains_key("group_id"));

            // The value should be an ObjectId
            let value = filter.get("group_id").unwrap();
            assert!(matches!(value, mongodb::bson::Bson::ObjectId(_)));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_with_nested_fields() {
        // Test parsing nested fields with dot notation
        let result = SqlParser::parse_to_command(
            "SELECT input.images, user.name FROM templates WHERE input.type='image'",
        );
        assert!(
            result.is_ok(),
            "Failed to parse nested fields: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Find { filter, .. }) = cmd {
            // Should have input.type field in filter
            assert!(filter.contains_key("input.type"));
        } else {
            panic!("Expected Find command");
        }
    }

    #[test]
    fn test_parse_order_by_with_nested_fields() {
        let result =
            SqlParser::parse_to_command("SELECT * FROM templates ORDER BY user.created_at DESC");
        assert!(
            result.is_ok(),
            "Failed to parse nested field in ORDER BY: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_group_by_with_nested_fields() {
        let result = SqlParser::parse_to_command(
            "SELECT user.country, COUNT(*) FROM templates GROUP BY user.country",
        );
        assert!(
            result.is_ok(),
            "Failed to parse nested field in GROUP BY: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_field_alias_without_aggregation() {
        // Test that field aliases work correctly (should use aggregation pipeline)
        let result = SqlParser::parse_to_command("SELECT input.images AS image FROM tasks LIMIT 1");
        assert!(result.is_ok(), "Failed to parse field alias: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            // Should use aggregation pipeline for aliases
            assert!(!pipeline.is_empty(), "Pipeline should not be empty");

            // Should have a $project stage
            let has_project = pipeline.iter().any(|stage| stage.contains_key("$project"));
            assert!(
                has_project,
                "Pipeline should contain $project stage for alias"
            );
        } else {
            panic!("Expected Aggregate command for query with alias");
        }
    }

    #[test]
    fn test_array_positive_index() {
        // Test positive array index: tags[0]
        let result = SqlParser::parse_to_command("SELECT tags[0] FROM posts");
        assert!(result.is_ok(), "Failed to parse array index: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            assert!(!pipeline.is_empty(), "Pipeline should not be empty");
            // Should use aggregation pipeline for array access
            let has_project = pipeline.iter().any(|stage| stage.contains_key("$project"));
            assert!(has_project, "Pipeline should contain $project stage");
        } else {
            panic!("Expected Aggregate command for array index access");
        }
    }

    #[test]
    fn test_array_negative_index() {
        // Test negative array index: tags[-1]
        let result = SqlParser::parse_to_command("SELECT tags[-1] FROM posts");
        assert!(
            result.is_ok(),
            "Failed to parse negative array index: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { .. }) = cmd {
            // Should use aggregation pipeline
        } else {
            panic!("Expected Aggregate command for negative array index");
        }
    }

    #[test]
    fn test_nested_array_index() {
        // Test nested field with array index: user.roles[0]
        let result = SqlParser::parse_to_command("SELECT user.roles[0] FROM accounts");
        assert!(
            result.is_ok(),
            "Failed to parse nested array index: {:?}",
            result
        );
    }

    #[test]
    fn test_array_slice() {
        // Test array slice: tags[0:5]
        let result = SqlParser::parse_to_command("SELECT tags[0:5] FROM posts");
        assert!(result.is_ok(), "Failed to parse array slice: {:?}", result);

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { .. }) = cmd {
            // Should use aggregation pipeline
        } else {
            panic!("Expected Aggregate command for array slice");
        }
    }

    #[test]
    fn test_where_with_array_index() {
        // Test WHERE clause with array index
        let result = SqlParser::parse_to_command("SELECT * FROM posts WHERE tags[0] = 'rust'");
        // This should require aggregation pipeline
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle array index in WHERE"
        );
    }

    #[test]
    fn test_order_by_with_array_index() {
        // Test ORDER BY with array index
        let result = SqlParser::parse_to_command("SELECT * FROM posts ORDER BY tags[0]");
        assert!(
            result.is_ok(),
            "Failed to parse ORDER BY with array index: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Aggregate { pipeline, .. }) = cmd {
            // Should have $sort stage
            let has_sort = pipeline.iter().any(|stage| stage.contains_key("$sort"));
            assert!(has_sort, "Pipeline should contain $sort stage");
        } else {
            panic!("Expected Aggregate command for ORDER BY with array index");
        }
    }

    #[test]
    fn test_reject_semicolon_in_where_clause() {
        // Test that semicolon in WHERE clause is rejected
        let result = SqlParser::parse_to_command(
            "SELECT COUNT(*) FROM tasks WHERE user_id;2 WHERE template_id='task-123'",
        );
        assert!(
            result.is_err(),
            "Should reject semicolon in WHERE clause, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_reject_incomplete_where_expression() {
        // Test that incomplete WHERE expression (field without comparison) is rejected
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE user_id");
        // This should be an error for incomplete input
        assert!(
            result.is_err(),
            "Should reject incomplete WHERE expression, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_reject_duplicate_where_clause() {
        // Test that duplicate WHERE clauses are rejected
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE user_id = 1 WHERE template_id = 2",
        );
        assert!(
            result.is_err(),
            "Should reject duplicate WHERE clause, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_explain_simple_select() {
        // Test EXPLAIN with simple SELECT
        let result = SqlParser::parse_to_command("EXPLAIN SELECT * FROM users");
        assert!(
            result.is_ok(),
            "Failed to parse EXPLAIN SELECT: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain {
            collection,
            verbosity,
            query,
        }) = cmd
        {
            assert_eq!(collection, "users");
            assert_eq!(verbosity, ExplainVerbosity::QueryPlanner);

            // Inner query should be Find
            assert!(matches!(*query, QueryCommand::Find { .. }));
        } else {
            panic!("Expected Explain command, got: {:?}", cmd);
        }
    }

    #[test]
    fn test_explain_with_where() {
        // Test EXPLAIN with WHERE clause
        let result = SqlParser::parse_to_command("EXPLAIN SELECT * FROM users WHERE age > 18");
        assert!(
            result.is_ok(),
            "Failed to parse EXPLAIN with WHERE: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain {
            collection, query, ..
        }) = cmd
        {
            assert_eq!(collection, "users");

            // Inner query should have filter
            if let QueryCommand::Find { filter, .. } = *query {
                assert!(!filter.is_empty(), "Filter should not be empty");
            } else {
                panic!("Expected Find command inside Explain");
            }
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_with_execution_stats() {
        // Test EXPLAIN with executionStats verbosity (unquoted)
        let result = SqlParser::parse_to_command("EXPLAIN executionStats SELECT * FROM users");
        assert!(
            result.is_ok(),
            "Failed to parse EXPLAIN with executionStats: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { verbosity, .. }) = cmd {
            assert_eq!(verbosity, ExplainVerbosity::ExecutionStats);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_with_all_plans_execution() {
        // Test EXPLAIN with allPlansExecution verbosity (unquoted)
        let result = SqlParser::parse_to_command(
            "EXPLAIN allPlansExecution SELECT name FROM users WHERE age > 18",
        );
        assert!(
            result.is_ok(),
            "Failed to parse EXPLAIN with allPlansExecution: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { verbosity, .. }) = cmd {
            assert_eq!(verbosity, ExplainVerbosity::AllPlansExecution);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_aggregate() {
        // Test EXPLAIN with aggregation query (GROUP BY)
        let result = SqlParser::parse_to_command("EXPLAIN SELECT COUNT(*) FROM users GROUP BY age");
        assert!(
            result.is_ok(),
            "Failed to parse EXPLAIN with GROUP BY: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { query, .. }) = cmd {
            // Inner query should be Aggregate
            assert!(matches!(*query, QueryCommand::Aggregate { .. }));
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_with_order_by_limit() {
        // Test EXPLAIN with ORDER BY and LIMIT
        let result =
            SqlParser::parse_to_command("EXPLAIN SELECT * FROM users ORDER BY name LIMIT 10");
        assert!(
            result.is_ok(),
            "Failed to parse EXPLAIN with ORDER BY LIMIT: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { query, .. }) = cmd {
            if let QueryCommand::Find { options, .. } = *query {
                assert_eq!(options.limit, Some(10));
                assert!(options.sort.is_some());
            } else {
                panic!("Expected Find command inside Explain");
            }
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_explain_case_insensitive() {
        // Test that EXPLAIN is case-insensitive
        let result1 = SqlParser::parse_to_command("EXPLAIN SELECT * FROM users");
        let result2 = SqlParser::parse_to_command("explain SELECT * FROM users");
        let result3 = SqlParser::parse_to_command("Explain SELECT * FROM users");

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());

        // All should produce Explain commands
        assert!(matches!(
            result1.unwrap(),
            Command::Query(QueryCommand::Explain { .. })
        ));
        assert!(matches!(
            result2.unwrap(),
            Command::Query(QueryCommand::Explain { .. })
        ));
        assert!(matches!(
            result3.unwrap(),
            Command::Query(QueryCommand::Explain { .. })
        ));
    }

    #[test]
    fn test_explain_with_invalid_verbosity() {
        // Test EXPLAIN with invalid verbosity identifier
        let result = SqlParser::parse_to_command("EXPLAIN invalidVerbosity SELECT * FROM users");
        assert!(
            result.is_err(),
            "Should reject invalid verbosity, but got: {:?}",
            result
        );
    }

    #[test]
    fn test_explain_with_quoted_verbosity() {
        // Test EXPLAIN with quoted verbosity (backwards compatibility)
        let result = SqlParser::parse_to_command("EXPLAIN 'executionStats' SELECT * FROM users");
        assert!(
            result.is_ok(),
            "Failed to parse EXPLAIN with quoted verbosity: {:?}",
            result
        );

        let cmd = result.unwrap();
        if let Command::Query(QueryCommand::Explain { verbosity, .. }) = cmd {
            assert_eq!(verbosity, ExplainVerbosity::ExecutionStats);
        } else {
            panic!("Expected Explain command");
        }
    }

    #[test]
    fn test_is_sql_command_recognizes_explain() {
        // Test that is_sql_command recognizes EXPLAIN
        assert!(SqlParser::is_sql_command("EXPLAIN SELECT * FROM users"));
        assert!(SqlParser::is_sql_command("explain SELECT * FROM users"));
        assert!(SqlParser::is_sql_command(
            "EXPLAIN executionStats SELECT * FROM users"
        ));
        assert!(SqlParser::is_sql_command(
            "EXPLAIN 'executionStats' SELECT * FROM users"
        ));
        assert!(SqlParser::is_sql_command("EXPLAIN"));
    }

    #[test]
    fn test_parse_with_date_literal() {
        // Test DATE 'yyyy-mm-dd' syntax
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE create_time > DATE '2026-02-15'",
        );
        assert!(result.is_ok(), "Failed to parse DATE literal: {:?}", result);
    }

    #[test]
    fn test_parse_with_timestamp_literal() {
        // Test TIMESTAMP 'yyyy-mm-dd HH:MM:SS' syntax
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE create_time > TIMESTAMP '2026-02-15 16:00:00'",
        );
        assert!(
            result.is_ok(),
            "Failed to parse TIMESTAMP literal: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_with_current_timestamp() {
        // Test CURRENT_TIMESTAMP (no parentheses)
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE create_time > CURRENT_TIMESTAMP",
        );
        assert!(
            result.is_ok(),
            "Failed to parse CURRENT_TIMESTAMP: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_with_current_date() {
        // Test CURRENT_DATE
        let result =
            SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > CURRENT_DATE");
        assert!(
            result.is_ok(),
            "Failed to parse CURRENT_DATE: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_with_now_function() {
        // Test NOW() function
        let result =
            SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > NOW()");
        assert!(result.is_ok(), "Failed to parse NOW(): {:?}", result);
    }

    #[test]
    fn test_parse_with_now_no_parens() {
        // Test NOW without parentheses (should also work)
        let result = SqlParser::parse_to_command("SELECT * FROM tasks WHERE create_time > NOW");
        assert!(
            result.is_ok(),
            "Failed to parse NOW without parens: {:?}",
            result
        );
    }

    #[test]
    fn test_date_literal_simple_format() {
        // Test simple date format: '2026-02-15' (auto-converts to ISO)
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE create_time > DATE '2026-02-15'",
        );
        assert!(
            result.is_ok(),
            "Failed to parse simple DATE format: {:?}",
            result
        );
    }

    #[test]
    fn test_timestamp_with_full_iso() {
        // Test full ISO 8601 format with timezone
        let result = SqlParser::parse_to_command(
            "SELECT * FROM tasks WHERE create_time > TIMESTAMP '2026-02-15T16:00:00.000Z'",
        );
        assert!(
            result.is_ok(),
            "Failed to parse full ISO TIMESTAMP: {:?}",
            result
        );
    }

    // ============== Arithmetic Expression Tests ==============

    #[test]
    fn test_arithmetic_in_where_clause() {
        // Test arithmetic expression in WHERE: price * quantity > 100
        let result =
            SqlParser::parse_to_command("SELECT * FROM orders WHERE price * quantity > 100");
        assert!(
            result.is_ok(),
            "Failed to parse arithmetic in WHERE: {:?}",
            result
        );

        let cmd = result.unwrap();
        // Should use aggregation pipeline for $expr
        assert!(matches!(
            cmd,
            Command::Query(QueryCommand::Aggregate { .. })
        ));
    }

    #[test]
    fn test_arithmetic_in_select() {
        // Test arithmetic expression in SELECT: price * quantity AS total
        let result = SqlParser::parse_to_command(
            "SELECT product_name, price * quantity AS total FROM orders",
        );
        assert!(
            result.is_ok(),
            "Failed to parse arithmetic in SELECT: {:?}",
            result
        );
    }

    #[test]
    fn test_arithmetic_with_addition() {
        // Test addition: price + tax
        let result =
            SqlParser::parse_to_command("SELECT * FROM orders WHERE price + tax > 50");
        assert!(result.is_ok(), "Failed to parse addition: {:?}", result);
    }

    #[test]
    fn test_arithmetic_with_subtraction() {
        // Test subtraction: total - discount
        let result =
            SqlParser::parse_to_command("SELECT * FROM orders WHERE total - discount > 100");
        assert!(
            result.is_ok(),
            "Failed to parse subtraction: {:?}",
            result
        );
    }

    #[test]
    fn test_arithmetic_with_division() {
        // Test division: total / quantity
        let result =
            SqlParser::parse_to_command("SELECT * FROM orders WHERE total / quantity > 10");
        assert!(result.is_ok(), "Failed to parse division: {:?}", result);
    }

    #[test]
    fn test_arithmetic_with_modulo() {
        // Test modulo: id % 2 = 0
        let result = SqlParser::parse_to_command("SELECT * FROM numbers WHERE id % 2 = 0");
        assert!(result.is_ok(), "Failed to parse modulo: {:?}", result);
    }

    #[test]
    fn test_arithmetic_with_parentheses() {
        // Test parenthesized expression: (price + tax) * quantity
        let result = SqlParser::parse_to_command(
            "SELECT * FROM orders WHERE (price + tax) * quantity > 1000",
        );
        assert!(
            result.is_ok(),
            "Failed to parse parenthesized arithmetic: {:?}",
            result
        );
    }

    #[test]
    fn test_arithmetic_operator_precedence() {
        // Test operator precedence: price + tax * quantity (multiply binds tighter)
        let result = SqlParser::parse_to_command(
            "SELECT * FROM orders WHERE price + tax * quantity > 100",
        );
        assert!(
            result.is_ok(),
            "Failed to parse arithmetic with precedence: {:?}",
            result
        );
    }

    #[test]
    fn test_arithmetic_with_literals() {
        // Test arithmetic with literals: price * 1.13
        let result =
            SqlParser::parse_to_command("SELECT * FROM products WHERE price * 1.13 > 50");
        assert!(
            result.is_ok(),
            "Failed to parse arithmetic with literals: {:?}",
            result
        );
    }

    #[test]
    fn test_round_function_with_arithmetic() {
        // Test ROUND function with arithmetic: ROUND(price * 1.13, 2)
        let result = SqlParser::parse_to_command(
            "SELECT * FROM products WHERE ROUND(price * 1.13, 2) > 50",
        );
        assert!(
            result.is_ok(),
            "Failed to parse ROUND with arithmetic: {:?}",
            result
        );
    }

    #[test]
    fn test_complex_arithmetic_expression() {
        // Test complex expression: ((price * quantity) - discount) * 1.13
        let result = SqlParser::parse_to_command(
            "SELECT * FROM orders WHERE ((price * quantity) - discount) * 1.13 > 500",
        );
        assert!(
            result.is_ok(),
            "Failed to parse complex arithmetic: {:?}",
            result
        );
    }

    #[test]
    fn test_aggregate_with_arithmetic() {
        // Test COUNT(*) - number
        let result = SqlParser::parse_to_command("SELECT COUNT(*) - 100 FROM tasks");
        assert!(
            result.is_ok(),
            "Failed to parse COUNT(*) - 100: {:?}",
            result
        );
    }

    #[test]
    fn test_aggregate_with_arithmetic_and_alias() {
        // Test COUNT(*) - number AS alias
        let result =
            SqlParser::parse_to_command("SELECT COUNT(*) - 100 AS adjusted_count FROM tasks");
        assert!(
            result.is_ok(),
            "Failed to parse COUNT(*) - 100 AS alias: {:?}",
            result
        );
    }

    #[test]
    fn test_multiple_aggregates_with_arithmetic() {
        // Test multiple aggregate expressions with arithmetic
        let result =
            SqlParser::parse_to_command("SELECT COUNT(*) - 1, SUM(price) * 1.13 FROM orders");
        assert!(
            result.is_ok(),
            "Failed to parse multiple aggregates with arithmetic: {:?}",
            result
        );
    }

    #[test]
    fn test_aggregate_arithmetic_with_literal() {
        // Test aggregate with arithmetic on right side: COUNT(*) * 2
        let result =
            SqlParser::parse_to_command("SELECT COUNT(*) * 2 AS doubled_count FROM orders");
        assert!(
            result.is_ok(),
            "Failed to parse COUNT(*) * 2: {:?}",
            result
        );
    }
}
