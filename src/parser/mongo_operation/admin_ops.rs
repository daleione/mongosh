//! Admin operation parsers for MongoDB
//!
//! This module contains parsers for all admin-related MongoDB operations:
//! - Index operations (create, drop, list)
//! - Collection operations (drop, rename, stats)

use mongodb::bson::Document;

use crate::error::{ParseError, Result};
use crate::parser::command::{AdminCommand, Command};
use crate::parser::mongo_ast::*;

use super::args::ArgParser;

/// Admin operation parsers
pub struct AdminOpsParser;

impl AdminOpsParser {
    /// Parse getIndexes operation
    pub fn parse_get_indexes(collection: &str) -> Result<Command> {
        Ok(Command::Admin(AdminCommand::ListIndexes(
            collection.to_string(),
        )))
    }

    /// Parse createIndex operation
    pub fn parse_create_index(collection: &str, args: &[Expr]) -> Result<Command> {
        let keys = ArgParser::get_doc_arg(args, 0)?;

        // Get options if provided
        let options = if args.len() > 1 {
            Some(ArgParser::get_doc_arg(args, 1)?)
        } else {
            None
        };

        // Build index spec
        let mut index_spec = Document::new();
        index_spec.insert("key", keys);

        if let Some(opts) = options {
            // Merge options into index spec
            for (key, value) in opts {
                index_spec.insert(key, value);
            }
        }

        Ok(Command::Admin(AdminCommand::CreateIndexes {
            collection: collection.to_string(),
            indexes: vec![index_spec],
        }))
    }

    /// Parse createIndexes operation
    pub fn parse_create_indexes(collection: &str, args: &[Expr]) -> Result<Command> {
        let indexes = ArgParser::get_doc_array_arg(args, 0)?;

        Ok(Command::Admin(AdminCommand::CreateIndexes {
            collection: collection.to_string(),
            indexes,
        }))
    }

    /// Parse dropIndex operation
    pub fn parse_drop_index(collection: &str, args: &[Expr]) -> Result<Command> {
        let index = ArgParser::get_string_arg(args, 0)?;

        Ok(Command::Admin(AdminCommand::DropIndex {
            collection: collection.to_string(),
            index,
        }))
    }

    /// Parse dropIndexes operation
    pub fn parse_drop_indexes(collection: &str) -> Result<Command> {
        Ok(Command::Admin(AdminCommand::DropIndex {
            collection: collection.to_string(),
            index: "*".to_string(),
        }))
    }

    /// Parse drop collection operation
    pub fn parse_drop_collection(collection: &str) -> Result<Command> {
        Ok(Command::Admin(AdminCommand::DropCollection(
            collection.to_string(),
        )))
    }

    /// Parse rename collection operation
    pub fn parse_rename_collection(collection: &str, args: &[Expr]) -> Result<Command> {
        // renameCollection(target, dropTarget)
        // target is required (string)
        // dropTarget is optional (boolean, defaults to false)

        if args.is_empty() {
            return Err(ParseError::InvalidCommand(
                "renameCollection() requires at least 1 argument: target name".to_string(),
            )
            .into());
        }

        if args.len() > 2 {
            return Err(ParseError::InvalidCommand(
                format!("renameCollection() expects at most 2 arguments, got {}", args.len()),
            )
            .into());
        }

        // Parse target name (required)
        let target = match &args[0] {
            Expr::String(s) => s.clone(),
            _ => {
                return Err(ParseError::InvalidCommand(
                    "renameCollection() target must be a string".to_string(),
                )
                .into());
            }
        };

        // Parse dropTarget (optional, defaults to false)
        let drop_target = if args.len() > 1 {
            match &args[1] {
                Expr::Boolean(b) => *b,
                _ => {
                    return Err(ParseError::InvalidCommand(
                        "renameCollection() dropTarget must be a boolean".to_string(),
                    )
                    .into());
                }
            }
        } else {
            false
        };

        Ok(Command::Admin(AdminCommand::RenameCollection {
            collection: collection.to_string(),
            target,
            drop_target,
        }))
    }

