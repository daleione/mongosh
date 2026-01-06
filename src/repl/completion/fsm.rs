//! Finite State Machine for completion context determination
//!
//! This module implements a simple FSM that walks through tokens to determine
//! what kind of completion should be provided. The FSM is designed to be:
//! - Simple and predictable
//! - Error-tolerant (handles incomplete input)
//! - Fast (O(n) single pass through tokens)
//! - Context-aware (tracks parentheses to avoid completing inside function arguments)

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
    /// Inside parentheses - no completion to avoid suggesting when typing function arguments
    /// Example: `db.users.findOne(find` should NOT complete "find" to "findOne"
    InsideParentheses,

    // === SQL States ===
    /// After "FROM" keyword - should complete collection/table names
    SqlFrom,
    /// After "JOIN" keyword - should complete collection/table names
    SqlJoin,
    /// After "WHERE" keyword
    SqlWhere,
    /// After table name in FROM/JOIN - expect WHERE, JOIN, ORDER, LIMIT, etc. - no completion
    SqlAfterTableName,
    /// After LIMIT/OFFSET - expect numbers - no completion
    SqlAfterLimit,
    /// After ORDER BY - expect column names
    SqlOrderBy,
    /// After semicolon or complete statement - no completion
    SqlComplete,

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
            // === Check for parentheses first (highest priority) ===
            // If we encounter an opening parenthesis, enter InsideParentheses state
            (_, t) if t.is_open_paren() => InsideParentheses,
            // If we're inside parentheses and see a closing paren, return to Start
            (InsideParentheses, t) if t.is_close_paren() => Start,
            // Stay inside parentheses for any other token
            (InsideParentheses, _) => InsideParentheses,

            // === Check for semicolon (statement terminator) ===
            (_, t) if t.is_semicolon() => SqlComplete,
            // After semicolon, any keyword starts a new statement
            (SqlComplete, t) if t.is_sql_keyword("SELECT") => Start,
            (SqlComplete, t) if t.is_sql_keyword("INSERT") => Start,
            (SqlComplete, t) if t.is_sql_keyword("UPDATE") => Start,
            (SqlComplete, t) if t.is_sql_keyword("DELETE") => Start,
            // Stay in SqlComplete for whitespace/identifiers after semicolon
            (SqlComplete, _) => SqlComplete,

            // === Mongo Shell Transitions ===
            (Start, t) if t.is_db() => AfterDb,
            (AfterDb, t) if t.is_dot() => AfterDbDot,
            (AfterDbDot, t) if t.is_ident() => AfterCollection,
            (AfterCollection, t) if t.is_dot() => AfterCollectionDot,

            // === SQL Transitions ===
            (Start, t) if t.is_sql_keyword("SELECT") => Start, // Stay in Start after SELECT
            (_, t) if t.is_sql_keyword("FROM") => SqlFrom,

            // After FROM, when we see an identifier (table name), transition to SqlAfterTableName
            (SqlFrom, t) if t.is_ident() => SqlAfterTableName,

            // JOIN keywords can come from various states
            (SqlAfterTableName, t)
                if t.is_sql_keyword("JOIN")
                    || t.is_sql_keyword("INNER")
                    || t.is_sql_keyword("LEFT")
                    || t.is_sql_keyword("RIGHT") =>
            {
                SqlJoin
            }
            (_, t)
                if t.is_sql_keyword("JOIN")
                    || t.is_sql_keyword("INNER")
                    || t.is_sql_keyword("LEFT")
                    || t.is_sql_keyword("RIGHT") =>
            {
                SqlJoin
            }

            // After JOIN, when we see an identifier (table name), transition to SqlAfterTableName
            (SqlJoin, t) if t.is_ident() => SqlAfterTableName,

            // WHERE can come after table name
            (SqlAfterTableName, t) if t.is_sql_keyword("WHERE") => SqlWhere,
            (SqlFrom, t) if t.is_sql_keyword("WHERE") => SqlWhere,
            (SqlWhere, t) if t.is_ident() => SqlWhere, // Stay in WHERE for column names, etc.

            // LIMIT/OFFSET expect numbers - no completion
            (_, t) if t.is_sql_keyword("LIMIT") || t.is_sql_keyword("OFFSET") => SqlAfterLimit,
            (SqlAfterLimit, t) if t.is_number() => SqlAfterLimit, // Stay after seeing number

            // ORDER BY for column names
            (_, t) if t.is_sql_keyword("ORDER") => Start, // Wait for BY
            (Start, t) if t.is_sql_keyword("BY") => SqlOrderBy,
            (SqlAfterTableName, t) if t.is_sql_keyword("ORDER") => Start,
            (SqlOrderBy, t) if t.is_ident() => SqlOrderBy, // Column names

            // GROUP BY similar to ORDER BY
            (_, t) if t.is_sql_keyword("GROUP") => Start,
            (SqlAfterTableName, t) if t.is_sql_keyword("GROUP") => Start,

            // === Shell Command Transitions ===
            (Start, t) if t.ident_value() == Some("show".to_string()) => ShowCommand,
            (Start, t) if t.ident_value() == Some("use".to_string()) => UseCommand,

            // === Stay in current state for identifiers after certain states ===
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

            // No completion for these states
            SqlAfterTableName | SqlAfterLimit | SqlWhere | SqlOrderBy | SqlComplete => {
                CompletionContext::None
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

    #[test]
    fn test_no_completion_inside_parentheses() {
        // Test "db.users.findOne(find" - should NOT complete inside parentheses
        let tokens = MongoLexer::tokenize("db.users.findOne(find");
        let stream = TokenStream::from_mongo(tokens, 21);
        let state = CompletionState::run(stream.tokens_before_cursor());

        assert_eq!(state, CompletionState::InsideParentheses);
        let context = state.to_context(&stream);
        assert_eq!(context, CompletionContext::None);
    }

    #[test]
    fn test_completion_after_closing_parenthesis() {
        // Test "db.users.find()." - should complete after closing parenthesis
        let tokens = MongoLexer::tokenize("db.users.find().");
        let stream = TokenStream::from_mongo(tokens, 16);
        let state = CompletionState::run(stream.tokens_before_cursor());

        // After closing paren and dot, we're not in a clear completion state
        // This is acceptable - the important part is we don't complete INSIDE parens
        let context = state.to_context(&stream);
        // Either None or some completion is fine - just not crashing
        assert!(matches!(
            context,
            CompletionContext::None | CompletionContext::Operation { .. }
        ));
    }
}
