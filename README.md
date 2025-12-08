# Rust MongoDB Shell

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

🚀 **High-Performance MongoDB Shell Implementation in Rust**

A MongoDB Shell developed in Rust, designed to provide faster performance, smaller binary size, and better user experience.

## ✨ Features

- 🔥 **High Performance**: Written in Rust for ultimate performance
- 💾 **Lightweight**: Small compiled binary size
- 🔒 **Type Safe**: Rust's type system ensures memory safety
- ⚡ **Async Execution**: Built on Tokio async runtime
- 🎨 **Syntax Highlighting**: Command syntax highlighting support
- 📝 **Smart Completion**: Context-aware command auto-completion
- 🔌 **Extensible**: Plugin system support
- 🌍 **Cross-Platform**: Supports Linux, macOS, Windows

## 📦 Installation

### Build from Source

```bash
# Clone the repository
git clone https://github.com/dalei/mongosh.git
cd mongosh

# Build the project
cargo build --release

# Run
./target/release/mongosh
```

### Install via Cargo (Coming Soon)

```bash
cargo install mongosh-rs
```

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

### Basic Operations

```javascript
// Show all databases
show dbs

// Switch database
use mydb

// Show all collections
show collections

// Insert document
db.users.insertOne({ name: "John Doe", age: 25 })

// Query documents
db.users.find({ age: { $gte: 18 } })

// Update document
db.users.updateOne(
  { name: "John Doe" },
  { $set: { age: 26 } }
)

// Delete document
db.users.deleteOne({ name: "John Doe" })

// Aggregation query
db.orders.aggregate([
  { $match: { status: "completed" } },
  { $group: { _id: "$userId", total: { $sum: "$amount" } } }
])
```
