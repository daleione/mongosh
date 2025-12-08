//! Command execution engine for mongosh
//!
//! This module provides the execution layer that processes parsed commands
//! and performs the corresponding MongoDB operations. It includes:
//! - Execution context for managing state
//! - Query executor for CRUD operations
//! - Admin executor for administrative commands
//! - Command router for dispatching commands
//! - Result collection and formatting

use mongodb::bson::Document;
use mongodb::{Client, Collection, Database};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::connection::ConnectionManager;
use crate::error::{ExecutionError, MongoshError, Result};
use crate::parser::{AdminCommand, Command, FindOptions, QueryCommand, UtilityCommand};

/// Execution context that maintains state across commands
pub struct ExecutionContext {
    /// Connection manager
    connection: Arc<RwLock<ConnectionManager>>,

    /// Current database name
    current_database: Arc<RwLock<String>>,
}

impl ExecutionContext {
    /// Create a new execution context
    ///
    /// # Arguments
    /// * `connection` - Connection manager
    /// * `default_database` - Default database name
    ///
    /// # Returns
    /// * `Self` - New execution context
    pub fn new(connection: ConnectionManager, default_database: String) -> Self {
        Self {
            connection: Arc::new(RwLock::new(connection)),
            current_database: Arc::new(RwLock::new(default_database)),
        }
    }

    /// Get current database name
    ///
    /// # Returns
    /// * `String` - Current database name
    pub async fn get_current_database(&self) -> String {
        self.current_database.read().await.clone()
    }

    /// Set current database
    ///
    /// # Arguments
    /// * `name` - Database name
    pub async fn set_current_database(&self, name: String) {
        *self.current_database.write().await = name;
    }

    /// Get database handle
    ///
    /// # Returns
    /// * `Result<Database>` - Database handle or error
    pub async fn get_database(&self) -> Result<Database> {
        let conn = self.connection.read().await;
        let db_name = self.current_database.read().await;
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

    /// Execute a command
    ///
    /// # Arguments
    /// * `command` - Parsed command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, command: Command) -> Result<ExecutionResult> {
        let router = CommandRouter::new(self.clone()).await?;
        router.route(command).await
    }
}

impl Clone for ExecutionContext {
    fn clone(&self) -> Self {
        Self {
            connection: Arc::clone(&self.connection),
            current_database: Arc::clone(&self.current_database),
        }
    }
}

/// Result of command execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Success status
    pub success: bool,

    /// Result data (documents, stats, etc.)
    pub data: ResultData,

    /// Execution statistics
    pub stats: ExecutionStats,

    /// Error message if failed
    pub error: Option<String>,
}

/// Data returned from command execution
#[derive(Debug, Clone)]
pub enum ResultData {
    /// List of documents
    Documents(Vec<Document>),

    /// Single document
    Document(Document),

    /// Insert one result
    InsertOne { inserted_id: String },

    /// Insert many result
    InsertMany { inserted_ids: Vec<String> },

    /// Update result
    Update { matched: u64, modified: u64 },

    /// Delete result
    Delete { deleted: u64 },

    /// Count result
    Count(u64),

    /// Text message
    Message(String),

    /// List of strings
    List(Vec<String>),

    /// No data
    None,
}

/// Execution statistics
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Number of documents returned
    pub documents_returned: usize,

    /// Number of documents affected
    pub documents_affected: Option<u64>,
}

impl Default for ExecutionStats {
    fn default() -> Self {
        Self {
            execution_time_ms: 0,
            documents_returned: 0,
            documents_affected: None,
        }
    }
}

/// Main command router that dispatches commands to appropriate executors
pub struct CommandRouter {
    /// Execution context
    context: ExecutionContext,
}

