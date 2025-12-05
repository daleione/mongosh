//! Output formatting and colorization for mongosh
//!
//! This module provides formatting functionality for command execution results:
//! - JSON formatting (plain and pretty-printed)
//! - Table formatting for document collections
//! - Compact formatting for minimal output
//! - Color highlighting for improved readability
//! - Custom formatters for specific result types

use mongodb::bson::{Bson, Document};
use serde_json::{json, Value};
use std::fmt;

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
}

/// Color scheme for output highlighting
pub struct Colorizer {
    /// Enable colors
    enabled: bool,
}

/// ANSI color codes for terminal output
pub struct AnsiColors;

/// Table formatter for document collections
pub struct TableFormatter {
    /// Maximum column width
    max_column_width: usize,

    /// Show borders
    show_borders: bool,

    /// Column separator
    separator: String,
}

/// JSON formatter with pretty printing support
pub struct JsonFormatter {
    /// Enable pretty printing
    pretty: bool,

    /// Indentation level
    indent: usize,
}

/// Statistics formatter for command execution
pub struct StatsFormatter {
    /// Show execution time
    show_time: bool,

    /// Show affected count
    show_count: bool,
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

    /// Format result data as JSON
    ///
    /// # Arguments
    /// * `data` - Result data to format
    /// * `pretty` - Enable pretty printing
    ///
    /// # Returns
    /// * `Result<String>` - JSON string or error
    pub fn format_json(&self, data: &ResultData, pretty: bool) -> Result<String> {
        let formatter = JsonFormatter::new(pretty);
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
            ResultData::Message(msg) => Ok(msg.clone()),
            ResultData::Count(count) => Ok(format!("Count: {}", count)),
            _ => Ok(format!("{:?}", data)),
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
            OutputFormat::JsonPretty => {
                serde_json::to_string_pretty(&doc).unwrap_or_else(|_| format!("{:?}", doc))
            }
            OutputFormat::Json => {
                serde_json::to_string(&doc).unwrap_or_else(|_| format!("{:?}", doc))
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

impl Colorizer {
    /// Create a new colorizer
    ///
    /// # Arguments
    /// * `enabled` - Enable color output
    ///
    /// # Returns
    /// * `Self` - New colorizer
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Colorize text as success (green)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn success(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::GREEN, text, AnsiColors::RESET)
        } else {
            text.to_string()
        }
    }

    /// Colorize text as error (red)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn error(&self, text: &str) -> String {
        if self.enabled {
            format!("{}Error: {}{}", AnsiColors::RED, text, AnsiColors::RESET)
        } else {
            format!("Error: {}", text)
        }
    }

    /// Colorize text as warning (yellow)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn warning(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::YELLOW, text, AnsiColors::RESET)
        } else {
            text.to_string()
        }
    }

    /// Colorize text as info (blue)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn info(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::BLUE, text, AnsiColors::RESET)
        } else {
            text.to_string()
        }
    }

    /// Colorize field name (cyan)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn field_name(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::CYAN, text, AnsiColors::RESET)
        } else {
            text.to_string()
        }
    }

    /// Colorize string value (green)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn string_value(&self, text: &str) -> String {
        if self.enabled {
            format!("{}\"{}\"{}", AnsiColors::GREEN, text, AnsiColors::RESET)
        } else {
            format!("\"{}\"", text)
        }
    }

    /// Colorize number value (magenta)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn number_value(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::MAGENTA, text, AnsiColors::RESET)
        } else {
            text.to_string()
        }
    }

    /// Enable or disable colors
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable colors
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl AnsiColors {
    pub const RESET: &'static str = "\x1b[0m";
    pub const BOLD: &'static str = "\x1b[1m";
    pub const DIM: &'static str = "\x1b[2m";

    // Foreground colors
    pub const BLACK: &'static str = "\x1b[30m";
    pub const RED: &'static str = "\x1b[31m";
    pub const GREEN: &'static str = "\x1b[32m";
    pub const YELLOW: &'static str = "\x1b[33m";
    pub const BLUE: &'static str = "\x1b[34m";
    pub const MAGENTA: &'static str = "\x1b[35m";
    pub const CYAN: &'static str = "\x1b[36m";
    pub const WHITE: &'static str = "\x1b[37m";

    // Bright foreground colors
    pub const BRIGHT_BLACK: &'static str = "\x1b[90m";
    pub const BRIGHT_RED: &'static str = "\x1b[91m";
    pub const BRIGHT_GREEN: &'static str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &'static str = "\x1b[93m";
    pub const BRIGHT_BLUE: &'static str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &'static str = "\x1b[95m";
    pub const BRIGHT_CYAN: &'static str = "\x1b[96m";
    pub const BRIGHT_WHITE: &'static str = "\x1b[97m";
}

impl TableFormatter {
    /// Create a new table formatter
    ///
    /// # Returns
    /// * `Self` - New table formatter
    pub fn new() -> Self {
        Self {
            max_column_width: 50,
            show_borders: true,
            separator: " | ".to_string(),
        }
    }

    /// Format result data as table
    ///
    /// # Arguments
    /// * `data` - Result data to format
    ///
    /// # Returns
    /// * `Result<String>` - Table string or error
    pub fn format(&self, data: &ResultData) -> Result<String> {
        match data {
            ResultData::Documents(docs) => self.format_documents(docs),
            ResultData::Document(doc) => self.format_documents(&vec![doc.clone()]),
            ResultData::Message(msg) => Ok(msg.clone()),
            _ => Ok(format!("{:?}", data)),
        }
    }

