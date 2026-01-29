//! JSON Lines writer for export operations
//!
//! This module provides functionality to export MongoDB documents to JSON Lines format,
//! where each line is a complete JSON document.

use async_trait::async_trait;
use mongodb::bson::Document;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use tracing::debug;

use crate::error::{ExecutionError, Result};
use crate::formatter::JsonFormatter;

use super::{create_writer, validate_path, FormatWriter};

/// Writer for JSON Lines format
///
/// JSON Lines format writes one JSON document per line, making it easy to
/// stream and process large datasets.
pub struct JsonLWriter {
    /// Buffered file writer
    writer: BufWriter<File>,
    /// Path to the output file
    path: String,
    /// Number of documents written
    written: usize,
    /// JSON formatter for converting BSON to JSON
    formatter: JsonFormatter,
}

impl JsonLWriter {
    /// Create a new JSON Lines writer
    ///
    /// # Arguments
    /// * `path` - Output file path
    ///
    /// # Returns
    /// * `Result<Self>` - New writer instance or error
    pub async fn new(path: &str) -> Result<Self> {
        validate_path(path)?;
        let writer = create_writer(path).await?;

        debug!("Created JSON Lines writer for: {}", path);

        Ok(Self {
            writer,
            path: path.to_string(),
            written: 0,
            // Use compact JSON format without extended JSON notation
            formatter: JsonFormatter::new(false, false, 0),
        })
    }
}

#[async_trait]
impl FormatWriter for JsonLWriter {
    async fn write_batch(&mut self, docs: &[Document]) -> Result<usize> {
        for doc in docs {
            // Convert BSON document to JSON string
            let json = self.formatter.format_document(doc)?;

            // Write JSON line
            self.writer.write_all(json.as_bytes()).await.map_err(|e| {
                ExecutionError::InvalidOperation(format!("Failed to write to file: {}", e))
            })?;
            self.writer.write_all(b"\n").await.map_err(|e| {
                ExecutionError::InvalidOperation(format!("Failed to write newline: {}", e))
            })?;
        }

        self.written += docs.len();
        debug!("Wrote {} documents to JSON Lines (total: {})", docs.len(), self.written);

        Ok(docs.len())
    }

    async fn finalize(&mut self) -> Result<()> {
        self.writer.flush().await.map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to flush file: {}", e))
        })?;

        debug!("Finalized JSON Lines file: {} ({} documents)", self.path, self.written);
        Ok(())
    }

    async fn file_size(&self) -> Result<u64> {
        let metadata = tokio::fs::metadata(&self.path).await.map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to get file metadata: {}", e))
        })?;
        Ok(metadata.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;
    use tokio::fs;

    #[tokio::test]
    async fn test_jsonl_writer_basic() {
        let path = "test_output.jsonl";
        let mut writer = JsonLWriter::new(path).await.unwrap();

        let docs = vec![
            doc! { "name": "Alice", "age": 30 },
            doc! { "name": "Bob", "age": 25 },
        ];

        let written = writer.write_batch(&docs).await.unwrap();
        assert_eq!(written, 2);

        writer.finalize().await.unwrap();

        // Verify file content
        let content = fs::read_to_string(path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        // Cleanup
        fs::remove_file(path).await.ok();
    }

    #[tokio::test]
    async fn test_jsonl_writer_multiple_batches() {
        let path = "test_batches.jsonl";
        let mut writer = JsonLWriter::new(path).await.unwrap();

        // Write first batch
        let batch1 = vec![doc! { "id": 1 }, doc! { "id": 2 }];
        writer.write_batch(&batch1).await.unwrap();

        // Write second batch
        let batch2 = vec![doc! { "id": 3 }];
        writer.write_batch(&batch2).await.unwrap();

        writer.finalize().await.unwrap();

        // Verify total lines
        let content = fs::read_to_string(path).await.unwrap();
        assert_eq!(content.lines().count(), 3);

        // Cleanup
        fs::remove_file(path).await.ok();
    }

    #[tokio::test]
    async fn test_jsonl_writer_file_size() {
        let path = "test_size.jsonl";
        let mut writer = JsonLWriter::new(path).await.unwrap();

        let docs = vec![doc! { "test": "data" }];
        writer.write_batch(&docs).await.unwrap();
        writer.finalize().await.unwrap();

        let size = writer.file_size().await.unwrap();
        assert!(size > 0);

        // Cleanup
        fs::remove_file(path).await.ok();
    }

    #[tokio::test]
    async fn test_jsonl_writer_invalid_directory() {
        let result = JsonLWriter::new("/nonexistent/directory/file.jsonl").await;
        assert!(result.is_err());
    }
}
