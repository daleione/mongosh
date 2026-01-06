//! Mongo Shell lexer for error-tolerant tokenization
//!
//! This lexer is designed to handle Mongo Shell syntax like `db.collection.operation()`
//! It is extremely forgiving and never panics, making it ideal for autocomplete scenarios.
//!
//! # Design Principles
//!
//! - **Never panic** - always return a valid token stream
//! - **Never reject input** - unknown characters become `Unknown` tokens
//! - **Simple grammar** - only handles `db.collection.operation` patterns
//! - **Performance** - simple character-by-character scanning

use std::ops::Range;

/// Token types for Mongo shell syntax
#[derive(Debug, Clone, PartialEq)]
pub enum MongoTokenKind {
    /// "db" keyword
    Db,
    /// Identifier (collection name, operation name, etc.)
    Ident(String),
    /// Dot separator
    Dot,
    /// Left parenthesis
    LParen,
    /// Right parenthesis
    RParen,
    /// Left brace
    LBrace,
    /// Right brace
    RBrace,
    /// Left bracket
    LBracket,
    /// Right bracket
    RBracket,
    /// Comma
    Comma,
    /// Colon
    Colon,
    /// Semicolon
    Semicolon,
    /// String literal
    String(String),
    /// Number literal
    Number(String),
    /// End of file
    EOF,
    /// Unknown character
    Unknown(char),
}

/// Token with position information
#[derive(Debug, Clone)]
pub struct MongoToken {
    pub kind: MongoTokenKind,
    pub span: Range<usize>,
}

impl MongoToken {
    /// Create a new token
    pub fn new(kind: MongoTokenKind, span: Range<usize>) -> Self {
        Self { kind, span }
    }
}

/// Mongo Shell Lexer - error-tolerant tokenizer
pub struct MongoLexer {
    input: Vec<char>,
    pos: usize,
}

impl MongoLexer {
    /// Create a new lexer from input string
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(input: &str) -> Vec<MongoToken> {
        let mut lexer = Self::new(input);
        let mut tokens = Vec::new();

        loop {
            let token = lexer.next_token();
            let is_eof = matches!(token.kind, MongoTokenKind::EOF);
            tokens.push(token);
            if is_eof {
                break;
            }
        }

        tokens
    }

    /// Get the next token
    fn next_token(&mut self) -> MongoToken {
        self.skip_whitespace();

        let start = self.pos;

        if self.is_at_end() {
            return MongoToken::new(MongoTokenKind::EOF, start..start);
        }

        let ch = self.current_char();

        match ch {
            '.' => {
                self.advance();
                MongoToken::new(MongoTokenKind::Dot, start..self.pos)
            }
            '(' => {
                self.advance();
                MongoToken::new(MongoTokenKind::LParen, start..self.pos)
            }
            ')' => {
                self.advance();
                MongoToken::new(MongoTokenKind::RParen, start..self.pos)
            }
            '{' => {
                self.advance();
                MongoToken::new(MongoTokenKind::LBrace, start..self.pos)
            }
            '}' => {
                self.advance();
                MongoToken::new(MongoTokenKind::RBrace, start..self.pos)
            }
            '[' => {
                self.advance();
                MongoToken::new(MongoTokenKind::LBracket, start..self.pos)
            }
            ']' => {
                self.advance();
                MongoToken::new(MongoTokenKind::RBracket, start..self.pos)
            }
            ',' => {
                self.advance();
                MongoToken::new(MongoTokenKind::Comma, start..self.pos)
            }
            ':' => {
                self.advance();
                MongoToken::new(MongoTokenKind::Colon, start..self.pos)
            }
            ';' => {
                self.advance();
                MongoToken::new(MongoTokenKind::Semicolon, start..self.pos)
            }
            '\'' | '"' => self.scan_string(ch, start),
            '0'..='9' => self.scan_number(start),
            'a'..='z' | 'A'..='Z' | '_' | '$' => self.scan_identifier(start),
            _ => {
                self.advance();
                MongoToken::new(MongoTokenKind::Unknown(ch), start..self.pos)
            }
        }
    }

    /// Scan a string literal
    fn scan_string(&mut self, quote: char, start: usize) -> MongoToken {
        self.advance(); // Skip opening quote

        let mut value = String::new();

        while !self.is_at_end() && self.current_char() != quote {
            let ch = self.current_char();
            if ch == '\\' && !self.is_at_end() {
                self.advance();
                // Handle escape sequences
                match self.current_char() {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '\'' => value.push('\''),
                    '"' => value.push('"'),
                    ch => {
                        value.push('\\');
                        value.push(ch);
                    }
                }
            } else {
                value.push(ch);
            }
            self.advance();
        }

        // Skip closing quote if present
        if self.current_char() == quote {
            self.advance();
        }

        MongoToken::new(MongoTokenKind::String(value), start..self.pos)
    }

