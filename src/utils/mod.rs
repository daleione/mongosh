//! Utility functions and helpers for mongosh
//!
//! This module provides common utility functions used throughout the application:
//! - String manipulation and formatting
//! - Time and duration utilities
//! - File system helpers
//! - Validation functions
//! - Conversion utilities

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::Result;

/// String utilities
pub mod string {
    /// Truncate string to maximum length
    ///
    /// # Arguments
    /// * `s` - String to truncate
    /// * `max_len` - Maximum length
    ///
    /// # Returns
    /// * `String` - Truncated string with ellipsis if needed
    pub fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        }
    }

    /// Check if string is a valid identifier
    ///
    /// # Arguments
    /// * `s` - String to check
    ///
    /// # Returns
    /// * `bool` - True if valid identifier
    pub fn is_valid_identifier(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }

        let first = s.chars().next().unwrap();
        if !first.is_alphabetic() && first != '_' && first != '$' {
            return false;
        }

        s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    }

    /// Convert snake_case to camelCase
    ///
    /// # Arguments
    /// * `s` - Snake case string
    ///
    /// # Returns
    /// * `String` - Camel case string
    pub fn snake_to_camel(s: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = false;

        for c in s.chars() {
            if c == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(c.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Escape special characters for JSON
    ///
    /// # Arguments
    /// * `s` - String to escape
    ///
    /// # Returns
    /// * `String` - Escaped string
    pub fn escape_json(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }
}

/// Time and duration utilities
pub mod time {
    use super::*;

    /// Get current timestamp in milliseconds
    ///
    /// # Returns
    /// * `u64` - Timestamp in milliseconds
    pub fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis() as u64
    }

    /// Get current timestamp in seconds
    ///
    /// # Returns
    /// * `u64` - Timestamp in seconds
    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }

    /// Format duration as human-readable string
    ///
    /// # Arguments
    /// * `duration` - Duration to format
    ///
    /// # Returns
    /// * `String` - Formatted duration (e.g., "1h 30m 45s")
    pub fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();

        if secs == 0 {
            return format!("{}ms", millis);
        }

        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;

        let mut parts = Vec::new();

        if hours > 0 {
            parts.push(format!("{}h", hours));
        }
        if minutes > 0 {
            parts.push(format!("{}m", minutes));
        }
        if seconds > 0 || parts.is_empty() {
            parts.push(format!("{}s", seconds));
        }

        parts.join(" ")
    }

    /// Parse duration string (e.g., "30s", "5m", "1h")
    ///
    /// # Arguments
    /// * `s` - Duration string
    ///
    /// # Returns
    /// * `Option<Duration>` - Parsed duration or None
    pub fn parse_duration(s: &str) -> Option<Duration> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let (num_str, unit) = s.split_at(s.len() - 1);
        let num: u64 = num_str.parse().ok()?;

        match unit {
            "s" => Some(Duration::from_secs(num)),
            "m" => Some(Duration::from_secs(num * 60)),
            "h" => Some(Duration::from_secs(num * 3600)),
            _ => None,
        }
    }
}

/// File system utilities
pub mod fs {
    use super::*;

    /// Ensure directory exists, create if not
    ///
    /// # Arguments
    /// * `path` - Directory path
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn ensure_dir_exists<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }

    /// Get file extension
    ///
    /// # Arguments
    /// * `path` - File path
    ///
    /// # Returns
    /// * `Option<String>` - File extension or None
    pub fn get_extension<P: AsRef<Path>>(path: P) -> Option<String> {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string())
    }

    /// Check if path is a valid file
    ///
    /// # Arguments
    /// * `path` - Path to check
    ///
    /// # Returns
    /// * `bool` - True if valid file
    pub fn is_valid_file<P: AsRef<Path>>(path: P) -> bool {
        let path = path.as_ref();
        path.exists() && path.is_file()
    }

    /// Expand home directory in path
    ///
    /// # Arguments
    /// * `path` - Path potentially starting with ~
    ///
    /// # Returns
    /// * `PathBuf` - Expanded path
    pub fn expand_home(path: &str) -> PathBuf {
        if path.starts_with("~/")
            && let Some(home) = dirs::home_dir() {
                return home.join(&path[2..]);
            }
        PathBuf::from(path)
    }
}

/// Validation utilities
pub mod validate {
    /// Validate MongoDB database name
    ///
    /// # Arguments
    /// * `name` - Database name to validate
    ///
    /// # Returns
    /// * `bool` - True if valid
    pub fn is_valid_database_name(name: &str) -> bool {
        if name.is_empty() || name.len() > 64 {
            return false;
        }

        let invalid_chars = ['/', '\\', '.', ' ', '"', '$', '*', '<', '>', ':', '|', '?'];
        !name.chars().any(|c| invalid_chars.contains(&c))
    }

    /// Validate MongoDB collection name
    ///
    /// # Arguments
    /// * `name` - Collection name to validate
    ///
    /// # Returns
    /// * `bool` - True if valid
    pub fn is_valid_collection_name(name: &str) -> bool {
        if name.is_empty() || name.len() > 120 {
            return false;
        }

        if name.starts_with("system.") {
            return false;
        }

        let invalid_chars = ['$', '\0'];
        !name.chars().any(|c| invalid_chars.contains(&c))
    }

