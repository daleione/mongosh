//! Execution context management
//!
//! This module provides the ExecutionContext which maintains state across
//! command executions, including database connections and execution history.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use mongodb::{Client, Database};
use tokio_util::sync::CancellationToken;

use crate::config::ConnectionConfig;
use crate::connection::ConnectionManager;
use crate::error::{MongoshError, Result};
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

    /// Connection configuration (holds all named datasources)
    connection_config: Arc<ConnectionConfig>,

    /// Name of the currently active datasource
    current_datasource: Arc<RwLock<String>>,

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
        Self::with_full_config(
            connection,
            shared_state,
            config_path,
            ConnectionConfig::default(),
            String::new(),
        )
    }

    /// Create a new execution context with full configuration (including all datasources).
    ///
    /// This is the constructor used by the MCP server so that `switch_datasource`
    /// can look up URIs from the named datasource map at runtime.
    ///
    /// # Arguments
    /// * `connection`          - Already-initialised connection manager
    /// * `shared_state`        - Shared REPL state
    /// * `config_path`         - Optional path to the config file on disk
    /// * `connection_config`   - Full connection configuration (contains all datasources)
    /// * `initial_datasource`  - Name of the datasource that `connection` was built from
    pub fn with_full_config(
        connection: ConnectionManager,
        shared_state: SharedState,
        config_path: Option<PathBuf>,
        connection_config: ConnectionConfig,
        initial_datasource: String,
    ) -> Self {
        // Generate a unique client ID for this session
        let client_id = Self::generate_client_id();

        Self {
            connection: Arc::new(RwLock::new(connection)),
            shared_state,
            config_path,
            connection_config: Arc::new(connection_config),
            current_datasource: Arc::new(RwLock::new(initial_datasource)),
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
        // SharedState holds Arc<RwLock<String>> internally, so we can write
        // through a shared reference without cloning.
        self.shared_state.set_database(database);
    }

    /// Switch to a different named datasource.
    ///
    /// Looks up the URI for `datasource_name` in the stored `ConnectionConfig`,
    /// builds a fresh `ConnectionManager`, connects it, and replaces the current
    /// connection in-place (via the shared `Arc<RwLock<ConnectionManager>>`).
    ///
    /// The current-database is updated to whatever database the new datasource's
    /// URI embeds, falling back to `"test"` if none is specified.
    ///
    /// # Arguments
    /// * `datasource_name` - Name of the datasource as defined in the config file
    ///
    /// # Returns
    /// * `Ok(String)` - The database name that was activated
    /// * `Err(...)` - Datasource not found, or connection failed
    pub async fn switch_datasource(&self, datasource_name: &str) -> Result<String> {
        // Look up the URI for this datasource name.
        let uri = self
            .connection_config
            .get_datasource(Some(datasource_name))
            .ok_or_else(|| {
                let available = self.connection_config.list_datasources().join(", ");
                MongoshError::Generic(format!(
                    "Datasource '{}' not found. Available: [{}]",
                    datasource_name, available
                ))
            })?;

        // Extract the database name embedded in the URI (e.g. .../mydb).
        let db_name = extract_db_from_uri(&uri).unwrap_or_else(|| "test".to_string());

        // Build and connect a fresh ConnectionManager.
        let mut new_conn = ConnectionManager::new(uri, (*self.connection_config).clone());
        new_conn.connect().await?;

        // Swap in the new connection atomically.
        {
            let mut conn = self.connection.write().await;
            *conn = new_conn;
        }

        // Update session state.
        self.shared_state.set_database(db_name.clone());
        *self.current_datasource.write().await = datasource_name.to_string();

        Ok(db_name)
    }

    /// Return the name of the currently active datasource.
    pub async fn get_current_datasource(&self) -> String {
        self.current_datasource.read().await.clone()
    }

    /// Return all datasource names defined in the configuration, sorted.
    pub fn list_datasources(&self) -> Vec<String> {
        self.connection_config.list_datasources()
    }

    /// Get database handle
    ///
    /// This method ensures the connection is healthy before returning a database handle.
    /// If the connection is stale or broken, it will attempt to reconnect.
    ///
    /// # Returns
    /// * `Result<Database>` - Database handle or error
    pub async fn get_database(&self) -> Result<Database> {
        // Ensure connection is alive before getting database
        self.ensure_connected().await?;

        let conn = self.connection.read().await;
        let db_name = self.shared_state.get_database();
        conn.get_database(&db_name)
    }

    /// Get client handle
    ///
    /// This method ensures the connection is healthy before returning a client handle.
    /// If the connection is stale or broken, it will attempt to reconnect.
    ///
    /// # Returns
    /// * `Result<Client>` - Client reference
    pub async fn get_client(&self) -> Result<Client> {
        // Ensure connection is alive before getting client
        self.ensure_connected().await?;

        let conn = self.connection.read().await;
        Ok(conn.get_client()?.clone())
    }

    /// Ensure connection is alive, reconnect if necessary
    ///
    /// This internal method checks if the connection is healthy and
    /// reconnects if it's stale or broken. This prevents "Broken pipe"
    /// errors when the connection has been idle for too long.
    ///
    /// # Returns
    /// * `Result<()>` - Success or reconnection error
    async fn ensure_connected(&self) -> Result<()> {
        let mut conn = self.connection.write().await;
        conn.ensure_connected().await
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

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Extract the database name from a MongoDB URI path component.
///
/// `mongodb://host:27017/mydb?opts` → `Some("mydb")`
/// `mongodb://host:27017/`         → `None`
/// `mongodb://host:27017`          → `None`
fn extract_db_from_uri(uri: &str) -> Option<String> {
    // Strip scheme (mongodb:// or mongodb+srv://)
    let rest = uri
        .strip_prefix("mongodb+srv://")
        .or_else(|| uri.strip_prefix("mongodb://"))?;

    // Drop everything after '?' (query string)
    let without_query = rest.split('?').next()?;

    // The path starts after the first '/' that follows the host[:port] part.
    // Structure: [user:pass@]host[:port][/db]
    let path_start = without_query.find('/')?;
    let db = &without_query[path_start + 1..];

    if db.is_empty() {
        None
    } else {
        Some(db.to_string())
    }
}
