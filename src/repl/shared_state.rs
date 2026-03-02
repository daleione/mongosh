use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

use crate::config::{DisplayConfig, OutputFormat};
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
    /// Uses Mutex because cursor needs mutable access and is not Clone
    cursor_state: Arc<Mutex<Option<CursorState>>>,
}

impl SharedState {
    /// Create a new shared state.
    ///
    /// * `database` - Initial database name
    pub fn new(database: String) -> Self {
        Self::with_config(database, &DisplayConfig::default())
    }

    /// Create a new shared state with display configuration.
    ///
    /// * `database` - Initial database name
    /// * `display_config` - Display configuration settings
    pub fn with_config(database: String, display_config: &DisplayConfig) -> Self {
        Self {
            current_database: Arc::new(RwLock::new(database)),
            connected: Arc::new(RwLock::new(false)),
            server_version: Arc::new(RwLock::new(None)),
            output_format: Arc::new(RwLock::new(display_config.format)),
            color_enabled: Arc::new(RwLock::new(display_config.color_output)),
            cursor_state: Arc::new(Mutex::new(None)),
        }
    }

    /// Set active cursor state
    ///
    /// # Arguments
    /// * `state` - The cursor state to store
    pub async fn set_cursor(&self, state: CursorState) {
        let mut cursor = self.cursor_state.lock().await;
        *cursor = Some(state);
    }

    /// Get mutable reference to cursor state
    ///
    /// Returns a lock guard that can be used to access and mutate the cursor.
    /// The cursor remains locked until the guard is dropped.
    ///
    /// # Returns
    /// * `MutexGuard` - Guard providing access to the cursor state
    pub async fn get_cursor_mut(&self) -> tokio::sync::MutexGuard<'_, Option<CursorState>> {
        self.cursor_state.lock().await
    }

    /// Clear the active cursor
    pub async fn clear_cursor(&self) {
        let mut cursor = self.cursor_state.lock().await;
        *cursor = None;
    }

    /// Check if there's an active cursor
    ///
    /// # Returns
    /// * `bool` - True if a cursor is active
    #[allow(dead_code)]
    pub async fn has_cursor(&self) -> bool {
        let cursor = self.cursor_state.lock().await;
        cursor.is_some()
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

    /// Get server version.
    pub fn get_server_version(&self) -> Option<String> {
        self.server_version.read().unwrap().clone()
    }
}
