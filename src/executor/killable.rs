//! Killable operations support for mongosh.
//!
//! This module provides infrastructure to gracefully cancel long-running MongoDB
//! operations when the user presses Ctrl+C. It works by:
//!
//! 1. Assigning a unique `comment` to each operation
//! 2. Listening for cancellation signals (via `CancellationToken`)
//! 3. Using `$currentOp` to find the operation by its comment
//! 4. Calling `killOp` on the server to terminate the operation

use crate::error::{ExecutionError, MongoshError, Result};
use bson::{doc, Bson, Document};
use futures::future::BoxFuture;
use mongodb::Client;

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// A handle representing a MongoDB operation that can be killed.
///
/// This handle contains a unique `comment` that will be attached to the
/// MongoDB command. If the operation needs to be cancelled, this comment
/// is used to locate the operation in `$currentOp` and kill it.
#[derive(Debug, Clone)]
pub struct OperationHandle {
    /// Unique comment identifying this operation.
    ///
    /// Format: `mongosh-<client_id>-<uuid>`
    pub comment: String,
}

impl OperationHandle {
    /// Create a new operation handle with a unique comment.
    ///
    /// # Arguments
    /// * `client_id` - Identifier for this mongosh instance (e.g., hostname, session ID)
    ///
    /// # Returns
    /// A new `OperationHandle` with a globally unique comment
    pub fn new(client_id: &str) -> Self {
        Self {
            comment: format!("mongosh-{}-{}", client_id, Uuid::new_v4()),
        }
    }

    /// Get the comment string to be passed to MongoDB command options.
    pub fn comment(&self) -> &str {
        &self.comment
    }
}

/// Helper for killing MongoDB operations by comment.
///
/// This type provides methods to locate and kill server-side operations
/// using the `$currentOp` and `killOp` commands.
pub struct MongoOpKiller {
    client: Client,
}

impl MongoOpKiller {
    /// Create a new `MongoOpKiller`.
    ///
    /// # Arguments
    /// * `client` - MongoDB client with access to the `admin` database
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Attempt to kill an operation identified by its comment.
    ///
    /// This method:
    /// 1. Queries `$currentOp` to find operations matching the comment
    /// 2. Extracts the `opid` from the first matching operation
    /// 3. Calls `killOp` with that `opid`
    ///
    /// # Arguments
    /// * `handle` - The operation handle containing the comment to search for
    ///
    /// # Returns
    /// * `Ok(())` if killOp was attempted (or no matching op was found)
    /// * `Err(...)` if there was a communication error with the server
    ///
    /// # Notes
    /// - If no matching operation is found, this returns `Ok(())` (operation may have already completed)
    /// - This method does not guarantee the operation was successfully killed
    /// - Requires appropriate permissions on the `admin` database
    pub async fn kill_by_comment(&self, handle: &OperationHandle) -> Result<()> {
        let admin_db = self.client.database("admin");

        // Use aggregation with $currentOp to find operations with matching comment
        let pipeline = vec![
            doc! {
                "$currentOp": {
                    "allUsers": true,
                    "localOps": true
                }
            },
            doc! {
                "$match": {
                    "command.comment": &handle.comment
                }
            },
        ];

        // Execute aggregation to find matching operations
        let mut cursor = match admin_db.aggregate(pipeline).await {
            Ok(cursor) => cursor,
            Err(_e) => {
                // If unauthorized or other error, silently return
                // Client-side cancellation still works
                return Ok(());
            }
        };

        // Try to get the first matching operation
        use futures::stream::StreamExt;
        if let Some(result) = cursor.next().await {
            let op_doc = match result {
                Ok(doc) => doc,
                Err(_e) => return Ok(()),
            };

            // Extract opid (can be i32 or i64)
            let opid = match extract_opid(&op_doc) {
                Ok(id) => id,
                Err(_e) => return Ok(()),
            };

            // Call killOp
            let kill_result = admin_db
                .run_command(doc! { "killOp": 1, "op": opid })
                .await;

            // Ignore result - we tried our best
            let _ = kill_result;
        }

        Ok(())
    }
}

