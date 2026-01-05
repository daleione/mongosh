use std::borrow::Cow;
use std::sync::Arc;

use rustyline::Helper as RustyHelper;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;

use crate::executor::ExecutionContext;
use crate::repl::completion::{CompletionEngine, MongoCandidateProvider};
use crate::repl::shared_state::SharedState;

/// Helper for rustyline providing completion, hints, and highlighting
pub struct ReplHelper {
    /// Completion engine for intelligent suggestions
    pub(crate) completion_engine: CompletionEngine,

    /// Enable syntax highlighting
    pub(crate) highlighting_enabled: bool,
}

impl ReplHelper {
    /// Create a new REPL helper
    ///
    /// # Arguments
    /// * `shared_state` - Shared state
    /// * `highlighting_enabled` - Enable syntax highlighting
    /// * `execution_context` - Optional execution context for database queries
    ///
    /// # Returns
    /// * `Self` - New helper
    pub fn new(
        shared_state: SharedState,
        highlighting_enabled: bool,
        execution_context: Option<Arc<ExecutionContext>>,
    ) -> Self {
        // Create the candidate provider
        let provider = Arc::new(MongoCandidateProvider::new(
            shared_state.clone(),
            execution_context,
        ));

        // Create the completion engine
        let completion_engine = CompletionEngine::new(provider);

        Self {
            completion_engine,
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
    pub fn is_complete_statement(&self, _input: &str) -> bool {
        // Simple check: balanced braces and parentheses
        // For now, always consider statements complete for single-line input
        // TODO: Implement multi-line statement detection if needed
        true
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
        // Delegate to the completion engine
        let (start, candidates) = self.completion_engine.complete(line, pos);
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
