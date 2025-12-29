# Rust MongoDB Power CLI

[![Crates.io](https://img.shields.io/crates/v/mongosh.svg)](https://crates.io/crates/mongosh)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A power-user oriented MongoDB CLI written in Rust, focused on productivity, scripting, and rich output.

> **Note:** This project is an independent, community-driven tool. It is **NOT** affiliated with MongoDB, and it is not intended to be a drop-in replacement for the official `mongosh`.

---

## ✨ Why Another MongoDB CLI?

The official MongoDB Shell (`mongosh`) is excellent for compatibility and JavaScript workflows. This project exists for engineers who want a faster, more scriptable, and CLI-native experience:

- 🧠 **Power-user workflows** — Batch queries, automation, CI/CD
- 📊 **Readable output** — Tables, highlighted JSON
- ⚡ **Fast startup & execution** — Compiled Rust binary
- 🧩 **Extensible architecture** — Plugins & future extensions

> If you rely heavily on JavaScript execution inside the shell, you should continue using the official `mongosh`.

---

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

## 🚧 Project Status

> ⚠️ **Active Development – Not Production Ready**
>
> - APIs and commands may change
> - Some MongoDB features are incomplete
> - Bugs and panics may exist
>
> Feedback, testing, and contributions are highly welcome.

---

## ✨ Features

- ⚡ **High Performance** — Native Rust, async I/O
- 💾 **Lightweight** — Small static binary
- 🔒 **Type Safety** — Memory-safe by design
- 🧵 **Async Execution** — Powered by Tokio
- 🎨 **Syntax Highlighting** — Readable command & JSON output
- 🧠 **Smart Completion** — Context-aware auto-completion
- 📊 **Rich Output** — Table & structured views (WIP)
- 🔌 **Extensible** — Plugin-friendly design
- 🌍 **Cross-Platform** — Linux, macOS, Windows

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

# Connect with authentication
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