    /// Scan a number (integer or decimal)
    fn scan_number(&mut self, start: usize) -> MongoToken {
        let mut value = String::new();

        while !self.is_at_end() && self.current_char().is_ascii_digit() {
            value.push(self.current_char());
            self.advance();
        }

        // Handle decimal point
        if self.current_char() == '.' && self.peek_char().is_ascii_digit() {
            value.push('.');
            self.advance();
            while !self.is_at_end() && self.current_char().is_ascii_digit() {
                value.push(self.current_char());
                self.advance();
            }
        }

        MongoToken::new(MongoTokenKind::Number(value), start..self.pos)
    }

    /// Scan an identifier or keyword
    fn scan_identifier(&mut self, start: usize) -> MongoToken {
        let mut value = String::new();

        while !self.is_at_end() {
            let ch = self.current_char();
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check if it's the "db" keyword
        let kind = if value == "db" {
            MongoTokenKind::Db
        } else {
            MongoTokenKind::Ident(value)
        };

        MongoToken::new(kind, start..self.pos)
    }

    /// Skip whitespace characters
    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            let ch = self.current_char();
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Get current character
    fn current_char(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.input[self.pos]
        }
    }

    /// Peek at next character
    fn peek_char(&self) -> char {
        if self.pos + 1 >= self.input.len() {
            '\0'
        } else {
            self.input[self.pos + 1]
        }
    }

    /// Advance position
    fn advance(&mut self) {
        if !self.is_at_end() {
            self.pos += 1;
        }
    }

    /// Check if at end of input
    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_db_collection() {
        let tokens = MongoLexer::tokenize("db.users");
        assert_eq!(tokens.len(), 4); // db, ., users, EOF

        assert!(matches!(tokens[0].kind, MongoTokenKind::Db));
        assert!(matches!(tokens[1].kind, MongoTokenKind::Dot));
        assert!(matches!(tokens[2].kind, MongoTokenKind::Ident(ref s) if s == "users"));
        assert!(matches!(tokens[3].kind, MongoTokenKind::EOF));
    }

    #[test]
    fn test_tokenize_db_collection_operation() {
        let tokens = MongoLexer::tokenize("db.users.find");
        assert!(matches!(tokens[0].kind, MongoTokenKind::Db));
        assert!(matches!(tokens[1].kind, MongoTokenKind::Dot));
        assert!(matches!(tokens[2].kind, MongoTokenKind::Ident(ref s) if s == "users"));
        assert!(matches!(tokens[3].kind, MongoTokenKind::Dot));
        assert!(matches!(tokens[4].kind, MongoTokenKind::Ident(ref s) if s == "find"));
        assert!(matches!(tokens[5].kind, MongoTokenKind::EOF));
    }

    #[test]
    fn test_tokenize_with_parentheses() {
        let tokens = MongoLexer::tokenize("db.users.find()");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::LParen))
        );
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::RParen))
        );
    }

    #[test]
    fn test_tokenize_partial_input() {
        let tokens = MongoLexer::tokenize("db.us");
        assert!(matches!(tokens[0].kind, MongoTokenKind::Db));
        assert!(matches!(tokens[1].kind, MongoTokenKind::Dot));
        assert!(matches!(tokens[2].kind, MongoTokenKind::Ident(ref s) if s == "us"));
        assert!(matches!(tokens[3].kind, MongoTokenKind::EOF));
    }

    #[test]
    fn test_tokenize_string_literal() {
        let tokens = MongoLexer::tokenize("db.users.find({name: 'John'})");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::String(ref s) if s == "John"))
        );
    }

    #[test]
    fn test_tokenize_number() {
        let tokens = MongoLexer::tokenize("db.users.find({age: 25})");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::Number(ref s) if s == "25"))
        );
    }

    #[test]
    fn test_tokenize_empty_input() {
        let tokens = MongoLexer::tokenize("");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].kind, MongoTokenKind::EOF));
    }

    #[test]
    fn test_tokenize_just_db() {
        let tokens = MongoLexer::tokenize("db");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].kind, MongoTokenKind::Db));
        assert!(matches!(tokens[1].kind, MongoTokenKind::EOF));
    }

    #[test]
    fn test_tokenize_db_dot() {
        let tokens = MongoLexer::tokenize("db.");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0].kind, MongoTokenKind::Db));
        assert!(matches!(tokens[1].kind, MongoTokenKind::Dot));
        assert!(matches!(tokens[2].kind, MongoTokenKind::EOF));
    }

    #[test]
    fn test_tokenize_braces() {
        let tokens = MongoLexer::tokenize("db.users.find({})");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::LBrace))
        );
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::RBrace))
        );
    }

    #[test]
    fn test_tokenize_with_dollar_sign() {
        let tokens = MongoLexer::tokenize("db.users.aggregate([{$match: {}}])");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::Ident(ref s) if s == "$match"))
        );
    }

    #[test]
    fn test_tokenize_unknown_chars() {
        let tokens = MongoLexer::tokenize("db.users@");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, MongoTokenKind::Unknown('@')))
        );
    }
}
