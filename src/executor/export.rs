//! Export executor for exporting query results to files
//!
//! This module provides functionality to export MongoDB query results to various formats:
//! - JSON Lines (jsonl): One JSON document per line
//! - CSV: Comma-separated values

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use chrono::Local;
use mongodb::bson::Document;
use tracing::{debug, info};

use crate::error::{ExecutionError, Result};
use crate::formatter::JsonFormatter;
use crate::formatter::bson_utils::{BsonConverter, PlainTextConverter};
use crate::parser::ExportFormat;

use super::result::ResultData;

/// Export executor for handling file exports
pub struct ExportExecutor;

impl ExportExecutor {
    /// Export result data to a file
    ///
    /// # Arguments
    /// * `data` - Result data to export
    /// * `format` - Export format
    /// * `file` - Optional file path (if None, prints to stdout)
    ///
    /// # Returns
    /// * `Result<String>` - Success message with exported count
    pub fn export(data: &ResultData, format: &ExportFormat, file: Option<&str>) -> Result<String> {
        match data {
            ResultData::Documents(docs) => Self::export_documents(docs, format, file),
            ResultData::DocumentsWithPagination { documents, .. } => {
                Self::export_documents(documents, format, file)
            }
            ResultData::Document(doc) => Self::export_documents(&vec![doc.clone()], format, file),
            _ => Err(ExecutionError::InvalidOperation(
                "Cannot export non-document data. Only query results can be exported.".to_string(),
            )
            .into()),
        }
    }

    /// Export documents to a file
    fn export_documents(
        docs: &[Document],
        format: &ExportFormat,
        file: Option<&str>,
    ) -> Result<String> {
        if docs.is_empty() {
            return Ok("No documents to export".to_string());
        }

        // Generate filename with timestamp if not provided
        let filename = match file {
            Some(f) => f.to_string(),
            None => Self::get_default_filename(format),
        };

        // Validate file path
        Self::validate_file_path(&filename, format)?;

        let count = docs.len();
        let message = match format {
            ExportFormat::JsonL => Self::export_jsonl(docs, &filename)?,
            ExportFormat::Csv => Self::export_csv(docs, &filename)?,
        };

        info!("Exported {} documents", count);
        Ok(message)
    }

    /// Export documents to JSON Lines format
    fn export_jsonl(docs: &[Document], path: &str) -> Result<String> {
        debug!("Exporting {} documents to JSON Lines format", docs.len());

        let file = File::create(path).map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to create file: {}", e))
        })?;
        let mut writer = BufWriter::new(file);

        // Use JsonFormatter to convert BSON to simplified JSON (no MongoDB extended JSON)
        let formatter = JsonFormatter::new(false, false, 2);

        for doc in docs {
            let json = formatter.format_document(doc)?;
            writeln!(writer, "{}", json).map_err(|e| {
                ExecutionError::InvalidOperation(format!("Failed to write to file: {}", e))
            })?;
        }

        writer.flush().map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to flush file: {}", e))
        })?;

        Ok(format!(
            "Exported {} documents to {} (jsonl)",
            docs.len(),
            path
        ))
    }

    /// Export documents to CSV format
    fn export_csv(docs: &[Document], path: &str) -> Result<String> {
        debug!("Exporting {} documents to CSV format", docs.len());

        // Collect all unique field names
        let mut field_names = std::collections::BTreeSet::new();
        for doc in docs {
            for key in doc.keys() {
                field_names.insert(key.clone());
            }
        }

        let field_names: Vec<String> = field_names.into_iter().collect();

        let file = File::create(path).map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to create file: {}", e))
        })?;
        let mut writer = BufWriter::new(file);

        let converter = PlainTextConverter::new();

        // Write header
        let header = field_names.join(",");
        writeln!(writer, "{}", header).map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to write header: {}", e))
        })?;

        // Write data rows
        for doc in docs {
            let values: Vec<String> = field_names
                .iter()
                .map(|field_name| {
                    let value = converter.convert_optional(doc.get(field_name));
                    // Escape CSV values if they contain comma, quote, or newline
                    if value.contains(',') || value.contains('"') || value.contains('\n') {
                        format!("\"{}\"", value.replace('"', "\"\""))
                    } else {
                        value
                    }
                })
                .collect();

            writeln!(writer, "{}", values.join(",")).map_err(|e| {
                ExecutionError::InvalidOperation(format!("Failed to write row: {}", e))
            })?;
        }

        writer.flush().map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to flush file: {}", e))
        })?;

        Ok(format!(
            "Exported {} documents to {} (csv)",
            docs.len(),
            path
        ))
    }

    /// Get suggested filename for format
    pub fn get_default_filename(format: &ExportFormat) -> String {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        match format {
            ExportFormat::JsonL => format!("export-{}.jsonl", timestamp),
            ExportFormat::Csv => format!("export-{}.csv", timestamp),
        }
    }

    /// Validate file path and extension
    pub fn validate_file_path(path: &str, format: &ExportFormat) -> Result<()> {
        let path_obj = Path::new(path);

        // Check if parent directory exists
        if let Some(parent) = path_obj.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(ExecutionError::InvalidOperation(format!(
                    "Directory does not exist: {}",
                    parent.display()
                ))
                .into());
            }
        }

        // Validate extension matches format
        if let Some(ext) = path_obj.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            let expected = match format {
                ExportFormat::JsonL => "jsonl",
                ExportFormat::Csv => "csv",
            };

            if ext_str != expected && ext_str != "json" {
                debug!(
                    "Warning: File extension '{}' does not match format '{}'",
                    ext_str, expected
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn test_export_csv() {
        use std::fs;
        let docs = vec![
            doc! { "name": "Alice", "age": 30 },
            doc! { "name": "Bob", "age": 25 },
        ];

        let path = "test_export.csv";
        let result = ExportExecutor::export_csv(&docs, path);
        assert!(result.is_ok());

        // Clean up
        let _ = fs::remove_file(path);
    }
}
