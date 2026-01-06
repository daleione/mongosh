//! Syntax highlighter for MongoDB shell and SQL queries
//!
//! This module provides a unified highlighter that supports both MongoDB shell syntax
//! and SQL syntax, with automatic detection capabilities.

use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};

/// Syntax highlighting mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxMode {
    /// MongoDB shell syntax
    Mongo,
    /// SQL syntax
    Sql,
    /// Automatically detect syntax based on input
    Auto,
}

/// Main syntax highlighter supporting multiple languages
pub struct SyntaxHighlighter {
    mode: SyntaxMode,
    enabled: bool,
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter
    pub fn new(mode: SyntaxMode, enabled: bool) -> Self {
        Self { mode, enabled }
    }

    /// Detect syntax mode from input
    fn detect_syntax(line: &str) -> SyntaxMode {
        let trimmed = line.trim_start().to_uppercase();

        // SQL keywords that typically start queries
        if trimmed.starts_with("SELECT")
            || trimmed.starts_with("INSERT")
            || trimmed.starts_with("UPDATE")
            || trimmed.starts_with("DELETE")
            || trimmed.starts_with("CREATE")
            || trimmed.starts_with("DROP")
            || trimmed.starts_with("ALTER")
            || trimmed.starts_with("WITH")
        {
            return SyntaxMode::Sql;
        }

        // MongoDB-specific patterns
        if trimmed.starts_with("DB.") || trimmed.starts_with("SHOW") || trimmed.starts_with("USE ")
        {
            return SyntaxMode::Mongo;
        }

        // Default to MongoDB
        SyntaxMode::Mongo
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new(SyntaxMode::Auto, true)
    }
}

impl Highlighter for SyntaxHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        if !self.enabled {
            let mut styled = StyledText::new();
            styled.push((Style::default(), line.to_string()));
            return styled;
        }

        let mode = match self.mode {
            SyntaxMode::Auto => Self::detect_syntax(line),
            other => other,
        };

        match mode {
            SyntaxMode::Sql => SqlHighlighter::highlight(line),
            SyntaxMode::Mongo => MongoHighlighter::highlight(line),
            SyntaxMode::Auto => unreachable!(),
        }
    }
}

// ============================================================================
// MongoDB Syntax Highlighter
// ============================================================================

struct MongoHighlighter;

