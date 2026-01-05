//! Finite State Machine for completion context determination
//!
//! This module implements a simple FSM that walks through tokens to determine
//! what kind of completion should be provided. The FSM is designed to be:
//! - Simple and predictable
//! - Error-tolerant (handles incomplete input)
//! - Fast (O(n) single pass through tokens)

use super::context::CompletionContext;
use super::token_stream::{TokenStream, UnifiedToken};

/// FSM states representing different positions in a command
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionState {
    /// Initial state
    Start,

    // === Mongo Shell States ===
    /// After "db" keyword
    AfterDb,
    /// After "db." - should complete collection names
    AfterDbDot,
    /// After "db.collection"
    AfterCollection,
    /// After "db.collection." - should complete operation names
    AfterCollectionDot,

    // === SQL States ===
    /// After "FROM" keyword - should complete collection/table names
    SqlFrom,
    /// After "JOIN" keyword - should complete collection/table names
    SqlJoin,
    /// After "WHERE" keyword
    SqlWhere,

    // === Shell Command States ===
    /// After "show" command - should complete subcommands
    ShowCommand,
    /// After "use" command - should complete database names
    UseCommand,
}

impl CompletionState {
    /// Perform state transition based on current state and token
    pub fn next(self, token: &UnifiedToken) -> Self {
        use CompletionState::*;

        match (self, token) {
            // === Mongo Shell Transitions ===
            (Start, t) if t.is_db() => AfterDb,
            (AfterDb, t) if t.is_dot() => AfterDbDot,
            (AfterDbDot, t) if t.is_ident() => AfterCollection,
            (AfterCollection, t) if t.is_dot() => AfterCollectionDot,

            // === SQL Transitions ===
            (Start, t) if t.is_sql_keyword("SELECT") => Start, // Stay in Start after SELECT
            (_, t) if t.is_sql_keyword("FROM") => SqlFrom,
            (_, t)
                if t.is_sql_keyword("JOIN")
                    || t.is_sql_keyword("INNER")
                    || t.is_sql_keyword("LEFT")
                    || t.is_sql_keyword("RIGHT") =>
            {
                SqlJoin
            }
            (SqlFrom, t) if t.is_sql_keyword("WHERE") => SqlWhere,
            (SqlJoin, t) if t.is_sql_keyword("WHERE") => SqlWhere,

            // === Shell Command Transitions ===
            (Start, t) if t.ident_value() == Some("show".to_string()) => ShowCommand,
            (Start, t) if t.ident_value() == Some("use".to_string()) => UseCommand,

            // === Stay in current state for identifiers after certain states ===
            (SqlFrom, t) if t.is_ident() => SqlFrom,
            (SqlJoin, t) if t.is_ident() => SqlJoin,
            (ShowCommand, t) if t.is_ident() => ShowCommand,
            (UseCommand, t) if t.is_ident() => UseCommand,

            // === Default: maintain current state ===
            (state, _) => state,
        }
    }

    /// Run the FSM on a sequence of tokens
    pub fn run(tokens: &[UnifiedToken]) -> Self {
        let mut state = CompletionState::Start;

        for token in tokens {
            state = state.next(token);
        }

        state
    }