    /// Parse collection stats operation
    pub fn parse_collection_stats(collection: &str, args: &[Expr]) -> Result<Command> {
        // stats() can be called with no arguments, a scale number, or an options document
        let scale = if args.is_empty() {
            None
        } else if args.len() == 1 {
            match &args[0] {
                // Legacy format: db.collection.stats(1024)
                Expr::Number(n) => Some(*n as i32),
                // New format: db.collection.stats({scale: 1024, indexDetails: true, ...})
                Expr::Object(_) => {
                    let options_doc = ArgParser::get_doc_arg(args, 0)?;
                    options_doc
                        .get_i32("scale")
                        .ok()
                        .or_else(|| options_doc.get_i64("scale").ok().map(|v| v as i32))
                }
                _ => {
                    return Err(ParseError::InvalidCommand(
                        "stats() argument must be a number or options document".to_string(),
                    )
                    .into());
                }
            }
        } else {
            return Err(ParseError::InvalidCommand(
                format!("stats() expects at most 1 argument, got {}", args.len()),
            )
            .into());
        };

        Ok(Command::Admin(AdminCommand::CollectionStats {
            collection: collection.to_string(),
            scale,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mongo_operation::DbOperationParser;
    use crate::parser::command::AdminCommand;

    #[test]
    fn test_parse_get_indexes() {
        let result = DbOperationParser::parse("db.users.getIndexes()");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_create_index() {
        let result = DbOperationParser::parse("db.users.createIndex({ email: 1 })");
        assert!(result.is_ok());
        if let Ok(Command::Admin(cmd)) = result {
            assert!(matches!(cmd, AdminCommand::CreateIndexes { .. }));
        }
    }

    #[test]
    fn test_parse_create_index_with_options() {
        let result = DbOperationParser::parse(
            "db.users.createIndex({ email: 1 }, { unique: true, name: 'email_idx' })"
        );
        assert!(result.is_ok());
        if let Ok(Command::Admin(AdminCommand::CreateIndexes { indexes, .. })) = result {
            assert_eq!(indexes.len(), 1);
            assert!(indexes[0].contains_key("unique"));
        }
    }

    #[test]
    fn test_parse_create_indexes() {
        let result = DbOperationParser::parse("db.users.createIndexes([{ key: { name: 1 } }])");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_drop_index() {
        let result = DbOperationParser::parse("db.users.dropIndex('email_1')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_drop_indexes() {
        let result = DbOperationParser::parse("db.users.dropIndexes()");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_drop_collection() {
        let result = DbOperationParser::parse("db.users.drop()");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_rename_collection() {
        let result = DbOperationParser::parse("db.users.renameCollection('customers')");
        assert!(result.is_ok());
        if let Ok(Command::Admin(AdminCommand::RenameCollection { target, drop_target, .. })) = result {
            assert_eq!(target, "customers");
            assert!(!drop_target);
        }
    }

    #[test]
    fn test_parse_rename_collection_with_drop_target() {
        let result = DbOperationParser::parse("db.users.renameCollection('customers', true)");
        assert!(result.is_ok());
        if let Ok(Command::Admin(AdminCommand::RenameCollection { drop_target, .. })) = result {
            assert!(drop_target);
        }
    }

    #[test]
    fn test_parse_rename_collection_with_drop_target_false() {
        let result = DbOperationParser::parse("db.users.renameCollection('customers', false)");
        assert!(result.is_ok());
        if let Ok(Command::Admin(AdminCommand::RenameCollection { drop_target, .. })) = result {
            assert!(!drop_target);
        }
    }

    #[test]
    fn test_parse_collection_stats_no_args() {
        let result = DbOperationParser::parse("db.users.stats()");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_collection_stats_with_scale_number() {
        let result = DbOperationParser::parse("db.users.stats(1024)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_collection_stats_with_options() {
        let result = DbOperationParser::parse("db.users.stats({ scale: 1024 })");
        assert!(result.is_ok());
    }
}
