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

/// Prompt user for confirmation
///
/// # Arguments
/// * `operation_desc` - Description of the operation to perform
///
/// # Returns
/// * `Result<bool>` - True if user confirmed, false if cancelled, error on I/O failure
pub fn prompt_confirmation() -> Result<bool> {
    println!("⚠️ Dangerous operation! Continue? (yes/no): ");
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
    prompt_confirmation()
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
    prompt_confirmation()
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
}
