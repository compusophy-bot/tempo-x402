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
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS used_nonces (
                nonce BLOB PRIMARY KEY,
                recorded_at INTEGER NOT NULL
            );
            PRAGMA journal_mode=WAL;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl NonceStore for SqliteNonceStore {
    fn is_used(&self, nonce: &FixedBytes<32>) -> bool {
        let conn = self.conn.lock().unwrap();
        // Fail-secure: if database query fails, assume nonce IS used to prevent replays.
        // Note: settlement uses try_use() which is atomic and handles errors safely.
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
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // Note: INSERT OR IGNORE means duplicate nonces are silently accepted (idempotent).
        // Errors here are rare (disk full, corruption) and would be caught by try_use() in settlement.
        if let Err(e) = conn.execute(
            "INSERT OR IGNORE INTO used_nonces (nonce, recorded_at) VALUES (?1, ?2)",
            rusqlite::params![nonce.as_slice(), now],
        ) {
            tracing::warn!(error = %e, "failed to record nonce - may allow replay if try_use also fails");
        }
    }

    fn try_use(&self, nonce: FixedBytes<32>) -> bool {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // INSERT will fail on PRIMARY KEY constraint if nonce exists.
        // This is atomic at the database level, safe across processes.
        // Returns true if successfully inserted, false if constraint violation.
        conn.execute(
            "INSERT INTO used_nonces (nonce, recorded_at) VALUES (?1, ?2)",
            rusqlite::params![nonce.as_slice(), now],
        )
        .is_ok()
    }

    fn purge_expired(&self, max_age_secs: u64) -> usize {
        let conn = self.conn.lock().unwrap();
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - max_age_secs as i64;
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
