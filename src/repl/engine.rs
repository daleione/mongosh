use nu_ansi_term::Color;
use reedline::{
    EditCommand, Emacs, FileBackedHistory, IdeMenu, KeyCode, KeyModifiers, MenuBuilder, Reedline,
    ReedlineEvent, ReedlineMenu, Signal, default_emacs_keybindings,
};

use std::sync::Arc;

use crate::config::HistoryConfig;
use crate::error::{MongoshError, Result};
use crate::executor::ExecutionContext;
use crate::parser::{Command, Parser};

use super::completer::MongoCompleter;
use super::highlighter::{SyntaxHighlighter, SyntaxMode};
use super::hinter::MongoHinter;
use super::prompt::MongoPrompt;
use super::shared_state::SharedState;
use super::validator::MongoValidator;

/// REPL engine for interactive command execution
pub struct ReplEngine {
    /// Line editor for command input
    editor: Reedline,

    /// Shared state with execution context
    shared_state: SharedState,

    /// Parser for command parsing
    parser: Parser,

    /// Whether to continue running
    running: bool,
}

impl ReplEngine {
    /// Create a new REPL engine with shared state
    ///
    /// # Arguments
    /// * `shared_state` - Shared state with execution context
    /// * `history_config` - History configuration
    /// * `highlighting_enabled` - Enable syntax highlighting
    /// * `execution_context` - Optional execution context for completion
    ///
    /// # Returns
    /// * `Result<Self>` - New REPL engine or error
    pub fn new(
        shared_state: SharedState,
        history_config: HistoryConfig,
        highlighting_enabled: bool,
        execution_context: Option<Arc<ExecutionContext>>,
    ) -> Result<Self> {
        // Setup history
        let history = if history_config.persist {
            Box::new(
                FileBackedHistory::with_file(
                    history_config.max_size,
                    history_config.file_path.clone(),
                )
                .map_err(|e| MongoshError::Generic(format!("Failed to setup history: {}", e)))?,
            )
        } else {
            Box::new(
                FileBackedHistory::new(history_config.max_size).map_err(|e| {
                    MongoshError::Generic(format!("Failed to create history: {}", e))
                })?,
            )
        };

        // Create completer
        let completer = Box::new(MongoCompleter::new(
            shared_state.clone(),
            execution_context.clone(),
        ));

        // Create completion menu with IdeMenu for better Tab completion behavior
        // IdeMenu completes common prefix first, then shows menu on subsequent Tab
        let completion_menu = Box::new(
            IdeMenu::default()
                .with_name("completion_menu")
                .with_text_style(Color::Cyan.normal()) // Cyan text for candidates
                .with_selected_text_style(Color::Black.on(Color::Cyan).bold()) // Black on cyan for selected
                .with_description_text_style(Color::DarkGray.normal()) // Gray for descriptions
                .with_marker(""), // Empty marker to avoid mode indicator change
        );

        // Setup keybindings
        let mut keybindings = default_emacs_keybindings();

        // Tab key activates completion menu or cycles through items
        // First Tab: opens menu, subsequent Tabs: cycle through items
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::MenuNext, // If menu is open, go to next item
                ReedlineEvent::Menu("completion_menu".to_string()), // Otherwise, open menu
            ]),
        );

        // Shift+Tab for previous completion
        keybindings.add_binding(
            KeyModifiers::SHIFT,
            KeyCode::BackTab,
            ReedlineEvent::MenuPrevious,
        );

        // Ctrl+F to accept hint (complete suggestion from history)
        // If no hint, move cursor forward (default behavior)
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('f'),
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::HistoryHintComplete,
                ReedlineEvent::Edit(vec![EditCommand::MoveRight { select: false }]),
            ]),
        );

        let edit_mode = Box::new(Emacs::new(keybindings));

        // Create highlighter with auto-detect mode
        let highlighter = Box::new(SyntaxHighlighter::new(
            SyntaxMode::Auto,
            highlighting_enabled,
        ));

        // Create hinter
        let hinter = Box::new(MongoHinter::new());

        // Create validator
        let validator = Box::new(MongoValidator::new());

        // Build the editor
        let editor = Reedline::create()
            .with_history(history)
            .with_completer(completer)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_edit_mode(edit_mode)
            .with_highlighter(highlighter)
            .with_hinter(hinter)
            .with_validator(validator)
            .with_quick_completions(true) // Show completions without waiting
            .with_partial_completions(true) // Allow partial completion
            .use_kitty_keyboard_enhancement(false); // Disable for better compatibility

        Ok(Self {
            editor,
            shared_state,
            parser: Parser::new(),
            running: true,
        })
    }

    /// Read a single line of input
    ///
    /// # Returns
    /// * `Result<Option<String>>` - Input line or None on EOF / interrupt
    pub fn read_line(&mut self) -> Result<Option<String>> {
        let database = self.shared_state.get_database();
        let connected = self.shared_state.is_connected();
        let prompt = MongoPrompt::new(database, connected);

        match self.editor.read_line(&prompt) {
            Ok(Signal::Success(buffer)) => Ok(Some(buffer)),
            Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => {
                // Ctrl-D or Ctrl-C
                Ok(None)
            }
            Err(err) => Err(MongoshError::Generic(format!("Read error: {}", err))),
        }
    }

    /// Process user input and parse into command
    ///
    /// # Arguments
    /// * `input` - User input string
    ///
    /// # Returns
    /// * `Result<Command>` - Parsed command or error
    pub fn process_input(&mut self, input: &str) -> Result<Command> {
        self.parser.parse(input)
    }

    /// Check if REPL is still running
    ///
    /// # Returns
    /// * `bool` - True if running
    pub fn is_running(&self) -> bool {
        self.running
    }
}
