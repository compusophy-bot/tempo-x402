//! The thinking loop: periodic observe → think → record cycle with dynamic attention.

use std::sync::Arc;

use crate::config::SoulConfig;
use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::gemini::GeminiClient;
use crate::memory::{Thought, ThoughtType};
use crate::observer::{NodeObserver, NodeSnapshot};

/// The thinking loop that drives the soul.
pub struct ThinkingLoop {
    config: SoulConfig,
    db: Arc<SoulDatabase>,
    gemini: Option<GeminiClient>,
    observer: Arc<dyn NodeObserver>,
}

impl ThinkingLoop {
    /// Create a new thinking loop.
    pub fn new(config: SoulConfig, db: Arc<SoulDatabase>, observer: Arc<dyn NodeObserver>) -> Self {
        let gemini = config.gemini_api_key.as_ref().map(|key| {
            GeminiClient::new(
                key.clone(),
                config.gemini_model_fast.clone(),
                config.gemini_model_think.clone(),
            )
        });

        Self {
            config,
            db,
            gemini,
            observer,
        }
    }

    /// Run the thinking loop forever with adaptive interval.
    pub async fn run(&self) {
        let base_interval = self.config.think_interval_secs as f64;
        let max_interval = base_interval * 10.0;
        let mut current_interval = base_interval;
        let mut prev_snapshot: Option<NodeSnapshot> = None;

        tracing::info!(
            base_interval_secs = self.config.think_interval_secs,
            max_interval_secs = max_interval,
            dormant = self.gemini.is_none(),
            "Soul thinking loop started (dynamic attention)"
        );

        loop {
            let snapshot = match self.observer.observe() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "Soul observe failed");
                    tokio::time::sleep(std::time::Duration::from_secs_f64(current_interval)).await;
                    continue;
                }
            };

            let urgency = compute_urgency(prev_snapshot.as_ref(), &snapshot);

            if let Err(e) = self.think_cycle_with_snapshot(&snapshot).await {
                tracing::warn!(error = %e, "Soul think cycle failed");
            }

            // Adapt interval based on urgency
            if urgency == 0.0 {
                // Nothing changed — exponentially back off
                current_interval = (current_interval * 1.5).min(max_interval);
            } else {
                // Something happened — snap back proportional to urgency
                current_interval = (base_interval / urgency).clamp(base_interval, max_interval);
            }

            tracing::debug!(
                urgency = urgency,
                next_interval_secs = current_interval,
                "Soul attention adjusted"
            );

            prev_snapshot = Some(snapshot);

            tokio::time::sleep(std::time::Duration::from_secs_f64(current_interval)).await;
        }
    }

    /// Execute one think cycle with a pre-captured snapshot.
    async fn think_cycle_with_snapshot(&self, snapshot: &NodeSnapshot) -> Result<(), SoulError> {
        let snapshot_json = serde_json::to_string(snapshot)?;

        // Record observation
        let obs_thought = Thought {
            id: uuid::Uuid::new_v4().to_string(),
            thought_type: ThoughtType::Observation,
            content: format!(
                "Uptime: {}s, Endpoints: {}, Revenue: {}, Payments: {}, Children: {}",
                snapshot.uptime_secs,
                snapshot.endpoint_count,
                snapshot.total_revenue,
                snapshot.total_payments,
                snapshot.children_count,
            ),
            context: Some(snapshot_json.clone()),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.db.insert_thought(&obs_thought)?;

        // If dormant (no API key), stop here
        let gemini = match &self.gemini {
            Some(g) => g,
            None => {
                tracing::debug!("Soul dormant — observation recorded, skipping LLM");
                self.increment_cycle_count()?;
                return Ok(());
            }
        };

        // Build prompt from snapshot + recent thoughts
        let recent = self.db.recent_thoughts(5)?;
        let recent_summary: Vec<String> = recent
            .iter()
            .map(|t| {
                format!(
                    "[{}] {}: {}",
                    t.thought_type.as_str(),
                    chrono::DateTime::from_timestamp(t.created_at, 0)
                        .map(|dt| dt.format("%H:%M:%S").to_string())
                        .unwrap_or_else(|| "?".to_string()),
                    t.content.chars().take(200).collect::<String>()
                )
            })
            .collect();

        let user_prompt = format!(
            "Current node state:\n{}\n\nRecent thoughts:\n{}\n\n\
             Analyze the node's current state. Note any concerns, opportunities, or decisions to consider. \
             If you suggest an action, prefix it with [DECISION].",
            snapshot_json,
            recent_summary.join("\n")
        );

        let system_prompt = format!(
            "{}\n\nYou are generation {} in the node lineage.{}",
            self.config.personality,
            self.config.generation,
            self.config
                .parent_id
                .as_ref()
                .map(|p| format!(" Your parent is {p}."))
                .unwrap_or_default()
        );

        // Think via Gemini
        let response = gemini.think(&system_prompt, &user_prompt).await?;

        // Record reasoning
        let reasoning = Thought {
            id: uuid::Uuid::new_v4().to_string(),
            thought_type: ThoughtType::Reasoning,
            content: response.clone(),
            context: Some(snapshot_json),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.db.insert_thought(&reasoning)?;

        // Extract and record decisions (lines starting with [DECISION])
        for line in response.lines() {
            let trimmed = line.trim();
            if let Some(decision_text) = trimmed.strip_prefix("[DECISION]") {
                let decision = Thought {
                    id: uuid::Uuid::new_v4().to_string(),
                    thought_type: ThoughtType::Decision,
                    content: decision_text.trim().to_string(),
                    context: None,
                    created_at: chrono::Utc::now().timestamp(),
                };
                self.db.insert_thought(&decision)?;
                tracing::info!(
                    decision = decision_text.trim(),
                    "Soul decision recorded (not executed)"
                );
            }
        }

        // Update state
        self.increment_cycle_count()?;

        Ok(())
    }

    /// Increment the total_think_cycles counter and update last_think_at.
    fn increment_cycle_count(&self) -> Result<(), SoulError> {
        let current: u64 = self
            .db
            .get_state("total_think_cycles")?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        self.db
            .set_state("total_think_cycles", &(current + 1).to_string())?;
        self.db
            .set_state("last_think_at", &chrono::Utc::now().timestamp().to_string())?;
        Ok(())
    }
}

/// Compute urgency score (0.0–1.0) by diffing two snapshots.
fn compute_urgency(prev: Option<&NodeSnapshot>, curr: &NodeSnapshot) -> f64 {
    let prev = match prev {
        Some(p) => p,
        None => return 1.0, // First observation ever — full analysis needed
    };

    if prev == curr {
        return 0.0; // Nothing changed
    }

    let mut urgency: f64 = 0.0;

    // Revenue changed
    if prev.total_revenue != curr.total_revenue {
        urgency = urgency.max(0.6);
    }

    // New endpoint registered
    if curr.endpoint_count > prev.endpoint_count {
        urgency = urgency.max(0.5);
    }

    // Endpoint count decreased
    if curr.endpoint_count < prev.endpoint_count {
        urgency = urgency.max(0.7);
    }

    // New child spawned
    if curr.children_count > prev.children_count {
        urgency = urgency.max(0.8);
    }

    // Child lost
    if curr.children_count < prev.children_count {
        urgency = urgency.max(0.9);
    }

    // Payment count changed but revenue didn't (unusual)
    if prev.total_payments != curr.total_payments && prev.total_revenue == curr.total_revenue {
        urgency = urgency.max(0.4);
    }

    urgency
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_snapshot() -> NodeSnapshot {
        NodeSnapshot {
            uptime_secs: 100,
            endpoint_count: 3,
            total_revenue: "1000000".to_string(),
            total_payments: 5,
            children_count: 1,
            wallet_address: Some("0xabc".to_string()),
            instance_id: Some("node-1".to_string()),
            generation: 0,
        }
    }

    #[test]
    fn first_observation_max_urgency() {
        let snap = base_snapshot();
        assert_eq!(compute_urgency(None, &snap), 1.0);
    }

    #[test]
    fn no_change_zero_urgency() {
        let snap = base_snapshot();
        assert_eq!(compute_urgency(Some(&snap), &snap), 0.0);
    }

    #[test]
    fn revenue_change_urgency() {
        let prev = base_snapshot();
        let mut curr = base_snapshot();
        curr.total_revenue = "2000000".to_string();
        assert_eq!(compute_urgency(Some(&prev), &curr), 0.6);
    }

    #[test]
    fn child_lost_highest_urgency() {
        let prev = base_snapshot();
        let mut curr = base_snapshot();
        curr.children_count = 0;
        assert_eq!(compute_urgency(Some(&prev), &curr), 0.9);
    }

    #[test]
    fn new_child_spawned() {
        let prev = base_snapshot();
        let mut curr = base_snapshot();
        curr.children_count = 2;
        assert_eq!(compute_urgency(Some(&prev), &curr), 0.8);
    }

    #[test]
    fn endpoint_added() {
        let prev = base_snapshot();
        let mut curr = base_snapshot();
        curr.endpoint_count = 4;
        assert_eq!(compute_urgency(Some(&prev), &curr), 0.5);
    }

    #[test]
    fn endpoint_removed() {
        let prev = base_snapshot();
        let mut curr = base_snapshot();
        curr.endpoint_count = 2;
        assert_eq!(compute_urgency(Some(&prev), &curr), 0.7);
    }

    #[test]
    fn multiple_signals_takes_highest() {
        let prev = base_snapshot();
        let mut curr = base_snapshot();
        curr.total_revenue = "2000000".to_string(); // 0.6
        curr.children_count = 0; // 0.9
        assert_eq!(compute_urgency(Some(&prev), &curr), 0.9);
    }

    #[test]
    fn uptime_only_change_is_zero() {
        // Uptime always changes but isn't tracked as a signal
        let prev = base_snapshot();
        let mut curr = base_snapshot();
        curr.uptime_secs = 200;
        // PartialEq will differ but no signal matches, so urgency stays 0
        // Actually the snapshots differ so we enter the loop, but no signal fires
        assert_eq!(compute_urgency(Some(&prev), &curr), 0.0);
    }
}
