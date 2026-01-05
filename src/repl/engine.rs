use rustyline::history::DefaultHistory;
use rustyline::{Config, Editor};

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::HistoryConfig;
use crate::error::{MongoshError, Result};
use crate::executor::ExecutionContext;
use crate::parser::{Command, Parser};

use super::helper::ReplHelper;
use super::shared_state::SharedState;

/// REPL engine for interactive command execution
pub struct ReplEngine {
    /// Line editor for command input
    editor: Editor<ReplHelper, DefaultHistory>,

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
        let config = Config::builder()
            .max_history_size(history_config.max_size)?
            .history_ignore_space(true)
            .auto_add_history(true)
            .build();

        let helper = ReplHelper::new(
            shared_state.clone(),
            highlighting_enabled,
            execution_context,
        );
        let mut editor = Editor::<ReplHelper, DefaultHistory>::with_config(config)?;
        editor.set_helper(Some(helper));

        // Load history if persistent
        if history_config.persist {
            let _ = editor.load_history(&history_config.file_path);
        }

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
        let prompt = self.generate_prompt();
        match self.editor.readline(&prompt) {
            Ok(line) => {
                // Add to history
                let _ = self.editor.add_history_entry(line.as_str());
                Ok(Some(line))
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                // Ctrl-C
                Ok(None)
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                // Ctrl-D
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

    /// Save history to file
    ///
    /// # Arguments
    /// * `path` - Path to history file
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn save_history(&mut self, path: &PathBuf) -> Result<()> {
        self.editor.save_history(path)?;
        Ok(())
    }

    /// Check if REPL is still running
    ///
    /// # Returns
    /// * `bool` - True if running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Generate prompt string based on shared state
    ///
    /// # Returns
    /// * `String` - Prompt string
    fn generate_prompt(&self) -> String {
        let database = self.shared_state.get_database();
        let connected = self.shared_state.is_connected();

        if connected {
            format!("{}> ", database)
        } else {
            format!("{} (disconnected)> ", database)
        }
    }
}
