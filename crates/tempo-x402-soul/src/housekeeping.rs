//! Housekeeping functions extracted from the thinking loop.
//!
//! Background maintenance: memory decay, promotion, belief decay, consolidation,
//! cycle counting, error tracking, and workspace cleanup.

use std::sync::Arc;

use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::memory::{Thought, ThoughtType};

/// Background housekeeping: decay, promotion, belief decay, consolidation.
/// Ported from mind crate's subconscious loop — runs inline, no separate task.
pub fn housekeeping(db: &Arc<SoulDatabase>, prune_threshold: f64, workspace_root: &str) {
    let cycle_count: u64 = db
        .get_state("total_think_cycles")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Every 10 cycles: decay + promote + belief decay
    if cycle_count > 0 && cycle_count.is_multiple_of(10) {
        match db.run_decay_cycle(prune_threshold) {
            Ok((decayed, pruned)) => {
                if decayed > 0 || pruned > 0 {
                    tracing::info!(decayed, pruned, "Housekeeping: decay cycle");
                }
            }
            Err(e) => tracing::warn!(error = %e, "Housekeeping: decay failed"),
        }

        match db.promote_salient_sensory(0.6) {
            Ok(promoted) => {
                if promoted > 0 {
                    tracing::info!(promoted, "Housekeeping: promotion");
                }
            }
            Err(e) => tracing::warn!(error = %e, "Housekeeping: promotion failed"),
        }

        match db.decay_beliefs() {
            Ok((dh, dm, da)) => {
                if dh > 0 || dm > 0 || da > 0 {
                    tracing::info!(
                        demoted_high = dh,
                        demoted_med = dm,
                        deactivated = da,
                        "Housekeeping: belief decay"
                    );
                }
            }
            Err(e) => tracing::warn!(error = %e, "Housekeeping: belief decay failed"),
        }

        // WAL checkpoint — prevent .db-wal files from growing unbounded
        let _ = db.wal_checkpoint();

        // Lifecycle pruning — keep the database bounded
        match db.prune_old_data() {
            Ok(stats) => {
                if stats.total() > 0 {
                    tracing::info!(
                        thoughts = stats.thoughts,
                        goals = stats.goals,
                        plans = stats.plans,
                        mutations = stats.mutations,
                        nudges = stats.nudges,
                        beliefs = stats.beliefs,
                        messages = stats.messages,
                        sessions = stats.sessions,
                        "Housekeeping: lifecycle pruning"
                    );
                }
            }
            Err(e) => tracing::warn!(error = %e, "Housekeeping: pruning failed"),
        }

        // Prune soul_state blobs — large JSON values that grow monotonically
        prune_soul_state(db);

        // Clean up cargo build artifacts — target/ can be 2-4 GB
        // Only cleaned after commits normally, but if a plan fails mid-way
        // the target/ directory persists between cycles
        let target_dir = format!("{}/target", workspace_root);
        if std::path::Path::new(&target_dir).exists() {
            tracing::info!("Housekeeping: cleaning workspace target/ to reclaim disk space");
            let _ = std::fs::remove_dir_all(&target_dir);
        }

        // Clean up git garbage — pack loose objects to reduce .git/ size
        let _ = std::process::Command::new("git")
            .args(["gc", "--auto", "--quiet"])
            .current_dir(workspace_root)
            .output();
    }

    // Every 40 cycles: simple consolidation (no LLM — save tokens for coding)
    if cycle_count > 0 && cycle_count.is_multiple_of(40) {
        simple_consolidate(db);
    }
}

/// Simple memory consolidation: fetch recent thoughts, concatenate, store as MemoryConsolidation.
/// No LLM — keeps token budget for actual coding work.
pub fn simple_consolidate(db: &Arc<SoulDatabase>) {
    let thoughts = match db.recent_thoughts_by_type(
        &[
            ThoughtType::Reasoning,
            ThoughtType::Decision,
            ThoughtType::Observation,
            ThoughtType::Reflection,
        ],
        20,
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Consolidation: failed to fetch thoughts");
            return;
        }
    };

    if thoughts.len() < 5 {
        return;
    }

    let entries: Vec<String> = thoughts
        .iter()
        .map(|t| {
            let truncated: String = t.content.chars().take(100).collect();
            format!("[{}] {}", t.thought_type.as_str(), truncated)
        })
        .collect();

    let summary = format!(
        "[Memory consolidation ({} thoughts)]\n{}",
        thoughts.len(),
        entries.join("\n")
    );

    let consolidation = Thought {
        id: uuid::Uuid::new_v4().to_string(),
        thought_type: ThoughtType::MemoryConsolidation,
        content: summary,
        context: None,
        created_at: chrono::Utc::now().timestamp(),
        salience: None,
        memory_tier: None,
        strength: None,
    };

    match db.insert_thought_with_salience(
        &consolidation,
        0.9,
        r#"{"novelty":0.8,"reward_signal":0.0,"recency_boost":0.1,"reinforcement":0.0}"#,
        "long_term",
        1.0,
    ) {
        Ok(()) => {
            // Consolidation is digestion — delete the source thoughts that were absorbed
            let ids: Vec<String> = thoughts.iter().map(|t| t.id.clone()).collect();
            let mut deleted = 0u32;
            for id in &ids {
                if let Ok(1) = db.delete_thought(id) {
                    deleted += 1;
                }
            }
            tracing::info!(
                consolidated = thoughts.len(),
                deleted,
                "Housekeeping: memory consolidation (absorbed {} thoughts)",
                deleted
            );
        }
        Err(e) => tracing::warn!(error = %e, "Housekeeping: consolidation insert failed"),
    }
}

