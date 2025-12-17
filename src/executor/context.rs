//! Execution context management
//!
//! This module provides the ExecutionContext which maintains state across
//! command executions, including database connections and execution history.

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

    /// Command execution history
    history: Arc<RwLock<Vec<String>>>,
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
            history: Arc::new(RwLock::new(Vec::new())),
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

    /// Add command to execution history
    ///
    /// # Arguments
    /// * `command` - Command string to add
    pub async fn add_to_history(&self, command: String) {
        let mut history = self.history.write().await;
        history.push(command);
    }

    /// Get execution history
    ///
    /// # Returns
    /// * `Vec<String>` - List of executed commands
    pub async fn get_history(&self) -> Vec<String> {
        let history = self.history.read().await;
        history.clone()
    }

    /// Clear execution history
    pub async fn clear_history(&self) {
        let mut history = self.history.write().await;
        history.clear();
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_history_management() {
        // This is a placeholder test - would need proper setup with ConnectionManager
        // and SharedState to fully test
    }
}
