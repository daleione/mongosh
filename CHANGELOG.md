# Changelog

All notable changes to this project will be documented in this file.

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
