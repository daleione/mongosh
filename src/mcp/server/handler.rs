//! MCP server handler implementation for MongoDB Shell

use bson::Document;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, service::RequestContext, tool, tool_handler,
    tool_router,
};
use std::sync::Arc;

use crate::connection::ConnectionManager;
use crate::executor::ExecutionContext;
use crate::mcp::security::{SecurityConfig, SecurityManager};
use crate::mcp::tools::UseDatabaseParams;
use crate::mcp::tools::*;
use crate::mcp::utils::*;
use crate::parser::{
    AdminCommand, AggregateOptions, Command, FindOptions, QueryCommand, UpdateOptions,
};
use crate::repl::SharedState;

/// MongoDB Shell MCP Server
///
/// This server exposes MongoDB operations as MCP tools that can be used
/// by AI models to interact with MongoDB databases.
#[derive(Clone)]
pub struct MongoShellServer {
    /// Execution context for MongoDB operations
    context: ExecutionContext,

    /// Tool router for MCP tools
    tool_router: ToolRouter<MongoShellServer>,

    /// Security manager for access control
    security: Arc<SecurityManager>,
}

#[tool_router]
impl MongoShellServer {
    /// Create a new MongoDB Shell MCP server
    ///
    /// # Arguments
    /// * `connection` - MongoDB connection manager
    /// * `state` - Shared REPL state
    /// * `security_config` - Security configuration
    pub fn new(
        connection: ConnectionManager,
        state: SharedState,
        security_config: SecurityConfig,
    ) -> Self {
        let context = ExecutionContext::new(connection, state);
        let security = Arc::new(SecurityManager::new(security_config));

        Self {
            context,
            tool_router: Self::tool_router(),
            security,
        }
    }

