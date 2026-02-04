//! User confirmation for dangerous operations
//!
//! This module provides functionality to prompt users for confirmation
//! before executing potentially dangerous operations like delete, update, or drop.

use std::io::{self, Write};

use crate::error::{MongoshError, Result};
use crate::parser::{AdminCommand, QueryCommand};

/// Check if a query command is dangerous and requires confirmation
pub fn is_dangerous_query(cmd: &QueryCommand) -> bool {
    matches!(
        cmd,
        QueryCommand::DeleteOne { .. }
            | QueryCommand::DeleteMany { .. }
            | QueryCommand::UpdateOne { .. }
            | QueryCommand::UpdateMany { .. }
            | QueryCommand::ReplaceOne { .. }
            | QueryCommand::FindOneAndDelete { .. }
            | QueryCommand::FindOneAndUpdate { .. }
            | QueryCommand::FindOneAndReplace { .. }
            | QueryCommand::FindAndModify { .. }
    )
}

/// Check if an admin command is dangerous and requires confirmation
pub fn is_dangerous_admin(cmd: &AdminCommand) -> bool {
    matches!(
        cmd,
        AdminCommand::CreateIndex { .. }
            | AdminCommand::CreateIndexes { .. }
            | AdminCommand::DropIndex { .. }
            | AdminCommand::DropIndexes { .. }
            | AdminCommand::DropCollection(..)
            | AdminCommand::RenameCollection { .. }
    )
}

/// Get a description of what the dangerous operation will do
pub fn get_operation_description(cmd: &QueryCommand) -> String {
    match cmd {
        QueryCommand::DeleteOne { collection, .. } => {
            format!(
                "This will DELETE ONE document from collection '{}'",
                collection
            )
        }
        QueryCommand::DeleteMany { collection, .. } => {
            format!(
                "This will DELETE MULTIPLE documents from collection '{}'",
                collection
            )
        }
        QueryCommand::UpdateOne { collection, .. } => {
            format!(
                "This will UPDATE ONE document in collection '{}'",
                collection
            )
        }
        QueryCommand::UpdateMany { collection, .. } => {
            format!(
                "This will UPDATE MULTIPLE documents in collection '{}'",
                collection
            )
        }
        QueryCommand::ReplaceOne { collection, .. } => {
            format!(
                "This will REPLACE ONE document in collection '{}'",
                collection
            )
        }
        QueryCommand::FindOneAndDelete { collection, .. } => {
            format!(
                "This will find and DELETE one document from collection '{}'",
                collection
            )
        }
        QueryCommand::FindOneAndUpdate { collection, .. } => {
            format!(
                "This will find and UPDATE one document in collection '{}'",
                collection
            )
        }
        QueryCommand::FindOneAndReplace { collection, .. } => {
            format!(
                "This will find and REPLACE one document in collection '{}'",
                collection
            )
        }
        QueryCommand::FindAndModify {
            collection, remove, ..
        } => {
            if *remove {
                format!(
                    "This will find and DELETE one document from collection '{}'",
                    collection
                )
            } else {
                format!(
                    "This will find and MODIFY one document in collection '{}'",
                    collection
                )
            }
        }
        _ => "Perform operation".to_string(),
    }
}