impl CommandRouter {
    /// Create a new command router
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Result<Self>` - New router or error
    pub async fn new(context: ExecutionContext) -> Result<Self> {
        Ok(Self { context })
    }

    /// Route command to appropriate executor
    ///
    /// # Arguments
    /// * `command` - Parsed command
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn route(&self, command: Command) -> Result<ExecutionResult> {
        debug!("Routing command: {:?}", command);

        let start = Instant::now();

        let result = match command {
            Command::Query(query_cmd) => {
                let executor = QueryExecutor::new(self.context.clone()).await?;
                executor.execute(query_cmd).await
            }
            Command::Admin(admin_cmd) => {
                let executor = AdminExecutor::new(self.context.clone()).await?;
                executor.execute(admin_cmd).await
            }
            Command::Utility(util_cmd) => {
                let executor = UtilityExecutor::new();
                executor.execute(util_cmd).await
            }
            Command::Help(topic) => self.execute_help(topic).await,
            Command::Exit => Ok(ExecutionResult {
                success: true,
                data: ResultData::Message("Exiting...".to_string()),
                stats: ExecutionStats::default(),
                error: None,
            }),
            _ => Err(MongoshError::NotImplemented(
                "Command type not yet implemented".to_string(),
            )),
        };

        let elapsed = start.elapsed().as_millis() as u64;
        debug!("Command executed in {}ms", elapsed);

        result
    }

    /// Execute help command
    ///
    /// # Arguments
    /// * `topic` - Optional help topic
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Help text
    async fn execute_help(&self, topic: Option<String>) -> Result<ExecutionResult> {
        let help_text = if let Some(t) = topic {
            format!("Help for: {}\n(Not yet implemented)", t)
        } else {
            r#"MongoDB Shell Commands:

Database Operations:
  db.collection.find(filter, projection)     - Find documents
  db.collection.insertOne(document)          - Insert one document
  db.collection.insertMany([documents])      - Insert multiple documents
  db.collection.updateOne(filter, update)    - Update one document
  db.collection.updateMany(filter, update)   - Update multiple documents
  db.collection.deleteOne(filter)            - Delete one document
  db.collection.deleteMany(filter)           - Delete multiple documents
  db.collection.count(filter)                - Count documents

Administrative:
  show dbs                                    - List databases
  show collections                            - List collections
  use <database>                              - Switch database

Utility:
  help                                        - Show this help
  help <command>                              - Show help for specific command
  exit / quit                                 - Exit shell
"#
            .to_string()
        };

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(help_text),
            stats: ExecutionStats::default(),
            error: None,
        })
    }
}

/// Executor for query operations (CRUD)
pub struct QueryExecutor {
    /// Execution context
    context: ExecutionContext,
}

impl QueryExecutor {
    /// Create a new query executor
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Result<Self>` - New executor or error
    pub async fn new(context: ExecutionContext) -> Result<Self> {
        Ok(Self { context })
    }

    /// Execute a query command
    ///
    /// # Arguments
    /// * `cmd` - Query command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, cmd: QueryCommand) -> Result<ExecutionResult> {
        let start = Instant::now();

        let result = match cmd {
            QueryCommand::Find {
                collection,
                filter,
                options,
            } => self.execute_find(collection, filter, options).await,

            QueryCommand::Count { collection, filter } => {
                self.execute_count(collection, filter).await
            }

            // Stub implementations for write operations (Phase 3)
            QueryCommand::InsertOne {
                collection: _,
                document: _,
            } => Err(MongoshError::NotImplemented(
                "insertOne not yet implemented (Phase 3)".to_string(),
            )),

            QueryCommand::InsertMany {
                collection: _,
                documents: _,
            } => Err(MongoshError::NotImplemented(
                "insertMany not yet implemented (Phase 3)".to_string(),
            )),

            QueryCommand::UpdateOne {
                collection: _,
                filter: _,
                update: _,
                options: _,
            } => Err(MongoshError::NotImplemented(
                "updateOne not yet implemented (Phase 3)".to_string(),
            )),

            QueryCommand::UpdateMany {
                collection: _,
                filter: _,
                update: _,
                options: _,
            } => Err(MongoshError::NotImplemented(
                "updateMany not yet implemented (Phase 3)".to_string(),
            )),

            QueryCommand::DeleteOne {
                collection: _,
                filter: _,
            } => Err(MongoshError::NotImplemented(
                "deleteOne not yet implemented (Phase 3)".to_string(),
            )),

            QueryCommand::DeleteMany {
                collection: _,
                filter: _,
            } => Err(MongoshError::NotImplemented(
                "deleteMany not yet implemented (Phase 3)".to_string(),
            )),

            QueryCommand::Aggregate {
                collection: _,
                pipeline: _,
                options: _,
            } => Err(MongoshError::NotImplemented(
                "aggregate not yet implemented (Phase 4)".to_string(),
            )),

            QueryCommand::FindAndModify {
                collection: _,
                query: _,
                update: _,
                remove: _,
                options: _,
            } => Err(MongoshError::NotImplemented(
                "findAndModify not yet implemented (Phase 4)".to_string(),
            )),
        };

