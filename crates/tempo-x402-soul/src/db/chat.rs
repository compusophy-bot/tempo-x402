// Chat session and message operations.
use super::*;

impl SoulDatabase {
    pub fn create_session(&self, title: &str) -> Result<String, SoulError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let session = ChatSession {
            id: id.clone(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            active: true,
        };
        let value = serde_json::to_vec(&session)?;
        self.chat_sessions.insert(id.as_bytes(), value)?;
        Ok(id)
    }

    /// Get or create the default (most recent active) session.
    pub fn get_or_create_default_session(&self) -> Result<String, SoulError> {
        // Find the most recently updated active session
        let existing = self
            .chat_sessions
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<ChatSession>(&v).ok())
            .filter(|s| s.active)
            .max_by_key(|s| s.updated_at);

        if let Some(session) = existing {
            return Ok(session.id);
        }

        // Create a new default session
        self.create_session("Chat")
    }

    /// List recent chat sessions, newest first.
    pub fn list_sessions(&self, limit: u32) -> Result<Vec<ChatSession>, SoulError> {
        let mut sessions: Vec<ChatSession> = self
            .chat_sessions
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<ChatSession>(&v).ok())
            .filter(|s| s.active)
            .collect();

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions.truncate(limit as usize);
        Ok(sessions)
    }

    /// Insert a chat message into a session.
    pub fn insert_chat_message(&self, msg: &ChatMessage) -> Result<(), SoulError> {
        let key = format!("{}:{}", msg.session_id, msg.id);
        let value = serde_json::to_vec(msg)?;
        self.chat_messages.insert(key.as_bytes(), value)?;

        // Touch session updated_at
        let now = chrono::Utc::now().timestamp();
        if let Some(raw) = self.chat_sessions.get(msg.session_id.as_bytes())? {
            let mut session: ChatSession = serde_json::from_slice(&raw)?;
            session.updated_at = now;
            self.chat_sessions
                .insert(msg.session_id.as_bytes(), serde_json::to_vec(&session)?)?;
        }

        Ok(())
    }

    /// Get messages for a session, ordered chronologically, with optional limit.
    pub fn get_session_messages(
        &self,
        session_id: &str,
        limit: u32,
    ) -> Result<Vec<ChatMessage>, SoulError> {
        let prefix = format!("{}:", session_id);
        let mut messages: Vec<ChatMessage> = self
            .chat_messages
            .scan_prefix(prefix.as_bytes())
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<ChatMessage>(&v).ok())
            .collect();

        messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        messages.truncate(limit as usize);
        Ok(messages)
    }
}
