# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- **SQL standard datetime support** - Added support for SQL-92 standard date and time syntax

## [0.9.0] - 2026-02-11

### Added

- **Query execution cancellation** - Ctrl+C support with automatic `killOp` to cancel slow MongoDB operations
- **Query explain support** - Added `explain()` method for query execution plan analysis
  - Chain method support: `db.collection.find().explain()`
  - SQL EXPLAIN syntax support with highlighting
- **New MongoDB commands**:
  - `stats` - Collection statistics and storage information
  - `findAndModify` - Atomic find and modify operations
  - `renameCollection` - Rename collections
- **Streaming export improvements**:
  - CSV and JSONL streaming export support
  - Live cursor-based pagination replacing skip-based pagination
  - Real-time progress tracking and cancellation support

### Changed

- **Code structure refactoring**:
  - Modularized `query.rs` for better maintainability
  - Refactored `mongo_operation.rs` with organized test structure
  - Improved chain parser with early returns and extracted methods
- **Enhanced error handling**:
  - Refined error messages and formatting
  - Improved SQL parser error handling for WHERE clauses
  - Simplified confirmation prompts
- **Parser improvements**:
  - Replaced Option with ChainParseResult for better chained call parsing
- **Dependency updates** - Updated packages to latest versions

## [0.8.0] - 2026-01-26

### Added

- **Custom MongoDB shell parser** - Refactored parser with custom oxc-based implementation specifically designed for MongoDB shell syntax
- **Dynamic shell completion** - Intelligent auto-completion with dynamic datasource support for context-aware suggestions
- **BSON utilities** - Added utility functions for working with BSON data types

### Changed

- **SQL enhancements**:
  - Added support for array index access in SQL queries (e.g., `SELECT tags[0] FROM collection`)
  - Improved nested field handling in SQL queries
  - Added support for field aliases in SQL projections
  - Enhanced ObjectId function support in SQL parser
  - Optimized `_id` field exclusion from projections unless explicitly requested
- **Improved export operations** - Added `execute_find_all` method for more efficient bulk export functionality

### Removed

- **Polars dependency removed** - Eliminated polars crate to reduce binary size and dependencies

## [0.7.0] - 2026-01-12

### Added

- **Export command** - Export query results to files in multiple formats (JSON Lines, CSV, Excel)
- **Named query parameters** - Support for named parameters in queries (e.g., `:name`, `:age`)
  - Improved parameter substitution with better error handling
- **Additional MongoDB commands**:
  - `findOneAndDelete` - Find and delete a single document atomically
  - `findOneAndUpdate` - Find and update a single document atomically
  - `findOneAndReplace` - Find and replace a single document atomically
  - `replaceOne` - Replace a single document
  - `drop` - Drop collection or database
  - `estimatedDocumentCount` - Fast document count using collection metadata
  - `distinct` - Get distinct values for a field
- **Method support documentation** - Added comprehensive documentation for supported MongoDB methods

## [0.6.0] - 2026-01-07

### Added

- Datasource support with named connections
- TOML configuration file support
- Unified syntax highlighter with MongoDB and SQL support

### Changed

- Improved completion sorting and Tab behavior
- Refined completion FSM for partial identifier handling
- Improved SQL completion FSM with semicolon token support

## [0.5.0] - 2026-01-06

### Added

- **Enhanced REPL with reedline** - Migrated from rustyline to reedline for improved terminal handling and better user experience
- **Advanced syntax highlighting** - Multi-color syntax highlighting for MongoDB commands, keywords, strings, numbers, and operators
- **Inline hints** - Context-aware hints based on command history
- **Custom prompt** - Dynamic prompt showing current database and connection status
- **Input validation** - Real-time validation of command syntax with helpful error messages
- **SQL COUNT(DISTINCT) support** - Added support for `COUNT(DISTINCT column)` in SQL queries

### Changed

- **Dependency optimization** - Removed unused dependencies (anyhow, thiserror, regex, toml, mockall, tokio-test) for faster builds and smaller binary size
- **Improved test assertions** - Replaced `assert_eq!` with `assert!(matches!)` for better pattern matching in validator tests
- **Simplified aggregate output** - Cleaner field name generation in SQL GROUP BY queries

### Fixed

- **Completion system improvements** - Added parentheses tracking to FSM for better auto-completion accuracy

## [0.4.0] - 2026-01-05

### Added

- **Intelligent auto-completion** - Smart auto-completion for MongoDB shell commands and SQL syntax with context-aware suggestions
- **URI sanitization** - Automatic sanitization of MongoDB connection URIs to protect sensitive information
- **Server version display** - Show MongoDB server version information on connection
- **User confirmation for dangerous operations** - Added safety prompts for potentially destructive commands
- **SQL aggregate queries without GROUP BY** - Support for SQL aggregate functions (COUNT, SUM, AVG, etc.) without requiring GROUP BY clause

### Changed

- **Optimized completion system** - Removed exact prefix matches from auto-completion suggestions for better user experience
- **SQL clause order validation** - Enhanced validation to ensure correct SQL clause ordering (SELECT, FROM, WHERE, GROUP BY, ORDER BY, LIMIT)

