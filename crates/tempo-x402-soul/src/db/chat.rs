// Chat session and message operations.
use super::*;

impl SoulDatabase {
    pub fn create_session(&self, title: &str) -> Result<String, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO chat_sessions (id, title, created_at, updated_at, active) VALUES (?1, ?2, ?3, ?4, 1)",
            params![id, title, now, now],
        )?;
        Ok(id)
    }

    /// Get or create the default (most recent active) session.
    pub fn get_or_create_default_session(&self) -> Result<String, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        // Try to find the most recently updated active session
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM chat_sessions WHERE active = 1 ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(id);
        }

        // Create a new default session
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO chat_sessions (id, title, created_at, updated_at, active) VALUES (?1, 'Chat', ?2, ?3, 1)",
            params![id, now, now],
        )?;
        Ok(id)
    }

    /// List recent chat sessions, newest first.
    pub fn list_sessions(&self, limit: u32) -> Result<Vec<ChatSession>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, active FROM chat_sessions \
             WHERE active = 1 ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let sessions = stmt
            .query_map(params![limit], |row| {
                let active: i32 = row.get(4)?;
                Ok(ChatSession {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    active: active != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(sessions)
    }

    /// Insert a chat message into a session.
    pub fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, tool_executions, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                msg.id,
                msg.session_id,
                msg.role,
                msg.content,
                msg.tool_executions,
                msg.created_at,
            ],
        )?;
        // Touch session updated_at
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE chat_sessions SET updated_at = ?1 WHERE id = ?2",
            params![now, msg.session_id],
        )?;
        Ok(())
    }

    /// Get messages for a session, ordered chronologically, with optional limit.
    pub fn get_session_messages(
        &self,
        session_id: &str,
        limit: u32,
    ) -> Result<Vec<ChatMessage>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, tool_executions, created_at \
             FROM chat_messages WHERE session_id = ?1 ORDER BY created_at ASC LIMIT ?2",
        )?;
        let messages = stmt
            .query_map(params![session_id, limit], |row| {
                Ok(ChatMessage {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    tool_executions: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }
}