impl MongoHighlighter {
    /// MongoDB keywords and commands
    const KEYWORDS: &'static [&'static str] = &[
        "db",
        "show",
        "use",
        "exit",
        "quit",
        "help",
        "let",
        "const",
        "var",
        "function",
        "return",
        "if",
        "else",
        "for",
        "while",
        "break",
        "continue",
        "true",
        "false",
        "null",
        "undefined",
        "new",
        "this",
    ];

    /// MongoDB collection methods
    const METHODS: &'static [&'static str] = &[
        "find",
        "findOne",
        "insertOne",
        "insertMany",
        "updateOne",
        "updateMany",
        "deleteOne",
        "deleteMany",
        "aggregate",
        "count",
        "countDocuments",
        "estimatedDocumentCount",
        "distinct",
        "createIndex",
        "createIndexes",
        "dropIndex",
        "dropIndexes",
        "drop",
        "renameCollection",
        "stats",
        "dataSize",
        "storageSize",
        "totalIndexSize",
        "getIndexes",
        "explain",
    ];

    fn is_keyword(word: &str) -> bool {
        Self::KEYWORDS.contains(&word)
    }

    fn is_method(word: &str) -> bool {
        Self::METHODS.contains(&word)
    }

    fn get_style(word: &str) -> Style {
        if Self::is_keyword(word) {
            Color::Blue.bold().into()
        } else if Self::is_method(word) {
            Color::Green.into()
        } else {
            Style::default()
        }
    }

    fn highlight(line: &str) -> StyledText {
        let mut styled = StyledText::new();
        let mut current_word = String::new();
        let mut in_string = false;
        let mut string_delimiter = ' ';
        let mut string_buffer = String::new();
        let mut escape_next = false;

        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            // Handle line comments
            if !in_string && i + 1 < chars.len() && ch == '/' && chars[i + 1] == '/' {
                // Flush current word
                if !current_word.is_empty() {
                    styled.push((Self::get_style(&current_word), current_word.clone()));
                    current_word.clear();
                }
                // Capture rest of line as comment
                let comment_buffer: String = chars[i..].iter().collect();
                styled.push((Color::DarkGray.dimmed().into(), comment_buffer));
                break;
            }

            // Handle escape sequences in strings
            if escape_next {
                if in_string {
                    string_buffer.push('\\');
                    string_buffer.push(ch);
                }
                escape_next = false;
                i += 1;
                continue;
            }

            if ch == '\\' && in_string {
                escape_next = true;
                i += 1;
                continue;
            }

            // Handle string literals
            if (ch == '"' || ch == '\'' || ch == '`') && !escape_next {
                if in_string && ch == string_delimiter {
                    // End of string
                    string_buffer.push(ch);
                    styled.push((Color::Yellow.into(), string_buffer.clone()));
                    string_buffer.clear();
                    in_string = false;
                } else if !in_string {
                    // Flush current word
                    if !current_word.is_empty() {
                        styled.push((Self::get_style(&current_word), current_word.clone()));
                        current_word.clear();
                    }
                    // Start of string
                    in_string = true;
                    string_delimiter = ch;
                    string_buffer.push(ch);
                } else {
                    // Different quote inside string
                    string_buffer.push(ch);
                }
                i += 1;
                continue;
            }

            if in_string {
                string_buffer.push(ch);
                i += 1;
                continue;
            }

            // Handle word boundaries
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                current_word.push(ch);
            } else {
                // Flush current word
                if !current_word.is_empty() {
                    styled.push((Self::get_style(&current_word), current_word.clone()));
                    current_word.clear();
                }

                // Style operators and punctuation
                let style = match ch {
                    '(' | ')' | '{' | '}' | '[' | ']' => Color::Cyan.into(),
                    '.' | ',' | ';' | ':' => Color::DarkGray.into(),
                    '+' | '-' | '*' | '/' | '=' | '<' | '>' | '!' | '&' | '|' => {
                        Color::Magenta.into()
                    }
                    _ => Style::default(),
                };
                styled.push((style, ch.to_string()));
            }

            i += 1;
        }

        // Flush remaining content
        if !current_word.is_empty() {
            styled.push((Self::get_style(&current_word), current_word));
        }
        if in_string {
            // Unclosed string
            styled.push((Color::Yellow.into(), string_buffer));
        }

        styled
    }
}

// ============================================================================
// SQL Syntax Highlighter
// ============================================================================

struct SqlHighlighter;

