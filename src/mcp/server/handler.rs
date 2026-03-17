//! MCP server handler implementation for MongoDB Shell

use bson::Document;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, service::RequestContext, tool, tool_handler,
    tool_router,
};
use std::sync::Arc;

use crate::config::ConnectionConfig;
use crate::connection::ConnectionManager;
use crate::executor::ExecutionContext;
use crate::mcp::security::{SecurityConfig, SecurityManager};
use crate::mcp::tools::*;
use crate::mcp::utils::*;
use crate::parser::{
    AdminCommand, AggregateOptions, Command, FindOptions, QueryCommand, UpdateOptions,
};
use crate::repl::SharedState;

/// MongoDB Shell MCP Server
#[derive(Clone)]
pub struct MongoShellServer {
    context: ExecutionContext,
    tool_router: ToolRouter<MongoShellServer>,
    security: Arc<SecurityManager>,
}

#[tool_router]
impl MongoShellServer {
    /// Create a new MCP server with full datasource configuration.
    pub fn with_config(
        connection: ConnectionManager,
        state: SharedState,
        security_config: SecurityConfig,
        connection_config: ConnectionConfig,
        initial_datasource: String,
    ) -> Self {
        let context = ExecutionContext::with_full_config(
            connection,
            state,
            None,
            connection_config,
            initial_datasource,
        );
        Self {
            context,
            tool_router: Self::tool_router(),
            security: Arc::new(SecurityManager::new(security_config)),
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// If an explicit `datasource` is provided, switch to it first and derive
    /// the database from the URI (unless `database` overrides it).
    /// Then resolve the final database name: explicit param > active DB > error.
    ///
    /// This is the single entry-point for all tool methods so that every tool
    /// can optionally specify a datasource and skip a separate prepare step.
    async fn resolve_context(
        &self,
        datasource: Option<String>,
        database: Option<String>,
    ) -> Result<String, McpError> {
        if let Some(ds) = datasource {
            // Switch datasource; URI may embed a database name.
            let db_from_uri = self
                .context
                .switch_datasource(&ds)
                .await
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

            // Explicit database override wins over URI-embedded name.
            let effective_db = database.unwrap_or(db_from_uri);

            self.security
                .check_database_access(&effective_db)
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

            self.context
                .set_current_database(effective_db.clone())
                .await;
            return Ok(effective_db);
        }

        // No datasource switch — just resolve the database.
        match database {
            Some(db) if !db.is_empty() => {
                self.security
                    .check_database_access(&db)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                Ok(db)
            }
            _ => {
                let current = self.context.get_current_database().await;
                if current.is_empty() {
                    Err(McpError::invalid_params(
                        "No datasource or database specified and no active database is set. \
                         Provide a 'datasource' parameter or call mongo_prepare_context first."
                            .to_string(),
                        None,
                    ))
                } else {
                    Ok(current)
                }
            }
        }
    }

    /// Run security checks for read tools: permission + DB allowlist + collection denylist.
    fn check_read_access(&self, database: &str, collection: &str) -> Result<(), McpError> {
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(database, collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        Ok(())
    }

    /// Run security checks for write tools.
    fn check_write_access(&self, database: &str, collection: &str) -> Result<(), McpError> {
        self.security
            .check_write_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(database, collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        Ok(())
    }

    /// Run security checks for delete tools.
    fn check_delete_access(&self, database: &str, collection: &str) -> Result<(), McpError> {
        self.security
            .check_delete_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_collection_access(database, collection)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        Ok(())
    }

    /// Fetch collection names for the currently active database.
    async fn list_current_collections(&self) -> Vec<String> {
        match self.context.get_database().await {
            Ok(db) => db.list_collection_names().await.unwrap_or_default(),
            Err(_) => vec![],
        }
    }

    // ── Context / datasource tools ────────────────────────────────────────────

    /// Switch to a named datasource and return context info (database +
    /// collections).  Use this when the datasource is unknown or when you want
    /// to explore available collections before querying.
    ///
    /// If the user has already mentioned a datasource name (e.g. "shop_prod"),
    /// prefer passing `datasource` directly to the query tool instead of
    /// calling this first.
    #[tool(
        description = "Switch to a named datasource and return the active database and \
        its collection list. \
        Use ONLY when the datasource is unknown or you need to browse collections. \
        If the user already specified a datasource (e.g. 'shop_prod'), pass it directly \
        to the query tool via its 'datasource' field — no need to call this first. \
        Must provide a datasource name; do NOT call with an empty payload."
    )]
    async fn mongo_prepare_context(
        &self,
        Parameters(params): Parameters<PrepareContextParams>,
    ) -> Result<CallToolResult, McpError> {
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        self.security
            .audit_log("prepareContext", &params.datasource, "", "")
            .await;

        // Switch datasource; URI may embed a database name.
        let db_from_uri = self
            .context
            .switch_datasource(&params.datasource)
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        // Explicit database override wins over URI-embedded name.
        let effective_db = params.database.unwrap_or(db_from_uri);

        self.security
            .check_database_access(&effective_db)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        self.context
            .set_current_database(effective_db.clone())
            .await;

        let collections = self.list_current_collections().await;

        let output = serde_json::json!({
            "ok": 1,
            "datasource": params.datasource,
            "database": effective_db,
            "collections": collections,
            "collectionsCount": collections.len(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]))
    }

    /// List all named datasources defined in the config file.
    #[tool(
        description = "List all named datasources (connections) defined in the config file. \
        Use ONLY when you need to discover available datasource names. \
        Do NOT call this if the user has already mentioned a datasource name."
    )]
    async fn mongo_list_datasources(&self) -> Result<CallToolResult, McpError> {
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let datasources = self.context.list_datasources();
        let current = self.context.get_current_datasource().await;

        let output = serde_json::json!({
            "datasources": datasources,
            "count": datasources.len(),
            "currentDatasource": current,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]))
    }

    // ── Read tools ────────────────────────────────────────────────────────────

    /// Find documents in a collection.
    #[tool(description = "Find documents in a MongoDB collection. \
        Pass 'datasource' to target a specific connection (e.g. \"shop_prod\") — \
        the server switches automatically, no separate prepare step needed. \
        'database' and 'datasource' are both optional when context is already active.")]
    async fn mongo_find(
        &self,
        Parameters(params): Parameters<FindParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;
        self.security
            .validate_limit(Some(params.limit))
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .audit_log(
                "find",
                &database,
                &params.collection,
                &format!("limit={}", params.limit),
            )
            .await;

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

        let result = self
            .context
            .execute(Command::Query(QueryCommand::Find {
                collection: params.collection,
                filter,
                options,
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("find failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Find a single document in a collection.
    #[tool(description = "Find a single document in a MongoDB collection. \
        Pass 'datasource' to target a specific connection (e.g. \"shop_prod\") — \
        the server switches automatically, no separate prepare step needed. \
        'database' and 'datasource' are both optional when context is already active.")]
    async fn mongo_find_one(
        &self,
        Parameters(params): Parameters<FindOneParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;
        self.security
            .audit_log("findOne", &database, &params.collection, "")
            .await;

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

        let result = self
            .context
            .execute(Command::Query(QueryCommand::FindOne {
                collection: params.collection,
                filter,
                options,
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("findOne failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Execute an aggregation pipeline.
    #[tool(
        description = "Execute an aggregation pipeline on a MongoDB collection. \
        Pass 'datasource' to target a specific connection (e.g. \"shop_prod\") — \
        the server switches automatically, no separate prepare step needed. \
        'database' and 'datasource' are both optional when context is already active."
    )]
    async fn mongo_aggregate(
        &self,
        Parameters(params): Parameters<AggregateParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;

        let pipeline: Result<Vec<Document>, String> =
            params.pipeline.iter().map(json_to_bson_document).collect();
        let pipeline = pipeline.map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        self.security
            .validate_pipeline_stages(&pipeline)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .audit_log(
                "aggregate",
                &database,
                &params.collection,
                &format!("stages={}", pipeline.len()),
            )
            .await;

        let result = self
            .context
            .execute(Command::Query(QueryCommand::Aggregate {
                collection: params.collection,
                pipeline,
                options: AggregateOptions::default(),
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("aggregate failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Count documents matching a filter.
    #[tool(description = "Count documents in a MongoDB collection. \
        Pass 'datasource' to target a specific connection (e.g. \"shop_prod\") — \
        the server switches automatically, no separate prepare step needed. \
        'database' and 'datasource' are both optional when context is already active.")]
    async fn mongo_count(
        &self,
        Parameters(params): Parameters<CountParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;
        self.security
            .audit_log("count", &database, &params.collection, "")
            .await;

        let filter = params
            .filter
            .as_ref()
            .map(json_to_bson_document)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?
            .unwrap_or_default();

        let result = self
            .context
            .execute(Command::Query(QueryCommand::CountDocuments {
                collection: params.collection,
                filter,
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("count failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Get distinct values for a field.
    #[tool(
        description = "Get distinct values for a field in a MongoDB collection. \
        Pass 'datasource' to target a specific connection (e.g. \"shop_prod\") — \
        the server switches automatically, no separate prepare step needed. \
        'database' and 'datasource' are both optional when context is already active."
    )]
    async fn mongo_distinct(
        &self,
        Parameters(params): Parameters<DistinctParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;
        self.security
            .audit_log(
                "distinct",
                &database,
                &params.collection,
                &format!("field={}", params.field),
            )
            .await;

        let filter = params
            .filter
            .as_ref()
            .map(json_to_bson_document)
            .transpose()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let pipeline = match filter {
            Some(f) => vec![
                bson::doc! { "$match": f },
                bson::doc! { "$group": { "_id": format!("${}", params.field) } },
            ],
            None => vec![bson::doc! { "$group": { "_id": format!("${}", params.field) } }],
        };

        let result = self
            .context
            .execute(Command::Query(QueryCommand::Aggregate {
                collection: params.collection,
                pipeline,
                options: AggregateOptions::default(),
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("distinct failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    // ── Write tools ───────────────────────────────────────────────────────────

    /// Insert a single document.
    #[tool(description = "Insert a single document into a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed.")]
    async fn mongo_insert_one(
        &self,
        Parameters(params): Parameters<InsertOneParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_write_access(&database, &params.collection)?;
        self.security
            .audit_log("insertOne", &database, &params.collection, "")
            .await;

        let document = json_to_bson_document(&params.document)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let result = self
            .context
            .execute(Command::Query(QueryCommand::InsertOne {
                collection: params.collection,
                document,
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("insertOne failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Insert multiple documents.
    #[tool(description = "Insert multiple documents into a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed.")]
    async fn mongo_insert_many(
        &self,
        Parameters(params): Parameters<InsertManyParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_write_access(&database, &params.collection)?;
        self.security
            .audit_log(
                "insertMany",
                &database,
                &params.collection,
                &format!("count={}", params.documents.len()),
            )
            .await;

        let documents: Result<Vec<Document>, String> =
            params.documents.iter().map(json_to_bson_document).collect();
        let documents = documents.map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let result = self
            .context
            .execute(Command::Query(QueryCommand::InsertMany {
                collection: params.collection,
                documents,
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("insertMany failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Update the first document matching a filter.
    #[tool(
        description = "Update the first document matching a filter in a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed."
    )]
    async fn mongo_update_one(
        &self,
        Parameters(params): Parameters<UpdateOneParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_write_access(&database, &params.collection)?;
        self.security
            .audit_log("updateOne", &database, &params.collection, "")
            .await;

        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let update = json_to_bson_document(&params.update)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let result = self
            .context
            .execute(Command::Query(QueryCommand::UpdateOne {
                collection: params.collection,
                filter,
                update,
                options: UpdateOptions::default(),
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("updateOne failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Update all documents matching a filter.
    #[tool(
        description = "Update all documents matching a filter in a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed."
    )]
    async fn mongo_update_many(
        &self,
        Parameters(params): Parameters<UpdateManyParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_write_access(&database, &params.collection)?;
        self.security
            .audit_log("updateMany", &database, &params.collection, "")
            .await;

        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let update = json_to_bson_document(&params.update)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let result = self
            .context
            .execute(Command::Query(QueryCommand::UpdateMany {
                collection: params.collection,
                filter,
                update,
                options: UpdateOptions::default(),
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("updateMany failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    // ── Delete tools ──────────────────────────────────────────────────────────

    /// Delete the first document matching a filter.
    #[tool(
        description = "Delete the first document matching a filter in a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed."
    )]
    async fn mongo_delete_one(
        &self,
        Parameters(params): Parameters<DeleteOneParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_delete_access(&database, &params.collection)?;
        self.security
            .audit_log("deleteOne", &database, &params.collection, "")
            .await;

        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let result = self
            .context
            .execute(Command::Query(QueryCommand::DeleteOne {
                collection: params.collection,
                filter,
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("deleteOne failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Delete all documents matching a filter.
    #[tool(
        description = "Delete all documents matching a filter in a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed."
    )]
    async fn mongo_delete_many(
        &self,
        Parameters(params): Parameters<DeleteManyParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_delete_access(&database, &params.collection)?;
        self.security
            .audit_log("deleteMany", &database, &params.collection, "")
            .await;

        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let result = self
            .context
            .execute(Command::Query(QueryCommand::DeleteMany {
                collection: params.collection,
                filter,
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("deleteMany failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    // ── Admin tools ───────────────────────────────────────────────────────────

    /// List all databases in the current MongoDB connection.
    #[tool(description = "List all databases in the current MongoDB connection.")]
    async fn mongo_list_databases(&self) -> Result<CallToolResult, McpError> {
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security.audit_log("listDatabases", "", "", "").await;

        let result = self
            .context
            .execute(Command::Admin(AdminCommand::ShowDatabases))
            .await
            .map_err(|e| McpError::internal_error(format!("listDatabases failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// List all collections in a database.
    #[tool(description = "List all collections in a MongoDB database. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed.")]
    async fn mongo_list_collections(
        &self,
        Parameters(params): Parameters<ListCollectionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.security
            .check_read_permission()
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .check_database_access(&database)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        self.security
            .audit_log("listCollections", &database, "", "")
            .await;

        let result = self
            .context
            .execute(Command::Admin(AdminCommand::ShowCollections))
            .await
            .map_err(|e| McpError::internal_error(format!("listCollections failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// List all indexes on a collection.
    #[tool(description = "List all indexes on a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed.")]
    async fn mongo_list_indexes(
        &self,
        Parameters(params): Parameters<ListIndexesParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;
        self.security
            .audit_log("listIndexes", &database, &params.collection, "")
            .await;

        let result = self
            .context
            .execute(Command::Admin(AdminCommand::ListIndexes(params.collection)))
            .await
            .map_err(|e| McpError::internal_error(format!("listIndexes failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Get storage and count statistics for a collection.
    #[tool(
        description = "Get storage and count statistics for a MongoDB collection. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed."
    )]
    async fn mongo_collection_stats(
        &self,
        Parameters(params): Parameters<CollectionStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;
        self.security
            .audit_log(
                "collStats",
                &database,
                &params.collection,
                &format!("scale={}", params.scale),
            )
            .await;

        let result = self
            .context
            .execute(Command::Admin(AdminCommand::CollectionStats {
                collection: params.collection,
                scale: Some(params.scale as i32),
            }))
            .await
            .map_err(|e| McpError::internal_error(format!("collStats failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }

    /// Explain the execution plan for a query.
    #[tool(description = "Explain the execution plan for a MongoDB find query. \
        Pass 'datasource' to target a specific connection — \
        the server switches automatically, no separate prepare step needed.")]
    async fn mongo_explain(
        &self,
        Parameters(params): Parameters<ExplainParams>,
    ) -> Result<CallToolResult, McpError> {
        let database = self
            .resolve_context(params.datasource, params.database)
            .await?;
        self.check_read_access(&database, &params.collection)?;
        self.security
            .audit_log(
                "explain",
                &database,
                &params.collection,
                &format!("verbosity={}", params.verbosity),
            )
            .await;

        let filter = json_to_bson_document(&params.filter)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let find_cmd = Command::Query(QueryCommand::Find {
            collection: params.collection,
            filter,
            options: FindOptions::default(),
        });
        let command = Command::Pipe(Box::new(find_cmd), crate::parser::PipeCommand::Explain);

        let result = self
            .context
            .execute(command)
            .await
            .map_err(|e| McpError::internal_error(format!("explain failed: {e}"), None))?;

        Ok(execution_result_to_mcp_tool_result(result))
    }
}

#[tool_handler]
impl ServerHandler for MongoShellServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "mongosh-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "This server provides MongoDB tools. Answer in the fewest tool calls possible.\n\
                \n\
                ── DIRECT QUERY (preferred) ──────────────────────────────────────────────\n\
                Every query/write/delete tool accepts an optional 'datasource' field.\n\
                When the user mentions a datasource (e.g. \"shop_prod\"), pass it directly:\n\
                \n\
                  mongo_count({ \"datasource\": \"shop_prod\", \"collection\": \"orders\" })\n\
                \n\
                The server switches the connection automatically — NO separate prepare step.\n\
                This is the preferred pattern. Use it whenever the datasource name is known.\n\
                \n\
                ── WHEN TO USE mongo_prepare_context ─────────────────────────────────────\n\
                Use ONLY when:\n\
                  a) The datasource is ambiguous and you need to browse available names, OR\n\
                  b) You need to explore which collections exist before querying.\n\
                Do NOT call it as a routine first step.\n\
                Do NOT call it without a datasource name (empty payload is invalid).\n\
                \n\
                ── WHEN TO USE mongo_list_datasources ────────────────────────────────────\n\
                Use ONLY when the user has NOT mentioned any datasource name and you\n\
                genuinely cannot infer one from context.\n\
                \n\
                ── FIELD DEFAULTS ────────────────────────────────────────────────────────\n\
                'datasource' — optional; omit when context is already active.\n\
                'database'   — optional; defaults to the database embedded in the\n\
                               datasource URI, or the previously active database.\n\
                \n\
                ── BSON TYPES ────────────────────────────────────────────────────────────\n\
                Results use MongoDB Extended JSON v2 (Relaxed) format.\n\
                Use the same wrappers in filters and documents:\n\
                  ObjectId : {\"$oid\": \"69297ddcb4c39276cb39b05b\"}\n\
                  DateTime : {\"$date\": \"2025-01-01T00:00:00Z\"}\n\
                  Date only: {\"$date\": \"2025-01-01\"}   (midnight UTC)\n\
                  Epoch ms : {\"$date\": 1735689600000}\n\
                Plain strings will NOT match ObjectId or DateTime fields.\n\
                \n\
                ── TOOLS ─────────────────────────────────────────────────────────────────\n\
                Context : mongo_prepare_context, mongo_list_datasources\n\
                Read    : mongo_find, mongo_find_one, mongo_aggregate,\n\
                          mongo_count, mongo_distinct\n\
                Write   : mongo_insert_one, mongo_insert_many,\n\
                          mongo_update_one, mongo_update_many\n\
                Delete  : mongo_delete_one, mongo_delete_many\n\
                Admin   : mongo_list_databases, mongo_list_collections,\n\
                          mongo_list_indexes, mongo_collection_stats, mongo_explain\n\
                \n\
                All operations are subject to the security policy configured on the server."
                    .to_string(),
            )
    }

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

    fn make_server() -> MongoShellServer {
        let config = Config::default();
        let connection = ConnectionManager::new(
            "mongodb://localhost:27017".to_string(),
            config.connection.clone(),
        );
        let state = SharedState::new("test".to_string());
        MongoShellServer::with_config(
            connection,
            state,
            SecurityConfig::default(),
            ConnectionConfig::default(),
            String::new(),
        )
    }

    #[test]
    fn server_info_has_tools_capability() {
        let info = make_server().get_info();
        assert!(info.capabilities.tools.is_some());
        assert_eq!(info.server_info.name, "mongosh-mcp");
    }

    #[test]
    fn server_instructions_mention_direct_query_pattern() {
        let info = make_server().get_info();
        let instructions = info.instructions.unwrap();
        // Should guide LLM toward direct datasource on query tools
        assert!(instructions.contains("datasource"));
        assert!(instructions.contains("mongo_count"));
        assert!(instructions.contains("mongo_prepare_context"));
    }

    #[tokio::test]
    async fn resolve_context_uses_explicit_database() {
        let server = make_server();
        let db = server
            .resolve_context(None, Some("explicit_db".to_string()))
            .await
            .unwrap();
        assert_eq!(db, "explicit_db");
    }

    #[tokio::test]
    async fn resolve_context_falls_back_to_active_database() {
        let server = make_server();
        // SharedState was initialised with "test" in make_server
        let db = server.resolve_context(None, None).await.unwrap();
        assert_eq!(db, "test");
    }

    #[tokio::test]
    async fn resolve_context_errors_when_no_db_available() {
        let config = Config::default();
        let connection = ConnectionManager::new(
            "mongodb://localhost:27017".to_string(),
            config.connection.clone(),
        );
        // Initialise with an empty database name so fallback also fails.
        let state = SharedState::new(String::new());
        let server = MongoShellServer::with_config(
            connection,
            state,
            SecurityConfig::default(),
            ConnectionConfig::default(),
            String::new(),
        );
        assert!(server.resolve_context(None, None).await.is_err());
    }
}
