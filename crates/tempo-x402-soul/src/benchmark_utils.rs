//! Diagnostic tools for measuring execution time and lock contention.

use std::time::{Duration, Instant};
use tracing::warn;

/// A utility to track duration of an operation and log if it exceeds a threshold.
pub struct LockMonitor {
    label: String,
    start: Instant,
    threshold: Duration,
}

impl LockMonitor {
    /// Start tracking a block of code.
    pub fn new(label: &str, threshold_ms: u64) -> Self {
        Self {
            label: label.to_string(),
            start: Instant::now(),
            threshold: Duration::from_millis(threshold_ms),
        }
    }
}

impl Drop for LockMonitor {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        if elapsed >= self.threshold {
            warn!(
                target: "benchmark_utils",
                "Performance Warning: {} took {}ms (threshold: {}ms)",
                self.label,
                elapsed.as_millis(),
                self.threshold.as_millis()
            );
        }
    }
}
