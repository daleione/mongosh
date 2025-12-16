//! REPL (Read-Eval-Print Loop) engine for mongosh
//!
//! This module provides an interactive shell interface with features:
//! - Command line editing with rustyline
//! - Command history management
//! - Auto-completion for commands and collections
//! - Syntax highlighting
//! - Multi-line input support
//! - Contextual prompts

use rustyline::completion::{Completer, Pair};

use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Config, Editor, Helper};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::config::{HistoryConfig, OutputFormat};
use crate::error::Result;
use crate::parser::Parser;

/// Shared state between REPL and execution context
#[derive(Debug, Clone)]
pub struct SharedState {
    /// Current database name
    pub current_database: Arc<RwLock<String>>,

    /// Current connection URI
    pub connection_uri: String,

    /// Whether connected to server
    pub connected: Arc<RwLock<bool>>,

    /// Server version
    pub server_version: Arc<RwLock<Option<String>>>,

    /// Output format setting
    pub output_format: Arc<RwLock<OutputFormat>>,

    /// Color output setting
    pub color_enabled: Arc<RwLock<bool>>,
}

impl SharedState {
    /// Create a new shared state
    ///
    /// # Arguments
    /// * `database` - Initial database name
    /// * `uri` - Connection URI
    ///
    /// # Returns
    /// * `Self` - New shared state
    pub fn new(database: String, uri: String) -> Self {
        Self {
            current_database: Arc::new(RwLock::new(database)),
            connection_uri: uri,
            connected: Arc::new(RwLock::new(false)),
            server_version: Arc::new(RwLock::new(None)),
            output_format: Arc::new(RwLock::new(OutputFormat::Shell)),
            color_enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Get current database name
    ///
    /// # Returns
    /// * `String` - Current database name
    pub fn get_database(&self) -> String {
        self.current_database.read().unwrap().clone()
    }

    /// Set current database name
    ///
    /// # Arguments
    /// * `database` - New database name
    pub fn set_database(&mut self, database: String) {
        *self.current_database.write().unwrap() = database;
    }

    /// Get current output format
    ///
    /// # Returns
    /// * `OutputFormat` - Current output format
    pub fn get_format(&self) -> OutputFormat {
        *self.output_format.read().unwrap()
    }

    /// Set output format
    ///
    /// # Arguments
    /// * `format` - New output format
    pub fn set_format(&self, format: OutputFormat) {
        *self.output_format.write().unwrap() = format;
    }

    /// Get current color setting
    ///
    /// # Returns
    /// * `bool` - True if color output is enabled
    pub fn get_color_enabled(&self) -> bool {
        *self.color_enabled.read().unwrap()
    }

    /// Set color output
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable color output
    pub fn set_color_enabled(&self, enabled: bool) {
        *self.color_enabled.write().unwrap() = enabled;
    }

    /// Check if connected
    ///
    /// # Returns
    /// * `bool` - True if connected
    pub fn is_connected(&self) -> bool {
        *self.connected.read().unwrap()
    }

    /// Mark as connected
    ///
    /// # Arguments
    /// * `version` - Server version
    pub fn set_connected(&mut self, version: Option<String>) {
        *self.connected.write().unwrap() = true;
        *self.server_version.write().unwrap() = version;
    }

    /// Mark as disconnected
    pub fn set_disconnected(&mut self) {
        *self.connected.write().unwrap() = false;
        *self.server_version.write().unwrap() = None;
    }
}

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

/// REPL context holding state information (deprecated - use SharedState)
#[derive(Debug, Clone)]
pub struct ReplContext {
    /// Current database name
    pub current_database: String,

    /// Current connection URI
    pub connection_uri: String,

    /// Whether connected to server
    pub connected: bool,

    /// Server version
    pub server_version: Option<String>,

    /// Enable colored output
    pub color_enabled: bool,

    /// Enable syntax highlighting
    pub highlighting_enabled: bool,
}

/// Helper for rustyline providing completion, hints, and highlighting
pub struct ReplHelper {
    /// Shared state for contextual completion
    shared_state: SharedState,

    /// Available commands for completion
    commands: Vec<String>,

    /// Available collections for completion
    collections: Vec<String>,

    /// Enable colored output
    color_enabled: bool,

    /// Enable syntax highlighting
    highlighting_enabled: bool,
}

impl ReplHelper {
    /// Check if input is a complete statement
    ///
    /// # Arguments
    /// * `input` - Input string to check
    ///
    /// # Returns
    /// * `bool` - True if complete
    fn is_complete_statement(&self, input: &str) -> bool {
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

/// Command completer for auto-completion
pub struct CommandCompleter {
    /// Available commands
    commands: Vec<String>,
}

/// Prompt configuration and generation
pub struct PromptGenerator {
    /// Current context
    context: ReplContext,
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

    /// Create from legacy ReplContext (for backward compatibility)
    ///
    /// # Arguments
    /// * `context` - Initial REPL context
    /// * `history_config` - History configuration
    ///
    /// # Returns
    /// * `Result<Self>` - New REPL engine or error
    pub fn from_context(context: ReplContext, history_config: HistoryConfig) -> Result<Self> {
        let shared_state = SharedState::new(context.current_database, context.connection_uri);
        Self::new(
            shared_state,
            history_config,
            context.color_enabled,
            context.highlighting_enabled,
        )
    }

    /// Start the REPL loop
    ///
    /// Continuously reads input, parses commands, and returns them for execution.
    ///
    /// # Returns
    /// * `Result<()>` - Ok when REPL exits normally, error on failure
    pub async fn run(&mut self) -> Result<()> {
        todo!("Main REPL loop: read input, parse, and yield commands")
    }

    /// Read a single line of input
    ///
    /// # Returns
    /// * `Result<Option<String>>` - Input line or None on EOF
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
            Err(err) => Err(crate::error::MongoshError::Generic(format!(
                "Read error: {}",
                err
            ))),
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
                    return Err(crate::error::MongoshError::Generic(
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
    /// * `Result<crate::parser::Command>` - Parsed command or error
    pub fn process_input(&mut self, input: &str) -> Result<crate::parser::Command> {
        self.parser.parse(input)
    }

    /// Get shared state reference
    ///
    /// # Returns
    /// * `&SharedState` - Shared state reference
    pub fn shared_state(&self) -> &SharedState {
        &self.shared_state
    }

    /// Update REPL context (deprecated - state is automatically synchronized)
    ///
    /// # Arguments
    /// * `context` - New context
    #[deprecated(note = "Use shared_state instead - state is automatically synchronized")]
    pub fn update_context(&mut self, _context: ReplContext) {
        // No-op: state is now automatically synchronized via SharedState
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
        // Now we can call synchronously!
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
            .map_or(true, |h| h.is_complete_statement(input))
    }
}

impl ReplContext {
    /// Create a new REPL context
    ///
    /// # Arguments
    /// * `database` - Initial database name
    /// * `uri` - Connection URI
    ///
    /// # Returns
    /// * `Self` - New context
    pub fn new(database: String, uri: String) -> Self {
        Self {
            current_database: database,
            connection_uri: uri,
            connected: false,
            server_version: None,
            color_enabled: true,
            highlighting_enabled: true,
        }
    }

    /// Mark as connected
    ///
    /// # Arguments
    /// * `version` - Server version
    pub fn set_connected(&mut self, version: Option<String>) {
        self.connected = true;
        self.server_version = version;
    }

    /// Mark as disconnected
    pub fn set_disconnected(&mut self) {
        self.connected = false;
        self.server_version = None;
    }

    /// Change current database
    ///
    /// # Arguments
    /// * `database` - New database name
    pub fn set_database(&mut self, database: String) {
        self.current_database = database;
    }
}

impl ReplHelper {
    /// Create a new REPL helper
    ///
    /// # Arguments
    /// * `shared_state` - Shared state
    /// * `color_enabled` - Enable colored output
    /// * `highlighting_enabled` - Enable syntax highlighting
    ///
    /// # Returns
    /// * `Self` - New helper
    pub fn new(shared_state: SharedState, color_enabled: bool, highlighting_enabled: bool) -> Self {
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
            color_enabled,
            highlighting_enabled,
        }
    }

    /// Add collection for completion
    ///
    /// # Arguments
    /// * `collection` - Collection name
    pub fn add_collection(&mut self, collection: String) {
        if !self.collections.contains(&collection) {
            self.collections.push(collection);
        }
    }

    /// Set collections for completion
    ///
    /// # Arguments
    /// * `collections` - List of collection names
    pub fn set_collections(&mut self, collections: Vec<String>) {
        self.collections = collections;
    }
}

impl Helper for ReplHelper {}

impl Completer for ReplHelper {
    type Candidate = Pair;

    /// Complete input at given position
    ///
    /// # Arguments
    /// * `line` - Current line
    /// * `pos` - Cursor position
    /// * `ctx` - Readline context
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
    /// * `line` - Current line
    /// * `pos` - Cursor position
    /// * `ctx` - Readline context
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
    /// * `pos` - Cursor position
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
    /// * `line` - Current line
    /// * `pos` - Character position
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

impl PromptGenerator {
    /// Create a new prompt generator
    ///
    /// # Arguments
    /// * `context` - REPL context (deprecated)
    ///
    /// # Returns
    /// * `Self` - New generator
    pub fn new(context: ReplContext) -> Self {
        Self { context }
    }

    /// Generate prompt string
    ///
    /// Format: "database> " or "database (disconnected)> "
    ///
    /// # Returns
    /// * `String` - Formatted prompt
    pub fn generate(&self) -> String {
        if self.context.connected {
            format!("{}> ", self.context.current_database)
        } else {
            format!("{} (disconnected)> ", self.context.current_database)
        }
    }

    /// Generate continuation prompt for multi-line input
    ///
    /// # Returns
    /// * `String` - Continuation prompt
    pub fn generate_continuation(&self) -> String {
        "... ".to_string()
    }
}

impl CommandCompleter {
    /// Create a new command completer
    ///
    /// # Returns
    /// * `Self` - New completer
    pub fn new() -> Self {
        Self {
            commands: vec![
                "show dbs".to_string(),
                "show databases".to_string(),
                "show collections".to_string(),
                "show users".to_string(),
                "use".to_string(),
                "exit".to_string(),
                "quit".to_string(),
                "help".to_string(),
            ],
        }
    }

    /// Get completions for partial input
    ///
    /// # Arguments
    /// * `partial` - Partial input string
    ///
    /// # Returns
    /// * `Vec<String>` - Matching completions
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(partial))
            .cloned()
            .collect()
    }
}

impl Default for CommandCompleter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_context_creation() {
        let context = ReplContext::new("test".to_string(), "mongodb://localhost".to_string());
        assert_eq!(context.current_database, "test");
        assert!(!context.connected);
    }

    #[test]
    fn test_repl_context_connection() {
        let mut context = ReplContext::new("test".to_string(), "mongodb://localhost".to_string());
        context.set_connected(Some("5.0.0".to_string()));
        assert!(context.connected);
        assert_eq!(context.server_version, Some("5.0.0".to_string()));
    }

    #[test]
    fn test_prompt_generation() {
        let context = ReplContext::new("mydb".to_string(), "mongodb://localhost".to_string());
        let prompt = PromptGenerator::new(context).generate();
        assert!(prompt.contains("mydb"));
    }

    #[test]
    fn test_command_completer() {
        let completer = CommandCompleter::new();
        let completions = completer.get_completions("show");
        assert!(!completions.is_empty());
        assert!(completions.iter().all(|c| c.starts_with("show")));
    }
}
