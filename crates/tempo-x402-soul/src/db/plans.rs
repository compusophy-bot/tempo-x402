// Plan CRUD operations — sled backend.
use super::*;

impl SoulDatabase {
    pub fn insert_plan(&self, plan: &Plan) -> Result<(), SoulError> {
        let value = serde_json::to_vec(plan)?;
        self.plans.insert(plan.id.as_bytes(), value)?;
        Ok(())
    }

    /// Get the currently active plan (if any). There should be at most one.
    pub fn get_active_plan(&self) -> Result<Option<Plan>, SoulError> {
        let mut active: Vec<Plan> = self
            .plans
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Plan>(&v).ok())
            .filter(|p| matches!(p.status, PlanStatus::Active))
            .collect();
        active.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(active.into_iter().next())
    }

    /// Get the plan for a specific goal (most recent).
    pub fn get_plan_for_goal(&self, goal_id: &str) -> Result<Option<Plan>, SoulError> {
        let mut plans: Vec<Plan> = self
            .plans
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Plan>(&v).ok())
            .filter(|p| p.goal_id == goal_id)
            .collect();
        plans.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(plans.into_iter().next())
    }

    /// Update a plan's state (current_step, status, context).
    pub fn update_plan(&self, plan: &Plan) -> Result<bool, SoulError> {
        if self.plans.get(plan.id.as_bytes())?.is_none() {
            return Ok(false);
        }
        let mut updated = plan.clone();
        updated.updated_at = chrono::Utc::now().timestamp();
        let value = serde_json::to_vec(&updated)?;
        self.plans.insert(plan.id.as_bytes(), value)?;
        Ok(true)
    }

    /// Count plans by status (e.g., "failed", "completed", "active").
    pub fn count_plans_by_status(&self, status: &str) -> Result<u64, SoulError> {
        let count = self
            .plans
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Plan>(&v).ok())
            .filter(|p| p.status.as_str() == status)
            .count();
        Ok(count as u64)
    }

    pub fn get_pending_approval_plan(&self) -> Result<Option<Plan>, SoulError> {
        let mut pending: Vec<Plan> = self
            .plans
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Plan>(&v).ok())
            .filter(|p| p.status.as_str() == "pending_approval")
            .collect();
        pending.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(pending.into_iter().next())
    }

    /// Approve a pending plan — set status to 'active'.
    pub fn approve_plan(&self, plan_id: &str) -> Result<bool, SoulError> {
        let Some(raw) = self.plans.get(plan_id.as_bytes())? else {
            return Ok(false);
        };
        let mut plan: Plan = serde_json::from_slice(&raw)?;
        if plan.status.as_str() != "pending_approval" {
            return Ok(false);
        }
        plan.status = PlanStatus::Active;
        plan.updated_at = chrono::Utc::now().timestamp();
        let value = serde_json::to_vec(&plan)?;
        self.plans.insert(plan_id.as_bytes(), value)?;
        Ok(true)
    }

    /// Reject a pending plan — set status to 'abandoned'.
    pub fn reject_plan(&self, plan_id: &str) -> Result<bool, SoulError> {
        let Some(raw) = self.plans.get(plan_id.as_bytes())? else {
            return Ok(false);
        };
        let mut plan: Plan = serde_json::from_slice(&raw)?;
        if plan.status.as_str() != "pending_approval" {
            return Ok(false);
        }
        plan.status = PlanStatus::parse("abandoned").unwrap_or(PlanStatus::Active);
        plan.updated_at = chrono::Utc::now().timestamp();
        let value = serde_json::to_vec(&plan)?;
        self.plans.insert(plan_id.as_bytes(), value)?;
        Ok(true)
    }
}
