# Model Context Protocol (MCP) Integration

This guide shows you how to use `mongosh` as an MCP server so an AI assistant can interact with MongoDB on your behalf.

> **stdio only:** `mongosh --mcp` communicates exclusively over stdio. It cannot run as a standalone network service. The AI assistant client (e.g., Claude Desktop) is responsible for launching and managing the `mongosh` process. You do not start it manually and leave it running in the background.

> **Security note:** MCP makes it easier to execute database operations. Treat it like giving a tool direct access to your database. Use least privilege, strong authentication, and restrictive policies—especially in production.

## Overview

The MCP integration allows an AI assistant (for example, Claude) to send structured requests to `mongosh`. `mongosh` then executes MongoDB operations and returns results.

You control access via:

- MongoDB credentials and roles (server-side authorization)
- MCP security policy in `mongosh` config (client-side guardrails)
- Network and TLS controls
- Audit logging

**MCP policies are not a substitute for MongoDB auth.** Always use MongoDB roles to enforce true access control.

## How It Works

`mongosh --mcp` uses the [stdio transport](https://modelcontextprotocol.io/docs/concepts/transports#standard-input-output-stdio) defined by the MCP specification:

1. The AI assistant client reads the `command` and `args` from its config and **spawns** `mongosh --mcp ...` as a child process.
2. The client communicates with `mongosh` over the process's stdin/stdout using JSON-RPC messages.
3. The assistant calls tools (e.g., `mongo_find`, `mongo_aggregate`, `mongo_list_collections`).
4. `mongosh` checks your MCP security policy and MongoDB permissions.
5. If permitted, the operation executes and the result is written back to stdout for the client to read.
6. When the assistant session ends, the client terminates the child process.

Because the transport is stdio, `mongosh --mcp` has no open network port and is not reachable from other processes. There is no way to connect to a running instance from a second client.

## Quick Start

### 1. Add `mongosh` to your AI assistant's config

You do not start `mongosh --mcp` yourself. Instead, declare it in your assistant's MCP server configuration and the assistant will launch it on demand.

For **Claude Desktop**, edit `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "mongodb": {
      "command": "mongosh",
      "args": ["--mcp", "mongodb://localhost:27017", "--database", "mydb"]
    }
  }
}
```

The client passes `command` + `args` to the OS, spawns the process, and wires its stdin/stdout for MCP communication. The supported `args` are:

| Argument            | Description                                     |
| ------------------- | ----------------------------------------------- |
| `URI`               | MongoDB connection URI (positional)             |
| `-d <name>`         | Use a named datasource from `~/.mongoshrc`      |
| `--database <name>` | Default database to use                         |
| `-c <file>`         | Path to a config file (default: `~/.mongoshrc`) |
| `--host`, `--port`  | Server address (alternative to URI)             |
| `-u`, `-p`          | Username and password                           |
| `--tls`             | Enable TLS                                      |

### 2. Restart your AI assistant

After saving the config, restart the assistant application. It will spawn `mongosh --mcp` automatically when you start a conversation that uses the MongoDB tools.

### 3. Ask questions or run tasks

Once configured, your assistant translates natural language requests into MongoDB operations:

- "Show me all active users in the `users` collection"
- "How many orders were placed yesterday?"
- "What indexes exist on `orders`?"
- "Why is my query on `email` slow?"

## Configure Your AI Assistant

### Basic configuration

```json
{
  "mcpServers": {
    "mongodb": {
      "command": "mongosh",
      "args": ["--mcp", "mongodb://localhost:27017", "--database", "mydb"]
    }
  }
}
```

### With a custom config file

Use `-c` to point to a config file that contains your MCP security policy. This is recommended for anything beyond local development:

```json
{
  "mcpServers": {
    "mongodb": {
      "command": "mongosh",
      "args": [
        "--mcp",
        "mongodb://localhost:27017",
        "--database",
        "mydb",
        "-c",
        "/path/to/my-config.toml"
      ]
    }
  }
}
```

### Using a named datasource

If you have datasources defined in `~/.mongoshrc`, you can reference them by name instead of embedding a URI in the assistant config:

```json
{
  "mcpServers": {
    "mongodb": {
      "command": "mongosh",
      "args": ["--mcp", "-d", "production", "--database", "mydb"]
    }
  }
}
```

This keeps credentials out of the assistant config file entirely.

## Configuration File

The default config file is `~/.mongoshrc`. You can specify a different path with `-c`.

MCP settings live under the `[mcp]` and `[mcp.security]` sections:

```toml
[mcp]
enabled = true

[mcp.security]
allow_read = true
allow_write = false
allow_delete = false

max_documents_per_query = 1000
max_pipeline_stages = 10
query_timeout_seconds = 30

# Empty array means all databases are allowed (use with caution in production)
allowed_databases = []

# Wildcard patterns to block sensitive collections
forbidden_collections = ["system.*", "admin.*"]

audit_enabled = true
```

### Field reference

| Field                     | Description                                                                    |
| ------------------------- | ------------------------------------------------------------------------------ |
| `allow_read`              | Allow read operations: `find`, `findOne`, `aggregate`, `count`, `distinct`     |
| `allow_write`             | Allow write operations: `insertOne`, `insertMany`, `updateOne`, `updateMany`   |
| `allow_delete`            | Allow delete operations: `deleteOne`, `deleteMany`                             |
| `max_documents_per_query` | Cap on documents returned per query. Prevents accidental large data transfers. |
| `max_pipeline_stages`     | Maximum stages in an aggregation pipeline.                                     |
| `query_timeout_seconds`   | Maximum seconds a query may run.                                               |
| `allowed_databases`       | Allowlist of databases. Empty means all databases are permitted.               |
| `forbidden_collections`   | Denylist of collection name patterns (supports `*` wildcard).                  |
| `audit_enabled`           | Log all MCP operations via the tracing system.                                 |

## Recommended Security Profiles

### Read-only (recommended for production)

```toml
[mcp]
enabled = true

[mcp.security]
allow_read = true
allow_write = false
allow_delete = false

max_documents_per_query = 500
max_pipeline_stages = 10
query_timeout_seconds = 60

allowed_databases = ["production_db"]
forbidden_collections = ["system.*", "admin.*", "*.sensitive", "*.pii"]

audit_enabled = true
```

### Development

```toml
[mcp]
enabled = true

[mcp.security]
allow_read = true
allow_write = true
allow_delete = true

max_documents_per_query = 200
max_pipeline_stages = 15
query_timeout_seconds = 30

allowed_databases = []
forbidden_collections = ["system.*"]

audit_enabled = true
```

### Analytics / reporting (large reads, no writes)

```toml
[mcp]
enabled = true

[mcp.security]
allow_read = true
allow_write = false
allow_delete = false

max_documents_per_query = 5000
max_pipeline_stages = 20
query_timeout_seconds = 120

allowed_databases = ["analytics", "reporting"]
forbidden_collections = ["system.*", "admin.*"]

audit_enabled = true
```

## Available Operations

The MCP server exposes the following tools. Your MCP security policy and MongoDB user permissions determine which ones actually execute.

### Read operations (require `allow_read = true`)

| Tool              | Description                                                            |
| ----------------- | ---------------------------------------------------------------------- |
| `mongo_find`      | Find documents with optional filter, projection, sort, skip, and limit |
| `mongo_find_one`  | Find a single document                                                 |
| `mongo_aggregate` | Execute an aggregation pipeline                                        |
| `mongo_count`     | Count documents matching a filter                                      |
| `mongo_distinct`  | Get distinct values for a field                                        |

### Write operations (require `allow_write = true`)

| Tool                | Description                                 |
| ------------------- | ------------------------------------------- |
| `mongo_insert_one`  | Insert a single document                    |
| `mongo_insert_many` | Insert multiple documents                   |
| `mongo_update_one`  | Update the first document matching a filter |
| `mongo_update_many` | Update all documents matching a filter      |

### Delete operations (require `allow_delete = true`)

| Tool                | Description                                 |
| ------------------- | ------------------------------------------- |
| `mongo_delete_one`  | Delete the first document matching a filter |
| `mongo_delete_many` | Delete all documents matching a filter      |

### Administrative operations (require `allow_read = true`)

| Tool                     | Description                                           |
| ------------------------ | ----------------------------------------------------- |
| `mongo_list_databases`   | List all databases                                    |
| `mongo_list_collections` | List all collections in a database                    |
| `mongo_list_indexes`     | List all indexes on a collection                      |
| `mongo_collection_stats` | Get storage and count statistics for a collection     |
| `mongo_explain`          | Inspect the query execution plan for a find operation |

## Wildcard Patterns in `forbidden_collections`

Patterns are matched against both the full `database.collection` name and the collection name alone.

```toml
forbidden_collections = [
  "system.*",    # All collections whose name starts with "system."
  "admin.*",     # All collections in the admin database
  "*.internal",  # Any collection whose name ends with ".internal"
  "*.sensitive", # Any collection whose name ends with ".sensitive"
  "*_backup"     # Any collection whose name ends with "_backup"
]
```

## See Also

- [MCP Specification](https://modelcontextprotocol.io/)
