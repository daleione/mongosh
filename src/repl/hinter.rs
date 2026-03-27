//! Hinter for reedline - provides inline hints based on history and AI completion
//!
//! This module implements the reedline `Hinter` trait with two hint sources:
//!
//! - **Priority 1**: AI FIM completion (via DeepSeek API, cached, async)
//! - **Priority 2**: History prefix matching (existing behavior)
//!
//! AI hints are displayed in cyan italic; history hints in dark-gray italic,
//! so users can visually distinguish the source.

use std::sync::Arc;

use nu_ansi_term::{Color, Style};
use reedline::{Hinter, History};

use super::ai_completion::AiCompletionService;

/// Word-boundary delimiters used for `next_hint_token()`.
const HINT_DELIMITERS: &[char] = &[
    ' ', '.', ',', ':', ';', '{', '}', '(', ')', '[', ']', '"', '\'', '$', '\n',
];

/// MongoDB hinter for reedline with optional AI completion support.
pub struct MongoHinter {
    /// Style for history-based hints (gray italic).
    history_style: Style,
    /// Style for AI-based hints (cyan italic).
    ai_style: Style,
    /// The currently active hint text (plain, without ANSI).
    current_hint: String,

    // ── AI completion ───────────────────────────────────────────────────
    /// AI completion service. `None` when AI is not enabled.
    ai_service: Option<Arc<AiCompletionService>>,
    /// Maximum number of history lines to include in the FIM prompt.
    history_context_lines: usize,
}

