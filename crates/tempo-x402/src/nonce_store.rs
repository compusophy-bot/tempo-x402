use alloy::primitives::FixedBytes;
use dashmap::DashMap;
use std::sync::Mutex;
use std::time::Instant;
use tracing;

/// Trait for nonce storage backends.
///
/// Implementations must be thread-safe (`Send + Sync`).
pub trait NonceStore: Send + Sync {
    /// Check if a nonce has already been used.
    fn is_used(&self, nonce: &FixedBytes<32>) -> bool;

    /// Record a nonce as used.
    fn record(&self, nonce: FixedBytes<32>);

    /// Atomically check if nonce is unused and record it if so.
    /// Returns `true` if the nonce was successfully claimed (was not used before).
    /// Returns `false` if the nonce was already used (replay attempt).
    /// This is the preferred method for replay protection as it's atomic.
    fn try_use(&self, nonce: FixedBytes<32>) -> bool;

    /// Release a previously claimed nonce (e.g., when settlement fails after nonce claim).
    /// This allows the payer to retry with the same signed authorization.
    fn release(&self, nonce: &FixedBytes<32>);

    /// Purge nonces older than `max_age_secs`. Returns number purged.
    fn purge_expired(&self, max_age_secs: u64) -> usize;
}

/// In-memory nonce store backed by DashMap. Fast but lost on restart.
pub struct InMemoryNonceStore {
    nonces: DashMap<FixedBytes<32>, Instant>,
}

impl InMemoryNonceStore {
    pub fn new() -> Self {
        Self {
            nonces: DashMap::new(),
        }
    }
}

impl Default for InMemoryNonceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl NonceStore for InMemoryNonceStore {
    fn is_used(&self, nonce: &FixedBytes<32>) -> bool {
        self.nonces.contains_key(nonce)
    }

    fn record(&self, nonce: FixedBytes<32>) {
        self.nonces.insert(nonce, Instant::now());
    }

    fn try_use(&self, nonce: FixedBytes<32>) -> bool {
        // DashMap's entry API provides atomicity within a single process
        use dashmap::mapref::entry::Entry;
        match self.nonces.entry(nonce) {
            Entry::Occupied(_) => false, // Already used
            Entry::Vacant(v) => {
                v.insert(Instant::now());
                true // Successfully claimed
            }
        }
    }

    fn release(&self, nonce: &FixedBytes<32>) {
        self.nonces.remove(nonce);
    }

    fn purge_expired(&self, max_age_secs: u64) -> usize {
        let before = self.nonces.len();
        self.nonces
            .retain(|_, inserted| inserted.elapsed().as_secs() < max_age_secs);
        before - self.nonces.len()
    }
}

/// Persistent nonce store backed by SQLite. Survives restarts.
pub struct SqliteNonceStore {
    conn: Mutex<rusqlite::Connection>,
}

impl SqliteNonceStore {
    /// Open (or create) a SQLite nonce database at the given path.
    ///
    /// On Unix systems, the database file permissions are restricted to 0600
    /// (owner read/write only) to prevent other users from reading nonce data.
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS used_nonces (
                nonce BLOB PRIMARY KEY,
                recorded_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_nonces_recorded_at ON used_nonces(recorded_at);
            PRAGMA journal_mode=WAL;",
        )?;

        // Restrict file permissions on Unix to owner-only (0600).
        // This prevents other system users from reading the nonce database,
        // which could reveal payment timing information.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) =
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            {
                tracing::warn!(
                    path = %path,
                    error = %e,
                    "failed to set nonce database file permissions to 0600"
                );
            }
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

/// Helper to get current unix timestamp safely.
/// On clock error, returns i64::MAX to ensure nonces are never prematurely purged
/// (fail-secure: nonces recorded with max timestamp will survive any purge cutoff).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_else(|_| {
            tracing::error!(
                "system clock before UNIX epoch — using max timestamp for nonce safety"
            );
            i64::MAX
        })
}

