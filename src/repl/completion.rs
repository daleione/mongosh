use std::fmt;

/// Command completer for simple, static command auto-completion.
///
/// This type is intentionally minimal and self‑contained. It does not depend on
/// the rest of the REPL infrastructure and can be used independently anywhere
/// a list of shell‑style completions is useful.
#[derive(Clone, Debug)]
pub struct CommandCompleter {
    /// Available commands that can be suggested as completions.
    commands: Vec<String>,
}

impl CommandCompleter {
    /// Create a new command completer with the provided list of commands.
    ///
    /// # Arguments
    ///
    /// * `commands` - A collection of command strings to be used for completion.
    pub fn with_commands<I, S>(commands: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            commands: commands.into_iter().map(Into::into).collect(),
        }
    }

    /// Create a new command completer with a built‑in default command set.
    ///
    /// This mirrors typical MongoDB shell commands and some REPL meta‑commands.
    pub fn new() -> Self {
        Self::with_commands([
            "show dbs",
            "show databases",
            "show collections",
            "show users",
            "use",
            "exit",
            "quit",
            "help",
        ])
    }

    /// Get completions for a partial input string.
    ///
    /// The implementation is intentionally simple: it returns all commands that
    /// start with the provided `partial` prefix.
    ///
    /// # Arguments
    ///
    /// * `partial` - Partial input string to match against the available commands.
    ///
    /// # Returns
    ///
    /// A vector of matching command strings.
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(partial))
            .cloned()
            .collect()
    }

    /// Expose an immutable view of the underlying commands list.
    pub fn commands(&self) -> &[String] {
        &self.commands
    }
}

impl Default for CommandCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CommandCompleter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "CommandCompleter {{")?;
        for cmd in &self.commands {
            writeln!(f, "  - {cmd}")?;
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::CommandCompleter;

    #[test]
    fn default_completer_contains_basic_commands() {
        let completer = CommandCompleter::default();
        let commands = completer.commands();

        assert!(commands.iter().any(|c| c == "show dbs"));
        assert!(commands.iter().any(|c| c == "show databases"));
        assert!(commands.iter().any(|c| c == "show collections"));
        assert!(commands.iter().any(|c| c == "use"));
        assert!(commands.iter().any(|c| c == "help"));
    }

    #[test]
    fn get_completions_prefix_match() {
        let completer = CommandCompleter::default();

        let completions = completer.get_completions("show");
        assert!(!completions.is_empty());
        assert!(completions.iter().all(|c| c.starts_with("show")));
    }

    #[test]
    fn get_completions_custom_commands() {
        let completer = CommandCompleter::with_commands(["alpha", "beta", "gamma"]);

        let completions = completer.get_completions("b");
        assert_eq!(completions, vec!["beta".to_string()]);

        let none = completer.get_completions("z");
        assert!(none.is_empty());
    }

    #[test]
    fn commands_accessor_exposes_all() {
        let completer = CommandCompleter::with_commands(["one", "two"]);
        let cmds = completer.commands();
        assert_eq!(cmds.len(), 2);
        assert!(cmds.contains(&"one".to_string()));
        assert!(cmds.contains(&"two".to_string()));
    }
}
