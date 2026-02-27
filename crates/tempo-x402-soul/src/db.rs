//! Soul database: separate SQLite for thoughts and state.

use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::SoulError;
use crate::memory::{Thought, ThoughtType};

/// A recorded code mutation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutation {
    pub id: String,
    pub commit_sha: Option<String>,
    pub branch: String,
    pub description: String,
    /// JSON array of file paths.
    pub files_changed: String,
    pub cargo_check_passed: bool,
    pub cargo_test_passed: bool,
    pub created_at: i64,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS thoughts (
    id TEXT PRIMARY KEY,
    thought_type TEXT NOT NULL,
    content TEXT NOT NULL,
    context TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_thoughts_type ON thoughts(thought_type);
CREATE INDEX IF NOT EXISTS idx_thoughts_created ON thoughts(created_at);

CREATE TABLE IF NOT EXISTS soul_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mutations (
    id TEXT PRIMARY KEY,
    commit_sha TEXT,
    branch TEXT NOT NULL,
    description TEXT NOT NULL,
    files_changed TEXT NOT NULL,
    cargo_check_passed INTEGER NOT NULL DEFAULT 0,
    cargo_test_passed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mutations_created ON mutations(created_at);
"#;

/// The soul's dedicated SQLite database.
pub struct SoulDatabase {
    conn: Mutex<Connection>,
}

impl SoulDatabase {
    /// Open (or create) the soul database at the given path.
    pub fn new(path: &str) -> Result<Self, SoulError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Store a thought.
    pub fn insert_thought(&self, thought: &Thought) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO thoughts (id, thought_type, content, context, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                thought.id,
                thought.thought_type.as_str(),
                thought.content,
                thought.context,
                thought.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get the most recent N thoughts, newest first.
    pub fn recent_thoughts(&self, limit: u32) -> Result<Vec<Thought>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, thought_type, content, context, created_at FROM thoughts ORDER BY created_at DESC LIMIT ?1",
        )?;

        let thoughts = stmt
            .query_map(params![limit], |row| {
                let type_str: String = row.get(1)?;
                Ok(Thought {
                    id: row.get(0)?,
                    thought_type: ThoughtType::parse(&type_str).unwrap_or(ThoughtType::Observation),
                    content: row.get(2)?,
                    context: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(thoughts)
    }

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

    /// Record a mutation (code change attempt).
    pub fn insert_mutation(&self, mutation: &Mutation) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO mutations (id, commit_sha, branch, description, files_changed, cargo_check_passed, cargo_test_passed, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                mutation.id,
                mutation.commit_sha,
                mutation.branch,
                mutation.description,
                mutation.files_changed,
                mutation.cargo_check_passed as i32,
                mutation.cargo_test_passed as i32,
                mutation.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get recent mutations, newest first.
    pub fn recent_mutations(&self, limit: u32) -> Result<Vec<Mutation>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, commit_sha, branch, description, files_changed, cargo_check_passed, cargo_test_passed, created_at \
             FROM mutations ORDER BY created_at DESC LIMIT ?1",
        )?;

        let mutations = stmt
            .query_map(params![limit], |row| {
                let check: i32 = row.get(5)?;
                let test: i32 = row.get(6)?;
                Ok(Mutation {
                    id: row.get(0)?,
                    commit_sha: row.get(1)?,
                    branch: row.get(2)?,
                    description: row.get(3)?,
                    files_changed: row.get(4)?,
                    cargo_check_passed: check != 0,
                    cargo_test_passed: test != 0,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mutations)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_retrieve_thoughts() {
        let db = SoulDatabase::new(":memory:").unwrap();

        let thought = Thought {
            id: "t1".to_string(),
            thought_type: ThoughtType::Observation,
            content: "Node has 3 endpoints".to_string(),
            context: Some(r#"{"endpoints": 3}"#.to_string()),
            created_at: 1000,
        };
        db.insert_thought(&thought).unwrap();

        let thought2 = Thought {
            id: "t2".to_string(),
            thought_type: ThoughtType::Reasoning,
            content: "Node is healthy".to_string(),
            context: None,
            created_at: 2000,
        };
        db.insert_thought(&thought2).unwrap();

        let recent = db.recent_thoughts(5).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, "t2"); // newest first
        assert_eq!(recent[1].id, "t1");
    }

    #[test]
    fn test_state_upsert() {
        let db = SoulDatabase::new(":memory:").unwrap();

        assert!(db.get_state("cycles").unwrap().is_none());

        db.set_state("cycles", "1").unwrap();
        assert_eq!(db.get_state("cycles").unwrap().unwrap(), "1");

        db.set_state("cycles", "2").unwrap();
        assert_eq!(db.get_state("cycles").unwrap().unwrap(), "2");
    }
}
