//! Format writers for export operations
//!
//! This module provides a unified interface for writing documents to different
//! file formats (JSON Lines, CSV, Excel, etc.).

use async_trait::async_trait;
use mongodb::bson::Document;
use std::path::Path;
use tokio::fs::File;
use tokio::io::BufWriter;

use crate::error::Result;

pub mod jsonl;
pub mod csv;

pub use jsonl::JsonLWriter;
pub use csv::CsvWriter;

/// Trait for writing documents to different file formats
#[async_trait]
pub trait FormatWriter: Send {
    /// Write a batch of documents
    ///
    /// # Arguments
    /// * `docs` - Slice of documents to write
    ///
    /// # Returns
    /// * `Result<usize>` - Number of documents written
    async fn write_batch(&mut self, docs: &[Document]) -> Result<usize>;

    /// Finalize the output (flush buffers, write footers, etc.)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn finalize(&mut self) -> Result<()>;

    /// Get the current file size in bytes (if applicable)
    ///
    /// # Returns
    /// * `Result<u64>` - File size in bytes
    async fn file_size(&self) -> Result<u64>;
}

/// Helper function to create a buffered file writer
///
/// # Arguments
/// * `path` - File path to create
///
/// # Returns
/// * `Result<BufWriter<File>>` - Buffered writer or error
pub(crate) async fn create_writer(path: &str) -> Result<BufWriter<File>> {
    let file = File::create(path).await.map_err(|e| {
        crate::error::ExecutionError::InvalidOperation(format!("Failed to create file: {}", e))
    })?;
    Ok(BufWriter::with_capacity(8 * 1024 * 1024, file)) // 8MB buffer
}

/// Helper function to validate file path and directory
///
/// # Arguments
/// * `path` - File path to validate
///
/// # Returns
/// * `Result<()>` - Success or error
pub(crate) fn validate_path(path: &str) -> Result<()> {
    let path_obj = Path::new(path);

    // Check if parent directory exists
    if let Some(parent) = path_obj.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(crate::error::ExecutionError::InvalidOperation(format!(
                "Directory does not exist: {}",
                parent.display()
            ))
            .into());
        }
    }

    Ok(())
}
