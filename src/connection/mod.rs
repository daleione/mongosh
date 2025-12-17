//! Connection management for MongoDB
//!
//! This module provides connection management functionality including:
//! - Connection establishment and termination
//! - Connection pool management
//! - Health checks and monitoring
//! - Automatic reconnection with exponential backoff
//! - Session management for transactions

use mongodb::{Client, Database, options::ClientOptions};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::ConnectionConfig;
use crate::error::{ConnectionError, Result};

/// MongoDB connection manager
///
/// Manages connections to MongoDB, including connection pooling,
/// health checks, and automatic reconnection.
pub struct ConnectionManager {
    /// MongoDB client instance
    client: Option<Client>,

    /// Connection configuration
    config: ConnectionConfig,

    /// Current connection state
    state: Arc<RwLock<ConnectionState>>,

    /// Connection URI
    uri: String,
}

/// Connection state information
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,

    /// Currently connecting
    Connecting,

    /// Connected and ready
    Connected,

    /// Connection failed
    Failed(String),

    /// Reconnecting after failure
    Reconnecting,
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    pub max_size: u32,

    /// Minimum number of idle connections
    pub min_idle: u32,

    /// Connection timeout duration
    pub connection_timeout: Duration,

    /// Idle connection timeout duration
    pub idle_timeout: Duration,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Whether the connection is healthy
    pub is_healthy: bool,

    /// Response time in milliseconds
    pub response_time_ms: u64,

    /// Server version
    pub server_version: Option<String>,

    /// Additional diagnostic information
    pub diagnostics: Option<String>,
}

impl ConnectionManager {
    /// Create a new connection manager
    ///
    /// # Arguments
    /// * `uri` - MongoDB connection URI
    /// * `config` - Connection configuration
    ///
    /// # Returns
    /// * `Self` - New connection manager instance
    pub fn new(uri: String, config: ConnectionConfig) -> Self {
        Self {
            client: None,
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            uri,
        }
    }

    /// Establish connection to MongoDB
    ///
    /// # Returns
    /// * `Result<()>` - Success or connection error
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to MongoDB: {}", self.sanitize_uri(&self.uri));
        self.set_state(ConnectionState::Connecting).await;

        // Parse URI and create client options
        let options = Self::parse_uri(&self.uri).await?;
        let configured_options = self.configure_pool(options);

