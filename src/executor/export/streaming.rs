//! Streaming query abstractions for export operations
//!
//! This module provides a unified interface for streaming documents from different
//! query types (Find, Aggregate, etc.) without loading all results into memory.

use async_trait::async_trait;
use futures::TryStreamExt;
use mongodb::bson::Document;
use mongodb::Cursor;
use tracing::{debug, info};

use crate::error::Result;

/// Trait for streaming query results in batches
///
/// This provides a unified interface for different query types to stream
/// documents for export operations.
#[async_trait]
pub trait StreamingQuery: Send {
    /// Fetch the next batch of documents
    ///
    /// # Returns
    /// * `Result<Option<Vec<Document>>>` - Next batch of documents, or None if exhausted
    async fn next_batch(&mut self) -> Result<Option<Vec<Document>>>;

    /// Close the query and cleanup resources
    async fn close(&mut self) -> Result<()>;
}

/// Generic cursor-based streaming query implementation
///
/// This implementation works for both Find and Aggregate operations,
/// eliminating code duplication.
pub struct CursorStreamingQuery {
    cursor: Option<Cursor<Document>>,
    batch_size: u32,
    total_fetched: u64,
    query_type: &'static str,
    closed: bool,
}

impl CursorStreamingQuery {
    /// Create a new cursor streaming query
    ///
    /// # Arguments
    /// * `cursor` - MongoDB cursor from find or aggregate operation
    /// * `batch_size` - Number of documents to fetch per batch
    /// * `query_type` - Type of query for logging ("Find" or "Aggregate")
    pub fn new(cursor: Cursor<Document>, batch_size: u32, query_type: &'static str) -> Self {
        Self {
            cursor: Some(cursor),
            batch_size,
            total_fetched: 0,
            query_type,
            closed: false,
        }
    }
}

#[async_trait]
impl StreamingQuery for CursorStreamingQuery {
    async fn next_batch(&mut self) -> Result<Option<Vec<Document>>> {
        // Check if cursor is already closed
        if self.closed {
            return Ok(None);
        }

        let cursor = match self.cursor.as_mut() {
            Some(c) => c,
            None => return Ok(None),
        };

        let mut batch = Vec::with_capacity(self.batch_size as usize);

        for _ in 0..self.batch_size {
            match cursor.try_next().await {
                Ok(Some(doc)) => batch.push(doc),
                Ok(None) => break,
                Err(e) => {
                    // On error, close cursor to release resources
                    self.cursor = None;
                    self.closed = true;
                    return Err(e.into());
                }
            }
        }

        if batch.is_empty() {
            debug!(
                "{} streaming query exhausted after {} documents",
                self.query_type, self.total_fetched
            );
            // No more documents, close cursor
            self.cursor = None;
            self.closed = true;
            Ok(None)
        } else {
            self.total_fetched += batch.len() as u64;
            debug!(
                "Fetched batch of {} documents (total: {})",
                batch.len(),
                self.total_fetched
            );
            Ok(Some(batch))
        }
    }

    async fn close(&mut self) -> Result<()> {
        if !self.closed {
            // Explicitly drop cursor to release server resources
            self.cursor = None;
            self.closed = true;
            info!(
                "Closed {} streaming query after fetching {} documents",
                self.query_type, self.total_fetched
            );
        }
        Ok(())
    }
}

impl Drop for CursorStreamingQuery {
    fn drop(&mut self) {
        // Ensure cursor is closed on drop
        if !self.closed {
            debug!("CursorStreamingQuery dropped without explicit close");
            self.cursor = None;
        }
    }
}

// Type aliases for backward compatibility
pub type FindStreamingQuery = CursorStreamingQuery;
pub type AggregateStreamingQuery = CursorStreamingQuery;

// Helper functions to maintain API compatibility
impl FindStreamingQuery {
    /// Create a new Find streaming query
    pub fn new_find(cursor: Cursor<Document>, batch_size: u32) -> Self {
        Self::new(cursor, batch_size, "Find")
    }
}

impl AggregateStreamingQuery {
    /// Create a new Aggregate streaming query
    pub fn new_aggregate(cursor: Cursor<Document>, batch_size: u32) -> Self {
        Self::new(cursor, batch_size, "Aggregate")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Real tests would require MongoDB connection
    // These are placeholder structure tests

    #[test]
    fn test_streaming_query_trait_object() {
        // Verify we can use StreamingQuery as a trait object
        fn _accepts_streaming_query(_query: Box<dyn StreamingQuery>) {}
    }
}
