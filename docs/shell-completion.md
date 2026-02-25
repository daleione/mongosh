# Shell Completion

This guide explains how to enable and use shell completion for mongosh command-line arguments, including dynamic completion for datasource names defined in your configuration file.

## Overview

mongosh provides shell completion scripts that enable tab completion for:
- Command-line options and flags
- Subcommands
- Datasource names (from your configuration file)

## Supported Shells

- Bash
- Zsh
- Fish
- PowerShell

## Installation

### Generate Completion Script

Generate the completion script for your shell:

```bash
# Bash
mongosh completion bash > ~/.mongosh-completion.bash

# Zsh
mongosh completion zsh > ~/.mongosh-completion.zsh

# Fish
mongosh completion fish > ~/.config/fish/completions/mongosh.fish

# PowerShell
mongosh completion powershell > $PROFILE
```

### Enable Completion

#### Bash

Add the following line to your `~/.bashrc`:

```bash
source ~/.mongosh-completion.bash
```

Then reload your configuration:

```bash
source ~/.bashrc
```

#### Zsh

Add the following line to your `~/.zshrc`:

```bash
source ~/.mongosh-completion.zsh
```

Then reload your configuration:

```bash
source ~/.zshrc
```

#### Fish

Fish automatically loads completion scripts from `~/.config/fish/completions/`. No additional configuration is needed.

Restart Fish to enable the completion:

```bash
exec fish
```

#### PowerShell

The completion script has been added to your PowerShell profile. Restart PowerShell to enable it.

## Usage

### Command Completion

```bash
# Show all available options
mongosh --<TAB>

# Complete partial option names
mongosh --hos<TAB>        # completes to --host
mongosh --forma<TAB>      # completes to --format
```

### Subcommand Completion

```bash
# Show available subcommands
mongosh <TAB>
# Shows: version  completion  config

# Complete subcommand options
mongosh completion <TAB>
# Shows: bash  zsh  fish  powershell

mongosh config --<TAB>
# Shows: --show  --validate
```

### Datasource Completion

Completion scripts automatically read datasource names from your configuration file and provide intelligent completion.

#### Configuration Example

Create or edit `~/.mongoshrc` or `~/.config/mongosh/config.toml`:

```toml
[connection]

[connection.datasources]
local = "mongodb://localhost:27017"
production = "mongodb://admin:password@prod.example.com:27017"
staging = "mongodb://staging.example.com:27017"
development = "mongodb://dev.example.com:27017"
testing = "mongodb://test.example.com:27017"
```

#### Completion Examples

```bash
# Show all available datasources
mongosh -d <TAB>
# Shows: local  production  staging  development  testing

# Filter datasources by prefix
mongosh -d prod<TAB>
# Completes to: mongosh -d production

mongosh -d dev<TAB>
# Completes to: mongosh -d development

# Works with long option format too
mongosh --datasource st<TAB>
# Completes to: mongosh --datasource staging
```

## Configuration File Locations

The completion script searches for configuration files in the following order:

1. `~/.config/mongosh/config.toml` (preferred)
2. `~/.mongoshrc`
3. `./config.default.toml` (default configuration in mongosh installation directory)

Ensure your configuration file exists and contains a `[connection.datasources]` section for datasource completion to work.

## Troubleshooting

### Completion Not Working

1. **Verify configuration file exists:**
   ```bash
   ls -la ~/.mongoshrc
   # or
   ls -la ~/.config/mongosh/config.toml
   ```

2. **Validate configuration file:**
   ```bash
   mongosh config --validate
   ```

3. **Check datasource definitions:**
   ```bash
   mongosh config --show | grep -A 10 datasources
   ```

4. **Reload shell configuration:**
   ```bash
   # Bash
   source ~/.bashrc
   
   # Zsh
   source ~/.zshrc
   
   # Fish
   exec fish
   ```

### Datasources Not Appearing

If datasource names don't appear in completion:

1. **Check file permissions:**
   ```bash
   chmod 644 ~/.mongoshrc
   ```

2. **Install bash-completion (Bash only):**
   ```bash
   # Ubuntu/Debian
   sudo apt-get install bash-completion
   
   # macOS
   brew install bash-completion@2
   ```

3. **Verify TOML syntax:**
   Ensure your configuration file uses valid TOML syntax:
   ```toml
   [connection.datasources]
   name = "connection-string"
   ```

### Zsh with Oh My Zsh

If using Oh My Zsh, ensure the completion script is sourced after Oh My Zsh is loaded in `~/.zshrc`:

```bash
# Correct order
source $ZSH/oh-my-zsh.sh
source ~/.mongosh-completion.zsh

# Incorrect order (won't work)
source ~/.mongosh-completion.zsh
source $ZSH/oh-my-zsh.sh
```

## Advanced Usage

### Using Custom Configuration Files

If you use a custom configuration file path, specify it with the `-c` or `--config` option:

```bash
mongosh -c /path/to/custom-config.toml -d <TAB>
```

Note: Dynamic completion with custom config paths may require additional shell configuration.

### Listing All Datasources

To see all configured datasources:

```bash
mongosh config --show | grep -A 20 "\[connection.datasources\]"
```

### Updating Datasources

When you add new datasources to your configuration file, completion is updated immediately—no need to regenerate the completion script:

```bash
# Edit configuration
vim ~/.mongoshrc

# Datasources are available immediately
mongosh -d new_<TAB>  # New datasource appears in suggestions
```

## Performance

Completion scripts read the configuration file on each completion request. For small configuration files (< 100 datasources), performance impact is negligible. If you have a large number of datasources, you may experience slight completion delays.

## Examples

### Complete Workflow

```bash
# Install completion
mongosh completion bash > ~/.mongosh-completion.bash
echo "source ~/.mongosh-completion.bash" >> ~/.bashrc
source ~/.bashrc

# Configure datasources
cat >> ~/.mongoshrc << EOF
[connection.datasources]
local = "mongodb://localhost:27017"
prod = "mongodb://prod.example.com:27017"
EOF

# Use completion
mongosh -d <TAB>           # Shows: local  prod
mongosh -d prod<TAB>       # Completes to: mongosh -d prod
mongosh --host <TAB>       # Shows hostname options
mongosh --format <TAB>     # Shows format options
```

## See Also

- [Configuration Guide](./configuration.md)
- [Command-Line Options](./cli-options.md)
- [Datasource Management](./datasources.md)
