// Plan CRUD operations (insert, get, update, count, approval).
use super::*;

impl SoulDatabase {
    pub fn insert_plan(&self, plan: &Plan) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let steps_json = serde_json::to_string(&plan.steps)
            .map_err(|e| SoulError::Config(format!("serialize steps: {e}")))?;
        let context_json = serde_json::to_string(&plan.context)
            .map_err(|e| SoulError::Config(format!("serialize context: {e}")))?;
        conn.execute(
            "INSERT INTO plans (id, goal_id, steps, current_step, status, context, replan_count, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                plan.id,
                plan.goal_id,
                steps_json,
                plan.current_step as i64,
                plan.status.as_str(),
                context_json,
                plan.replan_count,
                plan.created_at,
                plan.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Get the currently active plan (if any). There should be at most one.
    pub fn get_active_plan(&self) -> Result<Option<Plan>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let result = conn
            .query_row(
                "SELECT id, goal_id, steps, current_step, status, context, replan_count, created_at, updated_at \
                 FROM plans WHERE status = 'active' ORDER BY created_at DESC LIMIT 1",
                [],
                Self::row_to_plan,
            )
            .optional()?;
        Ok(result)
    }

    /// Get the plan for a specific goal (most recent).
    pub fn get_plan_for_goal(&self, goal_id: &str) -> Result<Option<Plan>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let result = conn
            .query_row(
                "SELECT id, goal_id, steps, current_step, status, context, replan_count, created_at, updated_at \
                 FROM plans WHERE goal_id = ?1 ORDER BY created_at DESC LIMIT 1",
                params![goal_id],
                Self::row_to_plan,
            )
            .optional()?;
        Ok(result)
    }

    /// Update a plan's state (current_step, status, context).
    pub fn update_plan(&self, plan: &Plan) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let steps_json = serde_json::to_string(&plan.steps)
            .map_err(|e| SoulError::Config(format!("serialize steps: {e}")))?;
        let context_json = serde_json::to_string(&plan.context)
            .map_err(|e| SoulError::Config(format!("serialize context: {e}")))?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE plans SET steps = ?2, current_step = ?3, status = ?4, context = ?5, \
             replan_count = ?6, updated_at = ?7 WHERE id = ?1",
            params![
                plan.id,
                steps_json,
                plan.current_step as i64,
                plan.status.as_str(),
                context_json,
                plan.replan_count,
                now,
            ],
        )?;
        Ok(rows > 0)
    }

    /// Count plans by status (e.g., "failed", "completed", "active").
    pub fn count_plans_by_status(&self, status: &str) -> Result<u64, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plans WHERE status = ?1",
            params![status],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn get_pending_approval_plan(&self) -> Result<Option<Plan>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let result = conn
            .query_row(
                "SELECT id, goal_id, steps, current_step, status, context, replan_count, created_at, updated_at \
                 FROM plans WHERE status = 'pending_approval' ORDER BY created_at DESC LIMIT 1",
                [],
                Self::row_to_plan,
            )
            .optional()?;
        Ok(result)
    }

    /// Approve a pending plan — set status to 'active'.
    pub fn approve_plan(&self, plan_id: &str) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE plans SET status = 'active', updated_at = ?1 WHERE id = ?2 AND status = 'pending_approval'",
            params![now, plan_id],
        )?;
        Ok(rows > 0)
    }

    /// Reject a pending plan — set status to 'abandoned'.
    pub fn reject_plan(&self, plan_id: &str) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE plans SET status = 'abandoned', updated_at = ?1 WHERE id = ?2 AND status = 'pending_approval'",
            params![now, plan_id],
        )?;
        Ok(rows > 0)
    }

    fn row_to_plan(row: &rusqlite::Row) -> Result<Plan, rusqlite::Error> {
        let steps_json: String = row.get(2)?;
        let context_json: String = row.get(5)?;
        let status_str: String = row.get(4)?;
        let steps: Vec<PlanStep> = serde_json::from_str(&steps_json).unwrap_or_default();
        let context: std::collections::HashMap<String, String> =
            serde_json::from_str(&context_json).unwrap_or_default();
        Ok(Plan {
            id: row.get(0)?,
            goal_id: row.get(1)?,
            steps,
            current_step: row.get::<_, i64>(3)? as usize,
            status: PlanStatus::parse(&status_str).unwrap_or(PlanStatus::Active),
            context,
            replan_count: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }
}
