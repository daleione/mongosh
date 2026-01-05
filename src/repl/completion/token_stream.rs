//! Token stream with cursor awareness for completion
//!
//! This module provides a unified token stream that wraps both SQL and Mongo tokens
//! and tracks the cursor position for intelligent completion.

use crate::parser::{MongoToken, MongoTokenKind, SqlToken, SqlTokenKind};

/// Unified token wrapper that can represent both SQL and Mongo tokens
#[derive(Debug, Clone)]
pub enum UnifiedToken {
    /// SQL token
    Sql(SqlToken),
    /// Mongo shell token
    Mongo(MongoToken),
}

impl UnifiedToken {
    /// Check if this token is an identifier
    pub fn is_ident(&self) -> bool {
        match self {
            UnifiedToken::Sql(t) => matches!(t.kind, SqlTokenKind::Ident(_)),
            UnifiedToken::Mongo(t) => matches!(t.kind, MongoTokenKind::Ident(_)),
        }
    }

    /// Get the identifier value if this is an identifier token
    pub fn ident_value(&self) -> Option<String> {
        match self {
            UnifiedToken::Sql(t) => {
                if let SqlTokenKind::Ident(s) = &t.kind {
                    Some(s.clone())
                } else {
                    None
                }
            }
            UnifiedToken::Mongo(t) => {
                if let MongoTokenKind::Ident(s) = &t.kind {
                    Some(s.clone())
                } else {
                    None
                }
            }
        }
    }

    /// Check if this token is a dot
    pub fn is_dot(&self) -> bool {
        match self {
            UnifiedToken::Sql(t) => matches!(t.kind, SqlTokenKind::Dot),
            UnifiedToken::Mongo(t) => matches!(t.kind, MongoTokenKind::Dot),
        }
    }

    /// Check if this token is an opening parenthesis
    pub fn is_open_paren(&self) -> bool {
        match self {
            UnifiedToken::Mongo(t) => matches!(t.kind, MongoTokenKind::LParen),
            _ => false,
        }
    }

    /// Check if this token is a closing parenthesis
    pub fn is_close_paren(&self) -> bool {
        match self {
            UnifiedToken::Mongo(t) => matches!(t.kind, MongoTokenKind::RParen),
            _ => false,
        }
    }

    /// Check if this token is "db" keyword
    pub fn is_db(&self) -> bool {
        match self {
            UnifiedToken::Mongo(t) => matches!(t.kind, MongoTokenKind::Db),
            _ => false,
        }
    }

    /// Check if this is a SQL keyword
    pub fn is_sql_keyword(&self, keyword: &str) -> bool {
        match self {
            UnifiedToken::Sql(t) => match keyword.to_uppercase().as_str() {
                "SELECT" => matches!(t.kind, SqlTokenKind::Select),
                "FROM" => matches!(t.kind, SqlTokenKind::From),
                "WHERE" => matches!(t.kind, SqlTokenKind::Where),
                "JOIN" => matches!(t.kind, SqlTokenKind::Join),
                "INNER" => matches!(t.kind, SqlTokenKind::Inner),
                "LEFT" => matches!(t.kind, SqlTokenKind::Left),
                "RIGHT" => matches!(t.kind, SqlTokenKind::Right),
                _ => false,
            },
            _ => false,
        }
    }

    /// Get the span (position range) of this token
    pub fn span(&self) -> std::ops::Range<usize> {
        match self {
            UnifiedToken::Sql(t) => t.span.clone(),
            UnifiedToken::Mongo(t) => t.span.clone(),
        }
    }
}

/// Token stream with cursor position tracking
pub struct TokenStream {
    /// All tokens (including EOF)
    pub tokens: Vec<UnifiedToken>,
    /// Cursor position (byte index in the original input)
    pub cursor: usize,
    /// Index of the token at or after the cursor
    pub token_index: usize,
}

impl TokenStream {
    /// Create a new token stream from SQL tokens
    pub fn from_sql(sql_tokens: Vec<SqlToken>, cursor: usize) -> Self {
        let tokens: Vec<UnifiedToken> = sql_tokens.into_iter().map(UnifiedToken::Sql).collect();

        let token_index = Self::find_token_at_cursor(&tokens, cursor);

        Self {
            tokens,
            cursor,
            token_index,
        }
    }

    /// Create a new token stream from Mongo tokens
    pub fn from_mongo(mongo_tokens: Vec<MongoToken>, cursor: usize) -> Self {
        let tokens: Vec<UnifiedToken> = mongo_tokens.into_iter().map(UnifiedToken::Mongo).collect();

        let token_index = Self::find_token_at_cursor(&tokens, cursor);

        Self {
            tokens,
            cursor,
            token_index,
        }
    }

    /// Find the token at the cursor position
    fn find_token_at_cursor(tokens: &[UnifiedToken], cursor: usize) -> usize {
        for (i, token) in tokens.iter().enumerate() {
            let span = token.span();
            // If cursor is strictly within this token
            if cursor > span.start && cursor < span.end {
                return i;
            }
            // If cursor is at the exact start of this token
            if cursor == span.start {
                return i;
            }
        }
        // Cursor is at or after the last token - return last valid index
        tokens.len().saturating_sub(1)
    }

    /// Get all tokens before the cursor (excluding the token at cursor)
    pub fn tokens_before_cursor(&self) -> &[UnifiedToken] {
        &self.tokens[..self.token_index]
    }

    /// Get the current token (the one at or after the cursor)
    pub fn current_token(&self) -> Option<&UnifiedToken> {
        self.tokens.get(self.token_index)
    }

