//! Diagnostic tools for measuring execution time and lock contention.
//! Specifically designed to track and debug resource availability issues.

use std::time::{Duration, Instant};
use tracing::{info, warn};

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
        } else {
            info!(
                target: "benchmark_utils",
                "{} took {}ms",
                self.label,
                elapsed.as_millis()
            );
        }
    }
}


/// Get current memory usage. Note: This requires system-specific implementation
/// or parsing /proc/self/stat on Linux.
pub fn get_memory_usage_kb() -> Option<u64> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open("/proc/self/statm").ok()?;
    let reader = BufReader::new(file);
    let line = reader.lines().next()?.ok()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    // The second field is resident set size in pages
    let pages = parts.get(1)?.parse::<u64>().ok()?;
    let page_size = 4096; // Assuming 4KB page size
    Some(pages * page_size / 1024)
}

/// A wrapper to measure execution time and log memory usage.
pub struct ResourceMonitor {
    label: String,
    start: Instant,
    start_mem: Option<u64>,
}

impl ResourceMonitor {
    pub fn new(label: &str) -> Self {
        let start_mem = get_memory_usage_kb();
        info!(target: "benchmark_utils", "{} starting (mem: {:?} KB)", label, start_mem);
        Self {
            label: label.to_string(),
            start: Instant::now(),
            start_mem,
        }
    }
}

impl Drop for ResourceMonitor {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let end_mem = get_memory_usage_kb();
        info!(
            target: "benchmark_utils",
            "{} finished in {}ms (mem: {:?} KB, diff: {:?})",
            self.label,
            elapsed.as_millis(),
            end_mem,
            end_mem.zip(self.start_mem).map(|(e, s)| e as i64 - s as i64)
        );
    }
}

