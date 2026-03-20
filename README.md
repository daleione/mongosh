# Mongosh - MongoDB Shell with SQL Support

[![Crates.io](https://img.shields.io/crates/v/mongosh.svg)](https://crates.io/crates/mongosh)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance MongoDB shell written in Rust that bridges SQL and MongoDB - query your MongoDB databases using familiar SQL syntax or native MongoDB commands.

> **Note:** This is an independent community project, not affiliated with MongoDB Inc.

## 🎯 Why Mongosh?

- **🔄 SQL to MongoDB** - Write SQL queries, execute as MongoDB commands automatically
- **⚡ Blazing Fast** - Native Rust implementation with async I/O
- **🎨 Beautiful Output** - Syntax highlighting, formatted tables, and pretty JSON
- **🧠 Smart Completion** - Context-aware auto-completion for collections, fields, and commands
- **💾 Named Queries** - Save and reuse complex queries with parameters
- **📊 Rich SQL Features** - Array indexing, date functions, arithmetic operations, and more
- **🤖 MCP Server** - Let AI assistants query MongoDB with 15 built-in tools

## 📦 Installation

```bash
cargo install mongosh
```

## 🚀 Quick Start

```bash
# Connect to MongoDB
mongosh mongodb://localhost:27017

# Use SQL syntax
SELECT name, email FROM users WHERE age > 18 ORDER BY name LIMIT 10

# Or native MongoDB syntax
db.users.find({ age: { $gt: 18 } }).sort({ name: 1 }).limit(10)

# Save frequently used queries
query save active_users db.users.find({status: 'active'})
query active_users
```

## ✨ Key Features

### 1. SQL Query Support

Query MongoDB using standard SQL syntax - automatically translated to MongoDB queries:

```sql
-- Basic queries
SELECT * FROM orders WHERE status = 'completed'

-- Aggregations
SELECT category, COUNT(*) as total, AVG(price) as avg_price
FROM products
GROUP BY category
HAVING total > 10

-- Array access
SELECT tags[0] AS primary_tag FROM posts WHERE tags[-1] = 'featured'

-- Date filtering
SELECT * FROM events WHERE created_at > DATE '2024-01-01'

-- Arithmetic operations
SELECT price * quantity * 1.13 AS total FROM orders

-- Query analysis
EXPLAIN SELECT * FROM users WHERE age > 18
```

### 2. Named Queries with Parameters

Save and reuse queries with parameter substitution:

```javascript
// Save a parameterized query
query save user_by_email db.users.findOne({email: '$1'})

// Execute with parameters
query user_by_email john@example.com

// List all saved queries
query list
```

### 3. Smart Shell Completion

Auto-complete datasource names from your configuration:

```bash
# Tab completion for datasources
mongosh -d prod<TAB>     # Completes to: mongosh -d production

# Works with options too
mongosh --datasource dev<TAB>
```

### 4. Rich Output Formats

```bash
# Pretty JSON (default)
db.users.find().limit(1)

# Compact JSON
mongosh --format json-compact

# Table format
mongosh --format table

# Shell-style output
mongosh --format shell
```

### 5. MCP Server (Model Context Protocol)

Enable AI assistants (Claude, Cursor, etc.) to query MongoDB directly:

```bash
mongosh --mcp mongodb://localhost:27017 --database mydb
```

**Claude Desktop configuration** (`claude_desktop_config.json`):

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

**15 built-in tools** — full CRUD, aggregation, and admin operations:

| Category | Tools |
|----------|-------|
| Context  | `mongo_prepare_context`, `mongo_list_datasources` |
| Read     | `mongo_find`, `mongo_find_one`, `mongo_aggregate`, `mongo_count`, `mongo_distinct` |
| Write    | `mongo_insert_one`, `mongo_insert_many`, `mongo_update_one`, `mongo_update_many` |
| Delete   | `mongo_delete_one`, `mongo_delete_many` |
| Admin    | `mongo_list_databases`, `mongo_list_collections`, `mongo_list_indexes`, `mongo_collection_stats`, `mongo_explain` |

**Security** — configure in `~/.mongoshrc`:

```toml
[mcp.security]
allow_read = true
allow_write = false
allow_delete = false
max_documents_per_query = 1000
forbidden_collections = ["system.*", "admin.*"]
```

See [MCP Documentation](./docs/model-contex-protocol.md) for details.

## 📚 Documentation

Comprehensive guides for all features:

- [SQL Array Indexing](./docs/array-indexing.md) - Access array elements with `arr[0]` or `arr[-1]`
- [SQL Arithmetic Operations](./docs/arithmetic-operations.md) - Math expressions and functions
- [DateTime Functions](./docs/datetime-functions.md) - `DATE`, `TIMESTAMP`, `CURRENT_DATE`
- [Query EXPLAIN](./docs/query-explain.md) - Analyze query performance
- [Named Queries](./docs/named-queries.md) - Save and reuse queries
- [Shell Completion](./docs/shell-completion.md) - Setup auto-completion
- [API Reference](./docs/api-reference.md) - MongoDB method support status
- [MCP Server](./docs/model-contex-protocol.md) - AI assistant integration via MCP

## 🆚 vs Official mongosh

| Feature            | Official mongosh   | This Project                   |
| ------------------ | ------------------ | ------------------------------ |
| Language           | JavaScript/Node.js | Rust                           |
| SQL Support        | ❌                 | ✅ Full SQL SELECT             |
| Startup Time       | ~500ms             | <50ms                          |
| Memory Usage       | ~50MB              | ~5MB                           |
| Output Formats     | JSON               | JSON, Table, Shell             |
| Named Queries      | ❌                 | ✅ With parameters             |
| Array Indexing     | MongoDB only       | SQL + MongoDB                  |
| Auto-completion    | Basic              | Advanced (datasources, fields) |
| MCP Server         | ❌                 | ✅ 15 tools with security      |
| JavaScript Runtime | ✅ Full            | ❌ Not a JS shell              |

## 🔧 Configuration

Create `~/.mongoshrc` or `~/.config/mongosh/config.toml`:

```toml
[connection]
default_datasource = "local"

[connection.datasources]
local = "mongodb://localhost:27017"
production = "mongodb://prod.example.com:27017"
staging = "mongodb://staging.example.com:27017"

[output]
format = "pretty-json"
highlight = true
```

## 🤝 Contributing

Contributions are welcome! Please check out our [documentation](./docs/) for implementation details.

## 📄 License

Licensed under the [MIT License](LICENSE).

## 🔗 Links

- [Documentation](./docs/README.md)
- [Crates.io](https://crates.io/crates/mongosh)
- [GitHub Issues](https://github.com/yourusername/mongosh/issues)