    /// Format multiple documents as table
    ///
    /// # Arguments
    /// * `docs` - Documents to format
    ///
    /// # Returns
    /// * `Result<String>` - Table string
    fn format_documents(&self, docs: &[Document]) -> Result<String> {
        todo!("Format documents as ASCII table with columns for each field")
    }

    /// Extract all unique field names from documents
    ///
    /// # Arguments
    /// * `docs` - Documents to analyze
    ///
    /// # Returns
    /// * `Vec<String>` - Unique field names
    fn get_field_names(&self, docs: &[Document]) -> Vec<String> {
        todo!("Extract all unique field names across all documents")
    }

    /// Format table header
    ///
    /// # Arguments
    /// * `fields` - Field names
    ///
    /// # Returns
    /// * `String` - Header row
    fn format_header(&self, fields: &[String]) -> String {
        todo!("Format table header with field names")
    }

    /// Format table row
    ///
    /// # Arguments
    /// * `doc` - Document to format
    /// * `fields` - Field names in order
    ///
    /// # Returns
    /// * `String` - Table row
    fn format_row(&self, doc: &Document, fields: &[String]) -> String {
        todo!("Format single document as table row")
    }
}

impl JsonFormatter {
    /// Create a new JSON formatter
    ///
    /// # Arguments
    /// * `pretty` - Enable pretty printing
    ///
    /// # Returns
    /// * `Self` - New formatter
    pub fn new(pretty: bool) -> Self {
        Self { pretty, indent: 2 }
    }

    /// Format result data as JSON
    ///
    /// # Arguments
    /// * `data` - Result data to format
    ///
    /// # Returns
    /// * `Result<String>` - JSON string or error
    pub fn format(&self, data: &ResultData) -> Result<String> {
        match data {
            ResultData::Documents(docs) => self.format_documents(docs),
            ResultData::Document(doc) => self.format_document(doc),
            ResultData::Message(msg) => Ok(format!("\"{}\"", msg)),
            ResultData::InsertOne(id) => Ok(format!("{{ \"insertedId\": \"{}\" }}", id)),
            ResultData::InsertMany(ids) => {
                let ids_json = ids
                    .iter()
                    .map(|id| format!("\"{}\"", id))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(format!("{{ \"insertedIds\": [{}] }}", ids_json))
            }
            ResultData::Update { matched, modified } => Ok(format!(
                "{{ \"matchedCount\": {}, \"modifiedCount\": {} }}",
                matched, modified
            )),
            ResultData::Delete { deleted } => Ok(format!("{{ \"deletedCount\": {} }}", deleted)),
            ResultData::Count(count) => Ok(format!("{}", count)),
            ResultData::None => Ok("null".to_string()),
        }
    }

    /// Format documents as JSON array
    ///
    /// # Arguments
    /// * `docs` - Documents to format
    ///
    /// # Returns
    /// * `Result<String>` - JSON array string
    fn format_documents(&self, docs: &[Document]) -> Result<String> {
        if self.pretty {
            Ok(serde_json::to_string_pretty(docs).unwrap_or_else(|_| format!("{:?}", docs)))
        } else {
            Ok(serde_json::to_string(docs).unwrap_or_else(|_| format!("{:?}", docs)))
        }
    }

    /// Format single document as JSON object
    ///
    /// # Arguments
    /// * `doc` - Document to format
    ///
    /// # Returns
    /// * `Result<String>` - JSON object string
    fn format_document(&self, doc: &Document) -> Result<String> {
        if self.pretty {
            Ok(serde_json::to_string_pretty(doc).unwrap_or_else(|_| format!("{:?}", doc)))
        } else {
            Ok(serde_json::to_string(doc).unwrap_or_else(|_| format!("{:?}", doc)))
        }
    }
}

impl StatsFormatter {
    /// Create a new statistics formatter
    ///
    /// # Arguments
    /// * `show_time` - Show execution time
    /// * `show_count` - Show affected count
    ///
    /// # Returns
    /// * `Self` - New formatter
    pub fn new(show_time: bool, show_count: bool) -> Self {
        Self {
            show_time,
            show_count,
        }
    }

    /// Format execution statistics
    ///
    /// # Arguments
    /// * `result` - Execution result
    ///
    /// # Returns
    /// * `String` - Formatted statistics
    pub fn format(&self, result: &ExecutionResult) -> String {
        let mut parts = Vec::new();

        if self.show_time && result.execution_time_ms > 0 {
            parts.push(format!("Execution time: {}ms", result.execution_time_ms));
        }

        if self.show_count {
            if let Some(count) = result.affected_count {
                parts.push(format!("Affected: {} document(s)", count));
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join(", ")
        }
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new(OutputFormat::JsonPretty, true)
    }
}

impl Default for TableFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new(true)
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
    fn test_colorizer_no_colors() {
        let colorizer = Colorizer::new(false);
        let result = colorizer.error("test error");
        assert_eq!(result, "Error: test error");
        assert!(!result.contains("\x1b"));
    }

    #[test]
    fn test_colorizer_with_colors() {
        let colorizer = Colorizer::new(true);
        let result = colorizer.success("test");
        assert!(result.contains("\x1b"));
    }

    #[test]
    fn test_json_formatter() {
        let formatter = JsonFormatter::new(false);
        let doc = doc! { "name": "test", "value": 42 };
        let result = formatter.format_document(&doc).unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_stats_formatter() {
        let formatter = StatsFormatter::new(true, true);
        let result = ExecutionResult {
            success: true,
            data: ResultData::None,
            execution_time_ms: 150,
            affected_count: Some(5),
            error: None,
        };
        let stats = formatter.format(&result);
        assert!(stats.contains("150ms"));
        assert!(stats.contains("5 document(s)"));
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
}
