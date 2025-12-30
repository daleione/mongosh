use std::borrow::Cow;

use rustyline::Helper as RustyHelper;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;

use crate::repl::shared_state::SharedState;

/// Helper for rustyline providing completion, hints, and highlighting
pub struct ReplHelper {
    /// Shared state for contextual completion
    pub(crate) shared_state: SharedState,

    /// Available commands for completion
    pub(crate) commands: Vec<String>,

    /// Available collections for completion
    pub(crate) collections: Vec<String>,

    /// Enable syntax highlighting
    pub(crate) highlighting_enabled: bool,
}

impl ReplHelper {
    /// Create a new REPL helper
    ///
    /// # Arguments
    /// * `shared_state` - Shared state
    /// * `highlighting_enabled` - Enable syntax highlighting
    ///
    /// # Returns
    /// * `Self` - New helper
    pub fn new(shared_state: SharedState, highlighting_enabled: bool) -> Self {
        let commands = vec![
            "show".to_string(),
            "use".to_string(),
            "exit".to_string(),
            "quit".to_string(),
            "help".to_string(),
            "db".to_string(),
        ];

        Self {
            shared_state,
            commands,
            collections: Vec::new(),
            highlighting_enabled,
        }
    }

    /// Check if input is a complete statement
    ///
    /// # Arguments
    /// * `input` - Input string to check
    ///
    /// # Returns
    /// * `bool` - True if complete
    pub fn is_complete_statement(&self, input: &str) -> bool {
        // Simple check: balanced braces and parentheses
        let mut brace_count = 0;
        let mut paren_count = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for ch in input.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' => escape_next = true,
                '"' | '\'' => in_string = !in_string,
                '{' if !in_string => brace_count += 1,
                '}' if !in_string => brace_count -= 1,
                '(' if !in_string => paren_count += 1,
                ')' if !in_string => paren_count -= 1,
                _ => {}
            }
        }

        brace_count == 0 && paren_count == 0
    }
}

impl RustyHelper for ReplHelper {}

impl Completer for ReplHelper {
    type Candidate = Pair;

    /// Complete input at given position
    ///
    /// # Arguments
    /// * `line` - Current line
    /// * `pos` - Cursor position
    /// * `_ctx` - Readline context
    ///
    /// # Returns
    /// * `Result<(usize, Vec<Pair>)>` - Completion position and candidates
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let mut candidates = Vec::new();

        // Get the word being completed
        let start = line[..pos]
            .rfind(|c: char| c.is_whitespace() || c == '.')
            .map(|i| i + 1)
            .unwrap_or(0);

        let word = &line[start..pos];

        // Determine what to complete based on context
        if line.starts_with("use ") {
            // Complete database names
            // For now, just suggest the current database
            let current_db = self.shared_state.get_database();
            if current_db.starts_with(word) {
                candidates.push(Pair {
                    display: current_db.clone(),
                    replacement: current_db,
                });
            }
        } else if line.starts_with("show ") {
            // Complete "show" subcommands
            let show_commands = vec!["dbs", "databases", "collections", "tables"];
            for cmd in show_commands {
                if cmd.starts_with(word) {
                    candidates.push(Pair {
                        display: cmd.to_string(),
                        replacement: cmd.to_string(),
                    });
                }
            }
        } else if line.contains("db.") {
            // Complete collection names after "db."
            let after_db = line.split("db.").last().unwrap_or("");
            let collection_part = after_db.split('.').next().unwrap_or("");

            if collection_part == word || word.is_empty() {
                for collection in &self.collections {
                    if collection.starts_with(word) {
                        candidates.push(Pair {
                            display: collection.clone(),
                            replacement: collection.clone(),
                        });
                    }
                }
            }

            // Also suggest common operations after collection name
            if after_db.contains('.') {
                let operations = vec![
                    "find",
                    "findOne",
                    "insertOne",
                    "insertMany",
                    "updateOne",
                    "updateMany",
                    "deleteOne",
                    "deleteMany",
                    "countDocuments",
                    "aggregate",
                    "drop",
                ];
                for op in operations {
                    if op.starts_with(word) {
                        candidates.push(Pair {
                            display: op.to_string(),
                            replacement: op.to_string(),
                        });
                    }
                }
            }
        } else {
            // Complete top-level commands
            for cmd in &self.commands {
                if cmd.starts_with(word) {
                    candidates.push(Pair {
                        display: cmd.clone(),
                        replacement: cmd.clone(),
                    });
                }
            }

            // Also suggest "db" if it matches
            if "db".starts_with(word) {
                candidates.push(Pair {
                    display: "db".to_string(),
                    replacement: "db".to_string(),
                });
            }
        }

        Ok((start, candidates))
    }
}

impl Hinter for ReplHelper {
    type Hint = String;

    /// Provide hints for current input
    ///
    /// # Arguments
    /// * `_line` - Current line
    /// * `_pos` - Cursor position
    /// * `_ctx` - Readline context
    ///
    /// # Returns
    /// * `Option<String>` - Hint text
    fn hint(&self, _line: &str, _pos: usize, _ctx: &rustyline::Context<'_>) -> Option<String> {
        // TODO: Implement command hints based on partial input
        None
    }
}

impl Highlighter for ReplHelper {
    /// Highlight input text
    ///
    /// # Arguments
    /// * `line` - Line to highlight
    /// * `_pos` - Cursor position
    ///
    /// # Returns
    /// * `Cow<str>` - Highlighted text
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // TODO: Implement syntax highlighting for MongoDB commands
        // For now, just return the line as-is
        Cow::Borrowed(line)
    }

    /// Highlight character at position
    ///
    /// # Arguments
    /// * `_line` - Current line
    /// * `_pos` - Character position
    /// * `_ctx` - Whether Rustyline currently considers this position "in focus"
    ///
    /// # Returns
    /// * `bool` - Whether to highlight
    fn highlight_char(&self, _line: &str, _pos: usize, _ctx: bool) -> bool {
        self.highlighting_enabled && _ctx
    }
}

impl Validator for ReplHelper {
    /// Validate input for completeness
    ///
    /// # Arguments
    /// * `ctx` - Validation context
    ///
    /// # Returns
    /// * `Result<ValidationResult>` - Validation result
    fn validate(
        &self,
        ctx: &mut rustyline::validate::ValidationContext,
    ) -> rustyline::Result<rustyline::validate::ValidationResult> {
        use rustyline::validate::ValidationResult;

        let input = ctx.input();

        // Check if input is a complete statement
        if self.is_complete_statement(input) {
            Ok(ValidationResult::Valid(None))
        } else {
            Ok(ValidationResult::Incomplete)
        }
    }
}
