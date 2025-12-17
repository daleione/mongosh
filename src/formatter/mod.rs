//! Output formatting and colorization for mongosh
//!
//! This module provides formatting functionality for command execution results.
//!
//! # Supported Formats
//!
//! - **Shell**: MongoDB shell-compatible format with type wrappers (default)
//!   - ObjectId('...'), ISODate('...'), Long('...')
//!   - Pretty-printed nested documents and arrays
//!   - Optional color highlighting
//!
//! - **Json**: Compact single-line JSON
//!   - Minified output without whitespace
//!   - Suitable for logging and piping
//!
//! - **JsonPretty**: Human-readable multi-line JSON
//!   - Indented and formatted
//!   - Suitable for terminal display and debugging
//!
//! - **Table**: ASCII table layout (TODO: full implementation)
//!   - Displays documents as structured tables
//!   - Suitable for comparing multiple documents
//!
//! - **Compact**: Summary format
//!   - Shows only count/summary, not full content
//!   - Example: "5 document(s) returned"
//!
//! # Module Structure
//!
//! - `colorizer`: ANSI color support for terminal output
//! - `shell`: Shell-style formatter (mongosh compatible)
//! - `json`: JSON formatter with BSON type simplification
//! - `table`: Table formatter for document collections
//! - `stats`: Statistics formatter for execution metrics

mod colorizer;
mod json;
mod shell;
mod stats;
mod table;

pub use colorizer::Colorizer;
pub use json::JsonFormatter;
pub use shell::ShellFormatter;
pub use stats::StatsFormatter;
pub use table::TableFormatter;

use mongodb::bson::Document;

use crate::config::OutputFormat;
use crate::error::Result;
use crate::executor::{ExecutionResult, ResultData};

/// Main formatter for execution results
pub struct Formatter {
    /// Output format type
    format_type: OutputFormat,

    /// Colorizer for output highlighting
    colorizer: Colorizer,

    /// Enable colored output
    use_colors: bool,

    /// JSON indentation (number of spaces)
    json_indent: usize,
}

impl Formatter {
    /// Create a new formatter
    ///
    /// # Arguments
    /// * `format_type` - Output format type
    /// * `use_colors` - Enable colored output
    ///
    /// # Returns
    /// * `Self` - New formatter instance
    pub fn new(format_type: OutputFormat, use_colors: bool) -> Self {
        Self {
            format_type,
            colorizer: Colorizer::new(use_colors),
            use_colors,
            json_indent: 2, // Default to 2 spaces
        }
    }

    /// Create a new formatter with custom JSON indentation
    ///
    /// # Arguments
    /// * `format_type` - Output format type
    /// * `use_colors` - Enable colored output
    /// * `json_indent` - Number of spaces for JSON indentation
    ///
    /// # Returns
    /// * `Self` - New formatter instance
    pub fn with_indent(format_type: OutputFormat, use_colors: bool, json_indent: usize) -> Self {
        Self {
            format_type,
            colorizer: Colorizer::new(use_colors),
            use_colors,
            json_indent,
        }
    }

    /// Format execution result according to configured format
    ///
    /// # Arguments
    /// * `result` - Execution result to format
    ///
    /// # Returns
    /// * `Result<String>` - Formatted output or error
    pub fn format(&self, result: &ExecutionResult) -> Result<String> {
        if !result.success {
            return self.format_error(result);
        }

        let output = match self.format_type {
            OutputFormat::Shell => self.format_shell(&result.data)?,
            OutputFormat::Json => self.format_json(&result.data, false)?,
            OutputFormat::JsonPretty => self.format_json(&result.data, true)?,
            OutputFormat::Table => self.format_table(&result.data)?,
            OutputFormat::Compact => self.format_compact(&result.data)?,
        };

        // Append statistics if enabled
        let stats = self.format_stats(result);
        if stats.is_empty() {
            Ok(output)
        } else {
            Ok(format!("{}\n{}", output, stats))
        }
    }

