//! The thinking loop: periodic observe → think → record cycle.

use std::sync::Arc;

use crate::config::SoulConfig;
use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::gemini::GeminiClient;
use crate::memory::{Thought, ThoughtType};
use crate::observer::NodeObserver;

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

    /// Run the thinking loop forever.
    pub async fn run(&self) {
        let interval = std::time::Duration::from_secs(self.config.think_interval_secs);
        tracing::info!(
            interval_secs = self.config.think_interval_secs,
            dormant = self.gemini.is_none(),
            "Soul thinking loop started"
        );

        loop {
            if let Err(e) = self.think_cycle().await {
                tracing::warn!(error = %e, "Soul think cycle failed");
            }
            tokio::time::sleep(interval).await;
        }
    }

    /// Execute one think cycle: observe → think → record.
    async fn think_cycle(&self) -> Result<(), SoulError> {
        // 1. Observe
        let snapshot = self.observer.observe()?;
        let snapshot_json = serde_json::to_string(&snapshot)?;

        // 2. Record observation
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

        // 3. If dormant (no API key), stop here
        let gemini = match &self.gemini {
            Some(g) => g,
            None => {
                tracing::debug!("Soul dormant — observation recorded, skipping LLM");
                self.increment_cycle_count()?;
                return Ok(());
            }
        };

        // 4. Build prompt from snapshot + recent thoughts
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

        // 5. Think via Gemini
        let response = gemini.think(&system_prompt, &user_prompt).await?;

        // 6. Record reasoning
        let reasoning = Thought {
            id: uuid::Uuid::new_v4().to_string(),
            thought_type: ThoughtType::Reasoning,
            content: response.clone(),
            context: Some(snapshot_json),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.db.insert_thought(&reasoning)?;

        // 7. Extract and record decisions (lines starting with [DECISION])
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

        // 8. Update state
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
