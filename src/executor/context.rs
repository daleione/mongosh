//! Execution context management
//!
//! This module provides the ExecutionContext which maintains state across
//! command executions, including database connections and execution history.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use mongodb::{Client, Database};
use tokio_util::sync::CancellationToken;

use crate::connection::ConnectionManager;
use crate::error::Result;
use crate::repl::SharedState;

/// Execution context that maintains state across commands
#[derive(Clone)]
pub struct ExecutionContext {
    /// Connection manager
    connection: Arc<RwLock<ConnectionManager>>,

    /// Shared state with REPL
    pub(crate) shared_state: SharedState,

    /// Configuration file path
    pub(crate) config_path: Option<PathBuf>,

    /// Client ID for this mongosh instance (used for killOp comment tagging)
    client_id: Arc<String>,

    /// Cancellation token for Ctrl+C handling
    cancel_token: CancellationToken,
}

impl ExecutionContext {
    /// Create a new execution context
    ///
    /// # Arguments
    /// * `connection` - Connection manager
    /// * `shared_state` - Shared state with REPL
    ///
    /// # Returns
    /// * `Self` - New execution context
    pub fn new(connection: ConnectionManager, shared_state: SharedState) -> Self {
        Self::with_config_path(connection, shared_state, None)
    }

    /// Create a new execution context with config path
    ///
    /// # Arguments
    /// * `connection` - Connection manager
    /// * `shared_state` - Shared state with REPL
    /// * `config_path` - Path to configuration file
    ///
    /// # Returns
    /// * `Self` - New execution context
    pub fn with_config_path(
        connection: ConnectionManager,
        shared_state: SharedState,
        config_path: Option<PathBuf>,
    ) -> Self {
        // Generate a unique client ID for this session
        let client_id = Self::generate_client_id();

        Self {
            connection: Arc::new(RwLock::new(connection)),
            shared_state,
            config_path,
            client_id: Arc::new(client_id),
            cancel_token: CancellationToken::new(),
        }
    }

    /// Generate a unique client ID for this mongosh instance
    ///
    /// Format: hostname-pid-timestamp
    fn generate_client_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());

        let pid = std::process::id();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        format!("{}-{}-{}", hostname, pid, timestamp)
    }

    /// Get current database name
    ///
    /// # Returns
    /// * `String` - Current database name
    pub async fn get_current_database(&self) -> String {
        self.shared_state.get_database()
    }

    /// Set current database name
    ///
    /// # Arguments
    /// * `database` - New database name
    pub async fn set_current_database(&self, database: String) {
        // Clone to get mutable access
        let mut state = self.shared_state.clone();
        state.set_database(database);
    }

    /// Get database handle
    ///
    /// # Returns
    /// * `Result<Database>` - Database handle or error
    pub async fn get_database(&self) -> Result<Database> {
        let conn = self.connection.read().await;
        let db_name = self.shared_state.get_database();
        conn.get_database(&db_name)
    }

    /// Get client handle
    ///
    /// # Returns
    /// * `Result<Client>` - Client reference
    pub async fn get_client(&self) -> Result<Client> {
        let conn = self.connection.read().await;
        Ok(conn.get_client()?.clone())
    }

    /// Get the client ID for this mongosh instance
    ///
    /// # Returns
    /// * `&str` - Client ID string
    pub fn get_client_id(&self) -> &str {
        &self.client_id
    }

    /// Get the cancellation token for this context
    ///
    /// # Returns
    /// * `CancellationToken` - Token that can be used to cancel operations
    pub fn get_cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Reset the cancellation token (after a cancellation, for the next command)
    ///
    /// This creates a fresh token so subsequent commands aren't pre-cancelled
    pub fn reset_cancel_token(&mut self) {
        self.cancel_token = CancellationToken::new();
    }
}