    /// Validate MongoDB connection URI
    ///
    /// # Arguments
    /// * `uri` - Connection URI to validate
    ///
    /// # Returns
    /// * `bool` - True if valid format
    pub fn is_valid_connection_uri(uri: &str) -> bool {
        uri.starts_with("mongodb://") || uri.starts_with("mongodb+srv://")
    }

    /// Validate field name
    ///
    /// # Arguments
    /// * `name` - Field name to validate
    ///
    /// # Returns
    /// * `bool` - True if valid
    pub fn is_valid_field_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Field names cannot start with $ (except for operators)
        if name.starts_with('$') && !name.starts_with("$set") && !name.starts_with("$inc") {
            return false;
        }

        // Field names cannot contain null character
        !name.contains('\0')
    }
}

/// Conversion utilities
pub mod convert {
    use mongodb::bson::Bson;

    /// Convert Bson value to human-readable string
    ///
    /// # Arguments
    /// * `value` - Bson value
    ///
    /// # Returns
    /// * `String` - String representation
    pub fn bson_to_string(value: &Bson) -> String {
        match value {
            Bson::Double(v) => format!("{}", v),
            Bson::String(v) => v.clone(),
            Bson::Boolean(v) => format!("{}", v),
            Bson::Int32(v) => format!("{}", v),
            Bson::Int64(v) => format!("{}", v),
            Bson::Null => "null".to_string(),
            _ => format!("{:?}", value),
        }
    }

    /// Format bytes as human-readable size
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes
    ///
    /// # Returns
    /// * `String` - Formatted size (e.g., "1.5 MB")
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size, UNITS[unit_index])
        }
    }

    /// Parse human-readable size to bytes
    ///
    /// # Arguments
    /// * `s` - Size string (e.g., "10MB")
    ///
    /// # Returns
    /// * `Option<u64>` - Size in bytes or None
    pub fn parse_bytes(s: &str) -> Option<u64> {
        let s = s.trim().to_uppercase();
        let (num_str, unit) = if s.ends_with("TB") {
            (s.trim_end_matches("TB"), 1024u64.pow(4))
        } else if s.ends_with("GB") {
            (s.trim_end_matches("GB"), 1024u64.pow(3))
        } else if s.ends_with("MB") {
            (s.trim_end_matches("MB"), 1024u64.pow(2))
        } else if s.ends_with("KB") {
            (s.trim_end_matches("KB"), 1024u64)
        } else if s.ends_with('B') {
            (s.trim_end_matches('B'), 1)
        } else {
            return s.parse().ok();
        };

        num_str
            .trim()
            .parse::<f64>()
            .ok()
            .map(|n| (n * unit as f64) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(string::truncate("hello", 10), "hello");
        assert_eq!(string::truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_valid_identifier() {
        assert!(string::is_valid_identifier("myVar"));
        assert!(string::is_valid_identifier("_private"));
        assert!(string::is_valid_identifier("$special"));
        assert!(!string::is_valid_identifier("123invalid"));
        assert!(!string::is_valid_identifier(""));
    }

    #[test]
    fn test_snake_to_camel() {
        assert_eq!(string::snake_to_camel("hello_world"), "helloWorld");
        assert_eq!(string::snake_to_camel("my_var_name"), "myVarName");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(time::format_duration(Duration::from_secs(0)), "0ms");
        assert_eq!(time::format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(time::format_duration(Duration::from_secs(3661)), "1h 1m 1s");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(time::parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(time::parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(time::parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(time::parse_duration("invalid"), None);
    }

    #[test]
    fn test_valid_database_name() {
        assert!(validate::is_valid_database_name("mydb"));
        assert!(validate::is_valid_database_name("test123"));
        assert!(!validate::is_valid_database_name("my/db"));
        assert!(!validate::is_valid_database_name(""));
    }

    #[test]
    fn test_valid_collection_name() {
        assert!(validate::is_valid_collection_name("users"));
        assert!(validate::is_valid_collection_name("my_collection"));
        assert!(!validate::is_valid_collection_name("system.users"));
        assert!(!validate::is_valid_collection_name("invalid$name"));
    }

    #[test]
    fn test_valid_connection_uri() {
        assert!(validate::is_valid_connection_uri(
            "mongodb://localhost:27017"
        ));
        assert!(validate::is_valid_connection_uri(
            "mongodb+srv://cluster.example.com"
        ));
        assert!(!validate::is_valid_connection_uri("http://localhost"));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(convert::format_bytes(500), "500 B");
        assert_eq!(convert::format_bytes(1024), "1.00 KB");
        assert_eq!(convert::format_bytes(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn test_parse_bytes() {
        assert_eq!(convert::parse_bytes("1024"), Some(1024));
        assert_eq!(convert::parse_bytes("1KB"), Some(1024));
        assert_eq!(convert::parse_bytes("1MB"), Some(1024 * 1024));
        assert_eq!(
            convert::parse_bytes("1.5GB"),
            Some((1.5 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
    }
}
