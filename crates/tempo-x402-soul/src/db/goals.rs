// Goal operations.
use super::*;

impl SoulDatabase {
    /// Insert a new goal.
    pub fn insert_goal(&self, goal: &Goal) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        conn.execute(
            "INSERT INTO goals (id, description, status, priority, success_criteria, \
             progress_notes, parent_goal_id, retry_count, created_at, updated_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                goal.id,
                goal.description,
                goal.status.as_str(),
                goal.priority,
                goal.success_criteria,
                goal.progress_notes,
                goal.parent_goal_id,
                goal.retry_count,
                goal.created_at,
                goal.updated_at,
                goal.completed_at,
            ],
        )?;
        Ok(())
    }

    /// Get all active goals, ordered by priority DESC then created_at ASC.
    pub fn get_active_goals(&self) -> Result<Vec<Goal>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, description, status, priority, success_criteria, progress_notes, \
             parent_goal_id, retry_count, created_at, updated_at, completed_at \
             FROM goals WHERE status = 'active' ORDER BY priority DESC, created_at ASC",
        )?;
        let goals = stmt
            .query_map([], Self::row_to_goal)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(goals)
    }

    /// Get a goal by ID (any status).
    pub fn get_goal(&self, id: &str) -> Result<Option<Goal>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let result = conn
            .query_row(
                "SELECT id, description, status, priority, success_criteria, progress_notes, \
                 parent_goal_id, retry_count, created_at, updated_at, completed_at \
                 FROM goals WHERE id = ?1",
                params![id],
                Self::row_to_goal,
            )
            .optional()?;
        Ok(result)
    }

    /// Update a goal's status and/or progress notes.
    pub fn update_goal(
        &self,
        id: &str,
        status: Option<&str>,
        progress_notes: Option<&str>,
        completed_at: Option<i64>,
    ) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE goals SET \
             status = COALESCE(?2, status), \
             progress_notes = COALESCE(?3, progress_notes), \
             completed_at = COALESCE(?4, completed_at), \
             updated_at = ?5 \
             WHERE id = ?1",
            params![id, status, progress_notes, completed_at, now],
        )?;
        Ok(rows > 0)
    }

    /// Increment retry count for a goal.
    pub fn increment_goal_retry(&self, id: &str) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE goals SET retry_count = retry_count + 1, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(rows > 0)
    }

    /// Get recently completed/abandoned goals (for reflection context).
    pub fn recent_finished_goals(&self, limit: u32) -> Result<Vec<Goal>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, description, status, priority, success_criteria, progress_notes, \
             parent_goal_id, retry_count, created_at, updated_at, completed_at \
             FROM goals WHERE status IN ('completed', 'abandoned', 'failed') \
             ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let goals = stmt
            .query_map(params![limit], Self::row_to_goal)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(goals)
    }

    /// Abandon all active goals. Returns number abandoned.
    pub fn abandon_all_active_goals(&self) -> Result<u32, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE goals SET status = 'abandoned', updated_at = ?1, completed_at = ?1 WHERE status = 'active'",
            params![now],
        )? as u32;
        Ok(rows)
    }

    /// Count ALL goals regardless of status (for first-boot seed detection).
    pub fn count_all_goals(&self) -> Result<u64, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM goals", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Get recently abandoned/failed goals (for retread detection).
    pub fn get_recently_abandoned_goals(&self, limit: u32) -> Result<Vec<Goal>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, description, status, priority, success_criteria, progress_notes, \
             parent_goal_id, retry_count, created_at, updated_at, completed_at \
             FROM goals WHERE status IN ('abandoned', 'completed') \
             ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let goals = stmt
            .query_map(params![limit], |row| {
                Ok(Goal {
                    id: row.get(0)?,
                    description: row.get(1)?,
                    status: GoalStatus::parse(&row.get::<_, String>(2)?)
                        .unwrap_or(GoalStatus::Abandoned),
                    priority: row.get(3)?,
                    success_criteria: row.get(4)?,
                    progress_notes: row.get::<_, String>(5).unwrap_or_default(),
                    parent_goal_id: row.get(6)?,
                    retry_count: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    completed_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(goals)
    }

    /// Helper: map a row to a Goal.
    fn row_to_goal(row: &rusqlite::Row) -> Result<Goal, rusqlite::Error> {
        let status_str: String = row.get(2)?;
        Ok(Goal {
            id: row.get(0)?,
            description: row.get(1)?,
            status: GoalStatus::parse(&status_str).unwrap_or(GoalStatus::Active),
            priority: row.get(3)?,
            success_criteria: row.get(4)?,
            progress_notes: row.get(5)?,
            parent_goal_id: row.get(6)?,
            retry_count: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            completed_at: row.get(10)?,
        })
    }
}