/// Get a description of what the dangerous admin operation will do
pub fn get_admin_description(cmd: &AdminCommand) -> String {
    match cmd {
        AdminCommand::CreateIndex { collection, .. } => {
            format!("This will CREATE an INDEX on collection '{}'", collection)
        }
        AdminCommand::CreateIndexes {
            collection,
            indexes,
        } => {
            format!(
                "This will CREATE {} INDEXES on collection '{}'",
                indexes.len(),
                collection
            )
        }
        AdminCommand::DropIndex { collection, index } => {
            format!(
                "This will DROP INDEX '{}' from collection '{}'",
                index, collection
            )
        }
        AdminCommand::DropIndexes {
            collection,
            indexes,
        } => match indexes {
            Some(names) => format!(
                "This will DROP {} INDEXES from collection '{}'",
                names.len(),
                collection
            ),
            None => format!(
                "This will DROP ALL INDEXES from collection '{}'",
                collection
            ),
        },
        AdminCommand::DropCollection(collection) => {
            format!("This will DROP the entire collection '{}'", collection)
        }
        AdminCommand::RenameCollection {
            collection,
            target,
            drop_target,
        } => {
            if *drop_target {
                format!(
                    "This will RENAME collection '{}' to '{}' and DROP the target if it exists",
                    collection, target
                )
            } else {
                format!(
                    "This will RENAME collection '{}' to '{}'",
                    collection, target
                )
            }
        }
        _ => "Perform administrative operation".to_string(),
    }
}

/// Prompt user for confirmation
///
/// # Arguments
/// * `operation_desc` - Description of the operation to perform
///
/// # Returns
/// * `Result<bool>` - True if user confirmed, false if cancelled, error on I/O failure
pub fn prompt_confirmation(operation_desc: &str) -> Result<bool> {
    println!("⚠️ WARNING: Dangerous operation!");
    println!("   {}", operation_desc);
    print!("   Continue? (yes/no): ");
    io::stdout()
        .flush()
        .map_err(|e| MongoshError::Generic(format!("Failed to flush stdout: {}", e)))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| MongoshError::Generic(format!("Failed to read input: {}", e)))?;

    let input = input.trim().to_lowercase();
    Ok(matches!(input.as_str(), "yes" | "y"))
}

/// Confirm a dangerous query operation
///
/// # Arguments
/// * `cmd` - Query command to check and confirm
///
/// # Returns
/// * `Result<bool>` - True if confirmed or not dangerous, false if cancelled
pub fn confirm_query_operation(cmd: &QueryCommand) -> Result<bool> {
    if !is_dangerous_query(cmd) {
        return Ok(true);
    }

    let description = get_operation_description(cmd);
    prompt_confirmation(&description)
}

/// Confirm a dangerous admin operation
///
/// # Arguments
/// * `cmd` - Admin command to check and confirm
///
/// # Returns
/// * `Result<bool>` - True if confirmed or not dangerous, false if cancelled
pub fn confirm_admin_operation(cmd: &AdminCommand) -> Result<bool> {
    if !is_dangerous_admin(cmd) {
        return Ok(true);
    }

    let description = get_admin_description(cmd);
    prompt_confirmation(&description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn test_is_dangerous_query() {
        let delete_one = QueryCommand::DeleteOne {
            collection: "test".to_string(),
            filter: doc! {},
        };
        assert!(is_dangerous_query(&delete_one));

        let find = QueryCommand::Find {
            collection: "test".to_string(),
            filter: doc! {},
            options: Default::default(),
        };
        assert!(!is_dangerous_query(&find));
    }

    #[test]
    fn test_get_operation_description() {
        let delete_many = QueryCommand::DeleteMany {
            collection: "users".to_string(),
            filter: doc! {},
        };
        let desc = get_operation_description(&delete_many);
        assert!(desc.contains("DELETE MULTIPLE"));
        assert!(desc.contains("users"));
    }

    #[test]
    fn test_is_dangerous_admin() {
        let create_index = AdminCommand::CreateIndex {
            collection: "users".to_string(),
            keys: doc! { "email": 1 },
            options: None,
        };
        assert!(is_dangerous_admin(&create_index));

        let show_dbs = AdminCommand::ShowDatabases;
        assert!(!is_dangerous_admin(&show_dbs));
    }

    #[test]
    fn test_get_admin_description() {
        let create_indexes = AdminCommand::CreateIndexes {
            collection: "users".to_string(),
            indexes: vec![doc! {}, doc! {}],
        };
        let desc = get_admin_description(&create_indexes);
        assert!(desc.contains("CREATE 2 INDEXES"));
        assert!(desc.contains("users"));
    }
}
