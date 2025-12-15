//! Color output support for terminal formatting
//!
//! This module provides colorization functionality for terminal output:
//! - ANSI color codes for different text styles
//! - Colorizer for applying colors to different types of messages
//! - Support for enabling/disabling colors dynamically

/// ANSI color codes for terminal output
pub struct AnsiColors;

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
        let result = colorizer.success("test");
        assert!(result.contains("\x1b"));
    }
}
