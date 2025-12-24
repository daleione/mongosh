use rustyline::history::DefaultHistory;
use rustyline::{Config, Editor};

use std::path::PathBuf;

use crate::config::HistoryConfig;
use crate::error::{MongoshError, Result};
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

    /// Enable colored output
    color_enabled: bool,

    /// Enable syntax highlighting
    highlighting_enabled: bool,
}

impl ReplEngine {
    /// Create a new REPL engine with shared state
    ///
    /// # Arguments
    /// * `shared_state` - Shared state with execution context
    /// * `history_config` - History configuration
    /// * `color_enabled` - Enable colored output
    /// * `highlighting_enabled` - Enable syntax highlighting
    ///
    /// # Returns
    /// * `Result<Self>` - New REPL engine or error
    pub fn new(
        shared_state: SharedState,
        history_config: HistoryConfig,
        color_enabled: bool,
        highlighting_enabled: bool,
    ) -> Result<Self> {
        let config = Config::builder()
            .max_history_size(history_config.max_size)?
            .history_ignore_space(true)
            .auto_add_history(true)
            .build();

        let helper = ReplHelper::new(shared_state.clone(), color_enabled, highlighting_enabled);
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
            color_enabled,
            highlighting_enabled,
        })
    }

    /// Start the REPL loop
    ///
    /// Continuously reads input, parses commands, and returns them for execution.
    /// Currently left as a TODO – to be implemented by the higher-level shell.
    ///
    /// # Returns
    /// * `Result<()>` - Ok when REPL exits normally, error on failure
    pub async fn run(&mut self) -> Result<()> {
        todo!("Main REPL loop: read input, parse, and yield commands")
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

    /// Read multi-line input (for complex commands)
    ///
    /// # Returns
    /// * `Result<String>` - Complete multi-line input
    pub fn read_multiline(&mut self) -> Result<String> {
        let mut lines = Vec::new();
        let mut complete = false;

        while !complete {
            let prompt = if lines.is_empty() {
                self.generate_prompt()
            } else {
                "... ".to_string()
            };

            match self.editor.readline(&prompt) {
                Ok(line) => {
                    lines.push(line.clone());
                    let combined = lines.join("\n");

                    // Check if statement is complete
                    if self.is_complete_statement(&combined) {
                        complete = true;
                        let _ = self.editor.add_history_entry(combined.as_str());
                    }
                }
                Err(_) => {
                    return Err(MongoshError::Generic(
                        "Multi-line input interrupted".to_string(),
                    ));
                }
            }
        }

        Ok(lines.join("\n"))
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

    /// Get shared state reference
    ///
    /// # Returns
    /// * `&SharedState` - Shared state reference
    pub fn shared_state(&self) -> &SharedState {
        &self.shared_state
    }

    /// Add collection name for auto-completion
    ///
    /// # Arguments
    /// * `collection` - Collection name
    pub fn add_collection(&mut self, collection: String) {
        if let Some(helper) = self.editor.helper_mut() {
            helper.add_collection(collection);
        }
    }

    /// Set available collections for completion
    ///
    /// # Arguments
    /// * `collections` - List of collection names
    pub fn set_collections(&mut self, collections: Vec<String>) {
        if let Some(helper) = self.editor.helper_mut() {
            helper.set_collections(collections);
        }
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

    /// Stop the REPL
    pub fn stop(&mut self) {
        self.running = false;
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

    /// Check if input is a complete statement (delegate to helper)
    ///
    /// # Arguments
    /// * `input` - Input string to check
    ///
    /// # Returns
    /// * `bool` - True if complete
    fn is_complete_statement(&self, input: &str) -> bool {
        self.editor
            .helper()
            .is_none_or(|h| h.is_complete_statement(input))
    }
}
