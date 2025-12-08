# Source Code Structure

This directory contains the Rust implementation of MongoDB Shell (mongosh). The codebase follows a modular architecture with clear separation of concerns.

## Module Overview

### `main.rs`
Application entry point. Handles:
- Command-line argument parsing
- Application initialization
- Mode selection (interactive vs script)
- Main execution loop

### `lib.rs`
Library interface exposing core functionality for external use as a library crate.

## Core Modules

### `cli/`
**Command-Line Interface**
- Argument parsing using `clap`
- Configuration file loading
- Connection URI construction
- Subcommand handling (version, completion, config)

**Key Types:**
- `CliArgs`: Command-line arguments structure
- `CliInterface`: Main CLI handler
- `Commands`: Subcommand definitions

### `config/`
**Configuration Management**
- TOML configuration file support
- Environment variable loading
- Configuration precedence handling
- Default values

**Key Types:**
- `Config`: Main configuration structure
- `ConnectionConfig`: Connection settings
- `DisplayConfig`: Output formatting settings
- `HistoryConfig`: Command history settings
- `LoggingConfig`: Logging configuration
- `PluginConfig`: Plugin system settings

### `connection/`
**MongoDB Connection Management**
- Connection establishment and pooling
- Health checks and monitoring
- Automatic reconnection
- Session management for transactions

**Key Types:**
- `ConnectionManager`: Main connection handler
- `ConnectionState`: Connection status tracking
- `PoolConfig`: Connection pool configuration
- `SessionManager`: Transaction session management

### `error/`
**Error Handling**
- Custom error types for different failure scenarios
- Error conversion implementations
- User-friendly error messages

**Key Types:**
- `MongoshError`: Main error enum
- `ConnectionError`: Connection-specific errors
- `ParseError`: Parsing errors
- `ExecutionError`: Execution errors
- `Result<T>`: Type alias for Result with MongoshError

### `parser/`
**Command and Query Parsing**
- MongoDB shell command parsing
- Query document parsing
- Aggregation pipeline parsing
- Lexical analysis and tokenization

**Key Types:**
- `Parser`: Main parser
- `Lexer`: Tokenizer
- `Command`: Parsed command enum
- `QueryCommand`: CRUD operation commands
- `AdminCommand`: Administrative commands
- `Token`: Lexer token types

### `executor/`
**Command Execution**
- Command routing and dispatch
- CRUD operation execution
- Administrative command execution
- Transaction support

**Key Types:**
- `CommandRouter`: Routes commands to appropriate executors
- `QueryExecutor`: Executes CRUD operations
- `AdminExecutor`: Executes administrative commands
- `UtilityExecutor`: Executes utility commands
- `ExecutionResult`: Command execution results

### `repl/`
**Interactive REPL Engine**
- Read-Eval-Print Loop implementation
- Command history management
- Auto-completion
- Syntax highlighting
- Multi-line input support

**Key Types:**
- `ReplEngine`: Main REPL handler
- `ReplContext`: REPL state and context
- `ReplHelper`: Rustyline helper for completion/hints
- `PromptGenerator`: Prompt string generation

### `formatter/`
**Output Formatting**
- JSON formatting (plain and pretty)
- Table formatting
- Compact output
- Color highlighting

**Key Types:**
- `Formatter`: Main formatter
- `Colorizer`: ANSI color support
- `TableFormatter`: Table output
- `JsonFormatter`: JSON output
- `StatsFormatter`: Statistics display

### `script/`
**Script Execution**
- JavaScript file loading
- Script execution engine
- MongoDB context binding
- Error handling and reporting

**Key Types:**
- `ScriptExecutor`: Main script executor
- `ScriptContext`: Execution context
- `ScriptLoader`: File loading and validation
- `ScriptResult`: Execution results
- `ScriptRuntime`: JavaScript runtime wrapper

### `plugins/`
**Plugin System**
- Plugin trait definition
- Plugin loading and management
- Command registration
- Sandboxed execution

**Key Types:**
- `Plugin`: Plugin trait
- `PluginManager`: Plugin lifecycle management
- `PluginContext`: Plugin execution context
- `PluginMetadata`: Plugin information
- `CommandRegistration`: Plugin command registration

### `utils/`
**Utility Functions**
- String manipulation
- Time and duration utilities
- File system helpers
- Validation functions
- Conversion utilities

**Modules:**
- `string`: String utilities
- `time`: Time/duration functions
- `fs`: File system helpers
- `validate`: Validation functions
- `convert`: Type conversion utilities

## Design Patterns

### Async/Await
All I/O operations use Tokio async runtime for non-blocking execution.

### Error Handling
Uses Result types throughout with custom error types for better error messages.

### Trait-Based Design
Core functionality uses traits (e.g., `Plugin`) for extensibility.

### Separation of Concerns
Each module has a single, well-defined responsibility.

### Dependency Injection
Components receive their dependencies through constructors, making testing easier.

## Module Dependencies

```
main.rs
  ├─> cli
  │    └─> config
  ├─> connection
  │    └─> config
  ├─> repl
  │    ├─> parser
  │    └─> config
  ├─> executor
  │    ├─> parser
  │    └─> connection
  ├─> formatter
  │    ├─> config
  │    └─> executor
  ├─> script
  │    └─> connection
  └─> error (used by all)
```

## Testing

Each module includes unit tests in a `tests` submodule. Run tests with:

```bash
cargo test
```

For integration tests, see the `tests/` directory in the project root.

## Adding New Features

1. **New Command Type**: Add to `parser/mod.rs` Command enum
2. **New Executor**: Implement in `executor/mod.rs`
3. **New Output Format**: Add to `formatter/mod.rs`
4. **New Plugin**: Implement `Plugin` trait in `plugins/mod.rs`

## Code Style

- Follow Rust naming conventions (snake_case for functions, PascalCase for types)
- Use comprehensive doc comments (///) for public APIs
- Include examples in doc comments where helpful
- Keep functions focused and concise
- Use `Result<T>` for operations that can fail
- Prefer `todo!()` for unimplemented functionality with descriptive messages

## Future Enhancements

- JavaScript runtime integration (QuickJS, Deno)
- More output formats (YAML, CSV)
- Enhanced plugin system with sandboxing
- Performance monitoring and profiling
- Distributed tracing support
