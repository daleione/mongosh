//! Field path and array access parsing for SQL parser
//!
//! This module handles parsing of:
//! - Nested field paths with dot notation (e.g., `user.address.city`)
//! - Array indexing (e.g., `tags[0]`, `items[-1]`)
//! - Array slicing (e.g., `tags[0:5]`, `items[::2]`)

use super::super::sql_context::{ArrayAccessError, ArrayIndex, ArraySlice, FieldPath, SliceIndex};
use super::super::sql_lexer::TokenKind;

impl super::SqlParser {
    /// Parse field path continuation (dots, brackets)
    pub(super) fn parse_field_path_continuation(
        &mut self,
        mut path: FieldPath,
    ) -> std::result::Result<FieldPath, ArrayAccessError> {
        loop {
            match self.peek_kind() {
                Some(TokenKind::Dot) => {
                    self.advance();
                    // Parse nested field
                    if let Some(TokenKind::Ident(field_name)) = self.peek_kind() {
                        let field_name = field_name.clone();
                        self.advance();
                        path = FieldPath::nested(path, field_name);
                    } else {
                        // Incomplete nested field, return what we have
                        break;
                    }
                }
                Some(TokenKind::LBracket) => {
                    self.advance();
                    path = self.parse_array_access(path)?;
                }
                _ => break,
            }
        }
        Ok(path)
    }

    /// Parse array access (index or slice)
    pub(super) fn parse_array_access(
        &mut self,
        base: FieldPath,
    ) -> std::result::Result<FieldPath, ArrayAccessError> {
        // Check for empty brackets
        if self.match_token(&TokenKind::RBracket) {
            return Err(ArrayAccessError::EmptyIndex);
        }

        // Parse first number (could be index or slice start)
        let first_value = self.parse_array_index_or_start()?;

        // Check what comes next
        match self.peek_kind() {
            Some(TokenKind::RBracket) => {
                // Simple index: arr[5]
                self.advance();
                Ok(FieldPath::index(base, first_value))
            }
            Some(TokenKind::Colon) => {
                // Slice: arr[start:end] or arr[start:end:step]
                self.advance();
                let slice = self.parse_array_slice(Some(first_value))?;
                Ok(FieldPath::slice(base, slice))
            }
            _ => {
                if !self.match_token(&TokenKind::RBracket) {
                    Err(ArrayAccessError::MissingCloseBracket)
                } else {
                    Ok(FieldPath::index(base, first_value))
                }
            }
        }
    }

    /// Parse array index or slice start
    pub(super) fn parse_array_index_or_start(
        &mut self,
    ) -> std::result::Result<ArrayIndex, ArrayAccessError> {
        match self.peek_kind() {
            Some(TokenKind::Number(num_str)) => {
                let num_str = num_str.clone();
                self.advance();

                if let Ok(idx) = num_str.parse::<i64>() {
                    if idx >= 0 {
                        Ok(ArrayIndex::positive(idx))
                    } else {
                        Ok(ArrayIndex::negative(idx))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType(num_str))
                }
            }
            Some(TokenKind::Minus) => {
                // Handle negative index with explicit minus sign
                self.advance();
                if let Some(TokenKind::Number(num_str)) = self.peek_kind() {
                    let num_str = num_str.clone();
                    self.advance();
                    if let Ok(idx) = num_str.parse::<i64>() {
                        Ok(ArrayIndex::negative(idx))
                    } else {
                        Err(ArrayAccessError::InvalidIndexType(format!("-{}", num_str)))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType("-".to_string()))
                }
            }
            _ => Err(ArrayAccessError::InvalidIndexType("".to_string())),
        }
    }

    /// Parse array slice after initial colon
    pub(super) fn parse_array_slice(
        &mut self,
        start: Option<ArrayIndex>,
    ) -> std::result::Result<ArraySlice, ArrayAccessError> {
        let start_idx = start.map(|idx| match idx {
            ArrayIndex::Positive(n) => SliceIndex::Positive(n),
            ArrayIndex::Negative(n) => SliceIndex::Negative(n),
        });

        // Check for immediate closing bracket (start:)
        if self.match_token(&TokenKind::RBracket) {
            return Ok(ArraySlice::new(start_idx, None, None));
        }

        // Parse end index if present
        let end_idx = if matches!(self.peek_kind(), Some(TokenKind::Colon)) {
            // Another colon means no end specified (:end:step or ::step)
            None
        } else {
            // Parse end index
            self.parse_slice_index().ok()
        };

        // Check for step
        let step = if self.match_token(&TokenKind::Colon) {
            // Parse step
            if self.match_token(&TokenKind::RBracket) {
                // No step specified, use default
                None
            } else {
                match self.peek_kind() {
                    Some(TokenKind::Number(num_str)) => {
                        let num_str = num_str.clone();
                        self.advance();
                        if let Ok(step_val) = num_str.parse::<i64>() {
                            if step_val == 0 {
                                return Err(ArrayAccessError::ZeroStepSize);
                            }
                            Some(step_val)
                        } else {
                            return Err(ArrayAccessError::InvalidSliceSyntax(format!(
                                "Invalid step value: {}",
                                num_str
                            )));
                        }
                    }
                    _ => None,
                }
            }
        } else {
            None
        };

        // Expect closing bracket
        if !self.match_token(&TokenKind::RBracket) {
            return Err(ArrayAccessError::MissingCloseBracket);
        }

        Ok(ArraySlice::new(start_idx, end_idx, step))
    }

    /// Parse a slice index (positive or negative)
    pub(super) fn parse_slice_index(
        &mut self,
    ) -> std::result::Result<SliceIndex, ArrayAccessError> {
        match self.peek_kind() {
            Some(TokenKind::Number(num_str)) => {
                let num_str = num_str.clone();
                self.advance();

                if let Ok(idx) = num_str.parse::<i64>() {
                    if idx >= 0 {
                        Ok(SliceIndex::Positive(idx))
                    } else {
                        Ok(SliceIndex::Negative(idx.abs()))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType(num_str))
                }
            }
            Some(TokenKind::Minus) => {
                // Handle negative slice index
                self.advance();
                if let Some(TokenKind::Number(num_str)) = self.peek_kind() {
                    let num_str = num_str.clone();
                    self.advance();
                    if let Ok(idx) = num_str.parse::<i64>() {
                        Ok(SliceIndex::Negative(idx))
                    } else {
                        Err(ArrayAccessError::InvalidIndexType(format!("-{}", num_str)))
                    }
                } else {
                    Err(ArrayAccessError::InvalidIndexType("-".to_string()))
                }
            }
            _ => Err(ArrayAccessError::InvalidIndexType("".to_string())),
        }
    }
}