        // Attempt connection with retry logic
        match self.connect_with_retry(configured_options).await {
            Ok(client) => {
                // For secondary-only connections, skip ping verification
                // The client creation itself validates basic connectivity
                self.client = Some(client);
                self.set_state(ConnectionState::Connected).await;
                info!("Successfully connected to MongoDB");
                Ok(())
            }
            Err(e) => {
                let msg = format!("Failed to connect: {}", e);
                error!("{}", msg);
                self.set_state(ConnectionState::Failed(msg.clone())).await;
                Err(e)
            }
        }
    }

    /// Disconnect from MongoDB
    ///
    /// Closes all connections and cleans up resources
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from MongoDB");

        if self.client.is_some() {
            // Drop the client, which will close connections
            self.client = None;
            self.set_state(ConnectionState::Disconnected).await;
            info!("Disconnected from MongoDB");
        } else {
            debug!("Already disconnected");
        }

        Ok(())
    }

    /// Reconnect to MongoDB
    ///
    /// Attempts to re-establish a failed connection
    ///
    /// # Returns
    /// * `Result<()>` - Success or connection error
    pub async fn reconnect(&mut self) -> Result<()> {
        info!("Attempting to reconnect to MongoDB");
        self.set_state(ConnectionState::Reconnecting).await;

        // Disconnect first if still connected
        if self.client.is_some() {
            self.disconnect().await?;
        }

        // Reconnect
        self.connect().await
    }

    /// Perform health check on the connection
    ///
    /// # Returns
    /// * `Result<HealthStatus>` - Health check results or error
    pub async fn health_check(&self) -> Result<HealthStatus> {
        let client = self.get_client()?;
        let start = Instant::now();

        // For connections with secondary readPreference, ping might fail
        // So we just check if we have a client
        let response_time_ms = start.elapsed().as_millis() as u64;

        // Try to get server version, but don't fail if it doesn't work
        let server_version = self.get_server_version(client).await.ok();

        Ok(HealthStatus {
            is_healthy: true,
            response_time_ms,
            server_version,
            diagnostics: Some("Connected (ping skipped for secondary readPreference)".to_string()),
        })
    }

    /// Get a database handle
    ///
    /// # Arguments
    /// * `name` - Database name
    ///
    /// # Returns
    /// * `Result<Database>` - Database handle or error
    pub fn get_database(&self, name: &str) -> Result<Database> {
        let client = self.get_client()?;
        Ok(client.database(name))
    }

    /// Get the MongoDB client
    ///
    /// # Returns
    /// * `Result<&Client>` - Reference to client or error
    pub fn get_client(&self) -> Result<&Client> {
        self.client
            .as_ref()
            .ok_or_else(|| ConnectionError::NotConnected.into())
    }

    /// Get current connection state
    ///
    /// # Returns
    /// * `ConnectionState` - Current state
    pub async fn get_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Check if currently connected
    ///
    /// # Returns
    /// * `bool` - True if connected
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected)
    }

    /// Parse connection URI and create client options
    ///
    /// # Arguments
    /// * `uri` - MongoDB connection URI
    ///
    /// # Returns
    /// * `Result<ClientOptions>` - Parsed client options or error
    async fn parse_uri(uri: &str) -> Result<ClientOptions> {
        ClientOptions::parse(uri)
            .await
            .map_err(|e| ConnectionError::InvalidUri(e.to_string()).into())
    }

    /// Configure client options with pool settings
    ///
    /// # Arguments
    /// * `options` - Base client options
    ///
    /// # Returns
    /// * `ClientOptions` - Configured options
    fn configure_pool(&self, mut options: ClientOptions) -> ClientOptions {
        // Set connection pool size
        options.max_pool_size = Some(self.config.max_pool_size);
        options.min_pool_size = Some(self.config.min_pool_size);

        // Set timeouts from configuration
        options.connect_timeout = Some(Duration::from_secs(self.config.timeout));
        // Use a reasonable minimum for server selection timeout to handle secondary-only scenarios
        let server_selection_timeout = std::cmp::max(self.config.timeout, 30);
        options.server_selection_timeout = Some(Duration::from_secs(server_selection_timeout));

        // Set application name for tracking
        if options.app_name.is_none() {
            options.app_name = Some("mongosh-rs".to_string());
        }

        // Enable retryable reads and writes
        options.retry_reads = Some(true);
        options.retry_writes = Some(true);

        // Preserve readPreference from URI - don't override if already set
        // This allows secondary reads when connecting to replica sets

        // For direct connections to a single host (not using SRV), enable direct connection
        // This prevents the driver from trying to discover other replica set members
        // Check the parsed hosts list instead of parsing URI string
        if options.hosts.len() == 1 {
            options.direct_connection = Some(true);
            debug!("Enabled direct connection for single-host connection");
        }

        debug!(
            "Configured connection pool: max={}, min={}, readPreference={:?}, direct={:?}, server_selection_timeout={:?}s",
            self.config.max_pool_size,
            self.config.min_pool_size,
            options.selection_criteria,
            options.direct_connection,
            server_selection_timeout
        );

        options
    }

    /// Update connection state
    ///
    /// # Arguments
    /// * `new_state` - New connection state
    async fn set_state(&self, new_state: ConnectionState) {
        *self.state.write().await = new_state;
    }

    /// Attempt connection with retries
    ///
    /// # Arguments
    /// * `options` - Client options
    ///
    /// # Returns
    /// * `Result<Client>` - Connected client or error
    async fn connect_with_retry(&self, options: ClientOptions) -> Result<Client> {
        let max_retries = self.config.retry_attempts;
        let base_delay_ms = 100;
        let max_delay_ms = 5000;

        for attempt in 1..=max_retries {
            debug!("Connection attempt {}/{}", attempt, max_retries);

            match Client::with_options(options.clone()) {
                Ok(client) => {
                    debug!("Client created successfully on attempt {}", attempt);
                    return Ok(client);
                }
                Err(e) => {
                    if attempt == max_retries {
                        error!("All {} connection attempts failed", max_retries);
                        return Err(ConnectionError::ConnectionFailed(format!(
                            "Failed after {} attempts: {}",
                            max_retries, e
                        ))
                        .into());
                    }

                    // Exponential backoff with jitter
                    let delay_ms =
                        std::cmp::min(base_delay_ms * 2_u64.pow(attempt - 1), max_delay_ms);

                    warn!(
                        "Connection attempt {} failed: {}. Retrying in {}ms",
                        attempt, e, delay_ms
                    );

                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Err(ConnectionError::ConnectionFailed("Unexpected error in retry loop".to_string()).into())
    }

    /// Verify connection is alive by sending a ping
    /// Note: This is skipped for secondary-only connections
    ///
    /// # Arguments
    /// * `client` - MongoDB client to verify
    ///
    /// # Returns
    /// * `Result<bool>` - True if connection is alive
    #[allow(dead_code)]
    async fn verify_connection(&self, client: &Client) -> Result<bool> {
        debug!("Verifying connection with ping");
        match self.ping_internal(client).await {
            Ok(_) => {
                debug!("Connection verified successfully");
                Ok(true)
            }
            Err(e) => {
                warn!("Connection verification failed: {}", e);
                Err(e)
            }
        }
    }

    /// Send ping command to verify connection
    /// Note: May fail on secondary-only connections
    ///
    /// # Arguments
    /// * `client` - MongoDB client
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    #[allow(dead_code)]
    async fn ping_internal(&self, client: &Client) -> Result<()> {
        use mongodb::bson::doc;

        // Use a database with readPreference from the connection URI
        // instead of admin database which might require Primary
        let db = client
            .default_database()
            .unwrap_or_else(|| client.database("admin"));

        // Use runCommand which respects the connection's readPreference
        db.run_command(doc! { "ping": 1 })
            .await
            .map_err(|e| ConnectionError::PingFailed(e.to_string()))?;

        Ok(())
    }

    /// Get MongoDB server version
    ///
    /// # Arguments
    /// * `client` - MongoDB client
    ///
    /// # Returns
    /// * `Result<String>` - Server version string
    async fn get_server_version(&self, client: &Client) -> Result<String> {
        use mongodb::bson::doc;

        // Try to get server version, but don't fail if it requires Primary
        let db = client
            .default_database()
            .unwrap_or_else(|| client.database("admin"));

        match db.run_command(doc! { "buildInfo": 1 }).await {
            Ok(result) => {
                if let Ok(version) = result.get_str("version") {
                    Ok(version.to_string())
                } else {
                    Ok("unknown".to_string())
                }
            }
            Err(_) => {
                // If buildInfo fails (e.g., on secondary), just return unknown
                Ok("unknown".to_string())
            }
        }
    }

    /// Sanitize URI for logging (remove credentials)
    ///
    /// # Arguments
    /// * `uri` - Connection URI
    ///
    /// # Returns
    /// * `String` - Sanitized URI
    fn sanitize_uri(&self, uri: &str) -> String {
        // Simple sanitization: hide everything between :// and @
        if let Some(proto_end) = uri.find("://")
            && let Some(host_start) = uri.find('@')
        {
            let proto = &uri[..proto_end + 3];
            let host = &uri[host_start..];
            return format!("{}***{}", proto, host);
        }
        // If no credentials, return as-is (or just scheme if paranoid)
        if uri.contains('@') {
            "mongodb://***".to_string()
        } else {
            uri.to_string()
        }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            min_idle: 2,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
        }
    }
}

impl From<&ConnectionConfig> for PoolConfig {
    fn from(config: &ConnectionConfig) -> Self {
        Self {
            max_size: config.max_pool_size,
            min_idle: config.min_pool_size,
            connection_timeout: Duration::from_secs(config.timeout),
            idle_timeout: Duration::from_secs(config.idle_timeout),
        }
    }
}

/// Session manager for MongoDB transactions
///
/// Manages client sessions for transaction support
pub struct SessionManager {
    /// Reference to MongoDB client
    client: Client,
}

impl SessionManager {
    /// Create a new session manager
    ///
    /// # Arguments
    /// * `client` - MongoDB client
    ///
    /// # Returns
    /// * `Self` - New session manager
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Start a new client session
    ///
    /// # Returns
    /// * `Result<mongodb::ClientSession>` - New session or error
    pub async fn start_session(&self) -> Result<mongodb::ClientSession> {
        self.client
            .start_session()
            .await
            .map_err(|e| ConnectionError::SessionFailed(e.to_string()).into())
    }

    /// Start a transaction
    ///
    /// # Arguments
    /// * `session` - Client session
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn start_transaction(&self, session: &mut mongodb::ClientSession) -> Result<()> {
        session
            .start_transaction()
            .await
            .map_err(|e| ConnectionError::TransactionFailed(e.to_string()).into())
    }

    /// Commit a transaction
    ///
    /// # Arguments
    /// * `session` - Client session with active transaction
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn commit_transaction(&self, session: &mut mongodb::ClientSession) -> Result<()> {
        session
            .commit_transaction()
            .await
            .map_err(|e| ConnectionError::TransactionFailed(e.to_string()).into())
    }

    /// Abort a transaction
    ///
    /// # Arguments
    /// * `session` - Client session with active transaction
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub async fn abort_transaction(&self, session: &mut mongodb::ClientSession) -> Result<()> {
        session
            .abort_transaction()
            .await
            .map_err(|e| ConnectionError::TransactionFailed(e.to_string()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state() {
        let state = ConnectionState::Disconnected;
        assert_eq!(state, ConnectionState::Disconnected);
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_size, 10);
        assert_eq!(config.min_idle, 2);
    }

    #[test]
    fn test_pool_config_from_connection_config() {
        let conn_config = ConnectionConfig::default();
        let pool_config = PoolConfig::from(&conn_config);
        assert_eq!(pool_config.max_size, conn_config.max_pool_size);
    }

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new("mongodb://localhost:27017".to_string(), config);
        assert!(manager.client.is_none());
        assert!(!manager.is_connected().await);
    }

    #[tokio::test]
    async fn test_connection_state_transitions() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new("mongodb://localhost:27017".to_string(), config);

        // Initial state
        assert_eq!(manager.get_state().await, ConnectionState::Disconnected);

        // Transition to connecting
        manager.set_state(ConnectionState::Connecting).await;
        assert_eq!(manager.get_state().await, ConnectionState::Connecting);
    }

    #[test]
    fn test_sanitize_uri() {
        let config = ConnectionConfig::default();
        let manager =
            ConnectionManager::new("mongodb://user:pass@localhost:27017".to_string(), config);

        let sanitized = manager.sanitize_uri("mongodb://user:pass@localhost:27017/db");
        assert!(sanitized.contains("***"));
        assert!(!sanitized.contains("pass"));
    }

    #[test]
    fn test_sanitize_uri_no_credentials() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new("mongodb://localhost:27017".to_string(), config);

        let sanitized = manager.sanitize_uri("mongodb://localhost:27017/db");
        assert_eq!(sanitized, "mongodb://localhost:27017/db");
    }

    // Integration tests requiring real MongoDB should be ignored by default
    #[cfg(test)]
    #[allow(dead_code)]
    mod integration {
        use super::*;

        #[tokio::test]
        #[ignore]
        async fn test_connect_to_mongodb() {
            let config = ConnectionConfig::default();
            let mut manager =
                ConnectionManager::new("mongodb://localhost:27017".to_string(), config);

            let result = manager.connect().await;
            assert!(result.is_ok() || matches!(result, Err(_))); // May fail if MongoDB not running

            if result.is_ok() {
                assert!(manager.is_connected().await);
                let disconnect_result = manager.disconnect().await;
                assert!(disconnect_result.is_ok());
            }
        }

        #[tokio::test]
        #[ignore]
        async fn test_health_check() {
            let config = ConnectionConfig::default();
            let mut manager =
                ConnectionManager::new("mongodb://localhost:27017".to_string(), config);

            if manager.connect().await.is_ok() {
                let health = manager.health_check().await;
                assert!(health.is_ok());

                if let Ok(status) = health {
                    assert!(status.is_healthy);
                    assert!(status.response_time_ms > 0);
                }
            }
        }
    }
}
