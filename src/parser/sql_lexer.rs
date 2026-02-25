//! SQL lexer for error-tolerant tokenization
//!
//! This lexer is designed to be extremely forgiving and never panic.
//! It handles incomplete input gracefully, which is essential for CLI
//! autocomplete and real-time error feedback.
//!
//! # Design Principles
//!
//! - **Never panic** - always return a valid token stream
//! - **Never reject input** - unknown characters become `Unknown` tokens
//! - **Don't detect syntax errors** - that's the parser's job
//! - **Performance** - simple character-by-character scanning

use std::ops::Range;

/// Token type enumeration (case-insensitive for keywords)
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // SQL Keywords (case-insensitive)
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Drop,
    Alter,
    From,
    Where,
    GroupBy,
    OrderBy,
    Limit,
    Offset,
    And,
    Or,
    Not,
    As,
    Join,
    Inner,
    Left,
    Right,
    On,
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Distinct,
    By,
    Asc,
    Desc,
    Like,
    In,
    Is,
    Null,
    True,
    False,
    Group,
    Order,
    Explain,
    Date,
    Timestamp,
    Time,
    Current,
    CurrentTimestamp,
    CurrentDate,
    CurrentTime,
    Cast,
    Interval,
    Now,

    // Identifiers and Literals
    Ident(String),
    Number(String),
    String(String),

    // Operators and Symbols
    Star,
    Comma,
    Dot,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Minus,
    Semicolon,

    // Special tokens
    EOF,
    Unknown(char),
}

/// Token with position information
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Range<usize>,
}

impl Token {
    /// Create a new token
    pub fn new(kind: TokenKind, span: Range<usize>) -> Self {
        Self { kind, span }
    }
}

/// SQL Lexer - error-tolerant tokenizer
pub struct SqlLexer {
    input: Vec<char>,
    pos: usize,
}

impl SqlLexer {
    /// Create a new lexer from input string
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(input: &str) -> Vec<Token> {
        let mut lexer = Self::new(input);
        let mut tokens = Vec::new();

        loop {
            let token = lexer.next_token();
            let is_eof = matches!(token.kind, TokenKind::EOF);
            tokens.push(token);
            if is_eof {
                break;
            }
        }

        tokens
    }

    /// Get the next token
    fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let start = self.pos;

        if self.is_at_end() {
            return Token::new(TokenKind::EOF, start..start);
        }

        let ch = self.current_char();

