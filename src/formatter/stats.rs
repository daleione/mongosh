//! Statistics formatting for command execution results
//!
//! This module provides formatting for execution statistics:
//! - Execution time display
//! - Document count tracking
//! - Affected document count
//! - Configurable statistics output

use crate::executor::ExecutionResult;

/// Statistics formatter for command execution
pub struct StatsFormatter {
    /// Show execution time
    show_time: bool,

    /// Show affected count
    show_count: bool,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{ExecutionStats, ResultData};

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
}
