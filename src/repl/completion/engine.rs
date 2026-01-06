//! Completion engine - orchestrates the completion flow
//!
//! This module provides the main completion engine that ties together all the
//! completion components: lexing, FSM, context determination, and candidate fetching.

use std::sync::Arc;

use super::context::CompletionContext;
use super::fsm::CompletionState;
use super::provider::CandidateProvider;
use super::token_stream::TokenStream;
use crate::parser::{MongoLexer, SqlLexer};

/// Completion pair representing a candidate suggestion
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionPair {
    /// Display text for the candidate
    pub display: String,
    /// Replacement text to insert
    pub replacement: String,
    /// Optional description for the candidate
    pub description: Option<String>,
}

/// Main completion engine
pub struct CompletionEngine {
    /// Candidate provider for fetching suggestions
    provider: Arc<dyn CandidateProvider>,
}

impl CompletionEngine {
    /// Create a new completion engine
    ///
    /// # Arguments
    /// * `provider` - Candidate provider for fetching suggestions
    pub fn new(provider: Arc<dyn CandidateProvider>) -> Self {
        Self { provider }
    }

    /// Complete the input at the given cursor position
    ///
    /// # Arguments
    /// * `line` - The input line
    /// * `pos` - Cursor position (byte index)
    ///
    /// # Returns
    /// * `(usize, Vec<CompletionPair>)` - Completion start position and candidate pairs
    pub fn complete(&self, line: &str, pos: usize) -> (usize, Vec<CompletionPair>) {
        // 1. Determine input type and tokenize
        let stream = self.tokenize(line, pos);

        // 2. Run FSM on tokens before cursor
        let state = CompletionState::run(stream.tokens_before_cursor());

        // 3. Convert state to completion context
        let context = state.to_context(&stream);

        // 4. Fetch candidates based on context
        let mut candidates = self.fetch_candidates(&context);

        // 5. Optimize: if prefix exactly matches a candidate, remove it from the list
        // This way, TAB will cycle through other options without showing the already-typed text
        let prefix = stream.current_prefix();
        if !prefix.is_empty() {
            candidates.retain(|c| c != &prefix);
        }

        // 6. Convert to CompletionPairs
        let pairs: Vec<CompletionPair> = candidates
            .into_iter()
            .map(|c| CompletionPair {
                display: c.clone(),
                replacement: c,
                description: None,
            })
            .collect();

        // 7. Return completion start position and pairs
        (stream.completion_start(), pairs)
    }

    /// Tokenize the input based on its type
    fn tokenize(&self, line: &str, cursor: usize) -> TokenStream {
        // Detect if this is SQL or Mongo shell syntax
        if Self::is_sql_command(line) {
            let tokens = SqlLexer::tokenize(line);
            TokenStream::from_sql(tokens, cursor)
        } else {
            let tokens = MongoLexer::tokenize(line);
            TokenStream::from_mongo(tokens, cursor)
        }
    }

    /// Check if the input is a SQL command
    fn is_sql_command(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.to_uppercase().starts_with("SELECT")
    }