        match ch {
            // Single-character tokens
            '*' => {
                self.advance();
                Token::new(TokenKind::Star, start..self.pos)
            }
            ',' => {
                self.advance();
                Token::new(TokenKind::Comma, start..self.pos)
            }
            '.' => {
                self.advance();
                Token::new(TokenKind::Dot, start..self.pos)
            }
            '(' => {
                self.advance();
                Token::new(TokenKind::LParen, start..self.pos)
            }
            ')' => {
                self.advance();
                Token::new(TokenKind::RParen, start..self.pos)
            }
            '[' => {
                self.advance();
                Token::new(TokenKind::LBracket, start..self.pos)
            }
            ']' => {
                self.advance();
                Token::new(TokenKind::RBracket, start..self.pos)
            }
            ':' => {
                self.advance();
                Token::new(TokenKind::Colon, start..self.pos)
            }
            ';' => {
                self.advance();
                Token::new(TokenKind::Semicolon, start..self.pos)
            }

            // Operators (possibly two characters)
            '=' => {
                self.advance();
                Token::new(TokenKind::Eq, start..self.pos)
            }
            '!' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Token::new(TokenKind::Ne, start..self.pos)
                } else {
                    Token::new(TokenKind::Unknown('!'), start..self.pos)
                }
            }
            '>' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Token::new(TokenKind::Ge, start..self.pos)
                } else {
                    Token::new(TokenKind::Gt, start..self.pos)
                }
            }
            '<' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Token::new(TokenKind::Le, start..self.pos)
                } else if self.current_char() == '>' {
                    self.advance();
                    Token::new(TokenKind::Ne, start..self.pos)
                } else {
                    Token::new(TokenKind::Lt, start..self.pos)
                }
            }
            '-' => {
                self.advance();
                Token::new(TokenKind::Minus, start..self.pos)
            }

            // String literals
            '\'' | '"' => self.scan_string(ch, start),

            // Numbers
            '0'..='9' => self.scan_number(start),

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier(start),

            // Unknown character - don't panic, just return it
            _ => {
                self.advance();
                Token::new(TokenKind::Unknown(ch), start..self.pos)
            }
        }
    }

    /// Scan a string literal
    fn scan_string(&mut self, quote: char, start: usize) -> Token {
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

        Token::new(TokenKind::String(value), start..self.pos)
    }

    /// Scan a number (integer or decimal)
    fn scan_number(&mut self, start: usize) -> Token {
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

        Token::new(TokenKind::Number(value), start..self.pos)
    }

    /// Scan an identifier or keyword
    fn scan_identifier(&mut self, start: usize) -> Token {
        let mut value = String::new();

        while !self.is_at_end() {
            let ch = self.current_char();
            if ch.is_alphanumeric() || ch == '_' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check if it's a keyword (case-insensitive)
        let kind = match value.to_uppercase().as_str() {
            "SELECT" => TokenKind::Select,
            "INSERT" => TokenKind::Insert,
            "UPDATE" => TokenKind::Update,
            "DELETE" => TokenKind::Delete,
            "CREATE" => TokenKind::Create,
            "DROP" => TokenKind::Drop,
            "ALTER" => TokenKind::Alter,
            "FROM" => TokenKind::From,
            "WHERE" => TokenKind::Where,
            "EXPLAIN" => TokenKind::Explain,
            "GROUP" => {
                // Check for "GROUP BY"
                let saved_pos = self.pos;
                self.skip_whitespace();
                if self.peek_word().to_uppercase() == "BY" {
                    self.skip_word();
                    TokenKind::GroupBy
                } else {
                    self.pos = saved_pos;
                    TokenKind::Group
                }
            }
            "ORDER" => {
                // Check for "ORDER BY"
                let saved_pos = self.pos;
                self.skip_whitespace();
                if self.peek_word().to_uppercase() == "BY" {
                    self.skip_word();
                    TokenKind::OrderBy
                } else {
                    self.pos = saved_pos;
                    TokenKind::Order
                }
            }
            "BY" => TokenKind::By,
            "LIMIT" => TokenKind::Limit,
            "OFFSET" => TokenKind::Offset,
            "AND" => TokenKind::And,
            "OR" => TokenKind::Or,
            "NOT" => TokenKind::Not,
            "AS" => TokenKind::As,
            "JOIN" => TokenKind::Join,
            "INNER" => TokenKind::Inner,
            "LEFT" => TokenKind::Left,
            "RIGHT" => TokenKind::Right,
            "ON" => TokenKind::On,
            "COUNT" => TokenKind::Count,
            "SUM" => TokenKind::Sum,
            "AVG" => TokenKind::Avg,
            "MIN" => TokenKind::Min,
            "MAX" => TokenKind::Max,
            "DISTINCT" => TokenKind::Distinct,
            "ASC" => TokenKind::Asc,
            "DESC" => TokenKind::Desc,
            "LIKE" => TokenKind::Like,
            "IN" => TokenKind::In,
            "IS" => TokenKind::Is,
            "NULL" => TokenKind::Null,
            "TRUE" => TokenKind::True,
            "FALSE" => TokenKind::False,
            "DATE" => TokenKind::Date,
            "TIMESTAMP" => TokenKind::Timestamp,
            "TIME" => TokenKind::Time,
            "CURRENT" => TokenKind::Current,
            "CURRENT_TIMESTAMP" => TokenKind::CurrentTimestamp,
            "CURRENT_DATE" => TokenKind::CurrentDate,
            "CURRENT_TIME" => TokenKind::CurrentTime,
            "CAST" => TokenKind::Cast,
            "INTERVAL" => TokenKind::Interval,
            "NOW" => TokenKind::Now,
            _ => TokenKind::Ident(value),
        };

        Token::new(kind, start..self.pos)
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

    /// Peek the next word without consuming it
    fn peek_word(&self) -> String {
        let mut pos = self.pos;
        let mut word = String::new();

        while pos < self.input.len() {
            let ch = self.input[pos];
            if ch.is_alphanumeric() || ch == '_' {
                word.push(ch);
                pos += 1;
            } else {
                break;
            }
        }

        word
    }

    /// Skip the next word
    fn skip_word(&mut self) {
        while !self.is_at_end() {
            let ch = self.current_char();
            if ch.is_alphanumeric() || ch == '_' {
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
    fn test_tokenize_simple_select() {
        let tokens = SqlLexer::tokenize("SELECT * FROM users");
        assert_eq!(tokens.len(), 5); // SELECT, *, FROM, users, EOF

        assert!(matches!(tokens[0].kind, TokenKind::Select));
        assert!(matches!(tokens[1].kind, TokenKind::Star));
        assert!(matches!(tokens[2].kind, TokenKind::From));
        assert!(matches!(tokens[3].kind, TokenKind::Ident(ref s) if s == "users"));
        assert!(matches!(tokens[4].kind, TokenKind::EOF));
    }

    #[test]
    fn test_tokenize_with_where() {
        let tokens = SqlLexer::tokenize("SELECT name FROM users WHERE age > 18");
        assert!(matches!(tokens[0].kind, TokenKind::Select));
        assert!(matches!(tokens[1].kind, TokenKind::Ident(ref s) if s == "name"));
        assert!(matches!(tokens[2].kind, TokenKind::From));
        assert!(matches!(tokens[3].kind, TokenKind::Ident(ref s) if s == "users"));
        assert!(matches!(tokens[4].kind, TokenKind::Where));
        assert!(matches!(tokens[5].kind, TokenKind::Ident(ref s) if s == "age"));
        assert!(matches!(tokens[6].kind, TokenKind::Gt));
        assert!(matches!(tokens[7].kind, TokenKind::Number(ref s) if s == "18"));
    }

    #[test]
    fn test_tokenize_partial_input() {
        let tokens = SqlLexer::tokenize("SELECT * FR");
        assert!(matches!(tokens[0].kind, TokenKind::Select));
        assert!(matches!(tokens[1].kind, TokenKind::Star));
        assert!(matches!(tokens[2].kind, TokenKind::Ident(ref s) if s == "FR"));
        assert!(matches!(tokens[3].kind, TokenKind::EOF));
    }

    #[test]
    fn test_tokenize_string_literal() {
        let tokens = SqlLexer::tokenize("SELECT * FROM users WHERE name = 'John'");
        assert!(matches!(
            tokens.iter().find(|t| matches!(t.kind, TokenKind::String(_))),
            Some(Token { kind: TokenKind::String(s), .. }) if s == "John"
        ));
    }

    #[test]
    fn test_tokenize_operators() {
        let tokens = SqlLexer::tokenize("a = 1 AND b != 2 AND c >= 3 AND d <= 4");
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Eq)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Ne)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Ge)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Le)));
    }

    #[test]
    fn test_tokenize_group_by() {
        let tokens = SqlLexer::tokenize("SELECT COUNT(*) FROM users GROUP BY age");
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::GroupBy)));
    }

    #[test]
    fn test_tokenize_order_by() {
        let tokens = SqlLexer::tokenize("SELECT * FROM users ORDER BY name ASC");
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::OrderBy)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Asc)));
    }

    #[test]
    fn test_tokenize_empty_input() {
        let tokens = SqlLexer::tokenize("");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].kind, TokenKind::EOF));
    }

    #[test]
    fn test_tokenize_case_insensitive() {
        let tokens1 = SqlLexer::tokenize("SELECT * FROM users");
        let tokens2 = SqlLexer::tokenize("select * from users");
        let tokens3 = SqlLexer::tokenize("SeLeCt * FrOm users");

        assert!(matches!(tokens1[0].kind, TokenKind::Select));
        assert!(matches!(tokens2[0].kind, TokenKind::Select));
        assert!(matches!(tokens3[0].kind, TokenKind::Select));
    }

    #[test]
    fn test_tokenize_decimal_numbers() {
        let tokens = SqlLexer::tokenize("SELECT * WHERE price = 19.99");
        assert!(matches!(
            tokens.iter().find(|t| matches!(t.kind, TokenKind::Number(_))),
            Some(Token { kind: TokenKind::Number(s), .. }) if s == "19.99"
        ));
    }

    #[test]
    fn test_tokenize_unknown_chars() {
        let tokens = SqlLexer::tokenize("SELECT @ FROM users");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t.kind, TokenKind::Unknown('@')))
        );
    }
}
