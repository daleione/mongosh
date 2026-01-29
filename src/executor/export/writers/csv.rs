//! CSV writer for export operations
//!
//! This module provides functionality to export MongoDB documents to CSV format,
//! with automatic header detection and proper value escaping.

use std::collections::BTreeSet;

use async_trait::async_trait;
use mongodb::bson::Document;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use tracing::debug;

use crate::error::{ExecutionError, Result};
use crate::formatter::bson_utils::{BsonConverter, PlainTextConverter};

use super::{create_writer, validate_path, FormatWriter};

/// Writer for CSV format
///
/// CSV format exports documents as comma-separated values with a header row.
/// Fields are automatically detected from the documents.
pub struct CsvWriter {
    /// Buffered file writer
    writer: BufWriter<File>,
    /// Path to the output file
    path: String,
    /// Column headers (field names)
    headers: Vec<String>,
    /// Whether headers have been written
    headers_written: bool,
    /// Number of documents written
    written: usize,
    /// Converter for BSON to plain text
    converter: PlainTextConverter,
}

impl CsvWriter {
    /// Create a new CSV writer
    ///
    /// # Arguments
    /// * `path` - Output file path
    ///
    /// # Returns
    /// * `Result<Self>` - New writer instance or error
    pub async fn new(path: &str) -> Result<Self> {
        validate_path(path)?;
        let writer = create_writer(path).await?;

        debug!("Created CSV writer for: {}", path);

        Ok(Self {
            writer,
            path: path.to_string(),
            headers: Vec::new(),
            headers_written: false,
            written: 0,
            converter: PlainTextConverter::new(),
        })
    }

    /// Collect headers from a batch of documents
    ///
    /// # Arguments
    /// * `docs` - Documents to scan for field names
    fn collect_headers(&mut self, docs: &[Document]) {
        if !self.headers.is_empty() {
            // Headers already collected, just add any new fields
            let mut new_fields = BTreeSet::new();
            for doc in docs {
                for key in doc.keys() {
                    if !self.headers.contains(key) {
                        new_fields.insert(key.clone());
                    }
                }
            }
            // Append new fields to maintain order
            self.headers.extend(new_fields);
        } else {
            // First time: collect all unique field names
            let mut field_set = BTreeSet::new();
            for doc in docs {
                for key in doc.keys() {
                    field_set.insert(key.clone());
                }
            }
            self.headers = field_set.into_iter().collect();
        }
    }

    /// Write CSV header row
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_headers(&mut self) -> Result<()> {
        let header_line = self.headers.join(",");
        self.writer.write_all(header_line.as_bytes()).await.map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to write headers: {}", e))
        })?;
        self.writer.write_all(b"\n").await.map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to write newline: {}", e))
        })?;
        debug!("Wrote CSV headers: {} fields", self.headers.len());
        Ok(())
    }

    /// Write a single document as a CSV row
    ///
    /// # Arguments
    /// * `doc` - Document to write
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_row(&mut self, doc: &Document) -> Result<()> {
        let values: Vec<String> = self
            .headers
            .iter()
            .map(|field_name| {
                let value = self.converter.convert_optional(doc.get(field_name));
                // Escape CSV values if they contain comma, quote, or newline
                Self::escape_csv_value(&value)
            })
            .collect();

        let row = values.join(",");
        self.writer.write_all(row.as_bytes()).await.map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to write row: {}", e))
        })?;
        self.writer.write_all(b"\n").await.map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to write newline: {}", e))
        })?;

        Ok(())
    }

    /// Escape a CSV value if necessary
    ///
    /// # Arguments
    /// * `value` - Value to escape
    ///
    /// # Returns
    /// * `String` - Escaped value
    fn escape_csv_value(value: &str) -> String {
        if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
            // Wrap in quotes and escape internal quotes by doubling them
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }
}

#[async_trait]
impl FormatWriter for CsvWriter {
    async fn write_batch(&mut self, docs: &[Document]) -> Result<usize> {
        if docs.is_empty() {
            return Ok(0);
        }

        // On first batch, collect headers and write them
        if !self.headers_written {
            self.collect_headers(docs);
            self.write_headers().await?;
            self.headers_written = true;
        } else {
            // Check if there are any new fields
            let old_header_count = self.headers.len();
            self.collect_headers(docs);
            if self.headers.len() > old_header_count {
                // Note: In streaming mode, we can't rewrite headers
                // New fields will be added to the end, and previous rows will have empty values
                debug!(
                    "Warning: Discovered {} new fields in batch, previous rows will have empty values",
                    self.headers.len() - old_header_count
                );
            }
        }

        // Write data rows
        for doc in docs {
            self.write_row(doc).await?;
        }

        self.written += docs.len();
        debug!("Wrote {} documents to CSV (total: {})", docs.len(), self.written);

        Ok(docs.len())
    }

    async fn finalize(&mut self) -> Result<()> {
        self.writer.flush().await.map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to flush file: {}", e))
        })?;

        debug!("Finalized CSV file: {} ({} documents)", self.path, self.written);
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
    async fn test_csv_writer_basic() {
        let path = "test_output.csv";
        let mut writer = CsvWriter::new(path).await.unwrap();

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
        assert_eq!(lines.len(), 3); // header + 2 data rows

        // Cleanup
        fs::remove_file(path).await.ok();
    }

    #[tokio::test]
    async fn test_csv_writer_with_special_characters() {
        let path = "test_special.csv";
        let mut writer = CsvWriter::new(path).await.unwrap();

        let docs = vec![
            doc! { "text": "Hello, world!" },
            doc! { "text": "Quote: \"test\"" },
            doc! { "text": "Newline\ntest" },
        ];

        writer.write_batch(&docs).await.unwrap();
        writer.finalize().await.unwrap();

        let content = fs::read_to_string(path).await.unwrap();
        assert!(content.contains("\"Hello, world!\""));
        assert!(content.contains("\"Quote: \"\"test\"\"\""));

        // Cleanup
        fs::remove_file(path).await.ok();
    }

    #[tokio::test]
    async fn test_csv_writer_multiple_batches() {
        let path = "test_batches.csv";
        let mut writer = CsvWriter::new(path).await.unwrap();

        // First batch
        let batch1 = vec![doc! { "id": 1, "name": "Alice" }];
        writer.write_batch(&batch1).await.unwrap();

        // Second batch
        let batch2 = vec![doc! { "id": 2, "name": "Bob" }];
        writer.write_batch(&batch2).await.unwrap();

        writer.finalize().await.unwrap();

        let content = fs::read_to_string(path).await.unwrap();
        assert_eq!(content.lines().count(), 3); // header + 2 rows

        // Cleanup
        fs::remove_file(path).await.ok();
    }

    #[test]
    fn test_csv_escape_value() {
        assert_eq!(CsvWriter::escape_csv_value("simple"), "simple");
        assert_eq!(CsvWriter::escape_csv_value("with,comma"), "\"with,comma\"");
        assert_eq!(CsvWriter::escape_csv_value("with\"quote"), "\"with\"\"quote\"");
        assert_eq!(CsvWriter::escape_csv_value("with\nnewline"), "\"with\nnewline\"");
    }

    #[tokio::test]
    async fn test_csv_writer_file_size() {
        let path = "test_size.csv";
        let mut writer = CsvWriter::new(path).await.unwrap();

        let docs = vec![doc! { "test": "data" }];
        writer.write_batch(&docs).await.unwrap();
        writer.finalize().await.unwrap();

        let size = writer.file_size().await.unwrap();
        assert!(size > 0);

        // Cleanup
        fs::remove_file(path).await.ok();
    }
}