    /// Format result data as Shell format
    ///
    /// # Arguments
    /// * `data` - Result data to format
    ///
    /// # Returns
    /// * `Result<String>` - Shell formatted string or error
    pub fn format_shell(&self, data: &ResultData) -> Result<String> {
        let shell_formatter = ShellFormatter::new(self.use_colors);
        match data {
            ResultData::Documents(docs) => {
                if docs.is_empty() {
                    return Ok("[]".to_string());
                }

                let mut result = String::from("[\n");
                for (i, doc) in docs.iter().enumerate() {
                    let formatted = shell_formatter.format_document(doc);
                    // Indent each document
                    let indented = formatted
                        .lines()
                        .map(|line| format!("  {}", line))
                        .collect::<Vec<_>>()
                        .join("\n");
                    result.push_str(&indented);

                    if i < docs.len() - 1 {
                        result.push_str(",\n");
                    } else {
                        result.push('\n');
                    }
                }
                result.push(']');
                Ok(result)
            }
            ResultData::Document(doc) => Ok(shell_formatter.format_document(doc)),
            ResultData::InsertOne { inserted_id } => Ok(format!(
                "{{\n  acknowledged: true,\n  insertedId: {}\n}}",
                inserted_id
            )),
            ResultData::InsertMany { inserted_ids } => {
                let ids_str = inserted_ids
                    .iter()
                    .enumerate()
                    .map(|(i, id)| format!("    '{}': {}", i, id))
                    .collect::<Vec<_>>()
                    .join(",\n");
                Ok(format!(
                    "{{\n  acknowledged: true,\n  insertedIds: {{\n{}\n  }}\n}}",
                    ids_str
                ))
            }
            ResultData::Update { matched, modified } => Ok(format!(
                "{{\n  acknowledged: true,\n  matchedCount: {},\n  modifiedCount: {}\n}}",
                matched, modified
            )),
            ResultData::Delete { deleted } => Ok(format!(
                "{{\n  acknowledged: true,\n  deletedCount: {}\n}}",
                deleted
            )),
            ResultData::Message(msg) => Ok(msg.clone()),
            ResultData::List(items) => Ok(items.join("\n")),
            ResultData::Count(count) => Ok(format!("{}", count)),
            ResultData::None => Ok("null".to_string()),
        }
    }

    /// Format result data as JSON
    ///
    /// # Arguments
    /// * `data` - Result data to format
    /// * `pretty` - Enable pretty printing
    ///
    /// # Returns
    /// * `Result<String>` - JSON string or error
    pub fn format_json(&self, data: &ResultData, pretty: bool) -> Result<String> {
        let formatter = JsonFormatter::new(pretty, self.use_colors, self.json_indent);
        formatter.format(data)
    }

    /// Format result data as table
    ///
    /// # Arguments
    /// * `data` - Result data to format
    ///
    /// # Returns
    /// * `Result<String>` - Table string or error
    pub fn format_table(&self, data: &ResultData) -> Result<String> {
        let formatter = TableFormatter::new();
        formatter.format(data)
    }

    /// Format result data in compact form
    ///
    /// # Arguments
    /// * `data` - Result data to format
    ///
    /// # Returns
    /// * `Result<String>` - Compact string or error
    pub fn format_compact(&self, data: &ResultData) -> Result<String> {
        match data {
            ResultData::Documents(docs) => Ok(format!("{} document(s) returned", docs.len())),
            ResultData::Document(doc) => Ok(format!("1 document: {}", doc)),
            ResultData::InsertOne { .. } => Ok("Inserted 1 document".to_string()),
            ResultData::InsertMany { inserted_ids } => {
                Ok(format!("Inserted {} document(s)", inserted_ids.len()))
            }
            ResultData::Update { matched, modified } => {
                Ok(format!("Matched: {}, Modified: {}", matched, modified))
            }
            ResultData::Delete { deleted } => Ok(format!("Deleted {} document(s)", deleted)),
            ResultData::Message(msg) => Ok(msg.clone()),
            ResultData::List(items) => Ok(format!("{} item(s)", items.len())),
            ResultData::Count(count) => Ok(format!("Count: {}", count)),
            ResultData::None => Ok("null".to_string()),
        }
    }

