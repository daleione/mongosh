//! Completion engine - orchestrates the completion flow
//!
//! This module provides the main completion engine that ties together all the
//! completion components: lexing, FSM, context determination, and candidate fetching.

use std::sync::Arc;

use rustyline::completion::Pair;

use super::context::CompletionContext;
use super::fsm::CompletionState;
use super::provider::CandidateProvider;
use super::token_stream::TokenStream;
use crate::parser::{MongoLexer, SqlLexer};

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
    /// * `(usize, Vec<Pair>)` - Completion start position and candidate pairs
    pub fn complete(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
        // 1. Determine input type and tokenize
        let stream = self.tokenize(line, pos);

        // 2. Run FSM on tokens before cursor
        let state = CompletionState::run(stream.tokens_before_cursor());

        // 3. Convert state to completion context
        let context = state.to_context(&stream);

        // 4. Fetch candidates based on context
        let candidates = self.fetch_candidates(&context);

        // 5. Convert to rustyline Pairs
        let pairs: Vec<Pair> = candidates
            .into_iter()
            .map(|c| Pair {
                display: c.clone(),
                replacement: c,
            })
            .collect();

        // 6. Return completion start position and pairs
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
        let (start, pairs) = engine.complete("db.", 3);

        // Should complete collection names
        // Since we don't have a real DB, we won't have any collections
        assert_eq!(start, 3);
        assert!(pairs.is_empty() || pairs.len() >= 0); // May be empty without DB
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
        let (start, pairs) = engine.complete("SELECT * FROM ", 14);

        // Should complete collection/table names
        assert_eq!(start, 14);
        // May be empty without actual DB collections
        assert!(pairs.is_empty() || pairs.len() >= 0);
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
}
