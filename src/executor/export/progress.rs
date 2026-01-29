//! Progress tracking for export operations
//!
//! This module provides progress bar and statistics tracking for long-running
//! export operations, giving users real-time feedback on export progress.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use indicatif::{ProgressBar, ProgressStyle};

/// Progress tracker for export operations
///
/// Tracks document processing progress and displays a progress bar
/// with statistics like speed and ETA.
pub struct ProgressTracker {
    /// Number of documents processed so far
    processed: AtomicU64,
    /// Start time of the operation
    start_time: Instant,
    /// Progress bar (optional, can be disabled)
    bar: Option<ProgressBar>,
}

impl ProgressTracker {
    /// Create a new progress tracker
    ///
    /// # Arguments
    /// * `total` - Total number of documents if known (None for unknown)
    /// * `enable_bar` - Whether to display a progress bar
    ///
    /// # Returns
    /// * `Self` - New progress tracker instance
    pub fn new(total: Option<u64>, enable_bar: bool) -> Self {
        let bar = if enable_bar {
            let pb = match total {
                Some(n) => {
                    let bar = ProgressBar::new(n);
                    bar.set_style(
                        ProgressStyle::default_bar()
                            .template(
                                "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}"
                            )
                            .unwrap()
                            .progress_chars("#>-")
                    );
                    bar
                }
                None => {
                    let bar = ProgressBar::new_spinner();
                    bar.set_style(
                        ProgressStyle::default_spinner()
                            .template(
                                "{spinner:.green} {pos} documents {msg}"
                            )
                            .unwrap()
                    );
                    bar
                }
            };
            Some(pb)
        } else {
            None
        };

        Self {
            processed: AtomicU64::new(0),
            start_time: Instant::now(),
            bar,
        }
    }

    /// Update progress with new count
    ///
    /// # Arguments
    /// * `count` - Total number of documents processed so far
    pub fn update(&self, count: u64) {
        self.processed.store(count, Ordering::Relaxed);

        if let Some(ref bar) = self.bar {
            bar.set_position(count);

            // Calculate and display speed
            let elapsed = self.start_time.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let speed = count as f64 / elapsed;
                bar.set_message(format!("({:.0} docs/sec)", speed));
            }
        }
    }

    /// Finish and clear the progress bar
    pub fn finish(&self) {
        if let Some(ref bar) = self.bar {
            bar.finish_and_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_tracker_with_total() {
        let tracker = ProgressTracker::new(Some(1000), false);
        tracker.update(500);
        // Progress updated successfully (no panic means success)
    }

    #[test]
    fn test_progress_tracker_without_total() {
        let tracker = ProgressTracker::new(None, false);
        tracker.update(500);
        // Progress updated successfully (no panic means success)
    }
}
