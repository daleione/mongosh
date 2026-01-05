//! Hinter for reedline - provides inline hints based on history

use nu_ansi_term::{Color, Style};
use reedline::{Hinter, History};

/// MongoDB hinter for reedline
pub struct MongoHinter {
    /// Style for hints
    style: Style,
    /// Current hint text
    current_hint: String,
}

impl MongoHinter {
    /// Create a new MongoDB hinter with default style
    ///
    /// # Returns
    /// * `Self` - New hinter
    pub fn new() -> Self {
        Self {
            style: Style::new().italic().fg(Color::DarkGray),
            current_hint: String::new(),
        }
    }
}

impl Default for MongoHinter {
    fn default() -> Self {
        Self::new()
    }
}

impl Hinter for MongoHinter {
    /// Provide a hint for the current line
    ///
    /// # Arguments
    /// * `line` - The current input line
    /// * `pos` - Cursor position
    /// * `history` - Command history
    /// * `use_ansi_coloring` - Whether to use ANSI colors
    /// * `_cwd` - Current working directory (unused)
    ///
    /// # Returns
    /// * `String` - Hint text to display after the cursor
    fn handle(
        &mut self,
        line: &str,
        pos: usize,
        history: &dyn History,
        use_ansi_coloring: bool,
        _cwd: &str,
    ) -> String {
        // Clear previous hint
        self.current_hint.clear();

        // Only provide hints if cursor is at the end of the line
        if pos != line.len() {
            return String::new();
        }

        // Don't hint for empty lines
        if line.trim().is_empty() {
            return String::new();
        }

        // Search history for matching commands
        let search_result = history
            .search(reedline::SearchQuery::last_with_prefix(
                line.to_string(),
                None,
            ))
            .ok()
            .and_then(|results| results.into_iter().next());

        if let Some(history_item) = search_result {
            let history_line = history_item.command_line.as_str();

            // Only show hint if history item is longer than current input
            if history_line.len() > line.len() && history_line.starts_with(line) {
                let hint = &history_line[line.len()..];

                // Store the complete hint for later use
                self.current_hint = hint.to_string();

                if use_ansi_coloring {
                    return self.style.paint(hint).to_string();
                } else {
                    return hint.to_string();
                }
            }
        }

        String::new()
    }

    /// Return the next hint token
    ///
    /// # Returns
    /// * `String` - Next hint token
    fn next_hint_token(&self) -> String {
        String::new()
    }

    /// Return the complete hint
    ///
    /// # Returns
    /// * `String` - Complete hint text
    fn complete_hint(&self) -> String {
        self.current_hint.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reedline::FileBackedHistory;
    use std::path::PathBuf;

    fn create_test_history() -> Box<dyn History> {
        // Create a temporary in-memory history for testing
        Box::new(
            FileBackedHistory::with_file(100, PathBuf::from("/tmp/test_history.txt"))
                .unwrap_or_else(|_| FileBackedHistory::new(100).expect("Failed to create history")),
        )
    }

    #[test]
    fn test_new_hinter() {
        let hinter = MongoHinter::new();
        assert_eq!(hinter.next_hint_token(), String::new());
    }

    #[test]
    fn test_empty_line_no_hint() {
        let mut hinter = MongoHinter::new();
        let history = create_test_history();
        let hint = hinter.handle("", 0, history.as_ref(), true, "/tmp");
        assert_eq!(hint, "");
    }

    #[test]
    fn test_cursor_not_at_end_no_hint() {
        let mut hinter = MongoHinter::new();
        let history = create_test_history();
        let hint = hinter.handle("db.users", 2, history.as_ref(), true, "/tmp");
        assert_eq!(hint, "");
    }

    #[test]
    fn test_hint_token() {
        let hinter = MongoHinter::new();
        assert_eq!(hinter.next_hint_token(), "");
    }

    #[test]
    fn test_default() {
        let hinter = MongoHinter::default();
        assert_eq!(hinter.next_hint_token(), String::new());
    }
}
