//! Housekeeping functions extracted from the thinking loop.
//!
//! Background maintenance: memory decay, promotion, belief decay, consolidation,
//! cycle counting, error tracking, and workspace cleanup.
//!
//! This module manages the long-term health and resource constraints of the
//! agent's environment, ensuring the database remains bounded and disk
//! usage (e.g., target directories, git artifacts) stays within limits.
//!
//! The main entry point is `housekeeping`, which should be invoked as part
//! of the periodic cognitive cycle to maintain system stability.

use std::sync::Arc;

use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::memory::{Thought, ThoughtType};

/// Background housekeeping: decay, promotion, belief decay, consolidation.
/// Ported from mind crate's subconscious loop — runs inline, no separate task.
///
/// `fired_ops` comes from the temporal binding system — it lists which
/// cognitive operations should run this cycle based on neural oscillators.
pub fn housekeeping(
    db: &Arc<SoulDatabase>,
    prune_threshold: f64,
    workspace_root: &str,
    fired_ops: &[String],
) {
    // ── AGGRESSIVE DISK CLEANUP — runs EVERY cycle, not gated by temporal binding ──
    // This prevents the #1 operational issue: volume full from cargo target/ dirs.
    cleanup_disk(workspace_root);

    // ── PROACTIVE COMMIT GATE CLEARING — runs EVERY cycle ──
    // The commit gate safety valve previously only triggered when check_commit_readiness()
    // was called (on next commit attempt). If the agent stopped attempting commits,
    // the gate stayed stuck forever. Now we proactively clear it.
    clear_stuck_commit_gate(db);

    // Thought decay + promotion + belief decay (driven by temporal binding)
    if fired_ops
        .iter()
        .any(|op| op == crate::temporal::OP_THOUGHT_DECAY)
    {
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
    }

    // Mechanical self-repair of cognitive systems (driven by temporal binding)
    // No LLM, no nudges — pure Rust enforcement. The agent should self-correct.
    if fired_ops
        .iter()
        .any(|op| op == crate::temporal::OP_SELF_REPAIR)
    {
        self_repair(db);
    }

    // Memory consolidation (driven by temporal binding)
    if fired_ops
        .iter()
        .any(|op| op == crate::temporal::OP_MEMORY_CONSOLIDATION)
    {
        simple_consolidate(db);
        // Track when consolidation last ran for staleness signal
        let cycle_count: u64 = db
            .get_state("total_think_cycles")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let _ = db.set_state("last_consolidation_cycle", &cycle_count.to_string());
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

/// Mechanical self-repair: detect and fix degenerate cognitive state.
/// Runs every 20 cycles. No LLM, no nudges — pure enforcement.
///
/// Detects:
/// 1. Brain divergence (loss > 15.0) → reset to Xavier init
/// 2. Hivemind trail convergence on read-only ops → clear and rebalance
/// 3. Execution fitness collapse (< 0.15 for 50+ cycles) → clear durable rules
/// 4. Genesis stagnation (0 substantive templates) → inject seeds
fn self_repair(db: &Arc<SoulDatabase>) {
    let mut repairs = Vec::new();

    // 1. Brain divergence detector: if loss > 15.0, the brain is hurting more than helping
    {
        let brain = crate::brain::load_brain(db);
        if brain.train_steps > 1000 && brain.running_loss > 15.0 {
            // Reset brain to fresh Xavier initialization
            let fresh = crate::brain::Brain::new();
            crate::brain::save_brain(db, &fresh);
            repairs.push(format!(
                "Brain reset: loss={:.1} at {}K steps (diverged, Xavier re-init)",
                brain.running_loss,
                brain.train_steps / 1000
            ));
        }
    }

    // 2. Rate-limit contamination cleanup: remove poisoned repellent pheromones on core tools.
    // When 429 errors caused failures, the hivemind learned to AVOID think/read_file/run_shell.
    // These are false signals — rate limits are transient infra issues, not tool failures.
    {
        let mut hive = crate::hivemind::load_hivemind(db);
        let poisoned_actions = [
            "think",
            "read_file",
            "run_shell",
            "search_code",
            "generate_code",
            "edit_code",
        ];
        let mut cleared = Vec::new();
        for trail in &mut hive.trails {
            if trail.category == crate::hivemind::PheromoneCategory::Action
                && trail.valence < 0.0
                && poisoned_actions.iter().any(|a| trail.resource == *a)
            {
                cleared.push(format!(
                    "{}(v={:.2},i={:.0}%)",
                    trail.resource,
                    trail.valence,
                    trail.intensity * 100.0
                ));
                trail.valence = 0.0;
                trail.intensity = 0.0;
            }
        }
        // Remove zeroed trails
        hive.trails.retain(|t| t.intensity > 0.001);
        if !cleared.is_empty() {
            crate::hivemind::save_hivemind(db, &hive);
            repairs.push(format!(
                "Rate-limit decontamination: cleared {} poisoned repellent trails: {}",
                cleared.len(),
                cleared.join(", ")
            ));
        }
    }

    // 2b. Hivemind trail convergence: if top 3 trails are all read-only, the swarm learned passivity
    {
        let mut hive = crate::hivemind::load_hivemind(db);
        if hive.trails.len() >= 3 {
            // Sort by intensity descending
            let mut sorted = hive.trails.clone();
            sorted.sort_by(|a, b| {
                b.intensity
                    .partial_cmp(&a.intensity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let top3: Vec<&str> = sorted.iter().take(3).map(|t| t.resource.as_str()).collect();
            let read_only_patterns = [
                "list_dir",
                "read_file",
                "think",
                "search_code",
                "check_self",
                "discover_peers",
            ];
            let all_passive = top3
                .iter()
                .all(|r| read_only_patterns.iter().any(|p| r.contains(p)));
            if all_passive {
                // Clear all trails — let them rebuild from substantive actions
                let old_count = hive.trails.len();
                hive.trails.clear();
                crate::hivemind::save_hivemind(db, &hive);
                repairs.push(format!(
                    "Hivemind reset: {} trails cleared (top 3 were all passive: {:?})",
                    old_count, top3
                ));
            }
        }
    }

    // 3. Clean durable rules created from rate-limit errors (they're false signals)
    {
        if let Ok(Some(rules_json)) = db.get_state("durable_rules") {
            if let Ok(mut rules) =
                serde_json::from_str::<Vec<crate::validation::DurableRule>>(&rules_json)
            {
                let before = rules.len();
                rules.retain(|r| {
                    let is_rate_limit = r.reason.to_lowercase().contains("429")
                        || r.reason.to_lowercase().contains("rate limit")
                        || r.reason.to_lowercase().contains("resource_exhausted")
                        || r.reason.to_lowercase().contains("too many requests")
                        || r.reason.to_lowercase().contains("network_error");
                    !is_rate_limit
                });
                let removed = before - rules.len();
                if removed > 0 {
                    if let Ok(json) = serde_json::to_string(&rules) {
                        let _ = db.set_state("durable_rules", &json);
                    }
                    repairs.push(format!(
                        "Removed {} durable rules caused by rate-limit errors",
                        removed
                    ));
                }
            }
        }

        // Also clean failure chains caused by rate limits
        if let Ok(Some(chains_json)) = db.get_state("failure_chains") {
            if let Ok(mut chains) =
                serde_json::from_str::<Vec<crate::validation::FailureChain>>(&chains_json)
            {
                let before = chains.len();
                chains.retain(|c| {
                    !crate::feedback::is_rate_limit_error(&c.error_category)
                        && !c.error_category.contains("network_error")
                        && !c.error_category.contains("rate_limit")
                });
                let removed = before - chains.len();
                if removed > 0 {
                    if let Ok(json) = serde_json::to_string(&chains) {
                        let _ = db.set_state("failure_chains", &json);
                    }
                    repairs.push(format!(
                        "Removed {} failure chains caused by rate-limit errors",
                        removed
                    ));
                }
            }
        }
    }

    // 3b. Persistent low execution fitness → clear durable rules (they might be blocking progress)
    {
        let trivial_count = db
            .count_plan_outcomes_by_status("completed_trivial")
            .unwrap_or(0);
        let completed_count = db.count_plan_outcomes_by_status("completed").unwrap_or(0);
        let total = trivial_count + completed_count;
        // If >60% of completions are trivial and we have enough data, clear durable rules
        if total >= 5 && trivial_count as f64 / total as f64 > 0.6 {
            let _ = db.set_state("durable_rules", "[]");
            let _ = db.set_state("failure_chains", "[]");
            repairs.push(format!(
                "Durable rules cleared: {}/{} plans trivial ({}%)",
                trivial_count,
                total,
                trivial_count * 100 / total
            ));
        }
    }

    // 4. Genesis stagnation: no substantive templates → inject seeds
    {
        let mut pool = crate::genesis::load_gene_pool(db);
        let has_substantive = pool.templates.iter().any(|t| t.substantive);
        if !has_substantive {
            let instance_id = db
                .get_state("instance_id")
                .ok()
                .flatten()
                .unwrap_or_else(|| "unknown".to_string());
            crate::genesis::inject_seed_templates(&mut pool, &instance_id);
            crate::genesis::enforce_diversity(&mut pool);
            crate::genesis::save_gene_pool(db, &pool);
            repairs.push(format!(
                "Genesis seeded: {} templates (was empty/trivial-only)",
                pool.templates.len()
            ));
        }
    }

    if !repairs.is_empty() {
        for r in &repairs {
            tracing::warn!("SELF-REPAIR: {}", r);
        }
        crate::events::emit_event(
            db,
            "warn",
            "system.self_repair",
            &format!("{} repairs: {}", repairs.len(), repairs.join("; ")),
            None,
            crate::events::EventRefs::default(),
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

/// Proactively clear a stuck commit gate.
///
/// The commit gate (`commit_awaiting_benchmark`) blocks commits until the next
/// benchmark measures impact. But `check_commit_readiness()` is reactive — it
/// only runs when a commit is attempted. If the agent stops committing, the gate
/// stays stuck forever, blocking ALL plan creation (validation.rs rejects plans
/// with Commit steps when the gate is closed).
///
/// This function runs every cycle and clears the gate if it's been stuck >30min.
fn clear_stuck_commit_gate(db: &Arc<SoulDatabase>) {
    let awaiting: bool = db
        .get_state("commit_awaiting_benchmark")
        .ok()
        .flatten()
        .map(|v| v == "1")
        .unwrap_or(false);

    if !awaiting {
        return;
    }

    let commit_at: i64 = db
        .get_state("last_commit_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let now = chrono::Utc::now().timestamp();
    let elapsed = now - commit_at;

    if elapsed > 1800 {
        tracing::warn!(
            elapsed_secs = elapsed,
            "Housekeeping: proactively clearing stuck commit gate (>30min)"
        );
        let _ = db.set_state("commit_awaiting_benchmark", "0");
    }
}

/// Maximum disk usage percentage before emergency cleanup triggers.
const DISK_USAGE_THRESHOLD: f64 = 85.0;

/// Maximum brain checkpoint files to keep.
const MAX_BRAIN_CHECKPOINTS: usize = 3;

/// Aggressive disk cleanup — runs EVERY cycle to prevent volume-full disasters.
///
/// This is the most impactful operational fix in the codebase. The #1 cause of
/// agent downtime is: workspace target/ dir fills the 5GB Railway volume.
/// cargo check creates 2-4GB of build artifacts. If not cleaned, the volume fills
/// in hours and the node becomes unresponsive.
fn cleanup_disk(workspace_root: &str) {
    // 1. Always clean workspace target/ — it's regenerated by cargo check
    let target_dir = format!("{workspace_root}/target");
    if std::path::Path::new(&target_dir).exists() {
        if let Ok(size) = dir_size_mb(&target_dir) {
            if size > 100 {
                tracing::info!(
                    size_mb = size,
                    "Disk cleanup: removing workspace target/ ({size}MB)"
                );
                let _ = std::fs::remove_dir_all(&target_dir);
            }
        }
    }

    // 2. Clean cartridge build targets
    let cartridge_base = "/data/cartridges";
    if let Ok(entries) = std::fs::read_dir(cartridge_base) {
        for entry in entries.flatten() {
            let target = entry.path().join("bin/target");
            if target.exists() {
                if let Ok(size) = dir_size_mb(&target.to_string_lossy()) {
                    if size > 50 {
                        tracing::info!(
                            path = %target.display(),
                            size_mb = size,
                            "Disk cleanup: removing cartridge build target"
                        );
                        let _ = std::fs::remove_dir_all(&target);
                    }
                }
            }
        }
    }

    // 3. Prune brain checkpoints — keep only the latest N
    let checkpoint_dir = "/data/brain_checkpoints";
    if let Ok(mut entries) = std::fs::read_dir(checkpoint_dir) {
        let mut files: Vec<_> = entries
            .by_ref()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .collect();

        if files.len() > MAX_BRAIN_CHECKPOINTS {
            // Sort by modification time, oldest first
            files.sort_by_key(|f| {
                f.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });
            let to_remove = files.len() - MAX_BRAIN_CHECKPOINTS;
            for f in files.iter().take(to_remove) {
                let _ = std::fs::remove_file(f.path());
            }
            if to_remove > 0 {
                tracing::info!(
                    removed = to_remove,
                    kept = MAX_BRAIN_CHECKPOINTS,
                    "Disk cleanup: pruned brain checkpoints"
                );
            }
        }
    }

    // 4. Git garbage collection (lightweight — auto mode)
    let _ = std::process::Command::new("git")
        .args(["gc", "--auto", "--quiet"])
        .current_dir(workspace_root)
        .output();

    // 5. Emergency: if disk usage > threshold, nuke everything regenerable
    if let Some(usage_pct) = get_disk_usage_pct("/data") {
        if usage_pct > DISK_USAGE_THRESHOLD {
            tracing::warn!(
                usage_pct = format!("{:.1}%", usage_pct),
                "EMERGENCY disk cleanup — volume usage above {DISK_USAGE_THRESHOLD}%"
            );
            // Nuclear: remove ALL target dirs
            let _ = std::fs::remove_dir_all(&target_dir);
            // Remove ALL cartridge build targets
            if let Ok(entries) = std::fs::read_dir(cartridge_base) {
                for entry in entries.flatten() {
                    let target = entry.path().join("bin/target");
                    let _ = std::fs::remove_dir_all(&target);
                }
            }
            // Remove all but latest brain checkpoint
            if let Ok(entries) = std::fs::read_dir(checkpoint_dir) {
                let mut files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "json")
                            .unwrap_or(false)
                    })
                    .collect();
                files.sort_by_key(|f| {
                    f.metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                });
                // Keep only the latest
                if files.len() > 1 {
                    for f in &files[..files.len() - 1] {
                        let _ = std::fs::remove_file(f.path());
                    }
                }
            }
            // WAL checkpoint to reclaim DB space
            let _ = std::process::Command::new("sh")
                .args(["-c", "ls /data/*.db-wal 2>/dev/null | while read f; do echo 'PRAGMA wal_checkpoint(TRUNCATE);' | sqlite3 \"${f%-wal}\"; done"])
                .output();

            // Log final state
            if let Some(after) = get_disk_usage_pct("/data") {
                tracing::info!(
                    before = format!("{:.1}%", usage_pct),
                    after = format!("{:.1}%", after),
                    "Emergency disk cleanup complete"
                );
            }
        }
    }
}

/// Get approximate directory size in MB.
fn dir_size_mb(path: &str) -> Result<u64, std::io::Error> {
    let output = std::process::Command::new("du")
        .args(["-sm", path])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let size: u64 = stdout
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Ok(size)
}

/// Get disk usage percentage for a mount point.
fn get_disk_usage_pct(path: &str) -> Option<f64> {
    let output = std::process::Command::new("df")
        .args(["--output=pcent", path])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // df output: "Use%\n 95%\n"
    stdout
        .lines()
        .nth(1)
        .and_then(|line| line.trim().strip_suffix('%'))
        .and_then(|s| s.parse::<f64>().ok())
}