    /// Format error result
    ///
    /// # Arguments
    /// * `result` - Execution result with error
    ///
    /// # Returns
    /// * `Result<String>` - Formatted error message
    fn format_error(&self, result: &ExecutionResult) -> Result<String> {
        let unknown_error = String::from("Unknown error");
        let error_msg = result.error.as_ref().unwrap_or(&unknown_error);

        if self.use_colors {
            Ok(self.colorizer.error(error_msg))
        } else {
            Ok(format!("Error: {}", error_msg))
        }
    }

    /// Format execution statistics
    ///
    /// # Arguments
    /// * `result` - Execution result
    ///
    /// # Returns
    /// * `String` - Formatted statistics
    fn format_stats(&self, result: &ExecutionResult) -> String {
        let formatter = StatsFormatter::new(true, true);
        formatter.format(result)
    }

    /// Format a single BSON document
    ///
    /// # Arguments
    /// * `doc` - Document to format
    ///
    /// # Returns
    /// * `String` - Formatted document
    pub fn format_document(&self, doc: &Document) -> String {
        match self.format_type {
            OutputFormat::Shell => {
                let shell_formatter = ShellFormatter::new(self.use_colors);
                shell_formatter.format_document(doc)
            }
            OutputFormat::JsonPretty | OutputFormat::Json => {
                let json_formatter = JsonFormatter::new(true, self.use_colors, self.json_indent);
                json_formatter
                    .format_document(doc)
                    .unwrap_or_else(|_| format!("{}", doc))
            }
            _ => format!("{}", doc),
        }
    }

    /// Set output format
    ///
    /// # Arguments
    /// * `format_type` - New output format
    pub fn set_format(&mut self, format_type: OutputFormat) {
        self.format_type = format_type;
    }

    /// Enable or disable colors
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable colors
    pub fn set_colors(&mut self, enabled: bool) {
        self.use_colors = enabled;
        self.colorizer.set_enabled(enabled);
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::with_indent(OutputFormat::Shell, true, 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn test_formatter_creation() {
        let formatter = Formatter::new(OutputFormat::Json, false);
        assert!(!formatter.use_colors);
    }

    #[test]
    fn test_format_compact() {
        let formatter = Formatter::new(OutputFormat::Compact, false);
        let docs = vec![doc! { "name": "test" }];
        let result = formatter
            .format_compact(&ResultData::Documents(docs))
            .unwrap();
        assert!(result.contains("1 document(s)"));
    }

    #[test]
    fn test_format_shell_documents_as_array() {
        let formatter = Formatter::new(OutputFormat::Shell, false);
        let docs = vec![
            doc! { "name": "Alice", "age": 25 },
            doc! { "name": "Bob", "age": 30 },
        ];
        let result = formatter
            .format_shell(&ResultData::Documents(docs))
            .unwrap();

        // Should start with [ and end with ]
        assert!(result.starts_with("["));
        assert!(result.ends_with("]"));

        // Should contain both documents
        assert!(result.contains("'Alice'"));
        assert!(result.contains("'Bob'"));

        // Should be comma separated
        assert!(result.contains("},"));
    }

    #[test]
    fn test_format_shell_empty_documents() {
        let formatter = Formatter::new(OutputFormat::Shell, false);
        let docs: Vec<mongodb::bson::Document> = vec![];
        let result = formatter
            .format_shell(&ResultData::Documents(docs))
            .unwrap();
        assert_eq!(result, "[]");
    }
}