/// Extract opid from a $currentOp document.
///
/// The `opid` field can be either i32 or i64 depending on server version.
fn extract_opid(doc: &Document) -> Result<i64> {
    if let Some(opid_bson) = doc.get("opid") {
        match opid_bson {
            Bson::Int32(v) => Ok(*v as i64),
            Bson::Int64(v) => Ok(*v),
            _ => Err(MongoshError::Generic(format!(
                "Unexpected type for opid field: {:?}",
                opid_bson
            ))),
        }
    } else {
        Err(MongoshError::Generic(
            "No opid field found in $currentOp result".to_string(),
        ))
    }
}

/// Execute a MongoDB command with killOp support on cancellation.
///
/// This function wraps command execution with automatic killOp behavior:
/// - If the command completes normally, returns its result
/// - If `cancel_token` is triggered (e.g., by Ctrl+C), attempts to kill
///   the server-side operation and returns a `Cancelled` error
///
/// # Arguments
/// * `client` - MongoDB client
/// * `client_id` - Identifier for this mongosh instance
/// * `cancel_token` - Token that will be triggered on Ctrl+C or other cancellation
/// * `exec_fn` - Async function that executes the actual command. It receives:
///   - `client`: MongoDB client to use
///   - `handle`: Operation handle (caller should pass `handle.comment()` to command options)
///
/// # Returns
/// * `Ok(T)` if command completed successfully
/// * `Err(ExecutionError::Cancelled)` if cancelled by user
/// * `Err(...)` for other errors
///
/// # Example
///
/// ```ignore
/// // See docs/killable_example.rs for complete working examples
/// ```
pub async fn run_killable_command<F, T>(
    client: Client,
    client_id: &str,
    cancel_token: CancellationToken,
    exec_fn: F,
) -> Result<T>
where
    F: FnOnce(Client, OperationHandle) -> BoxFuture<'static, Result<T>>,
{
    let handle = OperationHandle::new(client_id);
    let killer = MongoOpKiller::new(client.clone());

    // Execute the command
    let command_fut = exec_fn(client, handle.clone());

    // Race between command completion and cancellation
    tokio::select! {
        result = command_fut => {
            // Command completed (successfully or with error)
            result
        }
        _ = cancel_token.cancelled() => {
            // User pressed Ctrl+C - attempt to kill the server-side operation
            let _ = killer.kill_by_comment(&handle).await;

            // Return cancellation error
            Err(MongoshError::Execution(ExecutionError::Cancelled(
                "Operation cancelled by user (Ctrl+C)".to_string()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_handle_format() {
        let handle = OperationHandle::new("test-client");
        assert!(handle.comment.starts_with("mongosh-test-client-"));
        assert!(handle.comment.len() > "mongosh-test-client-".len());
    }

    #[test]
    fn test_operation_handle_uniqueness() {
        let handle1 = OperationHandle::new("test");
        let handle2 = OperationHandle::new("test");
        assert_ne!(handle1.comment, handle2.comment);
    }

    #[test]
    fn test_extract_opid_i32() {
        let doc = doc! { "opid": 12345i32 };
        let opid = extract_opid(&doc).unwrap();
        assert_eq!(opid, 12345i64);
    }

    #[test]
    fn test_extract_opid_i64() {
        let doc = doc! { "opid": 9876543210i64 };
        let opid = extract_opid(&doc).unwrap();
        assert_eq!(opid, 9876543210i64);
    }

    #[test]
    fn test_extract_opid_missing() {
        let doc = doc! { "other": "field" };
        let result = extract_opid(&doc);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_opid_wrong_type() {
        let doc = doc! { "opid": "string" };
        let result = extract_opid(&doc);
        assert!(result.is_err());
    }
}