    /// Find documents in a collection
    ///
    /// # Arguments
    /// * `params` - Find operation parameters
    ///
    /// # Returns
    /// MCP tool result containing matching documents
    #[tool(
        description = "Find documents in a MongoDB collection with optional filtering, projection, sorting, and pagination"
    )]
    async fn mongo_find(
        &self,
        Parameters(params): Parameters<FindParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .validate_limit(Some(params.limit))
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log(
                "find",
                &params.database,
                &params.collection,
                &format!("limit={}", params.limit),
            )
            .await;

        // Convert parameters
        let filter = params
            .filter
            .as_ref()
            .map(json_to_bson_document)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?
            .unwrap_or_default();

        let options = FindOptions {
            limit: Some(params.limit),
            skip: params.skip.map(|s| s as u64),
            sort: params
                .sort
                .as_ref()
                .map(json_to_bson_document)
                .transpose()
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?,
            projection: params
                .projection
                .as_ref()
                .map(json_to_bson_document)
                .transpose()
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?,
            ..Default::default()
        };

        // Execute find command
        let command = Command::Query(QueryCommand::Find {
            collection: params.collection,
            filter,
            options,
        });

        let result =
            self.context.execute(command).await.map_err(|e| {
                McpError::internal_error(format!("Find operation failed: {}", e), None)
            })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Find a single document in a collection
    ///
    /// # Arguments
    /// * `params` - FindOne operation parameters
    ///
    /// # Returns
    /// MCP tool result containing the matching document
    #[tool(description = "Find a single document in a MongoDB collection")]
    async fn mongo_find_one(
        &self,
        Parameters(params): Parameters<FindOneParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("findOne", &params.database, &params.collection, "")
            .await;

        // Convert parameters
        let filter = params
            .filter
            .as_ref()
            .map(json_to_bson_document)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?
            .unwrap_or_default();

        let options = FindOptions {
            projection: params
                .projection
                .as_ref()
                .map(json_to_bson_document)
                .transpose()
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?,
            limit: Some(1),
            ..Default::default()
        };

        // Execute findOne command
        let command = Command::Query(QueryCommand::FindOne {
            collection: params.collection,
            filter,
            options,
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("FindOne operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Execute an aggregation pipeline
    ///
    /// # Arguments
    /// * `params` - Aggregation operation parameters
    ///
    /// # Returns
    /// MCP tool result containing aggregation results
    #[tool(description = "Execute an aggregation pipeline on a MongoDB collection")]
    async fn mongo_aggregate(
        &self,
        Parameters(params): Parameters<AggregateParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Convert pipeline
        let pipeline_docs: Result<Vec<Document>, String> =
            params.pipeline.iter().map(json_to_bson_document).collect();
        let pipeline = pipeline_docs.map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        self.security
            .validate_pipeline_stages(&pipeline)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log(
                "aggregate",
                &params.database,
                &params.collection,
                &format!("stages={}", pipeline.len()),
            )
            .await;

        // Execute aggregate command
        let command = Command::Query(QueryCommand::Aggregate {
            collection: params.collection,
            pipeline,
            options: AggregateOptions::default(),
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("Aggregate operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Count documents in a collection
    ///
    /// # Arguments
    /// * `params` - Count operation parameters
    ///
    /// # Returns
    /// MCP tool result containing document count
    #[tool(description = "Count documents in a MongoDB collection matching a filter")]
    async fn mongo_count(
        &self,
        Parameters(params): Parameters<CountParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("count", &params.database, &params.collection, "")
            .await;

        // Convert filter
        let filter = params
            .filter
            .as_ref()
            .map(json_to_bson_document)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?
            .unwrap_or_default();

        // Execute count command
        let command = Command::Query(QueryCommand::CountDocuments {
            collection: params.collection,
            filter,
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("Count operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Get distinct values for a field
    ///
    /// # Arguments
    /// * `params` - Distinct operation parameters
    ///
    /// # Returns
    /// MCP tool result containing distinct values
    #[tool(description = "Get distinct values for a field in a MongoDB collection")]
    async fn mongo_distinct(
        &self,
        Parameters(params): Parameters<DistinctParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log(
                "distinct",
                &params.database,
                &params.collection,
                &format!("field={}", params.field),
            )
            .await;

        // Convert filter
        let filter = params
            .filter
            .as_ref()
            .map(json_to_bson_document)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Note: Distinct is not directly available in Command enum, use aggregation
        let pipeline = if let Some(f) = filter {
            vec![
                bson::doc! { "$match": f },
                bson::doc! { "$group": { "_id": format!("${}", params.field) } },
            ]
        } else {
            vec![bson::doc! { "$group": { "_id": format!("${}", params.field) } }]
        };

        // Execute distinct via aggregation
        let command = Command::Query(QueryCommand::Aggregate {
            collection: params.collection,
            pipeline,
            options: AggregateOptions::default(),
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("Distinct operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Insert a single document
    ///
    /// # Arguments
    /// * `params` - InsertOne operation parameters
    ///
    /// # Returns
    /// MCP tool result containing inserted document ID
    #[tool(description = "Insert a single document into a MongoDB collection")]
    async fn mongo_insert_one(
        &self,
        Parameters(params): Parameters<InsertOneParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_write_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("insertOne", &params.database, &params.collection, "")
            .await;

        // Convert document
        let document = json_to_bson_document(&params.document)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Execute insertOne command
        let command = Command::Query(QueryCommand::InsertOne {
            collection: params.collection,
            document,
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("InsertOne operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Insert multiple documents
    ///
    /// # Arguments
    /// * `params` - InsertMany operation parameters
    ///
    /// # Returns
    /// MCP tool result containing inserted document IDs
    #[tool(description = "Insert multiple documents into a MongoDB collection")]
    async fn mongo_insert_many(
        &self,
        Parameters(params): Parameters<InsertManyParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_write_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log(
                "insertMany",
                &params.database,
                &params.collection,
                &format!("count={}", params.documents.len()),
            )
            .await;

        // Convert documents
        let documents: Result<Vec<Document>, String> =
            params.documents.iter().map(json_to_bson_document).collect();
        let documents = documents.map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Execute insertMany command
        let command = Command::Query(QueryCommand::InsertMany {
            collection: params.collection,
            documents,
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("InsertMany operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Update a single document
    ///
    /// # Arguments
    /// * `params` - UpdateOne operation parameters
    ///
    /// # Returns
    /// MCP tool result containing update statistics
    #[tool(description = "Update a single document in a MongoDB collection")]
    async fn mongo_update_one(
        &self,
        Parameters(params): Parameters<UpdateOneParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_write_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("updateOne", &params.database, &params.collection, "")
            .await;

        // Convert filter and update
        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let update = json_to_bson_document(&params.update)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Execute updateOne command
        let command = Command::Query(QueryCommand::UpdateOne {
            collection: params.collection,
            filter,
            update,
            options: UpdateOptions::default(),
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("UpdateOne operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Update multiple documents
    ///
    /// # Arguments
    /// * `params` - UpdateMany operation parameters
    ///
    /// # Returns
    /// MCP tool result containing update statistics
    #[tool(description = "Update multiple documents in a MongoDB collection")]
    async fn mongo_update_many(
        &self,
        Parameters(params): Parameters<UpdateManyParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_write_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("updateMany", &params.database, &params.collection, "")
            .await;

        // Convert filter and update
        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let update = json_to_bson_document(&params.update)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Execute updateMany command
        let command = Command::Query(QueryCommand::UpdateMany {
            collection: params.collection,
            filter,
            update,
            options: UpdateOptions::default(),
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("UpdateMany operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Delete a single document
    ///
    /// # Arguments
    /// * `params` - DeleteOne operation parameters
    ///
    /// # Returns
    /// MCP tool result containing delete statistics
    #[tool(description = "Delete a single document from a MongoDB collection")]
    async fn mongo_delete_one(
        &self,
        Parameters(params): Parameters<DeleteOneParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_delete_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("deleteOne", &params.database, &params.collection, "")
            .await;

        // Convert filter
        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Execute deleteOne command
        let command = Command::Query(QueryCommand::DeleteOne {
            collection: params.collection,
            filter,
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("DeleteOne operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Delete multiple documents
    ///
    /// # Arguments
    /// * `params` - DeleteMany operation parameters
    ///
    /// # Returns
    /// MCP tool result containing delete statistics
    #[tool(description = "Delete multiple documents from a MongoDB collection")]
    async fn mongo_delete_many(
        &self,
        Parameters(params): Parameters<DeleteManyParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_delete_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("deleteMany", &params.database, &params.collection, "")
            .await;

        // Convert filter
        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Execute deleteMany command
        let command = Command::Query(QueryCommand::DeleteMany {
            collection: params.collection,
            filter,
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("DeleteMany operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Switch the active database context for this session.
    ///
    /// Call this tool BEFORE any query/write/delete tool when you need to operate
    /// on a database that is different from the current one. After switching, all
    /// subsequent tools that accept a "database" parameter will still use whatever
    /// database name you pass explicitly — but the session default is updated so
    /// that context-aware tooling (e.g. listCollections) reflects the right DB.
    ///
    /// Typical workflow:
    ///   1. mongo_list_databases  →  see what databases exist
    ///   2. mongo_use_database    →  switch to the right one (e.g. "imagen")
    ///   3. mongo_list_collections / mongo_find / …  →  operate on that DB
    #[tool(
        description = "Switch the active database context. Call this first when you need to work \
        with a specific database (e.g. 'imagen', 'paysync_test'). \
        Use mongo_list_databases to discover available databases first."
    )]
    async fn mongo_use_database(
        &self,
        Parameters(params): Parameters<UseDatabaseParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security check: verify the database is in the allowed list
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("useDatabase", &params.database, "", "")
            .await;

        // Switch the session-level current database
        self.context
            .set_current_database(params.database.clone())
            .await;

        let output = serde_json::json!({
            "ok": 1,
            "currentDatabase": params.database,
            "message": format!("Switched to database '{}'", params.database)
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
        )]))
    }

    /// Return the name of the database that is currently active for this session.
    ///
    /// Use this to verify which database you are on before running queries,
    /// especially after calling mongo_use_database.
    #[tool(
        description = "Return the currently active database name for this session. \
        Useful to verify context before running queries."
    )]
    async fn mongo_get_current_database(&self) -> Result<CallToolResult, McpError> {
        let db = self.context.get_current_database().await;

        let output = serde_json::json!({
            "currentDatabase": db
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
        )]))
    }

    /// List all databases
    ///
    /// # Returns
    /// MCP tool result containing list of database names
    #[tool(description = "List all databases in the MongoDB instance")]
    async fn mongo_list_databases(&self) -> Result<CallToolResult, McpError> {
        // Security check
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security.audit_log("listDatabases", "", "", "").await;

        // Execute listDatabases command
        let command = Command::Admin(AdminCommand::ShowDatabases);

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("ListDatabases operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// List collections in a database
    ///
    /// # Arguments
    /// * `params` - ListCollections operation parameters
    ///
    /// # Returns
    /// MCP tool result containing list of collection names
    #[tool(description = "List all collections in a MongoDB database")]
    async fn mongo_list_collections(
        &self,
        Parameters(params): Parameters<ListCollectionsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("listCollections", &params.database, "", "")
            .await;

        // Execute showCollections command
        let command = Command::Admin(AdminCommand::ShowCollections);

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("ListCollections operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// List indexes on a collection
    ///
    /// # Arguments
    /// * `params` - ListIndexes operation parameters
    ///
    /// # Returns
    /// MCP tool result containing list of indexes
    #[tool(description = "List all indexes on a MongoDB collection")]
    async fn mongo_list_indexes(
        &self,
        Parameters(params): Parameters<ListIndexesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log("listIndexes", &params.database, &params.collection, "")
            .await;

        // Execute listIndexes command - use admin command
        let command = Command::Admin(AdminCommand::ListIndexes(params.collection));

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("ListIndexes operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Get collection statistics
    ///
    /// # Arguments
    /// * `params` - CollectionStats operation parameters
    ///
    /// # Returns
    /// MCP tool result containing collection statistics
    #[tool(description = "Get statistics for a MongoDB collection")]
    async fn mongo_collection_stats(
        &self,
        Parameters(params): Parameters<CollectionStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log(
                "collStats",
                &params.database,
                &params.collection,
                &format!("scale={}", params.scale),
            )
            .await;

        // Execute collStats command
        let command = Command::Admin(AdminCommand::CollectionStats {
            collection: params.collection,
            scale: Some(params.scale as i32),
        });

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("CollStats operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Explain query execution plan
    ///
    /// # Arguments
    /// * `params` - Explain operation parameters
    ///
    /// # Returns
    /// MCP tool result containing query execution plan
    #[tool(description = "Explain the execution plan for a MongoDB query")]
    async fn mongo_explain(
        &self,
        Parameters(params): Parameters<ExplainParams>,
    ) -> Result<CallToolResult, McpError> {
        // Security checks
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&params.database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(&params.database, &params.collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Audit log
        self.security
            .audit_log(
                "explain",
                &params.database,
                &params.collection,
                &format!("verbosity={}", params.verbosity),
            )
            .await;

        // Convert filter
        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Execute explain command using pipe
        let find_cmd = Command::Query(QueryCommand::Find {
            collection: params.collection,
            filter,
            options: FindOptions::default(),
        });

        let command = Command::Pipe(Box::new(find_cmd), crate::parser::PipeCommand::Explain);

        let result = self.context.execute(command).await.map_err(|e| {
            McpError::internal_error(format!("Explain operation failed: {}", e), None)
        })?;

        Ok(execution_result_to_mcp_tool_result(result))
    }
}

#[tool_handler]
impl ServerHandler for MongoShellServer {
    /// Get server information and capabilities
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::new(
            "mongosh-mcp",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_instructions(
            "This server provides MongoDB operations through MCP tools.\n\
            \n\
            ── DATABASE CONTEXT ──────────────────────────────────────────────────────\n\
            This server may be connected to a MongoDB instance that hosts MULTIPLE\n\
            databases (e.g. 'imagen', 'paysync_test', 'analytics', …).\n\
            \n\
            MANDATORY workflow when the target database is not obvious:\n\
            1. Call mongo_list_databases   → discover all available databases.\n\
            2. Call mongo_use_database     → switch to the correct database\n\
               (e.g. image-related queries → 'imagen',\n\
                payment queries           → 'paysync_test').\n\
            3. Call mongo_get_current_database  → verify you are on the right DB.\n\
            4. Call mongo_list_collections → inspect the schema before querying.\n\
            5. Run your query / mutation.\n\
            \n\
            Every query/write/delete tool also accepts an explicit \"database\" field.\n\
            Always fill it in — do NOT leave it empty or assume a default.\n\
            \n\
            ── BSON TYPE HANDLING ────────────────────────────────────────────────────\n\
            Results use MongoDB Extended JSON v2 (Relaxed) format to preserve types.\n\
            - ObjectId  → {\"$oid\": \"69297ddcb4c39276cb39b05b\"}\n\
            - DateTime  → {\"$date\": \"2025-11-28T10:47:07.965Z\"}\n\
            - Decimal128→ {\"$numberDecimal\": \"3.14\"}\n\
            - Numbers remain plain JSON numbers (relaxed mode).\n\
            \n\
            When constructing filters/documents use the SAME wrappers.\n\
            Plain strings will NOT match ObjectId or DateTime fields.\n\
            \n\
            Filter examples:\n\
            - ObjectId exact:    {\"_id\": {\"$oid\": \"69297ddcb4c39276cb39b05b\"}}\n\
            - ObjectId $in:      {\"_id\": {\"$in\": [{\"$oid\": \"...\"}, {\"$oid\": \"...\"}]}}\n\
            - DateTime range:    {\"create_time\": {\"$gte\": {\"$date\": \"2025-01-01T00:00:00Z\"}}}\n\
            - Date only:         {\"create_time\": {\"$gte\": {\"$date\": \"2025-01-01\"}}}\n\
            - Epoch ms:          {\"create_time\": {\"$gte\": {\"$date\": 1735689600000}}}\n\
            \n\
            Insert/update examples:\n\
            - DateTime field:    {\"created_at\": {\"$date\": \"2025-01-01T00:00:00Z\"}}\n\
            - ObjectId ref:      {\"group_id\": {\"$oid\": \"69297ddcb4c39276cb39b05b\"}}\n\
            \n\
            ── AVAILABLE TOOLS ───────────────────────────────────────────────────────\n\
            Context : useDatabase, getCurrentDatabase\n\
            Read    : find, findOne, aggregate, count, distinct\n\
            Write   : insertOne, insertMany, updateOne, updateMany\n\
            Delete  : deleteOne, deleteMany\n\
            Admin   : listDatabases, listCollections, listIndexes,\n\
                      collectionStats, explain\n\
            \n\
            All operations are subject to security policies configured on the server."
                .to_string(),
        )
    }

    /// List available resources (not implemented for MongoDB)
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
            meta: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_server_info() {
        let config = Config::default();
        let connection = ConnectionManager::new(
            "mongodb://localhost:27017".to_string(),
            config.connection.clone(),
        );
        let state = SharedState::new("test".to_string());
        let security = SecurityConfig::default();

        let server = MongoShellServer::new(connection, state, security);
        let info = server.get_info();

        assert!(info.capabilities.tools.is_some());
        assert_eq!(info.server_info.name, "mongosh-mcp");
    }
}
