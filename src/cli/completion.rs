//! Shell completion generation for mongosh
//!
//! This module provides functionality to generate shell completion scripts
//! for bash, zsh, fish, and PowerShell, with dynamic completion for datasource names.

use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::io;

use crate::cli::CliArgs;
use crate::config::Config;
use crate::error::{ConfigError, MongoshError, Result};

/// Generate shell completion script
///
/// # Arguments
/// * `shell_name` - Shell type (bash, zsh, fish, powershell)
///
/// # Returns
/// * `Result<()>` - Success or error
pub fn generate_completion(shell_name: &str) -> Result<()> {
    let shell = parse_shell(shell_name)?;

    match shell {
        Shell::Bash => generate_bash_completion(),
        Shell::Zsh => generate_zsh_completion(),
        Shell::Fish => generate_fish_completion(),
        _ => Err(MongoshError::Config(ConfigError::Generic(format!(
            "Unsupported shell. Supported shells: bash, zsh, fish"
        )))),
    }
}

/// Parse shell name string to Shell enum
fn parse_shell(shell_name: &str) -> Result<Shell> {
    match shell_name.to_lowercase().as_str() {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "fish" => Ok(Shell::Fish),
        _ => Err(MongoshError::Config(ConfigError::Generic(format!(
            "Unsupported shell: {}. Supported shells: bash, zsh, fish",
            shell_name
        )))),
    }
}

/// Generate Bash completion with dynamic datasource support
fn generate_bash_completion() -> Result<()> {
    let mut cmd = CliArgs::command();
    let mut buffer = Vec::new();
    generate(Shell::Bash, &mut cmd, "mongosh", &mut buffer);

    let basic_completion = String::from_utf8_lossy(&buffer);

    // Add custom datasource completion
    let custom_completion = format!(
        r#"{}

# Custom completion for datasource names
_mongosh_list_datasources() {{
    # Use mongosh itself to list datasources
    mongosh config --list-datasources 2>/dev/null
}}

# Override completion for -d/--datasource flag
_mongosh_complete_datasource() {{
    local cur="$1"
    local datasources=$(_mongosh_list_datasources)
    if [ -n "$datasources" ]; then
        COMPREPLY=($(compgen -W "$datasources" -- "$cur"))
    fi
}}

# Enhance the completion function
_mongosh_enhanced() {{
    local cur prev words cword
    _init_completion || return

    # Check if previous word is -d or --datasource
    if [[ "$prev" == "-d" || "$prev" == "--datasource" ]]; then
        _mongosh_complete_datasource "$cur"
        return 0
    fi

    # Fall back to default completion
    _mongosh "$@"
}}

# Replace the completion function
complete -F _mongosh_enhanced mongosh
"#,
        basic_completion
    );

    print!("{}", custom_completion);
    Ok(())
}

/// Generate Zsh completion with dynamic datasource support
fn generate_zsh_completion() -> Result<()> {
    let mut cmd = CliArgs::command();
    let mut buffer = Vec::new();
    generate(Shell::Zsh, &mut cmd, "mongosh", &mut buffer);

    let basic_completion = String::from_utf8_lossy(&buffer);

    // Add custom datasource completion for zsh
    let custom_completion = format!(
        r#"{}

# Custom completion for datasource names
_mongosh_list_datasources() {{
    # Use mongosh itself to list datasources
    mongosh config --list-datasources 2>/dev/null
}}

# Datasource completion function
_mongosh_datasources() {{
    local -a datasources
    datasources=($(_mongosh_list_datasources))
    _describe 'datasources' datasources
}}

# Get original mongosh completion function
_mongosh_original() {{
    _mongosh "$@"
}}

# Enhanced completion function
_mongosh_enhanced() {{
    local curcontext="$curcontext" state line
    typeset -A opt_args

    # Check if we're completing the datasource argument
    if [[ ${{words[CURRENT-1]}} == "-d" || ${{words[CURRENT-1]}} == "--datasource" ]]; then
        _mongosh_datasources
        return 0
    fi

    # Otherwise use original completion
    _mongosh_original "$@"
}}

# Replace the completion function
compdef _mongosh_enhanced mongosh
"#,
        basic_completion
    );

    print!("{}", custom_completion);
    Ok(())
}

/// Generate Fish completion with dynamic datasource support
fn generate_fish_completion() -> Result<()> {
    let mut cmd = CliArgs::command();
    let mut buffer = Vec::new();
    generate(Shell::Fish, &mut cmd, "mongosh", &mut buffer);

    let basic_completion = String::from_utf8_lossy(&buffer);

    // Add custom datasource completion for fish
    let custom_completion = format!(
        r#"{}

# Custom completion for datasource names
function __mongosh_list_datasources
    # Use mongosh itself to list datasources
    mongosh config --list-datasources 2>/dev/null
end

# Add dynamic completion for -d/--datasource
complete -c mongosh -s d -l datasource -f -a "(__mongosh_list_datasources)" -d "Datasource name from config file"
"#,
        basic_completion
    );

    print!("{}", custom_completion);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shell() {
        assert!(matches!(parse_shell("bash"), Ok(Shell::Bash)));
        assert!(matches!(parse_shell("zsh"), Ok(Shell::Zsh)));
        assert!(matches!(parse_shell("fish"), Ok(Shell::Fish)));
        assert!(parse_shell("invalid").is_err());
    }

    #[test]
    fn test_parse_shell_case_insensitive() {
        assert!(matches!(parse_shell("BASH"), Ok(Shell::Bash)));
        assert!(matches!(parse_shell("Zsh"), Ok(Shell::Zsh)));
        assert!(matches!(parse_shell("FiSh"), Ok(Shell::Fish)));
    }
}