    /// Fetch candidates based on completion context
    fn fetch_candidates(&self, context: &CompletionContext) -> Vec<String> {
        match context {
            CompletionContext::Collection { prefix } => self.provider.collections(prefix),
            CompletionContext::Operation { prefix } => self.provider.operations(prefix),
            CompletionContext::ShowSubcommand { prefix } => self.provider.show_subcommands(prefix),
            CompletionContext::Database { prefix } => self.provider.databases(prefix),
            CompletionContext::Command { prefix } => self.provider.commands(prefix),
            CompletionContext::None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::SharedState;
    use crate::repl::completion::provider::MongoCandidateProvider;

    fn create_test_engine() -> CompletionEngine {
        let shared_state = SharedState::new("test".to_string());
        let provider = Arc::new(MongoCandidateProvider::new(shared_state, None));
        CompletionEngine::new(provider)
    }

    #[test]
    fn test_complete_db_dot() {
        let engine = create_test_engine();
        let (start, _pairs) = engine.complete("db.", 3);

        // Should complete collection names
        // Since we don't have a real DB, we won't have any collections
        assert_eq!(start, 3);
        // May be empty without DB (no assertion needed, just documenting)
    }

    #[test]
    fn test_complete_collection_dot() {
        let engine = create_test_engine();
        let (start, pairs) = engine.complete("db.users.", 9);

        // Should complete operation names
        assert_eq!(start, 9);
        assert!(!pairs.is_empty());
        assert!(pairs.iter().any(|p| p.replacement == "find"));
        assert!(pairs.iter().any(|p| p.replacement == "insertOne"));
    }

    #[test]
    fn test_complete_operation_prefix() {
        let engine = create_test_engine();
        let (start, pairs) = engine.complete("db.users.fi", 11);

        // Should complete operations starting with "fi"
        assert_eq!(start, 9); // Start of "fi"
        assert!(pairs.iter().any(|p| p.replacement == "find"));
        assert!(pairs.iter().any(|p| p.replacement == "findOne"));
        assert!(!pairs.iter().any(|p| p.replacement == "insertOne"));
    }

    #[test]
    fn test_complete_collection_prefix() {
        let engine = create_test_engine();
        let (start, _pairs) = engine.complete("db.tes", 6);

        // Should complete collections starting with "tes"
        // Tokens: Db(0..2), Dot(2..3), Ident("tes")(3..6), EOF(6..6)
        // Cursor at 6 is at the end of "tes"
        // completion_start should return 3 (start of "tes")
        assert_eq!(start, 3);
        // Since we don't have real collections, we can't test specific matches
        // but the mechanism should work
    }

    #[test]
    fn test_complete_collection_single_char() {
        let engine = create_test_engine();

        // Test "db.t" with single character prefix
        let (start, _pairs) = engine.complete("db.t", 4);

        // Tokens: Db(0..2), Dot(2..3), Ident("t")(3..4), EOF(4..4)
        // Cursor at 4 is at the end of "t"
        // completion_start should return 3 (start of "t")
        assert_eq!(start, 3);
        // Even with single character, completion should work
        // (though we may not have real collections to test)
    }

    #[test]
    fn test_complete_exact_match_optimization() {
        let engine = create_test_engine();

        // When "find" is typed completely, "find" should be removed from candidates
        let (_start, pairs) = engine.complete("db.users.find", 13);

        // "find" should not be in the candidate list at all
        assert!(!pairs.iter().any(|p| p.replacement == "find"));
        // Other operations starting with "find" should still be available
        if !pairs.is_empty() {
            assert!(pairs.iter().all(|p| p.replacement.starts_with("find")));
            assert!(pairs.iter().any(|p| p.replacement == "findOne"));
        }
    }

    #[test]
    fn test_complete_show_command() {
        let engine = create_test_engine();
        let (start, pairs) = engine.complete("show ", 5);

        // Should complete "show" subcommands
        assert_eq!(start, 5);
        assert!(pairs.iter().any(|p| p.replacement == "dbs"));
        assert!(pairs.iter().any(|p| p.replacement == "collections"));
    }

    #[test]
    fn test_complete_show_command_prefix() {
        let engine = create_test_engine();
        let (_start, pairs) = engine.complete("show c", 6);

        // Should complete "show" subcommands starting with "c"
        assert!(pairs.iter().any(|p| p.replacement == "collections"));
        assert!(!pairs.iter().any(|p| p.replacement == "dbs"));
    }

    #[test]
    fn test_complete_use_command() {
        let engine = create_test_engine();
        let (start, pairs) = engine.complete("use ", 4);

        // Should complete database names
        assert_eq!(start, 4);
        assert!(pairs.iter().any(|p| p.replacement == "test"));
    }

    #[test]
    fn test_complete_sql_from() {
        let engine = create_test_engine();
        let (start, _pairs) = engine.complete("SELECT * FROM ", 14);

        // Should complete collection/table names
        assert_eq!(start, 14);
        // May be empty without actual DB collections (no assertion needed, just documenting)
    }

    #[test]
    fn test_complete_top_level_command() {
        let engine = create_test_engine();
        let (_start, pairs) = engine.complete("sh", 2);

        // Should complete top-level commands starting with "sh"
        assert!(pairs.iter().any(|p| p.replacement == "show"));
        assert!(!pairs.iter().any(|p| p.replacement == "use"));
    }

    #[test]
    fn test_complete_empty_input() {
        let engine = create_test_engine();
        let (_start, pairs) = engine.complete("", 0);

        // Should return no completions for empty input
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_is_sql_command() {
        assert!(CompletionEngine::is_sql_command("SELECT * FROM users"));
        assert!(CompletionEngine::is_sql_command("select * from users"));
        assert!(CompletionEngine::is_sql_command("  SELECT"));
        assert!(!CompletionEngine::is_sql_command("db.users.find()"));
        assert!(!CompletionEngine::is_sql_command("show dbs"));
    }

    #[test]
    fn test_tokenize_mongo() {
        let engine = create_test_engine();
        let stream = engine.tokenize("db.users", 8);

        assert!(!stream.is_empty());
        assert_eq!(stream.cursor, 8);
    }

    #[test]
    fn test_tokenize_sql() {
        let engine = create_test_engine();
        let stream = engine.tokenize("SELECT * FROM users", 19);

        assert!(!stream.is_empty());
        assert_eq!(stream.cursor, 19);
    }

    #[test]
    fn test_completion_partial_table_name() {
        let engine = create_test_engine();

        // Test with partial table name "tem" - should complete to "templates"
        let (start, _pairs) = engine.complete("SELECT * FROM tem", 17);
        assert_eq!(start, 14, "Should start completion at beginning of 'tem'");
        // Pairs may be empty without real DB, but the important thing is we're attempting completion

        // Test with single character
        let (start, _pairs) = engine.complete("SELECT * FROM t", 15);
        assert_eq!(start, 14, "Should complete even with single character");

        // Test with longer prefix
        let (start, _pairs) = engine.complete("SELECT * FROM templates_", 24);
        assert_eq!(start, 14, "Should complete with longer prefix");
    }

    #[test]
    fn test_completion_partial_table_name_after_join() {
        let engine = create_test_engine();

        // Test with partial table name after JOIN
        let (start, _pairs) = engine.complete("SELECT * FROM users JOIN tem", 28);
        assert_eq!(
            start, 25,
            "Should start completion at beginning of 'tem' after JOIN"
        );
    }

    #[test]
    fn test_no_completion_after_sql_semicolon() {
        let engine = create_test_engine();

        // Test with semicolon at the end
        let (_start, pairs) = engine.complete("SELECT * FROM users;", 20);
        assert!(pairs.is_empty(), "Should not complete after semicolon");

        // Test with semicolon and whitespace
        let (_start, pairs) = engine.complete("SELECT * FROM users; ", 21);
        assert!(
            pairs.is_empty(),
            "Should not complete after semicolon with space"
        );

        // Test with semicolon and multiple spaces
        let (_start, pairs) = engine.complete("SELECT * FROM users;   ", 23);
        assert!(
            pairs.is_empty(),
            "Should not complete after semicolon with multiple spaces"
        );
    }

    #[test]
    fn test_no_completion_after_sql_limit() {
        let engine = create_test_engine();

        // Test after LIMIT - should not complete (expects number)
        let (_start, pairs) = engine.complete("SELECT * FROM users LIMIT ", 26);
        assert!(
            pairs.is_empty(),
            "Should not complete after LIMIT (expects number)"
        );

        // Test after LIMIT with partial number
        let (_start, pairs) = engine.complete("SELECT * FROM users LIMIT 1", 27);
        assert!(
            pairs.is_empty(),
            "Should not complete after LIMIT with number"
        );
    }

    #[test]
    fn test_no_completion_after_sql_offset() {
        let engine = create_test_engine();

        // Test after OFFSET - should not complete (expects number)
        let (_start, pairs) = engine.complete("SELECT * FROM users OFFSET ", 27);
        assert!(
            pairs.is_empty(),
            "Should not complete after OFFSET (expects number)"
        );
    }

    #[test]
    fn test_no_completion_after_table_name() {
        let engine = create_test_engine();

        // After table name, before WHERE/JOIN/etc - no completion
        let (_start, pairs) = engine.complete("SELECT * FROM users ", 20);
        assert!(
            pairs.is_empty(),
            "Should not complete after table name (expects WHERE, JOIN, etc.)"
        );
    }

    #[test]
    fn test_completion_at_from() {
        let engine = create_test_engine();

        // Should complete table names after FROM
        let (start, _pairs) = engine.complete("SELECT * FROM ", 14);
        assert_eq!(start, 14, "Should start completion after FROM");
        // Pairs may be empty without real DB, but position should be correct
    }

    #[test]
    fn test_no_completion_in_where_clause() {
        let engine = create_test_engine();

        // After WHERE - no completion (would need column name completion, not implemented yet)
        let (_start, pairs) = engine.complete("SELECT * FROM users WHERE ", 26);
        assert!(
            pairs.is_empty(),
            "Should not complete in WHERE clause (column completion not implemented)"
        );
    }

    #[test]
    fn test_partial_collection_name_mongo() {
        let engine = create_test_engine();

        // Test with partial collection name after "db."
        let (start, _pairs) = engine.complete("db.us", 5);
        assert_eq!(start, 3, "Should start completion at beginning of 'us'");

        // Test with single character
        let (start, _pairs) = engine.complete("db.u", 4);
        assert_eq!(start, 3, "Should complete even with single character");

        // Test with longer prefix
        let (start, _pairs) = engine.complete("db.users_col", 12);
        assert_eq!(start, 3, "Should complete with longer prefix");
    }

    #[test]
    fn test_partial_operation_name() {
        let engine = create_test_engine();

        // Test with partial operation name after "db.collection."
        let (start, pairs) = engine.complete("db.users.fin", 12);
        assert_eq!(start, 9, "Should start completion at beginning of 'fin'");
        // Should get "find" and "findOne"
        assert!(pairs.iter().any(|p| p.replacement == "find"));
        assert!(pairs.iter().any(|p| p.replacement == "findOne"));

        // Test with single character
        let (start, pairs) = engine.complete("db.users.f", 10);
        assert_eq!(start, 9, "Should complete even with single character");
        assert!(!pairs.is_empty());
    }

    #[test]
    fn test_partial_show_subcommand() {
        let engine = create_test_engine();

        // Test with partial show subcommand
        let (_start, pairs) = engine.complete("show d", 6);
        assert!(pairs.iter().any(|p| p.replacement == "dbs"));
        assert!(!pairs.iter().any(|p| p.replacement == "collections"));

        // Test with "col" prefix
        let (_start, pairs) = engine.complete("show col", 8);
        assert!(pairs.iter().any(|p| p.replacement == "collections"));
        assert!(!pairs.iter().any(|p| p.replacement == "dbs"));
    }

    #[test]
    fn test_partial_use_database() {
        let engine = create_test_engine();

        // Test with partial database name
        let (start, pairs) = engine.complete("use tes", 7);
        assert_eq!(start, 4, "Should start completion at beginning of 'tes'");
        // Should include "test" database
        assert!(pairs.iter().any(|p| p.replacement == "test"));
    }

    #[test]
    fn test_partial_top_level_command() {
        let engine = create_test_engine();

        // Test with partial command at start
        let (_start, pairs) = engine.complete("sh", 2);
        assert!(pairs.iter().any(|p| p.replacement == "show"));
        assert!(!pairs.iter().any(|p| p.replacement == "use"));

        // Test with "us" prefix
        let (_start, pairs) = engine.complete("us", 2);
        assert!(pairs.iter().any(|p| p.replacement == "use"));
        assert!(!pairs.iter().any(|p| p.replacement == "show"));
    }

    #[test]
    fn test_complete_after_partial_keyword() {
        let engine = create_test_engine();

        // Even if user types partial SQL keyword, it should be treated as identifier
        // and we should handle it gracefully
        let (_start, _pairs) = engine.complete("SEL", 3);
        // Should not panic or error
    }

    #[test]
    fn test_no_completion_after_complete_table_name_with_space() {
        let engine = create_test_engine();

        // After complete table name with space, no completion until next keyword
        let (_start, pairs) = engine.complete("SELECT * FROM users ", 20);
        assert!(
            pairs.is_empty(),
            "Should not complete after complete table name (expects keyword)"
        );
    }
}
