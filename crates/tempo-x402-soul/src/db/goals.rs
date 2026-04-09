// Goal operations — sled backend.
use super::*;

impl SoulDatabase {
    /// Insert a new goal.
    pub fn insert_goal(&self, goal: &Goal) -> Result<(), SoulError> {
        let value = serde_json::to_vec(goal)?;
        self.goals.insert(goal.id.as_bytes(), value)?;
        Ok(())
    }

    /// Get all active goals, ordered by priority DESC then created_at ASC.
    pub fn get_active_goals(&self) -> Result<Vec<Goal>, SoulError> {
        let mut goals: Vec<Goal> = self
            .goals
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Goal>(&v).ok())
            .filter(|g| matches!(g.status, GoalStatus::Active))
            .collect();
        goals.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        Ok(goals)
    }

    /// Get a goal by ID (any status).
    pub fn get_goal(&self, id: &str) -> Result<Option<Goal>, SoulError> {
        match self.goals.get(id.as_bytes())? {
            Some(v) => Ok(Some(serde_json::from_slice(&v)?)),
            None => Ok(None),
        }
    }

    /// Update a goal's status and/or progress notes.
    pub fn update_goal(
        &self,
        id: &str,
        status: Option<&str>,
        progress_notes: Option<&str>,
        completed_at: Option<i64>,
    ) -> Result<bool, SoulError> {
        let Some(raw) = self.goals.get(id.as_bytes())? else {
            return Ok(false);
        };
        let mut goal: Goal = serde_json::from_slice(&raw)?;
        if let Some(s) = status {
            goal.status = GoalStatus::parse(s).unwrap_or(goal.status);
        }
        if let Some(notes) = progress_notes {
            goal.progress_notes = notes.to_string();
        }
        if let Some(ts) = completed_at {
            goal.completed_at = Some(ts);
        }
        goal.updated_at = chrono::Utc::now().timestamp();
        let value = serde_json::to_vec(&goal)?;
        self.goals.insert(id.as_bytes(), value)?;
        Ok(true)
    }

    /// Increment retry count for a goal.
    pub fn increment_goal_retry(&self, id: &str) -> Result<bool, SoulError> {
        let Some(raw) = self.goals.get(id.as_bytes())? else {
            return Ok(false);
        };
        let mut goal: Goal = serde_json::from_slice(&raw)?;
        goal.retry_count += 1;
        goal.updated_at = chrono::Utc::now().timestamp();
        let value = serde_json::to_vec(&goal)?;
        self.goals.insert(id.as_bytes(), value)?;
        Ok(true)
    }

    /// Get recently completed/abandoned goals (for reflection context).
    pub fn recent_finished_goals(&self, limit: u32) -> Result<Vec<Goal>, SoulError> {
        let mut goals: Vec<Goal> = self
            .goals
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Goal>(&v).ok())
            .filter(|g| {
                matches!(
                    g.status,
                    GoalStatus::Completed | GoalStatus::Abandoned | GoalStatus::Failed
                )
            })
            .collect();
        goals.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        goals.truncate(limit as usize);
        Ok(goals)
    }

    /// Abandon all active goals. Returns number abandoned.
    pub fn abandon_all_active_goals(&self) -> Result<u32, SoulError> {
        let now = chrono::Utc::now().timestamp();
        let mut count = 0u32;
        for entry in self.goals.iter() {
            let (key, val) = entry?;
            let mut goal: Goal = match serde_json::from_slice(&val) {
                Ok(g) => g,
                Err(_) => continue,
            };
            if matches!(goal.status, GoalStatus::Active) {
                goal.status = GoalStatus::Abandoned;
                goal.completed_at = Some(now);
                goal.updated_at = now;
                let value = serde_json::to_vec(&goal)?;
                self.goals.insert(key, value)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Count ALL goals regardless of status (for first-boot seed detection).
    pub fn count_all_goals(&self) -> Result<u64, SoulError> {
        Ok(self.goals.len() as u64)
    }

    /// Get recently abandoned/failed goals (for retread detection).
    pub fn get_recently_abandoned_goals(&self, limit: u32) -> Result<Vec<Goal>, SoulError> {
        let mut goals: Vec<Goal> = self
            .goals
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Goal>(&v).ok())
            .filter(|g| matches!(g.status, GoalStatus::Abandoned | GoalStatus::Completed))
            .collect();
        goals.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        goals.truncate(limit as usize);
        Ok(goals)
    }
}
