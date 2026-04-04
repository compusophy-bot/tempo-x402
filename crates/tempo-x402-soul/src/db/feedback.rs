// Feedback loop: plan outcomes + capability events — sled backend.
use super::*;

impl SoulDatabase {
    pub fn insert_plan_outcome(
        &self,
        outcome: &crate::feedback::PlanOutcome,
    ) -> Result<(), SoulError> {
        let value = serde_json::to_vec(outcome)?;
        self.plan_outcomes.insert(outcome.id.as_bytes(), value)?;
        Ok(())
    }

    /// Get recent plan outcomes, ordered newest first.
    pub fn get_recent_plan_outcomes(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::feedback::PlanOutcome>, SoulError> {
        let mut outcomes: Vec<crate::feedback::PlanOutcome> = self
            .plan_outcomes
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice(&v).ok())
            .collect();
        outcomes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        outcomes.truncate(limit as usize);
        Ok(outcomes)
    }

    /// Update a plan outcome's status (e.g., reclassify "completed" → "completed_trivial").
    pub fn update_plan_outcome_status(
        &self,
        outcome_id: &str,
        new_status: &str,
    ) -> Result<(), SoulError> {
        if let Some(raw) = self.plan_outcomes.get(outcome_id.as_bytes())? {
            let mut outcome: crate::feedback::PlanOutcome = serde_json::from_slice(&raw)?;
            outcome.status = new_status.to_string();
            let value = serde_json::to_vec(&outcome)?;
            self.plan_outcomes
                .insert(outcome_id.as_bytes(), value)?;
        }
        Ok(())
    }

    /// Count plan outcomes by status (e.g., "completed", "completed_trivial", "failed").
    pub fn count_plan_outcomes_by_status(&self, status: &str) -> Result<u64, SoulError> {
        let count = self
            .plan_outcomes
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<crate::feedback::PlanOutcome>(&v).ok())
            .filter(|o| o.status == status)
            .count();
        Ok(count as u64)
    }

    // ── Capability event operations ──

    /// Insert a capability event.
    pub fn insert_capability_event(
        &self,
        event: &crate::capability::CapabilityEvent,
    ) -> Result<(), SoulError> {
        let value = serde_json::to_vec(event)?;
        self.capability_events.insert(event.id.as_bytes(), value)?;
        Ok(())
    }

    /// Get recent capability events, ordered newest first.
    pub fn get_recent_capability_events(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::capability::CapabilityEvent>, SoulError> {
        let mut events: Vec<crate::capability::CapabilityEvent> = self
            .capability_events
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice(&v).ok())
            .collect();
        events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        events.truncate(limit as usize);
        Ok(events)
    }
}
