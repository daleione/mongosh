//! Table formatting for MongoDB document collections using tabled
//!
//! This module provides table-style formatting for displaying multiple documents:
//! - Builder pattern for dynamic schema support
//! - Automatic column extraction from all documents
//! - BSON type handling with shell-style wrappers
//! - Configurable styles and width limits
//! - Nested document and array support

use mongodb::bson::{Bson, Document};
use tabled::{
    Table,
    builder::Builder,
    settings::{Alignment, Color, Modify, Style, object::Rows, width::Width},
};

use crate::error::Result;
use crate::executor::ResultData;

/// Maximum width for a single column (characters)
const DEFAULT_MAX_COLUMN_WIDTH: usize = 40;

/// Maximum width for the entire table (characters)
const DEFAULT_MAX_TABLE_WIDTH: usize = 150;

/// Table formatter for document collections
pub struct TableFormatter {
    /// Maximum column width
    max_column_width: usize,

    /// Maximum table width
    #[allow(dead_code)]
    max_table_width: usize,

    /// Table style
    style: TableStyle,

    /// Enable colored output
    use_colors: bool,
}

/// Available table styles
#[derive(Debug, Clone, Copy)]
pub enum TableStyle {
    /// Modern style with rounded corners
    Modern,
    /// ASCII style with basic characters
    Ascii,
    /// Rounded style
    Rounded,
    /// Markdown style
    Markdown,
    /// Psql style
    Psql,
}

impl TableFormatter {
    /// Create a new table formatter with default settings
    ///
    /// # Returns
    /// * `Self` - New table formatter
    pub fn new() -> Self {
        Self {
            max_column_width: DEFAULT_MAX_COLUMN_WIDTH,
            max_table_width: DEFAULT_MAX_TABLE_WIDTH,
            style: TableStyle::Modern,
            use_colors: false,
        }
    }

    /// Create a new table formatter with color support
    ///
    /// # Arguments
    /// * `use_colors` - Enable colored output
    ///
    /// # Returns
    /// * `Self` - New table formatter
    pub fn with_colors(use_colors: bool) -> Self {
        Self {
            max_column_width: DEFAULT_MAX_COLUMN_WIDTH,
            max_table_width: DEFAULT_MAX_TABLE_WIDTH,
            style: TableStyle::Modern,
            use_colors,
        }
    }

    /// Set the table style
    ///
    /// # Arguments
    /// * `style` - Table style to use
    ///
    /// # Returns
    /// * `Self` - Modified formatter
    pub fn with_style(mut self, style: TableStyle) -> Self {
        self.style = style;
        self
    }

    /// Set maximum column width
    ///
    /// # Arguments
    /// * `width` - Maximum column width
    ///
    /// # Returns
    /// * `Self` - Modified formatter
    pub fn with_max_column_width(mut self, width: usize) -> Self {
        self.max_column_width = width;
        self
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
            ResultData::Documents(docs) => {
                if docs.is_empty() {
                    return Ok("(empty result set)".to_string());
                }
                self.format_documents(docs)
            }
            ResultData::DocumentsWithPagination { documents, .. } => {
                if documents.is_empty() {
                    return Ok("(empty result set)".to_string());
                }
                self.format_documents(documents)
            }
            ResultData::Document(doc) => self.format_documents(&[doc.clone()]),
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
        // Extract all unique field names
        let fields = self.extract_field_names(docs);

        if fields.is_empty() {
            return Ok("(no fields found)".to_string());
        }

        // Build table using Builder pattern
        let mut builder = Builder::default();

        // Add header row
        builder.push_record(fields.clone());

        // Add data rows
        for doc in docs {
            let row: Vec<String> = fields
                .iter()
                .map(|field| self.format_field_value(doc, field))
                .collect();
            builder.push_record(row);
        }

        // Build and style the table
        let mut table = builder.build();

        // Apply style
        self.apply_style(&mut table);

        // Apply width constraints per column with wrapping
        // This ensures long values are wrapped instead of truncated
        for i in 0..fields.len() {
            use tabled::settings::object::Columns;
            table.with(Modify::new(Columns::new(i..=i)).with(Width::wrap(self.max_column_width)));
        }

        // Apply header styling
        table.with(Modify::new(Rows::first()).with(Alignment::center()));

        // Apply colorization if enabled
        if self.use_colors {
            table.modify(Rows::first(), Color::FG_CYAN | Color::BOLD);
        }

        Ok(table.to_string())
    }

