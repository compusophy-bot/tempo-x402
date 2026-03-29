// Housekeeping: WAL checkpoint, pruning, resets
use super::*;

impl SoulDatabase {
    pub fn wal_checkpoint(&self) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// Lifecycle pruning: keep the database bounded. Called every 10 cycles from housekeeping.
    /// Life is birth AND death — things that served their purpose must be released.
    pub fn prune_old_data(&self) -> Result<PruneStats, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let now = chrono::Utc::now().timestamp();
        let one_day = 86400i64;
        let three_days = one_day * 3;
        let seven_days = one_day * 7;

        // 1. Cap total thoughts at 500 — delete oldest beyond cap
        let thoughts_pruned: u32 = conn
            .execute(
                "DELETE FROM thoughts WHERE id IN (
                SELECT id FROM thoughts ORDER BY created_at DESC LIMIT -1 OFFSET 500
            )",
                [],
            )
            .unwrap_or(0) as u32;

        // 2. Delete completed/abandoned goals older than 7 days (keep last 10 regardless of age)
        let goals_pruned: u32 = conn
            .execute(
                "DELETE FROM goals WHERE status IN ('completed', 'abandoned') AND created_at < ?1
             AND id NOT IN (
                SELECT id FROM goals WHERE status IN ('completed', 'abandoned')
                ORDER BY updated_at DESC LIMIT 10
            )",
                params![now - seven_days],
            )
            .unwrap_or(0) as u32;

        // 3. Delete completed/failed/abandoned plans older than 3 days (keep last 10)
        let plans_pruned: u32 = conn.execute(
            "DELETE FROM plans WHERE status IN ('completed', 'failed', 'abandoned') AND created_at < ?1
             AND id NOT IN (
                SELECT id FROM plans WHERE status IN ('completed', 'failed', 'abandoned')
                ORDER BY updated_at DESC LIMIT 10
            )",
            params![now - three_days],
        ).unwrap_or(0) as u32;

        // 4. Cap mutations at 50 — delete oldest beyond cap
        let mutations_pruned: u32 = conn
            .execute(
                "DELETE FROM mutations WHERE id IN (
                SELECT id FROM mutations ORDER BY created_at DESC LIMIT -1 OFFSET 50
            )",
                [],
            )
            .unwrap_or(0) as u32;

        // 5. Delete processed nudges older than 24h
        let nudges_pruned: u32 = conn
            .execute(
                "DELETE FROM nudges WHERE processed = 1 AND created_at < ?1",
                params![now - one_day],
            )
            .unwrap_or(0) as u32;

        // 6. Delete inactive beliefs (already deactivated by decay)
        let beliefs_pruned: u32 = conn
            .execute(
                "DELETE FROM beliefs WHERE active = 0 AND updated_at < ?1",
                params![now - three_days],
            )
            .unwrap_or(0) as u32;

        // 7. Cap chat messages per session (keep last 100 per session)
        let messages_pruned: u32 = conn
            .execute(
                "DELETE FROM chat_messages WHERE id IN (
                SELECT cm.id FROM chat_messages cm
                INNER JOIN (
                    SELECT session_id, id,
                    ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY created_at DESC) as rn
                    FROM chat_messages
                ) ranked ON cm.id = ranked.id
                WHERE ranked.rn > 100
            )",
                [],
            )
            .unwrap_or(0) as u32;

        // 8. Delete old inactive chat sessions (keep last 20)
        let sessions_pruned: u32 = conn
            .execute(
                "DELETE FROM chat_sessions WHERE active = 0 AND id NOT IN (
                SELECT id FROM chat_sessions ORDER BY updated_at DESC LIMIT 20
            )",
                [],
            )
            .unwrap_or(0) as u32;

        // 9. Cap plan_outcomes at 100 — keep recent for feedback loop
        let _ = conn.execute(
            "DELETE FROM plan_outcomes WHERE id IN (
                SELECT id FROM plan_outcomes ORDER BY created_at DESC LIMIT -1 OFFSET 100
            )",
            [],
        );

        // 10. Cap capability_events at 500 — keep enough for accurate profiles
        let _ = conn.execute(
            "DELETE FROM capability_events WHERE id IN (
                SELECT id FROM capability_events ORDER BY created_at DESC LIMIT -1 OFFSET 500
            )",
            [],
        );

        // 11. Cap benchmark_runs at 200 — each stores generated_solution + error_output
        let _ = conn.execute(
            "DELETE FROM benchmark_runs WHERE id IN (
                SELECT id FROM benchmark_runs ORDER BY created_at DESC LIMIT -1 OFFSET 200
            )",
            [],
        );

        // 13. Prune events (tiered retention)
        let mut events_pruned = 0u32;
        events_pruned += conn
            .execute(
                "DELETE FROM events WHERE level = 'debug' AND created_at < ?1",
                params![now - one_day],
            )
            .unwrap_or(0) as u32;
        events_pruned += conn
            .execute(
                "DELETE FROM events WHERE level = 'info' AND created_at < ?1",
                params![now - three_days],
            )
            .unwrap_or(0) as u32;
        events_pruned += conn
            .execute(
                "DELETE FROM events WHERE level = 'warn' AND created_at < ?1",
                params![now - seven_days],
            )
            .unwrap_or(0) as u32;
        events_pruned += conn
            .execute(
                "DELETE FROM events WHERE level = 'warn' AND resolved = 1 AND created_at < ?1",
                params![now - three_days],
            )
            .unwrap_or(0) as u32;
        events_pruned += conn
            .execute(
                "DELETE FROM events WHERE level = 'error' AND created_at < ?1",
                params![now - one_day * 30],
            )
            .unwrap_or(0) as u32;
        events_pruned += conn
            .execute(
                "DELETE FROM events WHERE level = 'error' AND resolved = 1 AND created_at < ?1",
                params![now - seven_days],
            )
            .unwrap_or(0) as u32;
        events_pruned += conn
            .execute(
                "DELETE FROM events WHERE id IN (
                    SELECT id FROM events ORDER BY created_at DESC LIMIT -1 OFFSET 5000
                )",
                [],
            )
            .unwrap_or(0) as u32;

        // 14. VACUUM to reclaim disk space after deletions
        // Only run occasionally (check if we deleted anything substantial)
        if thoughts_pruned + goals_pruned + plans_pruned + mutations_pruned + events_pruned > 10 {
            let _ = conn.execute_batch("VACUUM;");
        }

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
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        // Delete all thoughts (they accumulate endlessly)
        let thoughts_deleted: u64 = conn
            .execute("DELETE FROM thoughts", [])
            .unwrap_or(0)
            .try_into()
            .unwrap_or(0);

        // Delete ALL plans — including active ones that may be stuck.
        // A stuck active plan can trap the thinking loop indefinitely.
        let plans_deleted: u64 = conn
            .execute("DELETE FROM plans", [])
            .unwrap_or(0)
            .try_into()
            .unwrap_or(0);
        // Clear the active plan pointer
        let _ = conn.execute(
            "INSERT OR REPLACE INTO soul_state (key, value) VALUES ('active_plan_id', '')",
            [],
        );

        // Delete processed nudges
        let nudges_deleted: u64 = conn
            .execute("DELETE FROM nudges WHERE processed = 1", [])
            .unwrap_or(0)
            .try_into()
            .unwrap_or(0);

        // Reset cycle counters
        let _ = conn.execute(
            "INSERT OR REPLACE INTO soul_state (key, value) VALUES ('total_think_cycles', '0')",
            [],
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO soul_state (key, value) VALUES ('cycles_since_last_commit', '0')",
            [],
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO soul_state (key, value) VALUES ('recent_errors', '[]')",
            [],
        );

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

        // Clean up old thoughts and completed plans to start fresh
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute("DELETE FROM thoughts", []);
            let _ = conn.execute(
                "DELETE FROM plans WHERE status IN ('completed', 'failed', 'abandoned')",
                [],
            );
            let _ = conn.execute("DELETE FROM nudges WHERE processed = 1", []);
            // Abandon any active goals from old deploy — they may reference old code
            let _ = conn.execute(
                "UPDATE goals SET status = 'abandoned' WHERE status = 'active'",
                [],
            );
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

        tracing::warn!(
            old_version = %last_version,
            new_version = %version,
            "Cognitive architecture changed — FULL RESET of learned state"
        );

        if let Ok(conn) = self.conn.lock() {
            // Wipe all learned behavioral data (trained on old architecture)
            let _ = conn.execute("DELETE FROM thoughts", []);
            let _ = conn.execute("DELETE FROM plans", []);
            let _ = conn.execute("DELETE FROM plan_outcomes", []);
            let _ = conn.execute("DELETE FROM capability_events", []);
            let _ = conn.execute("DELETE FROM nudges", []);
            let _ = conn.execute(
                "UPDATE goals SET status = 'abandoned' WHERE status = 'active'",
                [],
            );

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
                let _ = conn.execute(
                    "DELETE FROM soul_state WHERE key = ?1",
                    rusqlite::params![key],
                );
            }
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
