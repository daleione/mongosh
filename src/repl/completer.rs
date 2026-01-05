//! Completer for reedline - provides completion suggestions

use std::sync::Arc;

use reedline::{Completer, Span, Suggestion};

use super::completion::{CompletionEngine, MongoCandidateProvider};
use super::shared_state::SharedState;
use crate::executor::ExecutionContext;

/// MongoDB completer for reedline
pub struct MongoCompleter {
    /// Completion engine for intelligent suggestions
    completion_engine: CompletionEngine,
}

impl MongoCompleter {
    /// Create a new MongoDB completer
    ///
    /// # Arguments
    /// * `shared_state` - Shared state
    /// * `execution_context` - Optional execution context for database queries
    ///
    /// # Returns
    /// * `Self` - New completer
    pub fn new(
        shared_state: SharedState,
        execution_context: Option<Arc<ExecutionContext>>,
    ) -> Self {
        // Create the candidate provider
        let provider = Arc::new(MongoCandidateProvider::new(
            shared_state.clone(),
            execution_context,
        ));

        // Create the completion engine
        let completion_engine = CompletionEngine::new(provider);

        Self { completion_engine }
    }
}

impl Completer for MongoCompleter {
    /// Complete the input at the given cursor position
    ///
    /// # Arguments
    /// * `line` - The input line
    /// * `pos` - Cursor position (byte index)
    ///
    /// # Returns
    /// * `Vec<Suggestion>` - List of completion suggestions
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        // Delegate to the completion engine
        let (start, candidates) = self.completion_engine.complete(line, pos);

        // Convert to reedline Suggestions
        candidates
            .into_iter()
            .map(|pair| Suggestion {
                value: pair.replacement,
                description: pair.description,
                style: None,
                extra: None,
                span: Span::new(start, pos),
                append_whitespace: false,
                match_indices: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_completer() -> MongoCompleter {
        let shared_state = SharedState::new("test".to_string());
        MongoCompleter::new(shared_state, None)
    }

    #[test]
    fn test_complete_collection_dot() {
        let mut completer = create_test_completer();
        let suggestions = completer.complete("db.users.", 9);

        // Should complete operation names
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.value == "find"));
        assert!(suggestions.iter().any(|s| s.value == "insertOne"));
    }

    #[test]
    fn test_complete_with_prefix() {
        let mut completer = create_test_completer();
        let suggestions = completer.complete("db.users.fi", 11);

        // Should complete operations starting with "fi"
        assert!(suggestions.iter().any(|s| s.value == "find"));
        assert!(suggestions.iter().any(|s| s.value == "findOne"));
        assert!(!suggestions.iter().any(|s| s.value == "insertOne"));
    }

    #[test]
    fn test_span_position() {
        let mut completer = create_test_completer();
        let suggestions = completer.complete("db.users.find", 13);

        // Check that spans are correctly set
        for suggestion in suggestions {
            assert_eq!(suggestion.span.start, 9); // Start of "find"
            assert_eq!(suggestion.span.end, 13); // Current cursor position
        }
    }
}