        // Add execution time to result
        if let Ok(mut exec_result) = result {
            exec_result.stats.execution_time_ms = start.elapsed().as_millis() as u64;
            Ok(exec_result)
        } else {
            result
        }
    }

    /// Execute find operation
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    /// * `options` - Find options
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Documents or error
    async fn execute_find(
        &self,
        collection: String,
        filter: Document,
        options: FindOptions,
    ) -> Result<ExecutionResult> {
        info!(
            "Executing find on collection '{}' with filter: {:?}",
            collection, filter
        );

        // Get database and collection
        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        // Build MongoDB find options
        let mut find_opts = mongodb::options::FindOptions::default();

        if let Some(limit) = options.limit {
            find_opts.limit = Some(limit);
            debug!("Applied limit: {}", limit);
        }

        if let Some(skip) = options.skip {
            find_opts.skip = Some(skip);
            debug!("Applied skip: {}", skip);
        }

        if let Some(sort) = options.sort {
            find_opts.sort = Some(sort);
            debug!("Applied sort");
        }

        if let Some(projection) = options.projection {
            find_opts.projection = Some(projection);
            debug!("Applied projection");
        }

        if let Some(batch_size) = options.batch_size {
            find_opts.batch_size = Some(batch_size);
            debug!("Applied batch_size: {}", batch_size);
        }

        // Execute query
        let mut cursor = coll
            .find(filter.clone())
            .with_options(find_opts)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        // Collect results
        let mut documents = Vec::new();
        use futures::stream::TryStreamExt;

        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e| ExecutionError::CursorError(e.to_string()))?
        {
            documents.push(doc);
        }

        let count = documents.len();
        info!("Found {} documents", count);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Documents(documents),
            stats: ExecutionStats {
                execution_time_ms: 0, // Will be set by caller
                documents_returned: count,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Execute count operation
    ///
    /// # Arguments
    /// * `collection` - Collection name
    /// * `filter` - Query filter
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Count result or error
    async fn execute_count(
        &self,
        collection: String,
        filter: Option<Document>,
    ) -> Result<ExecutionResult> {
        info!("Executing count on collection '{}'", collection);

        let db = self.context.get_database().await?;
        let coll: Collection<Document> = db.collection(&collection);

        let count = if let Some(f) = filter {
            coll.count_documents(f)
                .await
                .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?
        } else {
            coll.estimated_document_count()
                .await
                .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?
        };

        info!("Count result: {}", count);

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Count(count),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: Some(count),
            },
            error: None,
        })
    }
}

/// Executor for administrative commands
pub struct AdminExecutor {
    /// Execution context
    context: ExecutionContext,
}

impl AdminExecutor {
    /// Create a new admin executor
    ///
    /// # Arguments
    /// * `context` - Execution context
    ///
    /// # Returns
    /// * `Result<Self>` - New executor or error
    pub async fn new(context: ExecutionContext) -> Result<Self> {
        Ok(Self { context })
    }