impl MongoHinter {
    /// Create a new hinter with optional AI completion support.
    ///
    /// # Arguments
    /// * `ai_service`           - The AI completion service (may be `None`).
    /// * `history_context_lines`- How many recent history commands to feed
    ///                            into the FIM prompt (0 = none).
    pub fn new(ai_service: Option<Arc<AiCompletionService>>, history_context_lines: usize) -> Self {
        Self {
            history_style: Style::new().italic().fg(Color::DarkGray),
            ai_style: Style::new().italic().fg(Color::Cyan),
            current_hint: String::new(),
            ai_service,
            history_context_lines,
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────

    /// Extract the most recent `n` commands from reedline history.
    fn extract_recent_history(history: &dyn History, n: usize) -> Vec<String> {
        if n == 0 {
            return Vec::new();
        }
        history
            .search(reedline::SearchQuery::last_with_search(
                reedline::SearchFilter::anything(None),
            ))
            .unwrap_or_default()
            .into_iter()
            .take(n)
            .map(|item| item.command_line)
            .collect()
    }

    /// Apply a styled hint, store it, and return the ANSI string.
    fn styled_hint(&mut self, hint: &str, style: &Style, use_ansi: bool) -> String {
        self.current_hint = hint.to_string();
        if use_ansi {
            style.paint(hint).to_string()
        } else {
            hint.to_string()
        }
    }
}

impl Default for MongoHinter {
    fn default() -> Self {
        Self::new(None, 0)
    }
}

impl Hinter for MongoHinter {
    /// Provide a hint for the current buffer.
    ///
    /// **Important:** In reedline 0.45 `line` is the *full* multi-line edit
    /// buffer (with embedded `\n`), and `pos` is the absolute byte offset of
    /// the cursor inside that buffer.  This lets us construct a proper FIM
    /// prompt (everything before cursor) and suffix (everything after cursor).
    fn handle(
        &mut self,
        line: &str,
        pos: usize,
        history: &dyn History,
        use_ansi_coloring: bool,
        _cwd: &str,
    ) -> String {
        self.current_hint.clear();

        // Don't hint for empty input.
        if line.trim().is_empty() {
            return String::new();
        }

        // Split buffer at cursor → prompt / suffix for FIM.
        let line_before = if pos <= line.len() {
            &line[..pos]
        } else {
            line
        };
        let suffix = if pos < line.len() { &line[pos..] } else { "" };

        // For history hints we only work when cursor is at the end.
        let cursor_at_end = pos == line.len();

        // ── Priority 1: AI completion (cache lookup — sync, <1µs) ───────
        //
        // NOTE: reedline always renders hints *after* the entire buffer
        // (i.e. after `after_cursor` text) and `HistoryHintComplete` only
        // inserts when `is_cursor_at_buffer_end()` is true.  Therefore AI
        // hints are only useful when the cursor is at the very end of the
        // buffer.  When the cursor is in the middle (e.g. editing inside
        // `{}`), displaying a hint would be visually misleading (it appears
        // after the suffix like `});me:1`) and Ctrl+F would not insert it.
        if cursor_at_end {
            if let Some(ref ai_service) = self.ai_service {
                // Cache lookup.
                if let Some(ai_hint) = ai_service.get_cached(line_before) {
                    if !ai_hint.is_empty() {
                        return self.styled_hint(
                            &ai_hint,
                            &self.ai_style.clone(),
                            use_ansi_coloring,
                        );
                    }
                    // ai_hint == "" means negative cache hit — skip to history.
                }

                // Cache miss → send async request (non-blocking).
                if line_before.len() >= ai_service.min_trigger_length() {
                    let recent = Self::extract_recent_history(history, self.history_context_lines);
                    ai_service.request_completion(line_before, suffix, recent);
                }
            }
        }

        // ── Priority 2: History prefix matching ─────────────────────────
        // History hints only make sense when cursor is at the very end of
        // the buffer; in the middle, FIM is the right tool.
        if cursor_at_end {
            let search_result = history
                .search(reedline::SearchQuery::last_with_prefix(
                    line.to_string(),
                    None,
                ))
                .ok()
                .and_then(|results| results.into_iter().next());

            if let Some(history_item) = search_result {
                let history_line = history_item.command_line.as_str();
                if history_line.len() > line.len() && history_line.starts_with(line) {
                    let hint = &history_line[line.len()..];
                    return self.styled_hint(hint, &self.history_style.clone(), use_ansi_coloring);
                }
            }
        }

        // ── Priority 3: No hint ─────────────────────────────────────────
        String::new()
    }

    /// Return the next "word" of the current hint.
    ///
    /// This is called by reedline when the user triggers word-wise hint
    /// acceptance (e.g. `Alt+F`).  We split at MongoDB-aware delimiters
    /// so that operators like `$gt` or field paths like `user.name` are
    /// accepted as single tokens.
    fn next_hint_token(&self) -> String {
        if self.current_hint.is_empty() {
            return String::new();
        }

        let hint = &self.current_hint;

        // Skip leading delimiters (include them in the token so they're
        // inserted too — e.g. the leading `:` in `: {$gt: 18}`).
        let content_start = hint
            .find(|c: char| !HINT_DELIMITERS.contains(&c))
            .unwrap_or(hint.len());

        // Find the next delimiter after the content.
        let content_end = hint[content_start..]
            .find(|c: char| HINT_DELIMITERS.contains(&c))
            .map(|i| content_start + i)
            .unwrap_or(hint.len());

        // Always return at least one character.
        let end = content_end.max(1).min(hint.len());
        hint[..end].to_string()
    }

    /// Return the complete hint (for full-hint acceptance via `Ctrl+F`).
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
        Box::new(
            FileBackedHistory::with_file(100, PathBuf::from("/tmp/test_mongosh_hinter.txt"))
                .unwrap_or_else(|_| FileBackedHistory::new(100).expect("Failed to create history")),
        )
    }

    // ── Construction ────────────────────────────────────────────────────

    #[test]
    fn test_new_hinter() {
        let hinter = MongoHinter::new(None, 0);
        assert_eq!(hinter.complete_hint(), "");
        assert_eq!(hinter.next_hint_token(), "");
    }

    #[test]
    fn test_default() {
        let hinter = MongoHinter::default();
        assert_eq!(hinter.complete_hint(), "");
    }

    // ── handle() basics ─────────────────────────────────────────────────

    #[test]
    fn test_empty_line_no_hint() {
        let mut hinter = MongoHinter::new(None, 0);
        let history = create_test_history();
        let hint = hinter.handle("", 0, history.as_ref(), true, "/tmp");
        assert_eq!(hint, "");
    }

    #[test]
    fn test_whitespace_only_no_hint() {
        let mut hinter = MongoHinter::new(None, 0);
        let history = create_test_history();
        let hint = hinter.handle("   ", 3, history.as_ref(), true, "/tmp");
        assert_eq!(hint, "");
    }

    #[test]
    fn test_cursor_not_at_end_no_history_hint() {
        // When cursor is in the middle, history hints are skipped
        // (only AI would fire, but we have no AI service here).
        let mut hinter = MongoHinter::new(None, 0);
        let history = create_test_history();
        let hint = hinter.handle("db.users", 2, history.as_ref(), false, "/tmp");
        assert_eq!(hint, "");
    }

    #[test]
    fn test_mid_buffer_no_ai_hint_displayed() {
        // Regression test: when cursor is in the middle of the buffer
        // (e.g. editing inside `{}` projection), AI hints must NOT be
        // returned.  reedline renders hints after the entire buffer, so
        // a mid-buffer hint like `me:1,create_time:1` would appear after
        // the suffix `});` → `db.templates.findOne({},{na});me:1,create_time:1`
        // which is visually wrong.  Also, HistoryHintComplete requires
        // is_cursor_at_buffer_end() so Ctrl+F would not insert it.
        let mut hinter = MongoHinter::new(None, 0);
        let history = create_test_history();

        // Simulate: buffer = "db.templates.findOne({},{na});"
        //           cursor after "na" (pos=28), suffix = "});"
        let buffer = "db.templates.findOne({},{na});";
        let cursor_pos = buffer.find("na").unwrap() + 2; // right after "na"
        assert!(cursor_pos < buffer.len(), "cursor must be in the middle");

        let hint = hinter.handle(buffer, cursor_pos, history.as_ref(), true, "/tmp");
        assert_eq!(
            hint, "",
            "no hint should be shown when cursor is not at buffer end"
        );
    }

    // ── next_hint_token ─────────────────────────────────────────────────

    #[test]
    fn test_next_hint_token_empty() {
        let hinter = MongoHinter::new(None, 0);
        assert_eq!(hinter.next_hint_token(), "");
    }

    #[test]
    fn test_next_hint_token_simple_word() {
        let mut hinter = MongoHinter::new(None, 0);
        hinter.current_hint = "findOne({})".to_string();
        assert_eq!(hinter.next_hint_token(), "findOne");
    }

    #[test]
    fn test_next_hint_token_starts_with_delimiter() {
        let mut hinter = MongoHinter::new(None, 0);
        hinter.current_hint = ": {$gt: 18}".to_string();
        // Delimiters: ':', ' ', '{', '$' are all in HINT_DELIMITERS.
        // content_start finds first non-delimiter char → 'g' at index 4.
        // content_end finds next delimiter in "gt: 18}" → ':' at index 6.
        // Result: hint[..6] = ": {$gt"
        let token = hinter.next_hint_token();
        assert_eq!(token, ": {$gt");
    }

    #[test]
    fn test_next_hint_token_operator() {
        let mut hinter = MongoHinter::new(None, 0);
        hinter.current_hint = "$gt".to_string();
        // '$' is a delimiter, so content_start=1, then "gt" until end
        let token = hinter.next_hint_token();
        assert_eq!(token, "$gt");
    }

    #[test]
    fn test_next_hint_token_single_char() {
        let mut hinter = MongoHinter::new(None, 0);
        hinter.current_hint = "}".to_string();
        // '}' is a delimiter, content_start = len=1, content_end = 1, end = max(1,1) = 1
        assert_eq!(hinter.next_hint_token(), "}");
    }

    // ── complete_hint ───────────────────────────────────────────────────

    #[test]
    fn test_complete_hint() {
        let mut hinter = MongoHinter::new(None, 0);
        hinter.current_hint = "some completion text".to_string();
        assert_eq!(hinter.complete_hint(), "some completion text");
    }

    // ── extract_recent_history ──────────────────────────────────────────

    #[test]
    fn test_extract_zero_history() {
        let history = create_test_history();
        let result = MongoHinter::extract_recent_history(history.as_ref(), 0);
        assert!(result.is_empty());
    }
}
