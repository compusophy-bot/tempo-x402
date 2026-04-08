// Housekeeping: flush, pruning, resets (sled-backed, lock-free).
use super::*;

impl SoulDatabase {
    /// Flush pending writes to disk. Replaces SQLite WAL checkpoint.
    pub fn wal_checkpoint(&self) -> Result<(), SoulError> {
        self.db.flush()?;
        Ok(())
    }

    /// Lifecycle pruning: keep the database bounded. Called every 10 cycles from housekeeping.
    /// Life is birth AND death — things that served their purpose must be released.
    pub fn prune_old_data(&self) -> Result<PruneStats, SoulError> {
        let now = chrono::Utc::now().timestamp();
        let one_day = 86400i64;
        let three_days = one_day * 3;
        let seven_days = one_day * 7;

        // 1. Cap total thoughts at 500 — delete oldest beyond cap
        let thoughts_pruned = prune_tree_by_cap::<crate::memory::Thought>(
            &self.thoughts,
            500,
            |t| t.created_at,
        );

        // 2. Delete completed/abandoned goals older than 7 days (keep last 10 regardless of age)
        let goals_pruned = {
            let mut items: Vec<(String, Goal)> = Vec::new();
            for entry in self.goals.iter() {
                let (key, val) = entry?;
                if let Ok(goal) = serde_json::from_slice::<Goal>(&val) {
                    let key_str = String::from_utf8(key.to_vec()).unwrap_or_default();
                    items.push((key_str, goal));
                }
            }
            // Only consider completed/abandoned goals
            let mut terminal: Vec<(String, Goal)> = items
                .into_iter()
                .filter(|(_, g)| {
                    matches!(g.status, GoalStatus::Completed | GoalStatus::Abandoned)
                })
                .collect();
            // Sort by updated_at DESC to keep the 10 most recent
            terminal.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
            let mut count = 0u32;
            for (key, goal) in terminal.iter().skip(10) {
                if goal.created_at < now - seven_days {
                    let _ = self.goals.remove(key.as_bytes());
                    count += 1;
                }
            }
            count
        };

        // 3. Delete completed/failed/abandoned plans older than 3 days (keep last 10)
        let plans_pruned = {
            let mut items: Vec<(String, Plan)> = Vec::new();
            for entry in self.plans.iter() {
                let (key, val) = entry?;
                if let Ok(plan) = serde_json::from_slice::<Plan>(&val) {
                    let key_str = String::from_utf8(key.to_vec()).unwrap_or_default();
                    items.push((key_str, plan));
                }
            }
            let mut terminal: Vec<(String, Plan)> = items
                .into_iter()
                .filter(|(_, p)| {
                    matches!(
                        p.status,
                        PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Abandoned
                    )
                })
                .collect();
            terminal.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
            let mut count = 0u32;
            for (key, plan) in terminal.iter().skip(10) {
                if plan.created_at < now - three_days {
                    let _ = self.plans.remove(key.as_bytes());
                    count += 1;
                }
            }
            count
        };

        // 4. Cap mutations at 50 — delete oldest beyond cap
        let mutations_pruned =
            prune_tree_by_cap::<Mutation>(&self.mutations, 50, |m| m.created_at);

        // 5. Delete processed nudges older than 24h
        let nudges_pruned = {
            let mut count = 0u32;
            for entry in self.nudges.iter() {
                let (key, val) = entry?;
                if let Ok(nudge) = serde_json::from_slice::<Nudge>(&val) {
                    if nudge.processed_at.is_some() && nudge.created_at < now - one_day {
                        let _ = self.nudges.remove(key);
                        count += 1;
                    }
                }
            }
            count
        };

        // 6. Delete inactive beliefs older than 3 days
        let beliefs_pruned = {
            let mut count = 0u32;
            for entry in self.beliefs.iter() {
                let (key, val) = entry?;
                if let Ok(belief) = serde_json::from_slice::<Belief>(&val) {
                    if !belief.active && belief.updated_at < now - three_days {
                        let _ = self.beliefs.remove(key);
                        count += 1;
                    }
                }
            }
            count
        };

        // 7. Cap chat messages per session (keep last 100 per session)
        let messages_pruned = {
            // Group messages by session_id
            let mut by_session: HashMap<String, Vec<(String, ChatMessage)>> = HashMap::new();
            for entry in self.chat_messages.iter() {
                let (key, val) = entry?;
                if let Ok(msg) = serde_json::from_slice::<ChatMessage>(&val) {
                    let key_str = String::from_utf8(key.to_vec()).unwrap_or_default();
                    by_session
                        .entry(msg.session_id.clone())
                        .or_default()
                        .push((key_str, msg));
                }
            }
            let mut count = 0u32;
            for (_session_id, mut msgs) in by_session {
                if msgs.len() <= 100 {
                    continue;
                }
                // Sort by created_at DESC, remove beyond 100
                msgs.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));
                for (key, _) in msgs.into_iter().skip(100) {
                    let _ = self.chat_messages.remove(key.as_bytes());
                    count += 1;
                }
            }
            count
        };

        // 8. Delete old inactive chat sessions (keep last 20)
        let sessions_pruned = {
            let mut inactive: Vec<(String, ChatSession)> = Vec::new();
            for entry in self.chat_sessions.iter() {
                let (key, val) = entry?;
                if let Ok(session) = serde_json::from_slice::<ChatSession>(&val) {
                    if !session.active {
                        let key_str = String::from_utf8(key.to_vec()).unwrap_or_default();
                        inactive.push((key_str, session));
                    }
                }
            }
            // Sort by updated_at DESC, keep top 20
            inactive.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
            let mut count = 0u32;
            for (key, _) in inactive.into_iter().skip(20) {
                let _ = self.chat_sessions.remove(key.as_bytes());
                count += 1;
            }
            count
        };

        // 9. Cap plan_outcomes at 100
        let _ = prune_tree_by_cap::<crate::feedback::PlanOutcome>(
            &self.plan_outcomes,
            100,
            |o| o.created_at,
        );

        // 10. Cap capability_events at 500
        let _ = prune_tree_by_cap::<crate::capability::CapabilityEvent>(
            &self.capability_events,
            500,
            |e| e.created_at,
        );

        // 11. Cap benchmark_runs at 200
        let _ = prune_tree_by_cap::<crate::benchmark::BenchmarkRun>(
            &self.benchmark_runs,
            200,
            |r| r.created_at,
        );

        // 12. Prune events (tiered retention)
        let mut events_pruned = 0u32;
        let thirty_days = one_day * 30;
        for entry in self.events.iter() {
            let (key, val) = match entry {
                Ok(kv) => kv,
                Err(_) => continue,
            };
            let event = match serde_json::from_slice::<SoulEvent>(&val) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let should_delete = match event.level.as_str() {
                "debug" => event.created_at < now - one_day,
                "info" => event.created_at < now - three_days,
                "warn" => {
                    if event.resolved {
                        event.created_at < now - three_days
                    } else {
                        event.created_at < now - seven_days
                    }
                }
                "error" => {
                    if event.resolved {
                        event.created_at < now - seven_days
                    } else {
                        event.created_at < now - thirty_days
                    }
                }
                _ => false,
            };
            if should_delete {
                let _ = self.events.remove(key);
                events_pruned += 1;
            }
        }
        // Hard cap events at 5000
        events_pruned += prune_tree_by_cap::<SoulEvent>(&self.events, 5000, |e| e.created_at);

        Ok(PruneStats {
            thoughts: thoughts_pruned,
            goals: goals_pruned,
            plans: plans_pruned,
            mutations: mutations_pruned,
            nudges: nudges_pruned,
            beliefs: beliefs_pruned,
            messages: messages_pruned,
            sessions: sessions_pruned,
            events: events_pruned,
        })
    }

    /// Reset historical data: clear old thoughts, failed/completed plans, processed nudges,
    /// and reset cycle counters. Keeps: active goals, active beliefs, active plans.
    /// Returns counts of what was deleted.
    pub fn reset_history(&self) -> Result<(u64, u64, u64), SoulError> {
        // Delete all thoughts
        let thoughts_deleted = self.thoughts.len() as u64;
        self.thoughts.clear()?;

        // Delete ALL plans — including active ones that may be stuck
        let plans_deleted = self.plans.len() as u64;
        self.plans.clear()?;

        // Delete processed nudges
        let mut nudges_deleted = 0u64;
        let mut to_remove = Vec::new();
        for entry in self.nudges.iter() {
            let (key, val) = entry?;
            if let Ok(nudge) = serde_json::from_slice::<Nudge>(&val) {
                if nudge.processed_at.is_some() {
                    to_remove.push(key.to_vec());
                }
            }
        }
        for key in to_remove {
            let _ = self.nudges.remove(key);
            nudges_deleted += 1;
        }

        // Clear the active plan pointer + reset cycle counters
        let _ = self.set_state("active_plan_id", "");
        let _ = self.set_state("total_think_cycles", "0");
        let _ = self.set_state("cycles_since_last_commit", "0");
        let _ = self.set_state("recent_errors", "[]");

        Ok((thoughts_deleted, plans_deleted, nudges_deleted))
    }

    /// Reset deploy-specific counters when the build SHA changes.
    /// Preserves: brain weights, benchmark scores, capability profiles, lessons, peer data.
    /// Resets: cycle counters, stagnation state, error lists, benchmark cooldowns.
    pub fn reset_deploy_counters(&self, build_sha: &str) -> bool {
        // Check if build changed
        let last_build = self
            .get_state("last_deploy_build")
            .ok()
            .flatten()
            .unwrap_or_default();

        if last_build == build_sha {
            return false; // Same build, no reset needed
        }

        tracing::info!(
            old_build = %last_build,
            new_build = %build_sha,
            "New deploy detected — resetting ephemeral counters"
        );

        // Reset cycle counters
        let _ = self.set_state("total_think_cycles", "0");
        let _ = self.set_state("cycles_since_last_commit", "0");
        let _ = self.set_state("recent_errors", "[]");
        let _ = self.set_state("last_benchmark_at", "0");
        let _ = self.set_state("last_benchmark_cycle", "0");

        // Clear stagnation markers
        let _ = self.set_state("stagnation_resets", "0");

        // Record this deploy
        let _ = self.set_state("last_deploy_build", build_sha);
        let _ = self.set_state(
            "last_deploy_at",
            &chrono::Utc::now().timestamp().to_string(),
        );

        // Clean up old thoughts
        let _ = self.thoughts.clear();

        // Remove completed/failed/abandoned plans
        let mut to_remove = Vec::new();
        for entry in self.plans.iter().flatten() {
            let (key, val) = entry;
            if let Ok(plan) = serde_json::from_slice::<Plan>(&val) {
                if matches!(
                    plan.status,
                    PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Abandoned
                ) {
                    to_remove.push(key.to_vec());
                }
            }
        }
        for key in to_remove {
            let _ = self.plans.remove(key);
        }

        // Remove processed nudges
        let mut to_remove = Vec::new();
        for entry in self.nudges.iter().flatten() {
            let (key, val) = entry;
            if let Ok(nudge) = serde_json::from_slice::<Nudge>(&val) {
                if nudge.processed_at.is_some() {
                    to_remove.push(key.to_vec());
                }
            }
        }
        for key in to_remove {
            let _ = self.nudges.remove(key);
        }

        // Abandon any active goals from old deploy — they may reference old code
        for entry in self.goals.iter().flatten() {
            let (key, val) = entry;
            if let Ok(mut goal) = serde_json::from_slice::<Goal>(&val) {
                if matches!(goal.status, GoalStatus::Active) {
                    goal.status = GoalStatus::Abandoned;
                    if let Ok(json) = serde_json::to_vec(&goal) {
                        let _ = self.goals.insert(key, json);
                    }
                }
            }
        }

        true
    }

    /// Full cognitive reset — wipe ALL learned state when the architecture changes.
    /// This is triggered when brain architecture version changes (e.g., 23K→89K params).
    /// Preserves: benchmark solutions (hard-won), benchmark history (ELO tracking).
    /// Wipes: brain weights, cortex, genesis, hivemind, synthesis, plan outcomes,
    /// capability events, durable rules, failure chains, thoughts, plans, goals.
    pub fn reset_cognitive_architecture(&self, version: &str) -> bool {
        let last_version = self
            .get_state("cognitive_architecture_version")
            .ok()
            .flatten()
            .unwrap_or_default();

        if last_version == version {
            return false; // Same architecture, no reset needed
        }

        // Empty old_version = fresh DB (first boot or /tmp wipe).
        // NOT an architecture change. Just set the version and move on.
        if last_version.is_empty() {
            let _ = self.set_state("cognitive_architecture_version", version);
            tracing::info!(version, "Cognitive architecture version set (first boot)");
            return false;
        }

        tracing::warn!(
            old_version = %last_version,
            new_version = %version,
            "Cognitive architecture changed — FULL RESET of learned state"
        );

        // Wipe all learned behavioral data (trained on old architecture)
        let _ = self.thoughts.clear();
        let _ = self.plans.clear();
        let _ = self.plan_outcomes.clear();
        let _ = self.capability_events.clear();
        let _ = self.nudges.clear();

        // Abandon active goals — they were crafted for the old architecture
        for entry in self.goals.iter().flatten() {
            let (key, val) = entry;
            if let Ok(mut goal) = serde_json::from_slice::<Goal>(&val) {
                if matches!(goal.status, GoalStatus::Active) {
                    goal.status = GoalStatus::Abandoned;
                    if let Ok(json) = serde_json::to_vec(&goal) {
                        let _ = self.goals.insert(key, json);
                    }
                }
            }
        }

        // Wipe cognitive subsystem state (all trained on stale data)
        let stale_keys = [
            "brain_weights",
            "cortex_state",
            "genesis_pool",
            "hivemind_state",
            "synthesis_state",
            "plan_transformer",
            "plan_transformer_vocab",
            "model_templates_trained",
            "model_plans_generated",
            "model_last_train_loss",
            "durable_rules",
            "failure_chains",
            "peer_failures",
            "benchmark_hints",
            "benchmark_force_next",
            "recent_errors",
            "total_think_cycles",
            "cycles_since_last_commit",
            "stagnation_resets",
            "last_benchmark_at",
            "last_benchmark_cycle",
            "active_plan_id",
            "active_genesis_template_id",
        ];
        for key in &stale_keys {
            let _ = self.state.remove(key.as_bytes());
        }

        // Record the new version
        let _ = self.set_state("cognitive_architecture_version", version);
        let _ = self.set_state(
            "cognitive_reset_at",
            &chrono::Utc::now().timestamp().to_string(),
        );

        tracing::warn!("Cognitive reset complete — all subsystems will reinitialize from scratch");
        true
    }
}

/// Generic helper: cap a sled tree at `max` items, sorted by a timestamp field (DESC).
/// Returns the number of items removed.
fn prune_tree_by_cap<T: serde::de::DeserializeOwned>(
    tree: &sled::Tree,
    max: usize,
    timestamp_fn: fn(&T) -> i64,
) -> u32 {
    let mut items: Vec<(Vec<u8>, i64)> = Vec::new();
    for entry in tree.iter().flatten() {
        let (key, val) = entry;
        if let Ok(item) = serde_json::from_slice::<T>(&val) {
            items.push((key.to_vec(), timestamp_fn(&item)));
        }
    }
    if items.len() <= max {
        return 0;
    }
    // Sort by timestamp DESC — keep the newest `max` items
    items.sort_by(|a, b| b.1.cmp(&a.1));
    let mut count = 0u32;
    for (key, _) in items.into_iter().skip(max) {
        let _ = tree.remove(key);
        count += 1;
    }
    count
}
