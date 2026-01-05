//! Custom prompt implementation for mongosh

use reedline::{Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus};

/// Custom prompt for mongosh REPL
pub struct MongoPrompt {
    /// Database name
    database: String,
    /// Whether connected to database
    connected: bool,
}

impl MongoPrompt {
    /// Create a new mongo prompt
    ///
    /// # Arguments
    /// * `database` - Database name
    /// * `connected` - Whether connected to database
    ///
    /// # Returns
    /// * `Self` - New prompt
    pub fn new(database: String, connected: bool) -> Self {
        Self {
            database,
            connected,
        }
    }
}

impl Prompt for MongoPrompt {
    /// Render the left prompt (main prompt)
    ///
    /// # Arguments
    /// * `_prompt_mode` - Current prompt mode
    /// * `_prompt_edit_mode` - Current edit mode
    /// * `_is_running` - Whether editor is running
    ///
    /// # Returns
    /// * `std::borrow::Cow<str>` - Prompt string
    fn render_prompt_left(&self) -> std::borrow::Cow<'_, str> {
        if self.connected {
            format!("{}> ", self.database).into()
        } else {
            format!("{} (disconnected)> ", self.database).into()
        }
    }

    /// Render the right prompt (empty in our case)
    ///
    /// # Returns
    /// * `std::borrow::Cow<str>` - Right prompt string (empty)
    fn render_prompt_right(&self) -> std::borrow::Cow<'_, str> {
        "".into()
    }

    /// Render the prompt indicator
    ///
    /// # Arguments
    /// * `_prompt_mode` - Current prompt mode
    /// * `_prompt_edit_mode` - Current edit mode
    ///
    /// # Returns
    /// * `std::borrow::Cow<str>` - Indicator string (empty since we include it in left prompt)
    fn render_prompt_indicator(&self, _prompt_mode: PromptEditMode) -> std::borrow::Cow<'_, str> {
        "".into()
    }

    /// Render the multiline prompt indicator
    ///
    /// # Arguments
    /// * `_prompt_mode` - Current prompt mode
    /// * `_prompt_edit_mode` - Current edit mode
    ///
    /// # Returns
    /// * `std::borrow::Cow<str>` - Multiline indicator
    fn render_prompt_multiline_indicator(&self) -> std::borrow::Cow<'_, str> {
        "... ".into()
    }

    /// Render the history search prompt
    ///
    /// # Arguments
    /// * `history_search` - History search state
    ///
    /// # Returns
    /// * `std::borrow::Cow<str>` - History search prompt
    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> std::borrow::Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };

        format!("({}reverse-search: {}) ", prefix, history_search.term).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_prompt() {
        let prompt = MongoPrompt::new("test".to_string(), true);
        let rendered = prompt.render_prompt_left();
        assert_eq!(rendered, "test> ");
    }

    #[test]
    fn test_disconnected_prompt() {
        let prompt = MongoPrompt::new("test".to_string(), false);
        let rendered = prompt.render_prompt_left();
        assert_eq!(rendered, "test (disconnected)> ");
    }

    #[test]
    fn test_right_prompt_empty() {
        let prompt = MongoPrompt::new("test".to_string(), true);
        let rendered = prompt.render_prompt_right();
        assert_eq!(rendered, "");
    }

    #[test]
    fn test_indicator_empty() {
        let prompt = MongoPrompt::new("test".to_string(), true);
        let rendered = prompt.render_prompt_indicator(PromptEditMode::Default);
        assert_eq!(rendered, "");
    }

    #[test]
    fn test_multiline_indicator() {
        let prompt = MongoPrompt::new("test".to_string(), true);
        let rendered = prompt.render_prompt_multiline_indicator();
        assert_eq!(rendered, "... ");
    }
}
