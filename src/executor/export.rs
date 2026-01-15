//! Export executor for exporting query results to files
//!
//! This module provides functionality to export MongoDB query results to various formats:
//! - JSON Lines (jsonl): One JSON document per line
//! - CSV: Comma-separated values
//! - Excel: Excel spreadsheet (.xlsx)

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use chrono::Local;
use mongodb::bson::Document;
use polars::prelude::*;
use polars_excel_writer::PolarsExcelWriter;
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
            ExportFormat::Excel => Self::export_excel(docs, &filename)?,
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

    /// Export documents to CSV format using polars
    fn export_csv(docs: &[Document], path: &str) -> Result<String> {
        debug!("Exporting {} documents to CSV format", docs.len());

        let df = Self::documents_to_dataframe(docs)?;

        // Write to file
        let mut file = File::create(path).map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to create file: {}", e))
        })?;

        CsvWriter::new(&mut file)
            .include_header(true)
            .with_separator(b',')
            .finish(&mut df.clone())
            .map_err(|e| ExecutionError::InvalidOperation(format!("Failed to write CSV: {}", e)))?;

        Ok(format!(
            "Exported {} documents to {} (csv)",
            docs.len(),
            path
        ))
    }

    /// Export documents to Excel format using polars-excel-writer
    fn export_excel(docs: &[Document], path: &str) -> Result<String> {
        debug!("Exporting {} documents to Excel format", docs.len());

        // Convert documents to DataFrame
        let df = Self::documents_to_dataframe(docs)?;

        // Use polars-excel-writer to create real Excel file
        let mut excel_writer = PolarsExcelWriter::new();

        excel_writer.set_autofit(true);

        // Write the dataframe to Excel
        excel_writer.write_dataframe(&df).map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to write DataFrame: {}", e))
        })?;

        // Save the file to disk
        excel_writer.save(path).map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to save Excel file: {}", e))
        })?;

        Ok(format!(
            "Exported {} documents to {} (excel)",
            docs.len(),
            path
        ))
    }

    /// Convert MongoDB documents to Polars DataFrame
    fn documents_to_dataframe(docs: &[Document]) -> Result<DataFrame> {
        if docs.is_empty() {
            return Err(ExecutionError::InvalidOperation(
                "Cannot create DataFrame from empty document list".to_string(),
            )
            .into());
        }

        // Collect all unique field names
        let mut field_names = std::collections::BTreeSet::new();
        for doc in docs {
            for key in doc.keys() {
                field_names.insert(key.clone());
            }
        }

        let field_names: Vec<String> = field_names.into_iter().collect();

        let mut series_vec = Vec::new();
        let converter = PlainTextConverter::new();

        for field_name in &field_names {
            let values: Vec<String> = docs
                .iter()
                .map(|doc| converter.convert_optional(doc.get(field_name)))
                .collect();

            let series = Series::new(PlSmallStr::from(field_name.as_str()), values);
            series_vec.push(series.into());
        }

        DataFrame::new(series_vec).map_err(|e| {
            ExecutionError::InvalidOperation(format!("Failed to create DataFrame: {}", e)).into()
        })
    }

    /// Get suggested filename for format
    pub fn get_default_filename(format: &ExportFormat) -> String {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        match format {
            ExportFormat::JsonL => format!("export-{}.jsonl", timestamp),
            ExportFormat::Csv => format!("export-{}.csv", timestamp),
            ExportFormat::Excel => format!("export-{}.xlsx", timestamp),
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
                ExportFormat::Excel => "xlsx",
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
    fn test_documents_to_dataframe() {
        let docs = vec![
            doc! { "name": "Alice", "age": 30 },
            doc! { "name": "Bob", "age": 25 },
        ];

        let df = ExportExecutor::documents_to_dataframe(&docs).unwrap();
        assert_eq!(df.height(), 2);
        let name_col = PlSmallStr::from_str("name");
        let age_col = PlSmallStr::from_str("age");
        assert!(df.get_column_names().contains(&&name_col));
        assert!(df.get_column_names().contains(&&age_col));
    }
}