    /// Extract all unique field names from documents, with _id first
    ///
    /// # Arguments
    /// * `docs` - Documents to analyze
    ///
    /// # Returns
    /// * `Vec<String>` - Sorted unique field names
    fn extract_field_names(&self, docs: &[Document]) -> Vec<String> {
        let mut fields = std::collections::BTreeSet::new();

        for doc in docs {
            for key in doc.keys() {
                fields.insert(key.clone());
            }
        }

        let mut field_vec: Vec<String> = fields.into_iter().collect();

        // Ensure _id comes first if it exists
        if let Some(pos) = field_vec.iter().position(|f| f == "_id") {
            field_vec.remove(pos);
            field_vec.insert(0, "_id".to_string());
        }

        field_vec
    }

    /// Format a field value from a document
    ///
    /// # Arguments
    /// * `doc` - Document containing the field
    /// * `field` - Field name to extract
    ///
    /// # Returns
    /// * `String` - Formatted field value
    fn format_field_value(&self, doc: &Document, field: &str) -> String {
        match doc.get(field) {
            Some(value) => self.format_bson_value(value),
            None => String::from(""),
        }
    }

    /// Format a BSON value for table display
    ///
    /// # Arguments
    /// * `value` - BSON value to format
    ///
    /// # Returns
    /// * `String` - Formatted value
    fn format_bson_value(&self, value: &Bson) -> String {
        match value {
            Bson::ObjectId(oid) => format!("ObjectId('{}')", oid),
            Bson::DateTime(dt) => {
                let iso = dt
                    .try_to_rfc3339_string()
                    .unwrap_or_else(|_| format!("{}", dt.timestamp_millis()));
                format!("ISODate('{}')", iso)
            }
            Bson::Int64(n) => format!("Long('{}')", n),
            Bson::Decimal128(d) => format!("NumberDecimal('{}')", d),
            Bson::String(s) => s.clone(),
            Bson::Int32(n) => n.to_string(),
            Bson::Double(f) => {
                // Format double with reasonable precision
                if f.fract() == 0.0 && f.abs() < 1e10 {
                    format!("{:.0}", f)
                } else {
                    format!("{}", f)
                }
            }
            Bson::Boolean(b) => b.to_string(),
            Bson::Null => String::from("null"),
            Bson::Array(arr) => {
                if arr.is_empty() {
                    String::from("[]")
                } else if arr.len() <= 3 {
                    // Show small arrays inline
                    let items: Vec<String> =
                        arr.iter().map(|v| self.format_bson_value(v)).collect();
                    format!("[{}]", items.join(", "))
                } else {
                    // Show array length for large arrays
                    format!("[Array({})]", arr.len())
                }
            }
            Bson::Document(doc) => {
                if doc.is_empty() {
                    String::from("{}")
                } else if doc.len() <= 2 {
                    // Show small documents inline
                    let fields: Vec<String> = doc
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, self.format_bson_value(v)))
                        .collect();
                    format!("{{{}}}", fields.join(", "))
                } else {
                    // Show field count for large documents
                    format!("{{Object({})}}", doc.len())
                }
            }
            Bson::Binary(bin) => {
                let hex = hex::encode(&bin.bytes);
                if hex.len() > 16 {
                    format!("Binary({}...)", &hex[..16])
                } else {
                    format!("Binary({})", hex)
                }
            }
            Bson::RegularExpression(regex) => {
                format!("/{}/{}", regex.pattern, regex.options)
            }
            Bson::Timestamp(ts) => {
                format!("Timestamp({}, {})", ts.time, ts.increment)
            }
            Bson::Undefined => String::from("undefined"),
            Bson::MinKey => String::from("MinKey"),
            Bson::MaxKey => String::from("MaxKey"),
            _ => format!("{:?}", value),
        }
    }

    /// Apply table style
    ///
    /// # Arguments
    /// * `table` - Table to style
    fn apply_style(&self, table: &mut Table) {
        match self.style {
            TableStyle::Modern => table.with(Style::modern()),
            TableStyle::Ascii => table.with(Style::ascii()),
            TableStyle::Rounded => table.with(Style::rounded()),
            TableStyle::Markdown => table.with(Style::markdown()),
            TableStyle::Psql => table.with(Style::psql()),
        };
    }
}