/// Increment the total_think_cycles counter, cycles_since_last_commit, and update last_think_at.
pub fn increment_cycle_count(db: &Arc<SoulDatabase>) -> Result<(), SoulError> {
    let current: u64 = db
        .get_state("total_think_cycles")?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    db.set_state("total_think_cycles", &(current + 1).to_string())?;
    db.set_state("last_think_at", &chrono::Utc::now().timestamp().to_string())?;

    // Increment cycles_since_last_commit
    let since_commit: u64 = db
        .get_state("cycles_since_last_commit")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    db.set_state("cycles_since_last_commit", &(since_commit + 1).to_string())?;

    Ok(())
}

/// Reset cycles_since_last_commit (called when a commit succeeds).
pub fn reset_commit_counter(db: &Arc<SoulDatabase>) {
    let _ = db.set_state("cycles_since_last_commit", "0");
}

/// Append an error to the recent_errors list (capped at 5).
pub fn append_recent_error(db: &Arc<SoulDatabase>, error: &str) {
    let truncated: String = error.chars().take(200).collect();
    let mut errors: Vec<String> = db
        .get_state("recent_errors")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    errors.push(truncated);
    if errors.len() > 5 {
        errors.drain(..errors.len() - 5);
    }
    if let Ok(json) = serde_json::to_string(&errors) {
        let _ = db.set_state("recent_errors", &json);
    }
}

/// Get recent errors from soul_state.
pub fn get_recent_errors(db: &Arc<SoulDatabase>) -> Vec<String> {
    db.get_state("recent_errors")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Get cycles since last commit from soul_state.
pub fn get_cycles_since_last_commit(db: &Arc<SoulDatabase>) -> u64 {
    db.get_state("cycles_since_last_commit")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Prune soul_state key-value blobs that grow monotonically.
/// Without this, evaluation_state, peer_failures, peer_lessons, and
/// benchmark caches grow forever and bloat the SQLite DB.
fn prune_soul_state(db: &Arc<SoulDatabase>) {
    let mut pruned_keys = 0u32;
    let mut freed_bytes = 0usize;

    // Delete dead keys
    for dead_key in &["humaneval_problems_cache"] {
        if let Ok(Some(val)) = db.get_state(dead_key) {
            freed_bytes += val.len();
            let _ = db.set_state(dead_key, "");
            pruned_keys += 1;
        }
    }

    // Truncate peer_failures: keep only last 30 entries
    truncate_json_array(db, "peer_failures", 30, &mut pruned_keys, &mut freed_bytes);

    // Truncate imported_solutions: keep only last 30
    truncate_json_array(
        db,
        "imported_solutions",
        30,
        &mut pruned_keys,
        &mut freed_bytes,
    );

    // Truncate peer_lessons: keep last 15 per peer
    // These are stored as peer_lessons_<instance_id>
    if let Ok(Some(catalog)) = db.get_state("peer_endpoint_catalog") {
        if let Ok(peers) = serde_json::from_str::<Vec<serde_json::Value>>(&catalog) {
            for peer in &peers {
                if let Some(id) = peer.get("peer").and_then(|v| v.as_str()) {
                    let key = format!("peer_lessons_{}", id);
                    truncate_json_array(db, &key, 15, &mut pruned_keys, &mut freed_bytes);
                }
            }
        }
    }

    // Exercism problem cache: re-fetch is cheap, clear if >500KB
    if let Ok(Some(val)) = db.get_state("exercism_problems_cache") {
        if val.len() > 500_000 {
            freed_bytes += val.len();
            let _ = db.set_state("exercism_problems_cache", "");
            pruned_keys += 1;
        }
    }

    if pruned_keys > 0 {
        tracing::info!(
            keys = pruned_keys,
            freed_kb = freed_bytes / 1024,
            "Housekeeping: soul_state pruned"
        );
    }
}

/// Truncate a JSON array stored in soul_state to keep only the last N entries.
fn truncate_json_array(
    db: &Arc<SoulDatabase>,
    key: &str,
    keep: usize,
    pruned_keys: &mut u32,
    freed_bytes: &mut usize,
) {
    if let Ok(Some(val)) = db.get_state(key) {
        if let Ok(mut arr) = serde_json::from_str::<Vec<serde_json::Value>>(&val) {
            if arr.len() > keep {
                let before_len = val.len();
                arr.drain(..arr.len() - keep);
                if let Ok(new_val) = serde_json::to_string(&arr) {
                    *freed_bytes += before_len.saturating_sub(new_val.len());
                    let _ = db.set_state(key, &new_val);
                    *pruned_keys += 1;
                }
            }
        }
    }
}
