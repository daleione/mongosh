//! Output formatting and colorization for mongosh
//!
//! This module provides formatting functionality for command execution results:
//! - JSON formatting (plain and pretty-printed)
//! - Table formatting for document collections
//! - Compact formatting for minimal output
//! - Color highlighting for improved readability
//! - Custom formatters for specific result types

use colored_json::prelude::*;
use mongodb::bson::{Bson, Document};

use crate::config::OutputFormat;
use crate::error::Result;
use crate::executor::{ExecutionResult, ExecutionStats, ResultData};

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

    /// Enable colored output
    use_colors: bool,
}

/// Shell-style formatter (mongosh compatible)
pub struct ShellFormatter {
    /// Enable colored output
    use_colors: bool,

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
                let mut result = String::new();
                for (i, doc) in docs.iter().enumerate() {
                    if i > 0 {
                        result.push_str("\n\n");
                    }
                    result.push_str(&shell_formatter.format_document(doc));
                }
                Ok(result)
            }
            ResultData::Document(doc) => Ok(shell_formatter.format_document(doc)),
            ResultData::Message(msg) => Ok(msg.clone()),
            ResultData::List(items) => Ok(items.join("\n")),
            ResultData::Count(count) => Ok(format!("{}", count)),
            ResultData::None => Ok("null".to_string()),
            _ => Ok(format!("{:?}", data)),
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
        let formatter = JsonFormatter::new(pretty, self.use_colors);
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
            OutputFormat::Shell => {
                let shell_formatter = ShellFormatter::new(self.use_colors);
                shell_formatter.format_document(doc)
            }
            OutputFormat::JsonPretty | OutputFormat::Json => {
                let json_formatter = JsonFormatter::new(true, self.use_colors);
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

impl ShellFormatter {
    /// Create a new shell formatter
    ///
    /// # Arguments
    /// * `use_colors` - Enable colored output
    ///
    /// # Returns
    /// * `Self` - New formatter
    pub fn new(use_colors: bool) -> Self {
        Self {
            use_colors,
            indent: 2,
        }
    }

    /// Format a BSON document in shell style
    ///
    /// # Arguments
    /// * `doc` - Document to format
    ///
    /// # Returns
    /// * `String` - Formatted document
    pub fn format_document(&self, doc: &Document) -> String {
        self.format_document_with_indent(doc, 0)
    }

    /// Format a BSON document with indentation
    fn format_document_with_indent(&self, doc: &Document, indent_level: usize) -> String {
        if doc.is_empty() {
            return "{}".to_string();
        }

        let mut result = String::from("{\n");
        let indent = " ".repeat((indent_level + 1) * self.indent);

        let entries: Vec<_> = doc.iter().collect();
        for (i, (key, value)) in entries.iter().enumerate() {
            let formatted_value = self.format_bson_value(value, indent_level + 1);
            result.push_str(&indent);

            // Key without quotes (shell style)
            if self.use_colors {
                result.push_str(&format!("\x1b[36m{}\x1b[0m", key));
            } else {
                result.push_str(key);
            }

            result.push_str(": ");
            result.push_str(&formatted_value);

            // Add comma except for last item
            if i < entries.len() - 1 {
                result.push(',');
            }
            result.push('\n');
        }

        result.push_str(&" ".repeat(indent_level * self.indent));
        result.push('}');
        result
    }

    /// Format a BSON value in shell style
    fn format_bson_value(&self, value: &Bson, indent_level: usize) -> String {
        match value {
            Bson::ObjectId(oid) => {
                let formatted = format!("ObjectId('{}')", oid);
                if self.use_colors {
                    format!("\x1b[33m{}\x1b[0m", formatted)
                } else {
                    formatted
                }
            }
            Bson::DateTime(dt) => {
                // Convert to ISO 8601 format
                let iso = dt.try_to_rfc3339_string().unwrap_or_else(|_| {
                    // Fallback to timestamp if conversion fails
                    format!("{}", dt.timestamp_millis())
                });
                let formatted = format!("ISODate('{}')", iso);
                if self.use_colors {
                    format!("\x1b[32m{}\x1b[0m", formatted)
                } else {
                    formatted
                }
            }
            Bson::Int64(n) => {
                let formatted = format!("Long('{}')", n);
                if self.use_colors {
                    format!("\x1b[33m{}\x1b[0m", formatted)
                } else {
                    formatted
                }
            }
            Bson::Decimal128(d) => {
                let formatted = format!("NumberDecimal('{}')", d);
                if self.use_colors {
                    format!("\x1b[33m{}\x1b[0m", formatted)
                } else {
                    formatted
                }
            }
            Bson::String(s) => {
                if self.use_colors {
                    format!("\x1b[32m'{}'\x1b[0m", s)
                } else {
                    format!("'{}'", s)
                }
            }
            Bson::Int32(n) => {
                if self.use_colors {
                    format!("\x1b[33m{}\x1b[0m", n)
                } else {
                    n.to_string()
                }
            }
            Bson::Double(f) => {
                if self.use_colors {
                    format!("\x1b[33m{}\x1b[0m", f)
                } else {
                    f.to_string()
                }
            }
            Bson::Boolean(b) => {
                if self.use_colors {
                    format!("\x1b[33m{}\x1b[0m", b)
                } else {
                    b.to_string()
                }
            }
            Bson::Null => {
                if self.use_colors {
                    format!("\x1b[90mnull\x1b[0m")
                } else {
                    "null".to_string()
                }
            }
            Bson::Array(arr) => self.format_array(arr, indent_level),
            Bson::Document(doc) => self.format_document_with_indent(doc, indent_level),
            Bson::Binary(bin) => {
                // Convert BinarySubtype to u8
                let subtype_num = match bin.subtype {
                    mongodb::bson::spec::BinarySubtype::Generic => 0u8,
                    mongodb::bson::spec::BinarySubtype::Function => 1u8,
                    mongodb::bson::spec::BinarySubtype::BinaryOld => 2u8,
                    mongodb::bson::spec::BinarySubtype::UuidOld => 3u8,
                    mongodb::bson::spec::BinarySubtype::Uuid => 4u8,
                    mongodb::bson::spec::BinarySubtype::Md5 => 5u8,
                    mongodb::bson::spec::BinarySubtype::Encrypted => 6u8,
                    mongodb::bson::spec::BinarySubtype::Column => 7u8,
                    mongodb::bson::spec::BinarySubtype::Sensitive => 8u8,
                    mongodb::bson::spec::BinarySubtype::UserDefined(n) => n,
                    _ => 0u8, // Default to generic for unknown subtypes
                };
                let formatted = format!("BinData({}, '{}')", subtype_num, hex::encode(&bin.bytes));
                if self.use_colors {
                    format!("\x1b[35m{}\x1b[0m", formatted)
                } else {
                    formatted
                }
            }
            Bson::RegularExpression(regex) => {
                let formatted = format!("/{}/{}", regex.pattern, regex.options);
                if self.use_colors {
                    format!("\x1b[31m{}\x1b[0m", formatted)
                } else {
                    formatted
                }
            }
            Bson::Timestamp(ts) => {
                let formatted = format!("Timestamp({}, {})", ts.time, ts.increment);
                if self.use_colors {
                    format!("\x1b[33m{}\x1b[0m", formatted)
                } else {
                    formatted
                }
            }
            _ => format!("{:?}", value),
        }
    }

    /// Format a BSON array in shell style
    fn format_array(&self, arr: &[Bson], indent_level: usize) -> String {
        if arr.is_empty() {
            return "[]".to_string();
        }

        let mut result = String::from("[\n");
        let indent = " ".repeat((indent_level + 1) * self.indent);

        for (i, value) in arr.iter().enumerate() {
            result.push_str(&indent);
            result.push_str(&self.format_bson_value(value, indent_level + 1));

            if i < arr.len() - 1 {
                result.push(',');
            }
            result.push('\n');
        }

        result.push_str(&" ".repeat(indent_level * self.indent));
        result.push(']');
        result
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
    /// * `use_colors` - Enable colored output
    ///
    /// # Returns
    /// * `Self` - New formatter
    pub fn new(pretty: bool, use_colors: bool) -> Self {
        Self {
            pretty,
            indent: 2,
            use_colors,
        }
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
            ResultData::List(items) => {
                let list_str = items.join("\n");
                Ok(list_str)
            }
            ResultData::InsertOne { inserted_id } => {
                Ok(format!("{{ \"insertedId\": \"{}\" }}", inserted_id))
            }
            ResultData::InsertMany { inserted_ids } => {
                let ids_json = inserted_ids
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
        // Convert BSON documents to simplified JSON
        let json_docs: Vec<serde_json::Value> = docs
            .iter()
            .map(|doc| self.bson_to_simplified_json(doc))
            .collect();

        let json_str = if self.pretty {
            serde_json::to_string_pretty(&json_docs).unwrap_or_else(|_| format!("{:?}", docs))
        } else {
            serde_json::to_string(&json_docs).unwrap_or_else(|_| format!("{:?}", docs))
        };

        if self.use_colors {
            Ok(json_str.to_colored_json_auto().unwrap_or(json_str))
        } else {
            Ok(json_str)
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
        // Convert BSON document to simplified JSON
        let json_value = self.bson_to_simplified_json(doc);

        let json_str = if self.pretty {
            serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| format!("{:?}", doc))
        } else {
            serde_json::to_string(&json_value).unwrap_or_else(|_| format!("{:?}", doc))
        };

        if self.use_colors {
            Ok(json_str.to_colored_json_auto().unwrap_or(json_str))
        } else {
            Ok(json_str)
        }
    }

    /// Convert BSON document to simplified JSON
    ///
    /// Converts BSON types to human-readable JSON:
    /// - ObjectId -> String
    /// - DateTime -> ISO 8601 String
    /// - Int64 -> Number
    /// - Decimal128 -> Number
    /// - Binary -> Base64 String
    fn bson_to_simplified_json(&self, doc: &Document) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        for (key, value) in doc {
            map.insert(key.clone(), self.bson_value_to_json(value));
        }

        serde_json::Value::Object(map)
    }

    /// Convert BSON value to simplified JSON value
    fn bson_value_to_json(&self, value: &Bson) -> serde_json::Value {
        use serde_json::Value as JsonValue;

        match value {
            Bson::ObjectId(oid) => JsonValue::String(oid.to_string()),
            Bson::DateTime(dt) => {
                let iso = dt
                    .try_to_rfc3339_string()
                    .unwrap_or_else(|_| format!("{}", dt.timestamp_millis()));
                JsonValue::String(iso)
            }
            Bson::Int64(n) => JsonValue::Number((*n).into()),
            Bson::Int32(n) => JsonValue::Number((*n).into()),
            Bson::Double(f) => serde_json::Number::from_f64(*f)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            Bson::Decimal128(d) => {
                // Convert Decimal128 to string then to number if possible
                let s = d.to_string();
                s.parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(JsonValue::Number)
                    .unwrap_or_else(|| JsonValue::String(s))
            }
            Bson::String(s) => JsonValue::String(s.clone()),
            Bson::Boolean(b) => JsonValue::Bool(*b),
            Bson::Null => JsonValue::Null,
            Bson::Array(arr) => {
                let json_arr: Vec<JsonValue> =
                    arr.iter().map(|v| self.bson_value_to_json(v)).collect();
                JsonValue::Array(json_arr)
            }
            Bson::Document(doc) => self.bson_to_simplified_json(doc),
            Bson::Binary(bin) => {
                // Convert binary to base64 string
                use base64::Engine;
                let base64_str = base64::engine::general_purpose::STANDARD.encode(&bin.bytes);
                JsonValue::String(base64_str)
            }
            Bson::RegularExpression(regex) => {
                JsonValue::String(format!("/{}/{}", regex.pattern, regex.options))
            }
            Bson::Timestamp(ts) => {
                // Convert timestamp to milliseconds
                let millis = (ts.time as i64) * 1000 + (ts.increment as i64);
                JsonValue::Number(millis.into())
            }
            _ => JsonValue::String(format!("{:?}", value)),
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

        if self.show_time && result.stats.execution_time_ms > 0 {
            parts.push(format!(
                "Execution time: {}ms",
                result.stats.execution_time_ms
            ));
        }

        if self.show_count {
            if let Some(count) = result.stats.documents_affected {
                parts.push(format!("Documents affected: {}", count));
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
        Self::new(OutputFormat::Shell, true)
    }
}

impl Default for TableFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new(true, false)
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
        let formatter = JsonFormatter::new(false, false);
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
            stats: ExecutionStats {
                execution_time_ms: 150,
                documents_returned: 0,
                documents_affected: Some(5),
            },
            error: None,
        };
        let stats = formatter.format(&result);
        assert!(stats.contains("150ms"));
        assert!(stats.contains("Documents affected: 5"));
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
    fn test_shell_formatter_objectid() {
        use mongodb::bson::oid::ObjectId;
        let formatter = ShellFormatter::new(false);
        let oid = ObjectId::parse_str("65705d84dfc3f3b5094e1f72").unwrap();
        let doc = doc! { "_id": oid };
        let result = formatter.format_document(&doc);
        assert!(result.contains("_id:"));
        assert!(result.contains("ObjectId('65705d84dfc3f3b5094e1f72')"));
    }

    #[test]
    fn test_shell_formatter_datetime() {
        use mongodb::bson::DateTime;
        let formatter = ShellFormatter::new(false);
        let dt = DateTime::from_millis(1701862788373);
        let doc = doc! { "created_time": dt };
        let result = formatter.format_document(&doc);
        assert!(result.contains("created_time:"));
        assert!(result.contains("ISODate("));
        assert!(result.contains("2023-12-06"));
    }

    #[test]
    fn test_shell_formatter_long() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "user_id": 1i64 };
        let result = formatter.format_document(&doc);
        assert!(result.contains("user_id:"));
        assert!(result.contains("Long('1')"));
    }

    #[test]
    fn test_shell_formatter_string() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "nickname": "dalei" };
        let result = formatter.format_document(&doc);
        assert!(result.contains("nickname:"));
        assert!(result.contains("'dalei'"));
    }

    #[test]
    fn test_shell_formatter_null() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "oauth2": null };
        let result = formatter.format_document(&doc);
        assert!(result.contains("oauth2:"));
        assert!(result.contains("null"));
    }

    #[test]
    fn test_shell_formatter_nested_document() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! {
            "user": {
                "name": "test",
                "age": 25
            }
        };
        let result = formatter.format_document(&doc);
        assert!(result.contains("user:"));
        assert!(result.contains("name:"));
        assert!(result.contains("'test'"));
        assert!(result.contains("age:"));
        assert!(result.contains("25"));
    }

    #[test]
    fn test_shell_formatter_array() {
        let formatter = ShellFormatter::new(false);
        let doc = doc! { "tags": ["rust", "mongodb"] };
        let result = formatter.format_document(&doc);
        assert!(result.contains("tags:"));
        assert!(result.contains("'rust'"));
        assert!(result.contains("'mongodb'"));
    }

    #[test]
    fn test_json_formatter_simplified_objectid() {
        use mongodb::bson::oid::ObjectId;
        let formatter = JsonFormatter::new(true, false);
        let oid = ObjectId::parse_str("65705d84dfc3f3b5094e1f72").unwrap();
        let doc = doc! { "_id": oid };
        let result = formatter.format_document(&doc).unwrap();
        // Should be simplified to string, not extended JSON
        assert!(result.contains("\"_id\""));
        assert!(result.contains("\"65705d84dfc3f3b5094e1f72\""));
        assert!(!result.contains("$oid"));
    }

    #[test]
    fn test_json_formatter_simplified_datetime() {
        use mongodb::bson::DateTime;
        let formatter = JsonFormatter::new(true, false);
        let dt = DateTime::from_millis(1701862788373);
        let doc = doc! { "created_time": dt };
        let result = formatter.format_document(&doc).unwrap();
        // Should be ISO 8601 string, not extended JSON
        assert!(result.contains("\"created_time\""));
        assert!(result.contains("2023-12-06"));
        assert!(!result.contains("$date"));
        assert!(!result.contains("$numberLong"));
    }

    #[test]
    fn test_json_formatter_simplified_long() {
        let formatter = JsonFormatter::new(true, false);
        let doc = doc! { "user_id": 1i64 };
        let result = formatter.format_document(&doc).unwrap();
        // Should be a number, not Long('1')
        assert!(result.contains("\"user_id\""));
        assert!(result.contains("1"));
        assert!(!result.contains("Long"));
    }

    #[test]
    fn test_json_formatter_complete_document() {
        use mongodb::bson::{oid::ObjectId, DateTime};
        let formatter = JsonFormatter::new(true, false);
        let oid = ObjectId::parse_str("65705d84dfc3f3b5094e1f72").unwrap();
        let dt = DateTime::from_millis(1701862788373);
        let doc = doc! {
            "_id": oid,
            "user_id": 1i64,
            "nickname": "dalei",
            "oauth2": null,
            "created_time": dt
        };
        let result = formatter.format_document(&doc).unwrap();

        // Verify all fields are simplified
        assert!(result.contains("\"_id\": \"65705d84dfc3f3b5094e1f72\""));
        assert!(result.contains("\"user_id\": 1"));
        assert!(result.contains("\"nickname\": \"dalei\""));
        assert!(result.contains("\"oauth2\": null"));
        assert!(result.contains("\"created_time\": \"2023-12-06"));

        // Verify no extended JSON formats
        assert!(!result.contains("$oid"));
        assert!(!result.contains("$date"));
    }
}
