//! Color output support for terminal formatting
//!
//! This module provides colorization functionality for terminal output:
//! - ANSI color codes for different text styles
//! - Colorizer for applying colors to different types of messages
//! - Support for enabling/disabling colors dynamically

/// ANSI color codes for terminal output
pub struct AnsiColors;

#[allow(dead_code)]
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

/// Color scheme for output highlighting
pub struct Colorizer {
    /// Enable colors
    enabled: bool,
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

    /// Colorize text as error (red) with "Error: " prefix
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

    /// Colorize field key (cyan) - for document field names
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn field_key(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::CYAN, text, AnsiColors::RESET)
        } else {
            text.to_string()
        }
    }

    /// Colorize string value (green) with quotes
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text with quotes
    pub fn string(&self, text: &str) -> String {
        if self.enabled {
            format!("{}'{}'{}", AnsiColors::GREEN, text, AnsiColors::RESET)
        } else {
            format!("'{}'", text)
        }
    }

    /// Colorize number value (yellow)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn number(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::YELLOW, text, AnsiColors::RESET)
        } else {
            text.to_string()
        }
    }

    /// Format a BSON type wrapper with separate colors for type name and value
    /// Example: ObjectId('...') -> ObjectId in blue, value in yellow
    ///
    /// # Arguments
    /// * `type_name` - The type wrapper name (e.g., "ObjectId", "Long")
    /// * `value` - The value inside the wrapper
    ///
    /// # Returns
    /// * `String` - Formatted and colorized wrapper
    pub fn type_wrapper(&self, type_name: &str, value: &str) -> String {
        if self.enabled {
            format!(
                "{}{}{}('{}{}{}')",
                AnsiColors::BLUE,
                type_name,
                AnsiColors::RESET,
                AnsiColors::YELLOW,
                value,
                AnsiColors::RESET
            )
        } else {
            format!("{}('{}')", type_name, value)
        }
    }

    /// Format ISODate with separate colors for type name and ISO string
    ///
    /// # Arguments
    /// * `iso_string` - The ISO 8601 date string
    ///
    /// # Returns
    /// * `String` - Formatted and colorized ISODate
    pub fn iso_date(&self, iso_string: &str) -> String {
        if self.enabled {
            format!(
                "{}ISODate{}('{}{}{}')",
                AnsiColors::BLUE,
                AnsiColors::RESET,
                AnsiColors::GREEN,
                iso_string,
                AnsiColors::RESET
            )
        } else {
            format!("ISODate('{}')", iso_string)
        }
    }

    /// Format BinData with separate colors for type name, subtype, and hex data
    ///
    /// # Arguments
    /// * `subtype` - Binary subtype number
    /// * `hex_data` - Hex-encoded binary data
    ///
    /// # Returns
    /// * `String` - Formatted and colorized BinData
    pub fn bin_data(&self, subtype: u8, hex_data: &str) -> String {
        if self.enabled {
            format!(
                "{}BinData{}({}{}{}, '{}{}{}')",
                AnsiColors::MAGENTA,
                AnsiColors::RESET,
                AnsiColors::YELLOW,
                subtype,
                AnsiColors::RESET,
                AnsiColors::CYAN,
                hex_data,
                AnsiColors::RESET
            )
        } else {
            format!("BinData({}, '{}')", subtype, hex_data)
        }
    }

    /// Format RegularExpression with colors
    ///
    /// # Arguments
    /// * `pattern` - Regex pattern
    /// * `options` - Regex options
    ///
    /// # Returns
    /// * `String` - Formatted and colorized regex
    pub fn regex(&self, pattern: &str, options: &str) -> String {
        if self.enabled {
            format!(
                "{}{}{}{}{}{}{}{}",
                AnsiColors::RED,
                "/",
                AnsiColors::YELLOW,
                pattern,
                AnsiColors::RED,
                "/",
                options,
                AnsiColors::RESET
            )
        } else {
            format!("/{}/{}", pattern, options)
        }
    }

    /// Format Timestamp with separate colors for type name and values
    ///
    /// # Arguments
    /// * `time` - Timestamp time value
    /// * `increment` - Timestamp increment value
    ///
    /// # Returns
    /// * `String` - Formatted and colorized Timestamp
    pub fn timestamp(&self, time: u32, increment: u32) -> String {
        if self.enabled {
            format!(
                "{}Timestamp{}({}{}{}, {}{}{})",
                AnsiColors::BLUE,
                AnsiColors::RESET,
                AnsiColors::YELLOW,
                time,
                AnsiColors::RESET,
                AnsiColors::YELLOW,
                increment,
                AnsiColors::RESET
            )
        } else {
            format!("Timestamp({}, {})", time, increment)
        }
    }

    /// Colorize null value (bright black/gray)
    ///
    /// # Arguments
    /// * `text` - Text to colorize
    ///
    /// # Returns
    /// * `String` - Colorized text
    pub fn null(&self, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", AnsiColors::BRIGHT_BLACK, text, AnsiColors::RESET)
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let result = colorizer.string("test");
        assert!(result.contains("\x1b"));
        assert!(result.contains("'test'"));
    }

    #[test]
    fn test_colorizer_field_key() {
        let colorizer = Colorizer::new(true);
        let result = colorizer.field_key("name");
        assert!(result.contains("\x1b[36m")); // cyan
        assert!(result.contains("name"));
    }

    #[test]
    fn test_colorizer_null() {
        let colorizer = Colorizer::new(false);
        let result = colorizer.null("null");
        assert_eq!(result, "null");
    }

    #[test]
    fn test_colorizer_type_wrapper() {
        let colorizer = Colorizer::new(true);
        let result = colorizer.type_wrapper("ObjectId", "65705d84dfc3f3b5094e1f72");
        assert!(result.contains("ObjectId"));
        assert!(result.contains("65705d84dfc3f3b5094e1f72"));
        assert!(result.contains("\x1b[")); // contains ANSI codes
    }

    #[test]
    fn test_colorizer_iso_date() {
        let colorizer = Colorizer::new(false);
        let result = colorizer.iso_date("2023-12-06T11:39:48.373Z");
        assert_eq!(result, "ISODate('2023-12-06T11:39:48.373Z')");
    }
}
