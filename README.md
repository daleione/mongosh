# Rust MongoDB Power CLI

[![Crates.io](https://img.shields.io/crates/v/mongosh.svg)](https://crates.io/crates/mongosh)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A powerful MongoDB CLI written in Rust, featuring intelligent auto-completion, SQL query support, and enhanced security features for productive database operations.

> **Note:** This project is an independent, community-driven tool. It is **NOT** affiliated with MongoDB, and it is not intended to be a drop-in replacement for the official `mongosh`.

## 🔍 Key Differences vs Official mongosh

| Feature        | Official mongosh | This Project              |
| -------------- | ---------------- | ------------------------- |
| Implementation | Node.js          | Rust (async)              |
| JS Runtime     | Full JavaScript  | ❌ Not a JS shell         |
| Startup Time   | Slower           | Fast                      |
| Output         | JSON-first       | Tables + highlighted JSON |
| Scripting      | JS-based         | CLI / batch-oriented      |
| Target Users   | General users    | Power users / DevOps      |

---

## ✨ Features

- ⚡ **High Performance** — Native Rust, async I/O
- 💾 **Lightweight** — Small static binary
- 🎨 **Syntax Highlighting** — Readable command & JSON output
- 📊 **Rich Output Formats** — JSON (pretty/compact), shell-style, and table views
- 🗄️ **SQL Query Support** — Query MongoDB using familiar SQL SELECT syntax
- 🧠 **Intelligent Auto-Completion** — Context-aware suggestions for MongoDB shell and SQL commands

---

## 📦 Installation

```bash
cargo install mongosh
```

> **Note:** The binary name may change in the future to avoid conflicts with the official MongoDB shell.

---

## 🚀 Quick Start

### Connect to MongoDB

```bash
# Connect to local MongoDB
mongosh

# Connect to a specific host
mongosh mongodb://localhost:27017

# Connect with authentication (credentials are automatically sanitized in logs)
mongosh mongodb://username:password@localhost:27017/dbname
```

---

## 🧪 Example Commands

### Show Databases

```javascript
show dbs
```

### Switch Database

```javascript
use mydb
```

### Show Collections

```javascript
show collections
```

### Insert a Document

```javascript
db.users.insertOne({ name: "John Doe", age: 25 });
```

### Query Documents

```javascript
db.users.find({ age: { $gte: 18 } });
```

### Update Documents

```javascript
db.users.updateOne({ name: "John Doe" }, { $set: { age: 26 } });
```

### Aggregation Pipeline

```javascript
db.orders.aggregate([
  { $match: { status: "completed" } },
  { $group: { _id: "$userId", total: { $sum: "$amount" } } },
]);
```

---

## 🔍 SQL Query Support

This shell now supports SQL SELECT queries that are automatically translated to MongoDB queries!

### Basic SELECT Queries

```sql
-- Simple query with filtering and sorting
SELECT name, age FROM users WHERE age > 18 ORDER BY name ASC

-- Pagination with LIMIT and OFFSET
SELECT * FROM users LIMIT 10 OFFSET 5
```

### Aggregate Functions

```sql
-- Column aliases support both identifiers and quoted strings
SELECT group_id AS 'group_id', COUNT(*) FROM templates GROUP BY group_id

-- Group by with multiple aggregates
SELECT
  category,
  COUNT(*) AS total,
  SUM(price) AS revenue
FROM products
GROUP BY category
```

### Supported SQL Features

- ✅ SELECT with column list or `*`
- ✅ FROM clause
- ✅ WHERE with comparison operators (`=`, `!=`, `>`, `<`, `>=`, `<=`)
- ✅ Logical operators (AND, OR)
- ✅ GROUP BY with aggregation functions (COUNT, SUM, AVG, MIN, MAX)
- ✅ ORDER BY with ASC/DESC
- ✅ LIMIT and OFFSET
- ✅ Column aliases with AS (supports both identifiers and string literals)

### SQL to MongoDB Translation Examples

| SQL                                     | MongoDB                                                  |
| --------------------------------------- | -------------------------------------------------------- |
| `SELECT * FROM users`                   | `db.users.find({})`                                      |
| `SELECT name, age FROM users`           | `db.users.find({}, {name: 1, age: 1})`                   |
| `WHERE age > 18`                        | `{age: {$gt: 18}}`                                       |
| `WHERE status = 'active' AND age >= 18` | `{$and: [{status: 'active'}, {age: {$gte: 18}}]}`        |
| `ORDER BY name ASC`                     | `{name: 1}`                                              |
| `LIMIT 10`                              | `limit(10)`                                              |
| `GROUP BY category`                     | `aggregate([{$group: {_id: "$category"}}])`              |
| `SELECT COUNT(*) FROM users`            | `aggregate([{$group: {_id: null, COUNT_*: {$sum: 1}}}])` |

### Notes

- SQL queries are automatically detected when starting with `SELECT`
- Complex JOIN operations are not yet supported
- Subqueries are not yet supported

---

## 📄 License

Licensed under the [MIT License](https://opensource.org/licenses/MIT).

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📬 Feedback

If you have any questions, suggestions, or issues, please open an issue on GitHub.
