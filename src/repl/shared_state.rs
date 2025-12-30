use std::sync::{Arc, RwLock};

use crate::config::OutputFormat;
use crate::repl::CursorState;

/// Shared state between REPL and execution context.
#[derive(Debug, Clone)]
pub struct SharedState {
    /// Current database name
    pub current_database: Arc<RwLock<String>>,

    /// Whether connected to server
    pub connected: Arc<RwLock<bool>>,

    /// Server version
    pub server_version: Arc<RwLock<Option<String>>>,

    /// Output format setting
    pub output_format: Arc<RwLock<OutputFormat>>,

    /// Color output setting
    pub color_enabled: Arc<RwLock<bool>>,

    /// Cursor state for pagination
    pub cursor_state: Arc<RwLock<Option<CursorState>>>,
}

impl SharedState {
    /// Create a new shared state.
    ///
    /// * `database` - Initial database name
    pub fn new(database: String) -> Self {
        Self {
            current_database: Arc::new(RwLock::new(database)),
            connected: Arc::new(RwLock::new(false)),
            server_version: Arc::new(RwLock::new(None)),
            output_format: Arc::new(RwLock::new(OutputFormat::Shell)),
            color_enabled: Arc::new(RwLock::new(true)),
            cursor_state: Arc::new(RwLock::new(None)),
        }
    }

    /// Get current cursor state (cloned).
    pub fn get_cursor_state(&self) -> Option<CursorState> {
        let cursor_state = self.cursor_state.read().unwrap();
        cursor_state.clone()
    }

    /// Set cursor state.
    pub fn set_cursor_state(&self, state: Option<CursorState>) {
        let mut cursor_state = self.cursor_state.write().unwrap();
        *cursor_state = state;
    }

    /// Clear cursor state.
    pub fn clear_cursor_state(&self) {
        let mut cursor_state = self.cursor_state.write().unwrap();
        *cursor_state = None;
    }

    /// Check if there's an active cursor.
    pub fn has_active_cursor(&self) -> bool {
        let cursor_state = self.cursor_state.read().unwrap();
        cursor_state.is_some()
    }

    /// Get current database name.
    pub fn get_database(&self) -> String {
        self.current_database.read().unwrap().clone()
    }

    /// Set current database name.
    pub fn set_database(&mut self, database: String) {
        *self.current_database.write().unwrap() = database;
    }

    /// Get current output format.
    pub fn get_format(&self) -> OutputFormat {
        *self.output_format.read().unwrap()
    }

    /// Set output format.
    pub fn set_format(&self, format: OutputFormat) {
        *self.output_format.write().unwrap() = format;
    }

    /// Get current color setting.
    pub fn get_color_enabled(&self) -> bool {
        *self.color_enabled.read().unwrap()
    }

    /// Set color output.
    pub fn set_color_enabled(&self, enabled: bool) {
        *self.color_enabled.write().unwrap() = enabled;
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        *self.connected.read().unwrap()
    }

    /// Mark as connected and update server version.
    pub fn set_connected(&mut self, version: Option<String>) {
        *self.connected.write().unwrap() = true;
        *self.server_version.write().unwrap() = version;
    }
}