    /// Get the current prefix being typed (if cursor is in the middle of an identifier)
    pub fn current_prefix(&self) -> String {
        if let Some(token) = self.current_token() {
            let span = token.span();
            // If cursor is within this token and it's an identifier, extract the prefix
            if self.cursor >= span.start && self.cursor <= span.end {
                if let Some(ident) = token.ident_value() {
                    let chars_typed = self.cursor - span.start;
                    return ident.chars().take(chars_typed).collect();
                }
            }
        }

        // If current token is not an identifier (e.g., EOF), check the previous token
        if self.token_index > 0 {
            if let Some(prev_token) = self.tokens.get(self.token_index - 1) {
                let span = prev_token.span();
                // If cursor is exactly at the end of the previous token and it's an identifier
                if self.cursor == span.end {
                    if let Some(ident) = prev_token.ident_value() {
                        return ident;
                    }
                }
            }
        }

        String::new()
    }

    /// Get the completion start position (where to insert the completion)
    pub fn completion_start(&self) -> usize {
        if let Some(token) = self.current_token() {
            let span = token.span();
            // If we're in the middle of an identifier token, start at the token's beginning
            if self.cursor >= span.start && self.cursor <= span.end && token.is_ident() {
                return span.start;
            }
        }

        // If current token is EOF (or not an identifier) and cursor is at end of previous identifier,
        // return the start of that identifier
        if self.token_index > 0 {
            if let Some(prev_token) = self.tokens.get(self.token_index - 1) {
                let span = prev_token.span();
                if self.cursor == span.end && prev_token.is_ident() {
                    return span.start;
                }
            }
        }

        // Otherwise, start at cursor position
        self.cursor
    }

    /// Check if the stream is empty (only EOF)
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tokens.len() <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{MongoLexer, SqlLexer};

    #[test]
    fn test_unified_token_mongo_ident() {
        let tokens = MongoLexer::tokenize("users");
        let unified = UnifiedToken::Mongo(tokens[0].clone());

        assert!(unified.is_ident());
        assert_eq!(unified.ident_value(), Some("users".to_string()));
        assert!(!unified.is_dot());
        assert!(!unified.is_db());
    }

    #[test]
    fn test_unified_token_mongo_db() {
        let tokens = MongoLexer::tokenize("db");
        let unified = UnifiedToken::Mongo(tokens[0].clone());

        assert!(!unified.is_ident());
        assert!(unified.is_db());
    }

    #[test]
    fn test_unified_token_sql_ident() {
        let tokens = SqlLexer::tokenize("users");
        let unified = UnifiedToken::Sql(tokens[0].clone());

        assert!(unified.is_ident());
        assert_eq!(unified.ident_value(), Some("users".to_string()));
    }

    #[test]
    fn test_unified_token_sql_keyword() {
        let tokens = SqlLexer::tokenize("SELECT * FROM users");
        let select_token = UnifiedToken::Sql(tokens[0].clone());
        let from_token = UnifiedToken::Sql(tokens[2].clone());

        assert!(select_token.is_sql_keyword("SELECT"));
        assert!(!select_token.is_sql_keyword("FROM"));
        assert!(from_token.is_sql_keyword("FROM"));
    }

    #[test]
    fn test_token_stream_mongo() {
        let tokens = MongoLexer::tokenize("db.users");
        let stream = TokenStream::from_mongo(tokens, 8); // cursor at end

        assert_eq!(stream.tokens.len(), 4); // db, ., users, EOF
        assert_eq!(stream.cursor, 8);
    }

    #[test]
    fn test_token_stream_sql() {
        let tokens = SqlLexer::tokenize("SELECT * FROM users");
        let stream = TokenStream::from_sql(tokens, 19); // cursor at end

        assert!(!stream.is_empty());
        assert_eq!(stream.cursor, 19);
    }

    #[test]
    fn test_tokens_before_cursor() {
        let tokens = MongoLexer::tokenize("db.users");
        // Tokens: Db(0..2), Dot(2..3), Ident("users")(3..8), EOF(8..8)
        let stream = TokenStream::from_mongo(tokens, 3); // cursor at start of "users"

        let before = stream.tokens_before_cursor();
        // Cursor at position 3 should be at token index 2 (the "users" token)
        // tokens_before_cursor should return tokens [0..2): Db and Dot
        assert_eq!(before.len(), 2);
    }

    #[test]
    fn test_current_prefix() {
        let tokens = MongoLexer::tokenize("db.us");
        // Tokens: Db(0..2), Dot(2..3), Ident("us")(3..5), EOF(5..5)
        let stream = TokenStream::from_mongo(tokens, 4); // cursor in middle of "us"

        let prefix = stream.current_prefix();
        // Cursor at position 4 is in the middle of "us" token (3..5)
        // chars_typed = 4 - 3 = 1, so we get first char: "u"
        assert_eq!(prefix, "u");
    }

    #[test]
    fn test_completion_start() {
        let tokens = MongoLexer::tokenize("db.users");
        // Tokens: Db(0..2), Dot(2..3), Ident("users")(3..8), EOF(8..8)
        let stream = TokenStream::from_mongo(tokens, 6); // cursor in middle of "users" (after "use")

        let start = stream.completion_start();
        // Cursor at position 6 is within "users" token, so start should be 3
        assert_eq!(start, 3);
    }

    #[test]
    fn test_empty_stream() {
        let tokens = MongoLexer::tokenize("");
        let stream = TokenStream::from_mongo(tokens, 0);

        assert!(stream.is_empty());
    }
}
