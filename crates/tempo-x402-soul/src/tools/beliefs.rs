//! Belief tools: world model belief and goal updates.
use super::*;

impl ToolExecutor {
    pub(super) async fn update_beliefs(
        &self,
        updates: &[serde_json::Value],
    ) -> Result<ToolResult, String> {
        use crate::world_model::{Belief, BeliefDomain, Confidence, ModelUpdate};

        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        let now = chrono::Utc::now().timestamp();
        let mut applied = 0u32;
        let mut errors = Vec::new();

        for (i, update_val) in updates.iter().enumerate() {
            let update: ModelUpdate = match serde_json::from_value(update_val.clone()) {
                Ok(u) => u,
                Err(e) => {
                    errors.push(format!("update[{i}]: invalid format: {e}"));
                    continue;
                }
            };

            let result = match &update {
                ModelUpdate::Create {
                    domain,
                    subject,
                    predicate,
                    value,
                    evidence,
                } => {
                    let domain = BeliefDomain::parse(domain).unwrap_or(BeliefDomain::Node);
                    let belief = Belief {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain,
                        subject: subject.clone(),
                        predicate: predicate.clone(),
                        value: value.clone(),
                        confidence: Confidence::Medium,
                        evidence: evidence.clone(),
                        confirmation_count: 1,
                        created_at: now,
                        updated_at: now,
                        active: true,
                    };
                    db.upsert_belief(&belief).map(|_| true)
                }
                ModelUpdate::Update {
                    id,
                    value,
                    evidence,
                } => {
                    let beliefs = db.get_all_active_beliefs().map_err(|e| format!("{e}"))?;
                    if let Some(existing) = beliefs.iter().find(|b| b.id == *id) {
                        let updated = Belief {
                            value: value.clone(),
                            evidence: if evidence.is_empty() {
                                existing.evidence.clone()
                            } else {
                                evidence.clone()
                            },
                            updated_at: now,
                            ..existing.clone()
                        };
                        db.upsert_belief(&updated).map(|_| true)
                    } else {
                        Ok(false)
                    }
                }
                ModelUpdate::Confirm { id } => db.confirm_belief(id),
                ModelUpdate::Invalidate { id, reason } => db.invalidate_belief(id, reason),
                // Goal operations
                ModelUpdate::CreateGoal {
                    description,
                    success_criteria,
                    priority,
                    parent_goal_id,
                } => {
                    use crate::world_model::{Goal, GoalStatus};
                    let active_count = db.get_active_goals().map(|g| g.len()).unwrap_or(0);
                    if active_count >= 10 {
                        errors.push(format!("update[{i}]: goal cap reached (10 active)"));
                        continue;
                    }
                    let goal = Goal {
                        id: uuid::Uuid::new_v4().to_string(),
                        description: description.clone(),
                        status: GoalStatus::Active,
                        priority: *priority,
                        success_criteria: success_criteria.clone(),
                        progress_notes: String::new(),
                        parent_goal_id: parent_goal_id.clone(),
                        retry_count: 0,
                        created_at: now,
                        updated_at: now,
                        completed_at: None,
                    };
                    db.insert_goal(&goal).map(|_| true)
                }
                ModelUpdate::UpdateGoal {
                    goal_id,
                    progress_notes,
                    status,
                } => db.update_goal(goal_id, status.as_deref(), progress_notes.as_deref(), None),
                ModelUpdate::CompleteGoal { goal_id, outcome } => {
                    let notes = if outcome.is_empty() {
                        None
                    } else {
                        Some(outcome.as_str())
                    };
                    db.update_goal(goal_id, Some("completed"), notes, Some(now))
                }
                ModelUpdate::AbandonGoal { goal_id, reason } => {
                    db.update_goal(goal_id, Some("abandoned"), Some(reason.as_str()), Some(now))
                }
            };

            match result {
                Ok(true) => applied += 1,
                Ok(false) => errors.push(format!("update[{i}]: no effect (belief not found)")),
                Err(e) => errors.push(format!("update[{i}]: {e}")),
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout = format!("Applied {applied}/{} belief updates", updates.len());
        let stderr = if errors.is_empty() {
            String::new()
        } else {
            errors.join("\n")
        };

        Ok(ToolResult {
            stdout,
            stderr,
            exit_code: if errors.is_empty() { 0 } else { 1 },
            duration_ms,
        })
    }
}
