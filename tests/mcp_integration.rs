//! Integration tests for MCP server functionality

use mongosh::config::Config;
use mongosh::connection::ConnectionManager;
use mongosh::mcp::{MongoShellServer, SecurityConfig};
use mongosh::repl::SharedState;
use rmcp::ServerHandler;

fn create_test_server() -> MongoShellServer {
    let config = Config::default();
    let connection = ConnectionManager::new(
        "mongodb://localhost:27017".to_string(),
        config.connection.clone()
    );
    let state = SharedState::new("test".to_string());
    let security = SecurityConfig::default();

    MongoShellServer::new(connection, state, security)
}

#[test]
fn test_server_creation() {
    let server = create_test_server();
    let info = server.get_info();

    assert!(info.capabilities.tools.is_some());
    assert_eq!(info.server_info.name, "mongosh-mcp");
    assert_eq!(info.protocol_version, rmcp::model::ProtocolVersion::V_2024_11_05);
}

#[test]
fn test_security_config_defaults() {
    let config = SecurityConfig::default();

    assert!(config.allow_read);
    assert!(!config.allow_write);
    assert!(!config.allow_delete);
    assert_eq!(config.max_documents_per_query, 1000);
    assert_eq!(config.max_pipeline_stages, 10);
    assert_eq!(config.query_timeout_seconds, 30);
}

#[test]
fn test_security_config_custom() {
    let config = SecurityConfig {
        allow_read: true,
        allow_write: true,
        allow_delete: true,
        max_documents_per_query: 500,
        max_pipeline_stages: 5,
        query_timeout_seconds: 60,
        allowed_databases: vec!["testdb".to_string()],
        forbidden_collections: vec!["*.internal".to_string()],
        audit_enabled: false,
    };

    assert!(config.allow_write);
    assert!(config.allow_delete);
    assert_eq!(config.max_documents_per_query, 500);
    assert_eq!(config.allowed_databases.len(), 1);
}

#[tokio::test]
async fn test_server_info_structure() {
    let server = create_test_server();
    let info = server.get_info();

    // Check that tools capability is enabled
    assert!(info.capabilities.tools.is_some());

    // Check server info
    assert_eq!(info.server_info.name, "mongosh-mcp");
    assert!(!info.server_info.version.is_empty());

    // Check protocol version
    assert_eq!(info.protocol_version, rmcp::model::ProtocolVersion::V_2024_11_05);

    // Check instructions
    assert!(info.instructions.is_some());
    let instructions = info.instructions.unwrap();
    assert!(instructions.contains("MongoDB"));
    assert!(instructions.contains("find"));
    assert!(instructions.contains("aggregate"));
}

#[test]
fn test_tool_router_initialization() {
    let server = create_test_server();

    // The server should have been created successfully
    // and should have a tool router initialized
    // This is a basic smoke test
    let info = server.get_info();
    assert!(info.capabilities.tools.is_some());
}
