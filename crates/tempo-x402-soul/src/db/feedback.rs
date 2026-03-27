// Feedback loop: plan outcomes + capability events
use super::*;

impl SoulDatabase {
    pub fn insert_plan_outcome(
        &self,
        outcome: &crate::feedback::PlanOutcome,
    ) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let steps_succeeded = serde_json::to_string(&outcome.steps_succeeded).unwrap_or_default();
        let steps_failed = serde_json::to_string(&outcome.steps_failed).unwrap_or_default();
        let error_category = outcome.error_category.as_ref().map(|c| c.as_str());

        conn.execute(
            "INSERT INTO plan_outcomes (id, plan_id, goal_id, goal_description, status, \
             steps_succeeded, steps_failed, error_category, error_message, lesson, \
             total_steps, steps_completed, replan_count, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                outcome.id,
                outcome.plan_id,
                outcome.goal_id,
                outcome.goal_description,
                outcome.status,
                steps_succeeded,
                steps_failed,
                error_category,
                outcome.error_message,
                outcome.lesson,
                outcome.total_steps as i64,
                outcome.steps_completed as i64,
                outcome.replan_count,
                outcome.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get recent plan outcomes, ordered newest first.
    pub fn get_recent_plan_outcomes(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::feedback::PlanOutcome>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, plan_id, goal_id, goal_description, status, \
             steps_succeeded, steps_failed, error_category, error_message, lesson, \
             total_steps, steps_completed, replan_count, created_at \
             FROM plan_outcomes ORDER BY created_at DESC LIMIT ?1",
        )?;

        let outcomes = stmt
            .query_map(params![limit], |row| {
                let steps_succeeded: String = row.get(5)?;
                let steps_failed: String = row.get(6)?;
                let error_category_str: Option<String> = row.get(7)?;
                let error_category = error_category_str.map(|s| match s.as_str() {
                    "compile_error" => crate::feedback::ErrorCategory::CompileError,
                    "test_failure" => crate::feedback::ErrorCategory::TestFailure,
                    "file_not_found" => crate::feedback::ErrorCategory::FileNotFound,
                    "shell_error" => crate::feedback::ErrorCategory::ShellError,
                    "network_error" => crate::feedback::ErrorCategory::NetworkError,
                    "protected_file" => crate::feedback::ErrorCategory::ProtectedFile,
                    "endpoint_error" => crate::feedback::ErrorCategory::EndpointError,
                    "git_error" => crate::feedback::ErrorCategory::GitError,
                    "llm_parse_error" => crate::feedback::ErrorCategory::LlmParseError,
                    "unsolvable" => crate::feedback::ErrorCategory::Unsolvable,
                    _ => crate::feedback::ErrorCategory::Unknown,
                });

                Ok(crate::feedback::PlanOutcome {
                    id: row.get(0)?,
                    plan_id: row.get(1)?,
                    goal_id: row.get(2)?,
                    goal_description: row.get(3)?,
                    status: row.get(4)?,
                    steps_succeeded: serde_json::from_str(&steps_succeeded).unwrap_or_default(),
                    steps_failed: serde_json::from_str(&steps_failed).unwrap_or_default(),
                    error_category,
                    error_message: row.get(8)?,
                    lesson: row.get(9)?,
                    total_steps: row.get::<_, i64>(10)? as usize,
                    steps_completed: row.get::<_, i64>(11)? as usize,
                    replan_count: row.get(12)?,
                    created_at: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(outcomes)
    }

    /// Update a plan outcome's status (e.g., reclassify "completed" → "completed_trivial").
    pub fn update_plan_outcome_status(
        &self,
        outcome_id: &str,
        new_status: &str,
    ) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        conn.execute(
            "UPDATE plan_outcomes SET status = ?2 WHERE id = ?1",
            params![outcome_id, new_status],
        )?;
        Ok(())
    }

    /// Count plan outcomes by status (e.g., "completed", "completed_trivial", "failed").
    pub fn count_plan_outcomes_by_status(&self, status: &str) -> Result<u64, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plan_outcomes WHERE status = ?1",
            params![status],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    // ── Capability event operations ──

    /// Insert a capability event.
    pub fn insert_capability_event(
        &self,
        event: &crate::capability::CapabilityEvent,
    ) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO capability_events (id, capability, succeeded, context, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.id,
                event.capability,
                event.succeeded as i32,
                event.context,
                event.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get recent capability events, ordered newest first.
    pub fn get_recent_capability_events(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::capability::CapabilityEvent>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, capability, succeeded, context, created_at \
             FROM capability_events ORDER BY created_at DESC LIMIT ?1",
        )?;

        let events = stmt
            .query_map(params![limit], |row| {
                let succeeded: i32 = row.get(2)?;
                Ok(crate::capability::CapabilityEvent {
                    id: row.get(0)?,
                    capability: row.get(1)?,
                    succeeded: succeeded != 0,
                    context: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }
}
