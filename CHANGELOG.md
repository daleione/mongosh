# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2025-12-16

### Added

- **AST-based parser using Oxc** - Complete rewrite replacing regex-based parsing
- **Chained method call support** - `.limit()`, `.skip()`, `.sort()`, `.projection()`, `.batchSize()`
- MongoDB-specific type constructors: `ObjectId()`, `ISODate()`, `NumberInt()`, `NumberLong()`, `NumberDecimal()`
- New query commands: `ReplaceOne`, `EstimatedDocumentCount`, `FindOneAndDelete`, `FindOneAndUpdate`, `FindOneAndReplace`, `Distinct`, `BulkWrite`
- Extended admin commands: `ShowUsers`, `ShowRoles`, `ShowProfile`, `ShowLogs`, `ServerStatus`, `CurrentOp`, `KillOp`, etc.
- Enhanced options support for find, update, aggregate, and findAndModify operations

### Changed

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