impl SqlHighlighter {
    /// SQL keywords (uppercase for comparison)
    const KEYWORDS: &'static [&'static str] = &[
        "SELECT",
        "FROM",
        "WHERE",
        "INSERT",
        "INTO",
        "UPDATE",
        "DELETE",
        "CREATE",
        "DROP",
        "ALTER",
        "TABLE",
        "DATABASE",
        "INDEX",
        "VIEW",
        "TRIGGER",
        "PROCEDURE",
        "FUNCTION",
        "JOIN",
        "INNER",
        "LEFT",
        "RIGHT",
        "OUTER",
        "CROSS",
        "ON",
        "USING",
        "AS",
        "AND",
        "OR",
        "NOT",
        "IN",
        "BETWEEN",
        "LIKE",
        "IS",
        "NULL",
        "TRUE",
        "FALSE",
        "ORDER",
        "BY",
        "GROUP",
        "HAVING",
        "LIMIT",
        "OFFSET",
        "DISTINCT",
        "ALL",
        "ANY",
        "SOME",
        "EXISTS",
        "CASE",
        "WHEN",
        "THEN",
        "ELSE",
        "END",
        "WITH",
        "RECURSIVE",
        "UNION",
        "INTERSECT",
        "EXCEPT",
        "VALUES",
        "SET",
        "DEFAULT",
        "CONSTRAINT",
        "PRIMARY",
        "FOREIGN",
        "KEY",
        "REFERENCES",
        "UNIQUE",
        "CHECK",
        "CASCADE",
        "RESTRICT",
        "ASC",
        "DESC",
        "NULLS",
        "FIRST",
        "LAST",
    ];

    /// SQL data types
    const TYPES: &'static [&'static str] = &[
        "INT",
        "INTEGER",
        "BIGINT",
        "SMALLINT",
        "TINYINT",
        "DECIMAL",
        "NUMERIC",
        "FLOAT",
        "REAL",
        "DOUBLE",
        "CHAR",
        "VARCHAR",
        "TEXT",
        "BLOB",
        "DATE",
        "TIME",
        "DATETIME",
        "TIMESTAMP",
        "BOOLEAN",
        "BOOL",
        "BINARY",
        "VARBINARY",
        "JSON",
        "JSONB",
        "UUID",
        "ARRAY",
        "ENUM",
        "SERIAL",
        "BIGSERIAL",
    ];

    /// SQL functions
    const FUNCTIONS: &'static [&'static str] = &[
        "COUNT",
        "SUM",
        "AVG",
        "MIN",
        "MAX",
        "UPPER",
        "LOWER",
        "LENGTH",
        "SUBSTRING",
        "CONCAT",
        "TRIM",
        "LTRIM",
        "RTRIM",
        "COALESCE",
        "NULLIF",
        "CAST",
        "CONVERT",
        "NOW",
        "CURRENT_DATE",
        "CURRENT_TIME",
        "CURRENT_TIMESTAMP",
        "EXTRACT",
        "ABS",
        "ROUND",
        "FLOOR",
        "CEIL",
        "POWER",
        "SQRT",
        "MOD",
    ];

    fn is_keyword(word: &str) -> bool {
        Self::KEYWORDS.contains(&word.to_uppercase().as_str())
    }

    fn is_type(word: &str) -> bool {
        Self::TYPES.contains(&word.to_uppercase().as_str())
    }

    fn is_function(word: &str) -> bool {
        Self::FUNCTIONS.contains(&word.to_uppercase().as_str())
    }

    fn get_style(word: &str) -> Style {
        let upper = word.to_uppercase();
        if Self::is_keyword(&upper) {
            Color::Green.bold().into()
        } else if Self::is_type(&upper) {
            Color::Cyan.bold().into()
        } else if Self::is_function(&upper) {
            Color::Magenta.into()
        } else {
            Style::default()
        }
    }

    fn highlight(line: &str) -> StyledText {
        let mut styled = StyledText::new();
        let mut current_word = String::new();
        let mut in_string = false;
        let mut string_delimiter = ' ';
        let mut string_buffer = String::new();
        let mut escape_next = false;

        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            // Handle line comments (-- style)
            if !in_string && i + 1 < chars.len() && ch == '-' && chars[i + 1] == '-' {
                // Flush current word
                if !current_word.is_empty() {
                    styled.push((Self::get_style(&current_word), current_word.clone()));
                    current_word.clear();
                }
                // Capture rest of line as comment
                let comment: String = chars[i..].iter().collect();
                styled.push((Color::DarkGray.dimmed().into(), comment));
                break;
            }

            // Handle escape sequences
            if escape_next {
                if in_string {
                    string_buffer.push('\\');
                    string_buffer.push(ch);
                }
                escape_next = false;
                i += 1;
                continue;
            }

            if ch == '\\' && in_string {
                escape_next = true;
                i += 1;
                continue;
            }

            // Handle string literals (single quotes in SQL)
            if (ch == '\'' || ch == '"') && !escape_next {
                if in_string && ch == string_delimiter {
                    // End of string
                    string_buffer.push(ch);
                    styled.push((Color::Yellow.into(), string_buffer.clone()));
                    string_buffer.clear();
                    in_string = false;
                } else if !in_string {
                    // Flush current word
                    if !current_word.is_empty() {
                        styled.push((Self::get_style(&current_word), current_word.clone()));
                        current_word.clear();
                    }
                    // Start of string
                    in_string = true;
                    string_delimiter = ch;
                    string_buffer.push(ch);
                } else {
                    // Different quote inside string
                    string_buffer.push(ch);
                }
                i += 1;
                continue;
            }

            if in_string {
                string_buffer.push(ch);
                i += 1;
                continue;
            }

            // Handle identifiers and keywords
            if ch.is_alphanumeric() || ch == '_' {
                current_word.push(ch);
            } else {
                // Flush current word (uppercasing keywords)
                if !current_word.is_empty() {
                    let style = Self::get_style(&current_word);
                    let display_word = if Self::is_keyword(&current_word)
                        || Self::is_type(&current_word)
                        || Self::is_function(&current_word)
                    {
                        current_word.to_uppercase()
                    } else {
                        current_word.clone()
                    };
                    styled.push((style, display_word));
                    current_word.clear();
                }

                // Style punctuation and operators
                let style = match ch {
                    '(' | ')' | '[' | ']' => Color::Cyan.into(),
                    ',' | ';' | '.' => Color::DarkGray.into(),
                    '*' | '+' | '-' | '/' | '=' | '<' | '>' | '!' => Color::Blue.into(),
                    _ => Style::default(),
                };
                styled.push((style, ch.to_string()));
            }

            i += 1;
        }

        // Flush remaining content
        if !current_word.is_empty() {
            let style = Self::get_style(&current_word);
            let display_word = if Self::is_keyword(&current_word)
                || Self::is_type(&current_word)
                || Self::is_function(&current_word)
            {
                current_word.to_uppercase()
            } else {
                current_word
            };
            styled.push((style, display_word));
        }
        if in_string {
            // Unclosed string
            styled.push((Color::Yellow.into(), string_buffer));
        }

        styled
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_detection() {
        assert_eq!(
            SyntaxHighlighter::detect_syntax("SELECT * FROM users"),
            SyntaxMode::Sql
        );
        assert_eq!(
            SyntaxHighlighter::detect_syntax("db.users.find()"),
            SyntaxMode::Mongo
        );
        assert_eq!(
            SyntaxHighlighter::detect_syntax("show dbs"),
            SyntaxMode::Mongo
        );
        assert_eq!(
            SyntaxHighlighter::detect_syntax("INSERT INTO table VALUES (1)"),
            SyntaxMode::Sql
        );
    }

    #[test]
    fn test_mongo_keywords() {
        assert!(MongoHighlighter::is_keyword("db"));
        assert!(MongoHighlighter::is_keyword("show"));
        assert!(!MongoHighlighter::is_keyword("users"));
    }

    #[test]
    fn test_mongo_methods() {
        assert!(MongoHighlighter::is_method("find"));
        assert!(MongoHighlighter::is_method("insertOne"));
        assert!(!MongoHighlighter::is_method("users"));
    }

    #[test]
    fn test_sql_keywords() {
        assert!(SqlHighlighter::is_keyword("SELECT"));
        assert!(SqlHighlighter::is_keyword("select"));
        assert!(SqlHighlighter::is_keyword("WHERE"));
        assert!(!SqlHighlighter::is_keyword("users"));
    }

    #[test]
    fn test_sql_types() {
        assert!(SqlHighlighter::is_type("INT"));
        assert!(SqlHighlighter::is_type("varchar"));
        assert!(!SqlHighlighter::is_type("SELECT"));
    }

    #[test]
    fn test_mongo_highlight() {
        let highlighter = SyntaxHighlighter::new(SyntaxMode::Mongo, true);
        let result = highlighter.highlight("db.users.find()", 0);
        assert!(!result.render_simple().is_empty());
    }

    #[test]
    fn test_sql_highlight() {
        let highlighter = SyntaxHighlighter::new(SyntaxMode::Sql, true);
        let result = highlighter.highlight("SELECT * FROM users", 0);
        assert!(!result.render_simple().is_empty());
    }

    #[test]
    fn test_auto_mode() {
        let highlighter = SyntaxHighlighter::new(SyntaxMode::Auto, true);

        let mongo_result = highlighter.highlight("db.users.find()", 0);
        assert!(!mongo_result.render_simple().is_empty());

        let sql_result = highlighter.highlight("SELECT * FROM users", 0);
        assert!(!sql_result.render_simple().is_empty());
    }

    #[test]
    fn test_disabled_highlighting() {
        let highlighter = SyntaxHighlighter::new(SyntaxMode::Auto, false);
        let result = highlighter.highlight("db.users.find()", 0);
        let rendered = result.render_simple();
        assert_eq!(rendered, "db.users.find()");
    }

    #[test]
    fn test_string_literals() {
        let highlighter = SyntaxHighlighter::new(SyntaxMode::Mongo, true);
        let result = highlighter.highlight(r#"db.users.find({name: "test"})"#, 0);
        assert!(!result.render_simple().is_empty());
    }

    #[test]
    fn test_comments() {
        let highlighter = SyntaxHighlighter::new(SyntaxMode::Mongo, true);
        let result = highlighter.highlight("db.users.find() // comment", 0);
        assert!(!result.render_simple().is_empty());

        let sql_highlighter = SyntaxHighlighter::new(SyntaxMode::Sql, true);
        let sql_result = sql_highlighter.highlight("SELECT * FROM users -- comment", 0);
        assert!(!sql_result.render_simple().is_empty());
    }
}
