use nu_ansi_term::Color;
use reedline::{
    EditCommand, Emacs, FileBackedHistory, IdeMenu, KeyCode, KeyModifiers, MenuBuilder, Reedline,
    ReedlineEvent, ReedlineMenu, Signal, default_emacs_keybindings,
};

use std::sync::Arc;

use crate::config::{AiConfig, HistoryConfig};
use crate::error::{MongoshError, Result};
use crate::executor::ExecutionContext;
use crate::parser::{Command, Parser};

use super::ai_completion::AiCompletionService;
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
    /// Create a new REPL engine with shared state and optional AI completion.
    ///
    /// # Arguments
    /// * `shared_state` - Shared state with execution context
    /// * `history_config` - History configuration
    /// * `highlighting_enabled` - Enable syntax highlighting
    /// * `execution_context` - Optional execution context for completion
    /// * `ai_config` - Optional AI completion configuration
    ///
    /// # Returns
    /// * `Result<Self>` - New REPL engine or error
    pub fn new(
        shared_state: SharedState,
        history_config: HistoryConfig,
        highlighting_enabled: bool,
        execution_context: Option<Arc<ExecutionContext>>,
        ai_config: Option<AiConfig>,
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

        // Ctrl+F to accept the full hint (AI or history)
        // If no hint, move cursor forward (default behavior)
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('f'),
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::HistoryHintComplete,
                ReedlineEvent::Edit(vec![EditCommand::MoveRight { select: false }]),
            ]),
        );

        // Alt+F to accept the next word of the hint (word-wise acceptance)
        // This is especially useful for long AI completions where the user
        // wants to accept only part of the suggestion.
        keybindings.add_binding(
            KeyModifiers::ALT,
            KeyCode::Char('f'),
            ReedlineEvent::HistoryHintWordComplete,
        );

        let edit_mode = Box::new(Emacs::new(keybindings));

        // Create highlighter with auto-detect mode
        let highlighter = Box::new(SyntaxHighlighter::new(
            SyntaxMode::Auto,
            highlighting_enabled,
        ));

        // Extract history_context_lines from ai_config BEFORE it's consumed.
        let history_context_lines = ai_config
            .as_ref()
            .filter(|c| c.is_effectively_enabled())
            .map(|c| c.history_context_lines)
            .unwrap_or(0);

        // Resolve the current datasource name for AI context file lookup.
        let datasource_name = execution_context
            .as_ref()
            .map(|ctx| {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(ctx.get_current_datasource())
                })
            })
            .unwrap_or_default();

        // Create AI completion service if configured and effectively enabled.
        let ai_service = ai_config
            .filter(|c| c.is_effectively_enabled())
            .map(|config| {
                tracing::info!("AI completion enabled (model: {})", config.model);
                Arc::new(AiCompletionService::new(
                    config,
                    shared_state.clone(),
                    datasource_name,
                ))
            });

        // Create hinter — with AI service if available
        let hinter = Box::new(MongoHinter::new(ai_service, history_context_lines));

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

    /// Read a line of input with the buffer pre-filled with `initial` text.
    ///
    /// The user sees the text already in the input line and can edit it
    /// freely before pressing Enter. Ctrl-C / Ctrl-D cancels as usual.
    ///
    /// # Arguments
    /// * `initial` - Text to pre-populate in the editor buffer
    ///
    /// # Returns
    /// * `Result<Option<String>>` - Edited line or None on cancel
    pub fn read_line_with_initial(&mut self, initial: &str) -> Result<Option<String>> {
        // Pre-fill the buffer before entering read_line.
        // run_edit_commands inserts text at the current cursor position;
        // since the buffer was cleared by the previous submit this starts
        // from an empty state.
        self.editor
            .run_edit_commands(&[EditCommand::InsertString(initial.to_string())]);

        self.read_line()
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
