//! A safe database connection wrapper that uses a retry-with-backoff
//! strategy to resolve 'Resource temporarily unavailable' (Os code 11) database lock panics.

use crate::db::SoulDatabase;
use crate::error::SoulError;
use tracing::warn;
use std::thread::sleep;
use std::time::Duration;

/// A robust wrapper for initializing the SoulDatabase with retries.
pub struct RobustDbManager;

impl RobustDbManager {
    /// Opens the SoulDatabase, retrying on 'Resource temporarily unavailable' locks.
    ///
    /// This is an alternative to `SoulDatabase::new` for environments prone to lock contention.
    pub fn open(path: &str) -> Result<SoulDatabase, SoulError> {
        let mut attempts = 0;
        let max_attempts = 20;

        loop {
            // Using unique memory paths for tests to avoid contention
            let unique_path = if path == ":memory:" {
                format!(":memory:/{:?}", uuid::Uuid::new_v4())
            } else {
                path.to_string()
            };

            match SoulDatabase::new(&unique_path) {
                Ok(db) => return Ok(db),
                Err(e) if attempts < max_attempts && is_lock_error(&e) => {
                    attempts += 1;
                    warn!(attempt = attempts, error = %e, "Database locked, retrying...");
                    sleep(Duration::from_millis(100 * attempts));
                }
                Err(e) => return Err(e),
            }
        }
    }
}

fn is_lock_error(e: &SoulError) -> bool {
    // Sled often wraps its own errors as Io in some versions.
    // We check the full debug representation.
    let msg = format!("{:?}", e);
    
    // Check for explicit "lock" related errors, including Corruption which can occur on interrupted locks
    msg.contains("Resource temporarily unavailable") || 
    msg.contains("code: 11") || 
    msg.contains("WouldBlock") || 
    msg.contains("could not acquire lock") ||
    msg.contains("Corruption")
}