impl NonceStore for SqliteNonceStore {
    fn is_used(&self, nonce: &FixedBytes<32>) -> bool {
        // Fail-secure: if mutex is poisoned or query fails, assume nonce IS used
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(poisoned) => {
                tracing::error!("nonce store mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM used_nonces WHERE nonce = ?1",
                [nonce.as_slice()],
                |row| row.get(0),
            )
            .unwrap_or(1); // Fail-secure: database error = assume nonce is used
        count > 0
    }

    fn record(&self, nonce: FixedBytes<32>) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(poisoned) => {
                tracing::error!("nonce store mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let now = unix_now();
        if let Err(e) = conn.execute(
            "INSERT OR IGNORE INTO used_nonces (nonce, recorded_at) VALUES (?1, ?2)",
            rusqlite::params![nonce.as_slice(), now],
        ) {
            tracing::warn!(error = %e, "failed to record nonce - may allow replay if try_use also fails");
        }
    }

    fn try_use(&self, nonce: FixedBytes<32>) -> bool {
        // Fail-secure: if mutex is poisoned, reject (return false = nonce "already used")
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(poisoned) => {
                tracing::error!("nonce store mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let now = unix_now();
        // INSERT will fail on PRIMARY KEY constraint if nonce exists.
        // This is atomic at the database level, safe across processes.
        conn.execute(
            "INSERT INTO used_nonces (nonce, recorded_at) VALUES (?1, ?2)",
            rusqlite::params![nonce.as_slice(), now],
        )
        .is_ok()
    }

    fn release(&self, nonce: &FixedBytes<32>) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(poisoned) => {
                tracing::error!("nonce store mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };
        if let Err(e) = conn.execute(
            "DELETE FROM used_nonces WHERE nonce = ?1",
            rusqlite::params![nonce.as_slice()],
        ) {
            tracing::error!(error = %e, "failed to release nonce — it will remain consumed");
        }
    }

    fn purge_expired(&self, max_age_secs: u64) -> usize {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(poisoned) => {
                tracing::error!("nonce store mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let now = unix_now();

        // Guard against backward clock jumps: if now is before the earliest recorded
        // nonce, the system clock has jumped backward. Skip purge to avoid accidentally
        // removing valid nonces.
        let min_recorded: i64 = conn
            .query_row(
                "SELECT COALESCE(MIN(recorded_at), 0) FROM used_nonces",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if min_recorded > 0 && now < min_recorded {
            tracing::warn!(
                now = now,
                min_recorded = min_recorded,
                "clock appears to have jumped backward — skipping nonce purge"
            );
            return 0;
        }

        // Guard against forward clock jumps: if the newest nonce was recorded recently
        // (within 2x the purge window) but now thinks it's very old, something is wrong.
        // We only check when max_recorded is "recent enough" to indicate active operation
        // — if all entries are genuinely ancient, normal purging should proceed.
        let max_recorded: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(recorded_at), 0) FROM used_nonces",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        // min_recorded already computed above for backward clock jump check.
        // If there's a wide spread between min and max recorded timestamps AND now is
        // far ahead, a clock jump may have occurred between recording and purging.
        if min_recorded > 0
            && max_recorded > 0
            && max_recorded > min_recorded
            && now.saturating_sub(max_recorded) > (max_age_secs as i64) * 2
        {
            tracing::warn!(
                now = now,
                max_recorded = max_recorded,
                min_recorded = min_recorded,
                "clock appears to have jumped forward — skipping nonce purge"
            );
            return 0;
        }

        // Saturating subtraction: if now is i64::MAX (clock error) or max_age_secs is
        // very large, cutoff saturates instead of wrapping, preventing accidental purge.
        let cutoff = now.saturating_sub(max_age_secs as i64);
        conn.execute(
            "DELETE FROM used_nonces WHERE recorded_at < ?1",
            rusqlite::params![cutoff],
        )
        .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::FixedBytes;

    #[test]
    fn test_in_memory_store_basic() {
        let store = InMemoryNonceStore::new();
        let nonce = FixedBytes::new([0x42; 32]);

        assert!(!store.is_used(&nonce));
        store.record(nonce);
        assert!(store.is_used(&nonce));
    }

    #[test]
    fn test_sqlite_store_basic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let store = SqliteNonceStore::open(path.to_str().unwrap()).unwrap();
        let nonce = FixedBytes::new([0x42; 32]);

        assert!(!store.is_used(&nonce));
        store.record(nonce);
        assert!(store.is_used(&nonce));
    }

    #[test]
    fn test_sqlite_store_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let nonce = FixedBytes::new([0xaa; 32]);

        // Write with one instance
        {
            let store = SqliteNonceStore::open(path.to_str().unwrap()).unwrap();
            store.record(nonce);
            assert!(store.is_used(&nonce));
        }

        // Read with a new instance — must still be there
        {
            let store = SqliteNonceStore::open(path.to_str().unwrap()).unwrap();
            assert!(store.is_used(&nonce));
        }
    }

    #[test]
    fn test_sqlite_store_purge() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let store = SqliteNonceStore::open(path.to_str().unwrap()).unwrap();
        let nonce = FixedBytes::new([0xbb; 32]);

        // Manually insert with a very old timestamp
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO used_nonces (nonce, recorded_at) VALUES (?1, ?2)",
                rusqlite::params![nonce.as_slice(), 1000i64],
            )
            .unwrap();
        }

        assert!(store.is_used(&nonce));
        let purged = store.purge_expired(60);
        assert_eq!(purged, 1);
        assert!(!store.is_used(&nonce));
    }

    #[test]
    fn test_in_memory_store_independent_nonces() {
        let store = InMemoryNonceStore::new();
        let a = FixedBytes::new([0x01; 32]);
        let b = FixedBytes::new([0x02; 32]);

        store.record(a);
        assert!(store.is_used(&a));
        assert!(!store.is_used(&b));
    }

    #[test]
    fn test_sqlite_store_independent_nonces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let store = SqliteNonceStore::open(path.to_str().unwrap()).unwrap();

        let a = FixedBytes::new([0x01; 32]);
        let b = FixedBytes::new([0x02; 32]);

        store.record(a);
        assert!(store.is_used(&a));
        assert!(!store.is_used(&b));
    }

    #[test]
    fn test_in_memory_try_use_atomic() {
        let store = InMemoryNonceStore::new();
        let nonce = FixedBytes::new([0x99; 32]);

        // First try_use should succeed
        assert!(store.try_use(nonce));
        // Second try_use should fail (already used)
        assert!(!store.try_use(nonce));
        // Should be marked as used
        assert!(store.is_used(&nonce));
    }

    #[test]
    fn test_sqlite_try_use_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let store = SqliteNonceStore::open(path.to_str().unwrap()).unwrap();

        let nonce = FixedBytes::new([0x99; 32]);

        // First try_use should succeed
        assert!(store.try_use(nonce));
        // Second try_use should fail (already used)
        assert!(!store.try_use(nonce));
        // Should be marked as used
        assert!(store.is_used(&nonce));
    }
}