    /// Convert state to completion context
    pub fn to_context(&self, stream: &TokenStream) -> CompletionContext {
        use CompletionState::*;

        match self {
            // Need to complete collection names
            AfterDbDot | SqlFrom | SqlJoin => {
                CompletionContext::collection(stream.current_prefix())
            }

            // If we're in AfterCollection state but have a prefix, user is still typing collection name
            AfterCollection if !stream.current_prefix().is_empty() => {
                CompletionContext::collection(stream.current_prefix())
            }

            // Need to complete operation names
            AfterCollectionDot => CompletionContext::operation(stream.current_prefix()),

            // Need to complete "show" subcommands
            ShowCommand => CompletionContext::show_subcommand(stream.current_prefix()),

            // Need to complete database names
            UseCommand => CompletionContext::database(stream.current_prefix()),

            // At the start, complete top-level commands
            Start if !stream.current_prefix().is_empty() => {
                CompletionContext::command(stream.current_prefix())
            }

            // No completion
            _ => CompletionContext::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{MongoLexer, SqlLexer};

    #[test]
    fn test_state_mongo_db() {
        let tokens = MongoLexer::tokenize("db");
        let unified: Vec<UnifiedToken> = tokens
            .into_iter()
            .take_while(|t| !matches!(t.kind, crate::parser::MongoTokenKind::EOF))
            .map(UnifiedToken::Mongo)
            .collect();

        let state = CompletionState::run(&unified);
        assert_eq!(state, CompletionState::AfterDb);
    }

    #[test]
    fn test_state_mongo_db_dot() {
        let tokens = MongoLexer::tokenize("db.");
        let unified: Vec<UnifiedToken> = tokens
            .into_iter()
            .take_while(|t| !matches!(t.kind, crate::parser::MongoTokenKind::EOF))
            .map(UnifiedToken::Mongo)
            .collect();

        let state = CompletionState::run(&unified);
        assert_eq!(state, CompletionState::AfterDbDot);
    }

    #[test]
    fn test_state_mongo_db_collection() {
        let tokens = MongoLexer::tokenize("db.users");
        let unified: Vec<UnifiedToken> = tokens
            .into_iter()
            .take_while(|t| !matches!(t.kind, crate::parser::MongoTokenKind::EOF))
            .map(UnifiedToken::Mongo)
            .collect();

        let state = CompletionState::run(&unified);
        assert_eq!(state, CompletionState::AfterCollection);
    }

    #[test]
    fn test_state_mongo_db_collection_dot() {
        let tokens = MongoLexer::tokenize("db.users.");
        let unified: Vec<UnifiedToken> = tokens
            .into_iter()
            .take_while(|t| !matches!(t.kind, crate::parser::MongoTokenKind::EOF))
            .map(UnifiedToken::Mongo)
            .collect();

        let state = CompletionState::run(&unified);
        assert_eq!(state, CompletionState::AfterCollectionDot);
    }

    #[test]
    fn test_state_sql_from() {
        let tokens = SqlLexer::tokenize("SELECT * FROM");
        let unified: Vec<UnifiedToken> = tokens
            .into_iter()
            .take_while(|t| !matches!(t.kind, crate::parser::SqlTokenKind::EOF))
            .map(UnifiedToken::Sql)
            .collect();

        let state = CompletionState::run(&unified);
        assert_eq!(state, CompletionState::SqlFrom);
    }

    #[test]
    fn test_state_show_command() {
        let tokens = MongoLexer::tokenize("show");
        let unified: Vec<UnifiedToken> = tokens
            .into_iter()
            .take_while(|t| !matches!(t.kind, crate::parser::MongoTokenKind::EOF))
            .map(UnifiedToken::Mongo)
            .collect();

        let state = CompletionState::run(&unified);
        assert_eq!(state, CompletionState::ShowCommand);
    }

    #[test]
    fn test_state_use_command() {
        let tokens = MongoLexer::tokenize("use");
        let unified: Vec<UnifiedToken> = tokens
            .into_iter()
            .take_while(|t| !matches!(t.kind, crate::parser::MongoTokenKind::EOF))
            .map(UnifiedToken::Mongo)
            .collect();

        let state = CompletionState::run(&unified);
        assert_eq!(state, CompletionState::UseCommand);
    }

    #[test]
    fn test_context_collection() {
        // Test "db." with cursor after the dot - should complete collections
        let tokens = MongoLexer::tokenize("db.");
        let stream = TokenStream::from_mongo(tokens, 3);
        let state = CompletionState::run(stream.tokens_before_cursor());

        let context = state.to_context(&stream);
        assert_eq!(context, CompletionContext::collection(""));
    }

    #[test]
    fn test_context_operation() {
        // Test "db.users." with cursor after second dot - should complete operations
        let tokens = MongoLexer::tokenize("db.users.");
        let stream = TokenStream::from_mongo(tokens, 9);
        let state = CompletionState::run(stream.tokens_before_cursor());

        let context = state.to_context(&stream);
        assert_eq!(context, CompletionContext::operation(""));
    }

    #[test]
    fn test_context_show_subcommand() {
        // Test "show " with cursor after space - should complete show subcommands
        let tokens = MongoLexer::tokenize("show ");
        let stream = TokenStream::from_mongo(tokens, 5);
        let state = CompletionState::run(stream.tokens_before_cursor());

        let context = state.to_context(&stream);
        assert_eq!(context, CompletionContext::show_subcommand(""));
    }

    #[test]
    fn test_context_sql_from() {
        // Test "SELECT * FROM " with cursor after FROM - should complete collections
        let tokens = SqlLexer::tokenize("SELECT * FROM ");
        let stream = TokenStream::from_sql(tokens, 14);
        let state = CompletionState::run(stream.tokens_before_cursor());

        let context = state.to_context(&stream);
        assert_eq!(context, CompletionContext::collection(""));
    }
}
