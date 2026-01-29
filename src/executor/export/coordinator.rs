//! Export coordinator for orchestrating export operations
//!
//! This module provides the main coordinator that brings together streaming queries,
//! progress tracking, and format writing to perform efficient exports.

use std::time::Instant;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::error::Result;

use super::progress::ProgressTracker;
use super::streaming::StreamingQuery;
use super::writers::FormatWriter;

/// Result of an export operation
#[derive(Debug)]
pub struct ExportResult {
    /// Number of documents exported
    pub documents_exported: u64,
    /// File size in bytes
    pub file_size_bytes: u64,
    /// Time taken for export
    pub elapsed_ms: u64,
    /// Whether the export was cancelled
    pub cancelled: bool,
}

/// Coordinator for export operations
///
/// Orchestrates the streaming query, progress tracking, and format writing
/// to perform efficient streaming exports of MongoDB data.
pub struct ExportCoordinator {
    /// Streaming query for fetching documents
    query: Box<dyn StreamingQuery>,
    /// Progress tracker for user feedback
    tracker: ProgressTracker,
    /// Format writer for output
    writer: Box<dyn FormatWriter>,
    /// Cancellation token for aborting export
    cancel_token: Option<CancellationToken>,
}

impl ExportCoordinator {
    /// Create a new export coordinator
    pub fn new(
        query: Box<dyn StreamingQuery>,
        tracker: ProgressTracker,
        writer: Box<dyn FormatWriter>,
    ) -> Self {
        Self {
            query,
            tracker,
            writer,
            cancel_token: None,
        }
    }

    /// Set cancellation token for this export operation
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// Execute the export operation
    ///
    /// This is the main entry point that orchestrates the entire export process:
    /// 1. Initialize the query
    /// 2. Stream documents in batches
    /// 3. Write to output format
    /// 4. Track progress
    /// 5. Finalize and return results
    ///
    /// # Returns
    /// * `Result<ExportResult>` - Export statistics or error
    pub async fn execute(&mut self) -> Result<ExportResult> {
        let start_time = Instant::now();

        // Step 1: Stream and write documents in batches
        info!("Starting export operation");
        let mut exported = 0u64;
        let mut batch_count = 0u32;

        loop {
            // Check for cancellation
            if let Some(ref token) = self.cancel_token {
                if token.is_cancelled() {
                    info!("Export operation cancelled by user");

                    // Finalize what we've written so far
                    let _ = self.writer.finalize().await;
                    let _ = self.query.close().await;
                    self.tracker.finish();

                    // Return success with cancellation info
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let file_size_bytes = self.writer.file_size().await.unwrap_or(0);

                    return Ok(ExportResult {
                        documents_exported: exported,
                        file_size_bytes,
                        elapsed_ms,
                        cancelled: true,
                    });
                }
            }

            debug!("Fetching batch #{}", batch_count + 1);

            match self.query.next_batch().await? {
                Some(docs) => {
                    let count = docs.len();
                    debug!("Received batch of {} documents", count);

                    // Write batch to output
                    self.writer.write_batch(&docs).await?;

                    // Update progress
                    exported += count as u64;
                    self.tracker.update(exported);

                    batch_count += 1;

                    // Log progress periodically
                    if batch_count % 10 == 0 {
                        info!(
                            "Progress: {} documents exported ({} batches)",
                            exported, batch_count
                        );
                    }
                }
                None => {
                    debug!("No more documents available");
                    break;
                }
            }
        }

        // Step 3: Finalize output
        debug!("Finalizing output file");
        self.writer.finalize().await?;

        // Step 4: Close query
        self.query.close().await?;

        // Step 5: Complete progress tracking
        let elapsed_ms = start_time.elapsed().as_millis() as u64;

        // Finish and clear progress bar
        self.tracker.finish();

        // Get file size
        let file_size_bytes = self.writer.file_size().await?;

        info!(
            "Export completed: {} documents, {} bytes, {} ms",
            exported, file_size_bytes, elapsed_ms
        );

        Ok(ExportResult {
            documents_exported: exported,
            file_size_bytes,
            elapsed_ms,
            cancelled: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mongodb::bson::{doc, Document};

    // Mock streaming query for testing
    struct MockStreamingQuery {
        batches: Vec<Vec<Document>>,
        current: usize,
    }

    impl MockStreamingQuery {
        fn new(batches: Vec<Vec<Document>>) -> Self {
            Self { batches, current: 0 }
        }
    }

    #[async_trait]
    impl StreamingQuery for MockStreamingQuery {
        async fn next_batch(&mut self) -> Result<Option<Vec<Document>>> {
            if self.current < self.batches.len() {
                let batch = self.batches[self.current].clone();
                self.current += 1;
                Ok(Some(batch))
            } else {
                Ok(None)
            }
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    // Mock format writer for testing
    struct MockWriter {
        written: Vec<Document>,
    }

    impl MockWriter {
        fn new() -> Self {
            Self {
                written: Vec::new(),
            }
        }
    }

    #[async_trait]
    impl FormatWriter for MockWriter {
        async fn write_batch(&mut self, docs: &[Document]) -> Result<usize> {
            self.written.extend_from_slice(docs);
            Ok(docs.len())
        }

        async fn finalize(&mut self) -> Result<()> {
            Ok(())
        }

        async fn file_size(&self) -> Result<u64> {
            Ok(self.written.len() as u64 * 100) // Mock size
        }
    }

    #[tokio::test]
    async fn test_coordinator_basic() {
        let batches = vec![
            vec![doc! { "id": 1 }, doc! { "id": 2 }],
            vec![doc! { "id": 3 }],
        ];

        let query = Box::new(MockStreamingQuery::new(batches));
        let tracker = ProgressTracker::new(Some(3), false);
        let writer = Box::new(MockWriter::new());

        let mut coordinator = ExportCoordinator::new(query, tracker, writer);
        let result = coordinator.execute().await.unwrap();

        assert_eq!(result.documents_exported, 3);
        // elapsed_ms is u64, so always >= 0
    }

    #[tokio::test]
    async fn test_coordinator_empty_query() {
        let batches: Vec<Vec<Document>> = vec![];

        let query = Box::new(MockStreamingQuery::new(batches));
        let tracker = ProgressTracker::new(None, false);
        let writer = Box::new(MockWriter::new());

        let mut coordinator = ExportCoordinator::new(query, tracker, writer);
        let result = coordinator.execute().await.unwrap();

        assert_eq!(result.documents_exported, 0);
    }
}
