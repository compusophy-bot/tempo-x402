//! Soul state key-value CRUD methods.
use super::*;

impl SoulDatabase {
    /// Get a soul state value by key.
    pub fn get_state(&self, key: &str) -> Result<Option<String>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let value = conn
            .query_row(
                "SELECT value FROM soul_state WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        Ok(value)
    }

    /// Get all soul state key-value pairs matching a prefix.
    pub fn get_all_state(&self) -> Result<Vec<(String, String)>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare("SELECT key, value FROM soul_state")?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Set a soul state value (upsert).
    pub fn set_state(&self, key: &str, value: &str) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO soul_state (key, value, updated_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            params![key, value, now],
        )?;
        Ok(())
    }
}
