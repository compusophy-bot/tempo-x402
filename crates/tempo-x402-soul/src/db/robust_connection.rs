//! A safe database connection wrapper that uses a retry-with-backoff
//! strategy to resolve 'Resource temporarily unavailable' (Os code 11) database lock panics.

use crate::db::SoulDatabase;
use crate::error::SoulError;
use tracing::warn;
use std::thread::sleep;
use std::time::Duration;

use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tokio::time::timeout;

/// A robust wrapper for initializing the SoulDatabase with retries and timeout support.
pub struct RobustDbManager;

impl RobustDbManager {
    /// Opens the SoulDatabase, retrying on 'Resource temporarily unavailable' locks.
    pub async fn open(path: &str) -> Result<SoulDatabase, SoulError> {
        let mut attempts = 0;
        let max_attempts = 10;

        loop {
            // All db opens are now synchronous but executed in tasks.
            // For memory DBs, we explicitly use unique paths if possible, 
            // but for existing tests, we must just retry.
            let path_clone = path.to_string();
            let result = tokio::task::spawn_blocking(move || SoulDatabase::new(&path_clone))
                .await
                .map_err(|e| SoulError::Internal(e.to_string()))?;

            match result {
                Ok(db) => return Ok(db),
                Err(e) if attempts < max_attempts && is_lock_error(&e) => {
                    attempts += 1;
                    warn!(attempt = attempts, error = %e, path = %path, "Database locked, retrying in {}ms...", 100 * attempts);
                    tokio::time::sleep(Duration::from_millis(100 * attempts)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// Helper to detect if a SoulError is a transient locking error.
fn is_lock_error(e: &SoulError) -> bool {
    if let SoulError::Database(ref sled_err) = e {
        let msg = sled_err.to_string();
        return msg.contains("Resource temporarily unavailable") || msg.contains("lock");
    }
    false
}
