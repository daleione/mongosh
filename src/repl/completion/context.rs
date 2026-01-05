//! Completion context definitions
//!
//! This module defines the completion context types that represent what kind of
//! completion should be provided based on the current input state.

/// Represents the type of completion needed based on the current context
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionContext {
    /// Complete collection names
    Collection {
        /// Prefix to filter collections
        prefix: String,
    },

    /// Complete operation/method names
    Operation {
        /// Prefix to filter operations
        prefix: String,
    },

    /// Complete "show" subcommands (dbs, databases, collections, tables, users, roles)
    ShowSubcommand {
        /// Prefix to filter subcommands
        prefix: String,
    },

    /// Complete database names
    Database {
        /// Prefix to filter databases
        prefix: String,
    },

    /// Complete top-level commands
    Command {
        /// Prefix to filter commands
        prefix: String,
    },

    /// No completion available
    None,
}

impl CompletionContext {
    /// Create a collection completion context
    #[allow(dead_code)]
    pub fn collection(prefix: impl Into<String>) -> Self {
        Self::Collection {
            prefix: prefix.into(),
        }
    }

    /// Create an operation completion context
    #[allow(dead_code)]
    pub fn operation(prefix: impl Into<String>) -> Self {
        Self::Operation {
            prefix: prefix.into(),
        }
    }

    /// Create a show subcommand completion context
    #[allow(dead_code)]
    pub fn show_subcommand(prefix: impl Into<String>) -> Self {
        Self::ShowSubcommand {
            prefix: prefix.into(),
        }
    }

    /// Create a database completion context
    #[allow(dead_code)]
    pub fn database(prefix: impl Into<String>) -> Self {
        Self::Database {
            prefix: prefix.into(),
        }
    }

    /// Create a command completion context
    #[allow(dead_code)]
    pub fn command(prefix: impl Into<String>) -> Self {
        Self::Command {
            prefix: prefix.into(),
        }
    }

    /// Get the prefix for this context
    #[allow(dead_code)]
    pub fn prefix(&self) -> &str {
        match self {
            Self::Collection { prefix } => prefix,
            Self::Operation { prefix } => prefix,
            Self::ShowSubcommand { prefix } => prefix,
            Self::Database { prefix } => prefix,
            Self::Command { prefix } => prefix,
            Self::None => "",
        }
    }

    /// Check if this is a None context
    #[allow(dead_code)]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_context() {
        let ctx = CompletionContext::collection("us");
        assert_eq!(ctx.prefix(), "us");
        assert!(!ctx.is_none());

        if let CompletionContext::Collection { prefix } = ctx {
            assert_eq!(prefix, "us");
        } else {
            panic!("Expected Collection context");
        }
    }

    #[test]
    fn test_operation_context() {
        let ctx = CompletionContext::operation("fi");
        assert_eq!(ctx.prefix(), "fi");
        assert!(!ctx.is_none());

        if let CompletionContext::Operation { prefix } = ctx {
            assert_eq!(prefix, "fi");
        } else {
            panic!("Expected Operation context");
        }
    }

    #[test]
    fn test_show_subcommand_context() {
        let ctx = CompletionContext::show_subcommand("c");
        assert_eq!(ctx.prefix(), "c");

        if let CompletionContext::ShowSubcommand { prefix } = ctx {
            assert_eq!(prefix, "c");
        } else {
            panic!("Expected ShowSubcommand context");
        }
    }

    #[test]
    fn test_database_context() {
        let ctx = CompletionContext::database("test");
        assert_eq!(ctx.prefix(), "test");

        if let CompletionContext::Database { prefix } = ctx {
            assert_eq!(prefix, "test");
        } else {
            panic!("Expected Database context");
        }
    }

    #[test]
    fn test_command_context() {
        let ctx = CompletionContext::command("sh");
        assert_eq!(ctx.prefix(), "sh");

        if let CompletionContext::Command { prefix } = ctx {
            assert_eq!(prefix, "sh");
        } else {
            panic!("Expected Command context");
        }
    }

    #[test]
    fn test_none_context() {
        let ctx = CompletionContext::None;
        assert_eq!(ctx.prefix(), "");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_context_equality() {
        let ctx1 = CompletionContext::collection("users");
        let ctx2 = CompletionContext::collection("users");
        let ctx3 = CompletionContext::collection("posts");

        assert_eq!(ctx1, ctx2);
        assert_ne!(ctx1, ctx3);
    }
}
