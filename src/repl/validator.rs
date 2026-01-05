//! Validator for reedline - validates line completeness

use reedline::{ValidationResult, Validator};

/// MongoDB validator for reedline
pub struct MongoValidator;

impl MongoValidator {
    /// Create a new MongoDB validator
    pub fn new() -> Self {
        Self
    }

    /// Check if input has balanced braces and parentheses
    fn is_balanced(&self, input: &str) -> bool {
        let mut brace_count = 0;
        let mut paren_count = 0;
        let mut bracket_count = 0;
        let mut in_string = false;
        let mut escape_next = false;
        let mut string_char = ' ';

        for ch in input.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            if ch == '\\' {
                escape_next = true;
                continue;
            }

            // Handle string literals
            if ch == '"' || ch == '\'' {
                if in_string && ch == string_char {
                    in_string = false;
                } else if !in_string {
                    in_string = true;
                    string_char = ch;
                }
                continue;
            }

            // Skip counting if inside string
            if in_string {
                continue;
            }

            match ch {
                '{' => brace_count += 1,
                '}' => brace_count -= 1,
                '(' => paren_count += 1,
                ')' => paren_count -= 1,
                '[' => bracket_count += 1,
                ']' => bracket_count -= 1,
                _ => {}
            }
        }

        !in_string && brace_count == 0 && paren_count == 0 && bracket_count == 0
    }
}

impl Default for MongoValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator for MongoValidator {
    /// Validate input for completeness
    ///
    /// # Arguments
    /// * `line` - The input line to validate
    ///
    /// # Returns
    /// * `ValidationResult` - Whether the input is complete, incomplete, or invalid
    fn validate(&self, line: &str) -> ValidationResult {
        let trimmed = line.trim();

        // Empty input is valid
        if trimmed.is_empty() {
            return ValidationResult::Complete;
        }

        // Check for balanced braces and parentheses
        if !self.is_balanced(trimmed) {
            return ValidationResult::Incomplete;
        }

        // If balanced, consider it complete
        ValidationResult::Complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let validator = MongoValidator::new();
        assert_eq!(validator.validate(""), ValidationResult::Complete);
        assert_eq!(validator.validate("   "), ValidationResult::Complete);
    }

    #[test]
    fn test_simple_command() {
        let validator = MongoValidator::new();
        assert_eq!(validator.validate("show dbs"), ValidationResult::Complete);
        assert_eq!(validator.validate("use test"), ValidationResult::Complete);
    }

    #[test]
    fn test_balanced_braces() {
        let validator = MongoValidator::new();
        assert_eq!(
            validator.validate("db.users.find({})"),
            ValidationResult::Complete
        );
        assert_eq!(
            validator.validate("db.users.insertOne({name: 'test'})"),
            ValidationResult::Complete
        );
    }

    #[test]
    fn test_unbalanced_braces() {
        let validator = MongoValidator::new();
        assert_eq!(
            validator.validate("db.users.find({"),
            ValidationResult::Incomplete
        );
        assert_eq!(
            validator.validate("db.users.find("),
            ValidationResult::Incomplete
        );
    }

    #[test]
    fn test_nested_braces() {
        let validator = MongoValidator::new();
        assert_eq!(
            validator.validate("db.users.find({filter: {age: {$gt: 18}}})"),
            ValidationResult::Complete
        );
        assert_eq!(
            validator.validate("db.users.find({filter: {age: {$gt: 18}}}"),
            ValidationResult::Incomplete
        );
    }

    #[test]
    fn test_string_literals() {
        let validator = MongoValidator::new();

        // Braces inside strings should be ignored
        assert_eq!(
            validator.validate(r#"db.users.find({name: "{test}"})"#),
            ValidationResult::Complete
        );

        // Unclosed string
        assert_eq!(
            validator.validate(r#"db.users.find({name: "test)"#),
            ValidationResult::Incomplete
        );
    }

    #[test]
    fn test_escaped_quotes() {
        let validator = MongoValidator::new();
        assert_eq!(
            validator.validate(r#"db.users.find({name: "test\"quote"})"#),
            ValidationResult::Complete
        );
    }

    #[test]
    fn test_mixed_brackets() {
        let validator = MongoValidator::new();
        assert_eq!(
            validator.validate("db.users.aggregate([{$match: {age: 18}}])"),
            ValidationResult::Complete
        );
        assert_eq!(
            validator.validate("db.users.aggregate([{$match: {age: 18}}"),
            ValidationResult::Incomplete
        );
    }
}
