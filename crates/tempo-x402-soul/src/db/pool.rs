//! Connection pooling for Sled to prevent OS contention and locking errors.
//!
//! Replaces direct `sled::Db` usage (which can hit OS open limits or trigger
//! Resource temporarily unavailable errors) with a thread-safe connection pool.

use crate::db::SoulDatabase;
use crate::error::SoulError;
use dashmap::DashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::sleep;

/// A thread-safe connection pool for the soul's sled databases.
pub struct DatabasePool {
    /// Maps directory paths to initialized database handles, protected by a RwLock
    /// to handle concurrent access without blocking.
    connections: Arc<RwLock<DashMap<String, SoulDatabase>>>,
}

impl DatabasePool {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(DashMap::new())),
        }
    }

    /// Acquires a handle to the database, ensuring it is initialized, with retries.
    pub fn get_db(&self, path: &str) -> Result<SoulDatabase, SoulError> {
        let max_retries = 5;
        let mut attempts = 0;

        while attempts < max_retries {
            // Try read lock first
            {
                let map = self.connections.read().map_err(|_| SoulError::Internal("Lock poisoned".into()))?;
                if let Some(db) = map.get(path) {
                    return Ok(db.clone());
                }
            }

            // Need to write/initialize
            let mut map = self.connections.write().map_err(|_| SoulError::Internal("Lock poisoned".into()))?;
            
            // Check again
            if let Some(db) = map.get(path) {
                return Ok(db.clone());
            }

            match SoulDatabase::new(path) {
                Ok(db) => {
                    map.insert(path.to_string(), db.clone());
                    return Ok(db);
                }
                Err(e) => {
                    if let SoulError::Database(ref sled_err) = e {
                        if sled_err.to_string().contains("Resource temporarily unavailable") {
                            attempts += 1;
                            std::thread::sleep(Duration::from_millis(100 * attempts));
                            continue;
                        }
                    }
                    return Err(e);
                }
            }
        }
        Err(SoulError::Internal("Failed to acquire database connection after retries".to_string()))
    }
}
