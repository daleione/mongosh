//! Highlighter for reedline - provides syntax highlighting

use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};

/// MongoDB syntax highlighter for reedline
pub struct MongoHighlighter {
    /// Whether highlighting is enabled
    enabled: bool,
}

impl MongoHighlighter {
    /// Create a new MongoDB highlighter
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable syntax highlighting
    ///
    /// # Returns
    /// * `Self` - New highlighter
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Check if a word is a MongoDB keyword
    fn is_keyword(&self, word: &str) -> bool {
        matches!(
            word,
            "db" | "show"
                | "use"
                | "exit"
                | "quit"
                | "help"
                | "SELECT"
                | "FROM"
                | "WHERE"
                | "ORDER"
                | "BY"
                | "LIMIT"
                | "OFFSET"
                | "GROUP"
                | "HAVING"
                | "JOIN"
                | "AS"
                | "AND"
                | "OR"
        )
    }

    /// Check if a word is a MongoDB collection method
    fn is_method(&self, word: &str) -> bool {
        matches!(
            word,
            "find"
                | "findOne"
                | "insertOne"
                | "insertMany"
                | "updateOne"
                | "updateMany"
                | "deleteOne"
                | "deleteMany"
                | "aggregate"
                | "count"
                | "distinct"
                | "createIndex"
                | "dropIndex"
                | "drop"
        )
    }

    /// Get style for a word based on its type
    fn get_word_style(&self, word: &str) -> Style {
        if self.is_keyword(word) {
            Color::Blue.bold().into()
        } else if self.is_method(word) {
            Color::Green.into()
        } else {
            Style::default()
        }
    }
}

impl Default for MongoHighlighter {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Highlighter for MongoHighlighter {
    /// Highlight the input line with syntax coloring
    ///
    /// # Arguments
    /// * `line` - The input line to highlight
    /// * `_cursor` - Cursor position (unused)
    ///
    /// # Returns
    /// * `StyledText` - The line with styled segments
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        if !self.enabled {
            let mut styled = StyledText::new();
            styled.push((Style::default(), line.to_string()));
            return styled;
        }

        let mut styled = StyledText::new();
        let mut current_word = String::new();
        let mut in_string = false;
        let mut string_char = ' ';
        let mut string_content = String::new();
        let mut escape_next = false;

        for ch in line.chars() {
            if escape_next {
                if in_string {
                    string_content.push('\\');
                    string_content.push(ch);
                }
                escape_next = false;
                continue;
            }

            if ch == '\\' && in_string {
                escape_next = true;
                continue;
            }

            // Handle string literals
            if (ch == '"' || ch == '\'') && !escape_next {
                if in_string && ch == string_char {
                    // End of string
                    string_content.push(ch);
                    styled.push((Color::Yellow.into(), string_content.clone()));
                    string_content.clear();
                    in_string = false;
                } else if !in_string {
                    // Flush current word before starting string
                    if !current_word.is_empty() {
                        styled.push((self.get_word_style(&current_word), current_word.clone()));
                        current_word.clear();
                    }
                    // Start of string
                    in_string = true;
                    string_char = ch;
                    string_content.push(ch);
                } else {
                    // Quote inside string (different type)
                    string_content.push(ch);
                }
                continue;
            }

            if in_string {
                string_content.push(ch);
                continue;
            }

            // Handle word boundaries
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                current_word.push(ch);
            } else {
                // Flush current word
                if !current_word.is_empty() {
                    styled.push((self.get_word_style(&current_word), current_word.clone()));
                    current_word.clear();
                }

                // Style special characters
                let style = match ch {
                    '(' | ')' | '{' | '}' | '[' | ']' => Color::Cyan.into(),
                    '.' => Color::DarkGray.into(),
                    _ => Style::default(),
                };
                styled.push((style, ch.to_string()));
            }
        }

        // Flush remaining content
        if !current_word.is_empty() {
            styled.push((self.get_word_style(&current_word), current_word));
        }
        if in_string {
            // Unclosed string - still highlight it
            styled.push((Color::Yellow.into(), string_content));
        }

        styled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_detection() {
        let highlighter = MongoHighlighter::new(true);
        assert!(highlighter.is_keyword("db"));
        assert!(highlighter.is_keyword("show"));
        assert!(highlighter.is_keyword("use"));
        assert!(!highlighter.is_keyword("users"));
    }

    #[test]
    fn test_method_detection() {
        let highlighter = MongoHighlighter::new(true);
        assert!(highlighter.is_method("find"));
        assert!(highlighter.is_method("insertOne"));
        assert!(highlighter.is_method("aggregate"));
        assert!(!highlighter.is_method("users"));
    }

    #[test]
    fn test_highlight_disabled() {
        let highlighter = MongoHighlighter::new(false);
        let input = "db.users.find()";
        let result = highlighter.highlight(input, 0);
        let rendered = result.render_simple();
        assert!(rendered.contains("db.users.find()"));
    }

    #[test]
    fn test_highlight_simple_command() {
        let highlighter = MongoHighlighter::new(true);
        let input = "show dbs";
        let result = highlighter.highlight(input, 0);
        let rendered = result.render_simple();
        assert!(!rendered.is_empty());
    }

    #[test]
    fn test_highlight_method_call() {
        let highlighter = MongoHighlighter::new(true);
        let input = "db.users.find";
        let result = highlighter.highlight(input, 0);
        let rendered = result.render_simple();
        assert!(!rendered.is_empty());
    }

    #[test]
    fn test_get_word_style() {
        let highlighter = MongoHighlighter::new(true);

        // Keywords should have bold blue style
        let keyword_style = highlighter.get_word_style("db");
        assert_ne!(keyword_style, Style::default());

        // Methods should have green style
        let method_style = highlighter.get_word_style("find");
        assert_ne!(method_style, Style::default());

        // Regular words should have default style
        let default_style = highlighter.get_word_style("users");
        assert_eq!(default_style, Style::default());
    }
}