## [0.3.0] - 2025-12-30

### Added

- **SQL query support** - Query MongoDB using SQL syntax (SELECT, FROM, WHERE, GROUP BY, ORDER BY, LIMIT)
- **Automatic cursor pagination** - Find results now show first 20 documents with "Type 'it' for more" prompt
- **`Long()` alias** - Added as shorthand for `NumberLong()`

### Changed

- **Code quality improvements** - Refactored REPL and error modules, removed plugin system, eliminated all compiler warnings

## [0.2.4] - 2025-12-23

### Added

- **Aggregate command execution** - Full implementation of MongoDB aggregation pipeline support with options including `allowDiskUse`, `batchSize`, `maxTimeMS`, `collation`, `hint`, `readConcern`, and `let` variables
- **Index management commands** - Added `createIndex` and `createIndexes` admin commands for creating single and multiple indexes
- **Index listing command** - Added `getIndexes` command to list all indexes on a collection

## [0.2.3] - 2025-12-18

### Added

- **Projection support** - Implemented field projection for query results to control which fields are returned
- **MongoDB 4.0+ compatibility** - Locked MongoDB driver version to 3.2.5 for better compatibility with MongoDB 4.0 and later versions

## [0.2.2] - 2025-12-17

### Added

- **Runtime configuration commands** - `format`, `color`, and `config` commands for changing output settings in REPL
- **Table formatter** - New table output format using tabled crate for MongoDB documents
- **Configurable JSON indentation** - Customizable indentation levels for JSON output
- **`count()` alias** - Added as alias for `countDocuments()` for convenience

### Changed

- **Rust edition updated to 2024**
- Refactored CLI argument handling into separate method for better code organization
- Refactored colorizer and shell formatter for BSON types
- Enforced minimum server selection timeout and direct connection for single hosts

### Removed

- Script mod removed from codebase

## [0.2.1] - 2025-12-16

### Changed

- **Default log level changed to WARN** - Cleaner interactive mode output, INFO logs only shown with `--verbose`
- `find()` results now displayed as array format - Matches MongoDB shell behavior

### Fixed

- Removed duplicate connection logging messages
- Fixed redundant error message prefixes (e.g., "Parse error: Parse error:")
- Removed extra blank line after connection banner

## [0.2.0] - 2025-12-16

### Added

- **AST-based parser using Oxc** - Complete rewrite replacing regex-based parsing
- **Chained method call support** - `.limit()`, `.skip()`, `.sort()`, `.projection()`, `.batchSize()`
- **Write operations** - Full implementation of `insertOne`, `insertMany`, `updateOne`, `updateMany`, `deleteOne`, `deleteMany`
- MongoDB-specific type constructors: `ObjectId()`, `ISODate()`, `NumberInt()`, `NumberLong()`, `NumberDecimal()`
- New query commands: `ReplaceOne`, `EstimatedDocumentCount`, `FindOneAndDelete`, `FindOneAndUpdate`, `FindOneAndReplace`, `Distinct`, `BulkWrite`
- Extended admin commands: `ShowUsers`, `ShowRoles`, `ShowProfile`, `ShowLogs`, `ServerStatus`, `CurrentOp`, `KillOp`, etc.
- Enhanced options support for find, update, aggregate, and findAndModify operations
- Result formatting for write operations in shell and JSON formats

### Changed

- **Refactored executor module** - Split into separate files: `context.rs`, `result.rs`, `router.rs`, `query.rs`, `admin.rs`, `utility.rs`
- Refactored parser into modular architecture (`command.rs`, `db_operation.rs`, `expr_converter.rs`, `shell_commands.rs`)
- `QueryCommand::Count` renamed to `QueryCommand::CountDocuments`

### Fixed

- Complex nested queries and MongoDB operators now parse correctly
- Proper handling of negative numbers, arrays, and nested objects
- Database name validation follows MongoDB naming rules

## [0.1.2] - 2025-12-15

### Added

- `findOne` command support with filter and projection options
- Shell-style output formatting (mongosh compatible)
- Simplified JSON format for programmatic use
- CLI `--format` argument support (shell, json, json-pretty, table, compact)

### Changed

- **Refactored formatter module into separate files for better maintainability**

### Fixed

- **JSON compact format now outputs single-line JSON without colorization**
- CLI arguments (e.g., `--format`, `--no-color`) now properly override configuration defaults
- Tab completion no longer causes panic in REPL

## [0.1.1] - 2025-12-12

### Fixed

- REPL prompt now updates correctly after `use <database>` command

### Added

- `--host` and `--port` CLI parameters for MongoDB connection
- Complete connection URI building from CLI arguments

### Changed

- Refactored state management using `SharedState` pattern
- Eliminated manual state synchronization (breaking change)
- Reduced memory overhead and unnecessary clone operations

## [0.1.0] - 2025-12-08

### Added

- Initial release of mongosh - MongoDB Shell in Rust
- Interactive REPL mode with history and multi-line support
- Script execution (`--file`, `--eval`)
- MongoDB connection management with auto-reconnection
- CRUD, admin, and utility commands
- Multiple output formats (JSON, table)
- Configuration system and plugin architecture
- TLS/SSL and authentication support
