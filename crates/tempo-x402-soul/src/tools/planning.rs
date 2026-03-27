//! Planning tools: plan approval, rejection, and request.
use super::*;

impl ToolExecutor {
    /// Approve a pending plan.
    pub(super) async fn approve_plan(&self, plan_id: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        match db.approve_plan(plan_id) {
            Ok(true) => Ok(ToolResult {
                stdout: format!("Plan {plan_id} approved — execution will begin next cycle"),
                stderr: String::new(),
                exit_code: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Ok(false) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("No pending plan with ID {plan_id}"),
                exit_code: 1,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => Err(format!("failed to approve plan: {e}")),
        }
    }

    /// Reject a pending plan with optional reason.
    pub(super) async fn reject_plan(
        &self,
        plan_id: &str,
        reason: &str,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        match db.reject_plan(plan_id) {
            Ok(true) => {
                if !reason.is_empty() {
                    let _ = db.insert_nudge("user", &format!("Plan rejected: {reason}"), 5);
                }
                Ok(ToolResult {
                    stdout: format!("Plan {plan_id} rejected"),
                    stderr: String::new(),
                    exit_code: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
            Ok(false) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("No pending plan with ID {plan_id}"),
                exit_code: 1,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => Err(format!("failed to reject plan: {e}")),
        }
    }

    /// Request a new plan by creating a goal + high-priority nudge.
    pub(super) async fn request_plan(
        &self,
        description: &str,
        priority: u32,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        let now = chrono::Utc::now().timestamp();
        let priority = priority.clamp(1, 5);

        // Create a goal
        let goal = crate::world_model::Goal {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.to_string(),
            status: crate::world_model::GoalStatus::Active,
            priority,
            success_criteria: String::new(),
            progress_notes: String::new(),
            parent_goal_id: None,
            retry_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        db.insert_goal(&goal)
            .map_err(|e| format!("failed to create goal: {e}"))?;

        // Create a high-priority nudge to trigger plan creation next cycle
        let _ = db.insert_nudge("user", &format!("User requested: {description}"), 5);

        Ok(ToolResult {
            stdout: format!(
                "Created goal '{}' (priority {priority}) — plan will be created next cycle",
                &description[..description.len().min(80)]
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