    /// Execute an administrative command
    ///
    /// # Arguments
    /// * `cmd` - Admin command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, cmd: AdminCommand) -> Result<ExecutionResult> {
        match cmd {
            AdminCommand::ShowDatabases => self.show_databases().await,
            AdminCommand::ShowCollections => self.show_collections().await,
            AdminCommand::UseDatabase(name) => self.use_database(name).await,
            _ => Err(MongoshError::NotImplemented(
                "Admin command not yet implemented".to_string(),
            )),
        }
    }

    /// Show all databases
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - List of database names
    async fn show_databases(&self) -> Result<ExecutionResult> {
        info!("Listing databases");

        let client = self.context.get_client().await?;

        let db_names = client
            .list_database_names()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!("Found {} databases", db_names.len());

        Ok(ExecutionResult {
            success: true,
            data: ResultData::List(db_names),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Show collections in current database
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - List of collection names
    async fn show_collections(&self) -> Result<ExecutionResult> {
        let db_name = self.context.get_current_database().await;
        info!("Listing collections in database '{}'", db_name);

        let db = self.context.get_database().await?;

        let collection_names = db
            .list_collection_names()
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;

        info!("Found {} collections", collection_names.len());

        Ok(ExecutionResult {
            success: true,
            data: ResultData::List(collection_names),
            stats: ExecutionStats {
                execution_time_ms: 0,
                documents_returned: 0,
                documents_affected: None,
            },
            error: None,
        })
    }

    /// Switch to a different database
    ///
    /// # Arguments
    /// * `name` - Database name
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Success message
    async fn use_database(&self, name: String) -> Result<ExecutionResult> {
        info!("Switching to database '{}'", name);

        self.context.set_current_database(name.clone()).await;

        Ok(ExecutionResult {
            success: true,
            data: ResultData::Message(format!("switched to db {}", name)),
            stats: ExecutionStats::default(),
            error: None,
        })
    }
}

/// Executor for utility commands
pub struct UtilityExecutor {}

impl UtilityExecutor {
    /// Create a new utility executor
    ///
    /// # Returns
    /// * `Self` - New executor
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a utility command
    ///
    /// # Arguments
    /// * `cmd` - Utility command to execute
    ///
    /// # Returns
    /// * `Result<ExecutionResult>` - Execution result or error
    pub async fn execute(&self, cmd: UtilityCommand) -> Result<ExecutionResult> {
        match cmd {
            UtilityCommand::Print(text) => Ok(ExecutionResult {
                success: true,
                data: ResultData::Message(text),
                stats: ExecutionStats::default(),
                error: None,
            }),
            _ => Err(MongoshError::NotImplemented(
                "Utility command not yet implemented".to_string(),
            )),
        }
    }
}

impl Default for UtilityExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_stats_default() {
        let stats = ExecutionStats::default();
        assert_eq!(stats.execution_time_ms, 0);
        assert_eq!(stats.documents_returned, 0);
        assert!(stats.documents_affected.is_none());
    }

    #[test]
    fn test_result_data_variants() {
        let data = ResultData::Message("test".to_string());
        match data {
            ResultData::Message(msg) => assert_eq!(msg, "test"),
            _ => panic!("Expected Message variant"),
        }
    }

    #[tokio::test]
    async fn test_execution_context_creation() {
        use crate::config::ConnectionConfig;
        let config = ConnectionConfig::default();
        let conn = ConnectionManager::new("mongodb://localhost:27017".to_string(), config);
        let context = ExecutionContext::new(conn, "test".to_string());

        assert_eq!(context.get_current_database().await, "test");
    }

    #[tokio::test]
    async fn test_execution_context_set_database() {
        use crate::config::ConnectionConfig;
        let config = ConnectionConfig::default();
        let conn = ConnectionManager::new("mongodb://localhost:27017".to_string(), config);
        let context = ExecutionContext::new(conn, "test".to_string());

        context.set_current_database("newdb".to_string()).await;
        assert_eq!(context.get_current_database().await, "newdb");
    }

    #[tokio::test]
    async fn test_utility_executor_print() {
        let executor = UtilityExecutor::new();
        let result = executor
            .execute(UtilityCommand::Print("Hello".to_string()))
            .await
            .unwrap();

        assert!(result.success);
        match result.data {
            ResultData::Message(msg) => assert_eq!(msg, "Hello"),
            _ => panic!("Expected Message result"),
        }
    }
}