impl Default for TableFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::{doc, oid::ObjectId};

    #[test]
    fn test_table_formatter_creation() {
        let formatter = TableFormatter::new();
        assert_eq!(formatter.max_column_width, DEFAULT_MAX_COLUMN_WIDTH);
        assert_eq!(formatter.max_table_width, DEFAULT_MAX_TABLE_WIDTH);
    }

    #[test]
    fn test_format_empty_documents() {
        let formatter = TableFormatter::new();
        let docs: Vec<Document> = vec![];
        let result = formatter.format(&ResultData::Documents(docs)).unwrap();
        assert_eq!(result, "(empty result set)");
    }

    #[test]
    fn test_format_single_document() {
        let formatter = TableFormatter::new();
        let doc = doc! {
            "name": "Alice",
            "age": 25
        };
        let result = formatter.format(&ResultData::Document(doc)).unwrap();
        assert!(result.contains("name"));
        assert!(result.contains("age"));
        assert!(result.contains("Alice"));
        assert!(result.contains("25"));
    }

    #[test]
    fn test_format_multiple_documents() {
        let formatter = TableFormatter::new();
        let docs = vec![
            doc! { "name": "Alice", "age": 25 },
            doc! { "name": "Bob", "age": 30 },
        ];
        let result = formatter.format(&ResultData::Documents(docs)).unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("Bob"));
        assert!(result.contains("25"));
        assert!(result.contains("30"));
    }

    #[test]
    fn test_extract_field_names_with_id() {
        let formatter = TableFormatter::new();
        let docs = vec![
            doc! { "_id": 1, "name": "Alice", "age": 25 },
            doc! { "_id": 2, "name": "Bob" },
        ];
        let fields = formatter.extract_field_names(&docs);
        assert_eq!(fields[0], "_id");
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"age".to_string()));
    }

    #[test]
    fn test_format_bson_objectid() {
        let formatter = TableFormatter::new();
        let oid = ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
        let result = formatter.format_bson_value(&Bson::ObjectId(oid));
        assert!(result.contains("ObjectId"));
        assert!(result.contains("507f1f77bcf86cd799439011"));
    }

    #[test]
    fn test_format_bson_null() {
        let formatter = TableFormatter::new();
        let result = formatter.format_bson_value(&Bson::Null);
        assert_eq!(result, "null");
    }

    #[test]
    fn test_format_bson_array_small() {
        let formatter = TableFormatter::new();
        let arr = Bson::Array(vec![Bson::Int32(1), Bson::Int32(2), Bson::Int32(3)]);
        let result = formatter.format_bson_value(&arr);
        assert!(result.contains("[1, 2, 3]"));
    }

    #[test]
    fn test_format_bson_array_large() {
        let formatter = TableFormatter::new();
        let arr = Bson::Array(vec![
            Bson::Int32(1),
            Bson::Int32(2),
            Bson::Int32(3),
            Bson::Int32(4),
            Bson::Int32(5),
        ]);
        let result = formatter.format_bson_value(&arr);
        assert!(result.contains("[Array(5)]"));
    }

    #[test]
    fn test_format_bson_document_small() {
        let formatter = TableFormatter::new();
        let doc = Bson::Document(doc! { "x": 1 });
        let result = formatter.format_bson_value(&doc);
        assert!(result.contains("x: 1"));
    }

    #[test]
    fn test_format_bson_document_large() {
        let formatter = TableFormatter::new();
        let doc = Bson::Document(doc! { "a": 1, "b": 2, "c": 3 });
        let result = formatter.format_bson_value(&doc);
        assert!(result.contains("{Object(3)}"));
    }

    #[test]
    fn test_format_missing_fields() {
        let formatter = TableFormatter::new();
        let docs = vec![
            doc! { "name": "Alice", "age": 25 },
            doc! { "name": "Bob" }, // missing age
        ];
        let result = formatter.format(&ResultData::Documents(docs)).unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("Bob"));
    }

    #[test]
    fn test_with_style() {
        let formatter = TableFormatter::new().with_style(TableStyle::Ascii);
        let doc = doc! { "name": "Alice" };
        let result = formatter.format(&ResultData::Document(doc)).unwrap();
        assert!(result.contains("+"));
        assert!(result.contains("|"));
    }

    #[test]
    fn test_with_max_column_width() {
        let formatter = TableFormatter::new().with_max_column_width(20);
        assert_eq!(formatter.max_column_width, 20);
    }

    #[test]
    fn test_actual_table_output() {
        use mongodb::bson::DateTime;

        let formatter = TableFormatter::new();
        let docs = vec![
            doc! {
                "_id": ObjectId::parse_str("65705d84dfc3f3b5094e1f72").unwrap(),
                "user_id": 1i64,
                "nickname": "dalei",
                "oauth2": null,
                "created_time": DateTime::from_millis(1701862788373),
                "age": 20i64,
            },
            doc! {
                "_id": ObjectId::parse_str("65705e2ab6204d1ed051a265").unwrap(),
                "user_id": 2i64,
                "nickname": "dalei",
                "oauth2": null,
                "created_time": DateTime::from_millis(1701862954533),
                "age": 6i64,
            },
        ];

        let result = formatter.format(&ResultData::Documents(docs)).unwrap();

        // Verify the table contains all field names
        assert!(result.contains("_id"));
        assert!(result.contains("user_id"));
        assert!(result.contains("nickname"));
        assert!(result.contains("oauth2"));
        assert!(result.contains("created_time"));
        assert!(result.contains("age"));

        // Verify the table contains data
        assert!(result.contains("dalei"));
        assert!(result.contains("20"));
    }
}
