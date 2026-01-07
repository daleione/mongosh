//! Execution context management
//!
//! This module provides the ExecutionContext which maintains state across
//! command executions, including database connections and execution history.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use mongodb::{Client, Database};

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
        Self {
            connection: Arc::new(RwLock::new(connection)),
            shared_state,
            config_path: None,
        }
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
        Self {
            connection: Arc::new(RwLock::new(connection)),
            shared_state,
            config_path,
        }
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
}
