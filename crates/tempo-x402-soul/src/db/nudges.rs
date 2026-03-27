// Nudge queue operations (insert, fetch unprocessed, mark processed).
use super::*;

impl SoulDatabase {
    pub fn insert_nudge(
        &self,
        source: &str,
        content: &str,
        priority: u32,
    ) -> Result<String, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO nudges (id, source, content, priority, created_at, active) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            params![id, source, content, priority, now],
        )?;
        Ok(id)
    }

    /// Get unprocessed nudges, ordered by priority DESC then created_at ASC.
    pub fn get_unprocessed_nudges(&self, limit: u32) -> Result<Vec<Nudge>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, source, content, priority, created_at, processed_at, active \
             FROM nudges WHERE active = 1 AND processed_at IS NULL \
             ORDER BY priority DESC, created_at ASC LIMIT ?1",
        )?;
        let nudges = stmt
            .query_map(params![limit], |row| {
                let active: i32 = row.get(6)?;
                Ok(Nudge {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    content: row.get(2)?,
                    priority: row.get(3)?,
                    created_at: row.get(4)?,
                    processed_at: row.get(5)?,
                    active: active != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(nudges)
    }

    /// Mark a nudge as processed.
    pub fn mark_nudge_processed(&self, id: &str) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE nudges SET processed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }
}
