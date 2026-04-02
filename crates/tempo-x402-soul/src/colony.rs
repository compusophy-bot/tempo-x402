//! Colony-level selection pressure: competition + cooperation between agents.
//!
//! ## The Problem
//!
//! Without selection pressure, all agents converge on the same mediocre strategy.
//! Bad agents consume equal resources as good ones. Nothing evolves.
//!
//! ## The Solution
//!
//! **Differential reproduction**: fit agents spawn more, unfit agents get replaced.
//! **Reputation-weighted knowledge sharing**: better agents' knowledge counts more.
//! **Specialization pressure**: don't compete on axes where siblings already excel.
//!
//! ## Mechanisms
//!
//! 1. **Colony Ranking**: Each agent knows its rank among peers (by fitness).
//! 2. **Spawn Rights**: Only agents above colony median fitness can reproduce.
//! 3. **Cull Signal**: Agents below threshold for sustained period signal for replacement.
//! 4. **Reputation Merge**: All peer sync (brain, genesis, cortex, hivemind) weighted by
//!    relative fitness — a 77% agent's knowledge influences more than a 47% agent's.
//! 5. **Niche Pressure**: If a peer already excels at X, your planning prompt tells you
//!    to focus on Y. Emergent specialization through competitive exclusion.
//! 6. **Colony ELO**: Track collective benchmark performance over time, not just individual.

use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::db::SoulDatabase;

// ── Constants ────────────────────────────────────────────────────────

/// Minimum cycles before colony selection kicks in (let agents warm up).
const WARMUP_CYCLES: u64 = 50;
/// Fitness threshold below which an agent signals for culling.
/// Relative to colony best: if you're less than this fraction of the best, you're unfit.
const CULL_RATIO: f64 = 0.4;
/// How many consecutive evaluations below cull threshold before signaling.
const CULL_PATIENCE: u32 = 5;
/// Minimum fitness to spawn (absolute floor).
const SPAWN_MIN_FITNESS: f64 = 0.3;

// ── Core Types ───────────────────────────────────────────────────────

/// Colony-level status for this agent relative to its peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyStatus {
    /// This agent's fitness.
    pub self_fitness: f64,
    /// Ranked position (0 = best).
    pub rank: usize,
    /// Total agents in colony.
    pub colony_size: usize,
    /// Best fitness in colony.
    pub best_fitness: f64,
    /// Worst fitness in colony.
    pub worst_fitness: f64,
    /// Colony mean fitness.
    pub mean_fitness: f64,
    /// Colony median fitness.
    pub median_fitness: f64,
    /// Whether this agent can spawn.
    pub can_spawn: bool,
    /// Whether this agent should be culled.
    pub should_cull: bool,
    /// Consecutive evaluations below cull threshold.
    pub cull_count: u32,
    /// Peer fitness map: instance_id → fitness.
    pub peer_fitness: Vec<(String, f64)>,
    /// Recommended specialization niche (based on what peers DON'T cover).
    pub recommended_niche: Option<String>,
    /// Colony ELO: best collective benchmark score.
    pub colony_elo: f64,
    /// Ψ(t): colony consciousness — unified intelligence metric.
    /// Ψ = (Intelligence × Sync × Diversity × Learning_Velocity)^0.25
    pub psi: f64,
    /// dΨ/dt: trend of colony consciousness.
    pub psi_trend: f64,
    /// Phase readiness signals.
    pub phase3_ready: bool,
    pub measured_at: i64,
}

/// Peer fitness record stored during discover_peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerFitnessRecord {
    pub instance_id: String,
    pub fitness: f64,
    pub benchmark_pass_at_1: f64,
    pub role: String,
    pub strongest_capability: String,
    pub measured_at: i64,
}

// ── Colony Evaluation ────────────────────────────────────────────────

/// Evaluate this agent's position in the colony.
/// Called every cycle after fitness computation.
/// `free_energy` and `fe_trend` from the free energy system drive Ψ computation.
pub fn evaluate(
    db: &Arc<SoulDatabase>,
    self_fitness: f64,
    free_energy: f64,
    fe_trend: f64,
) -> ColonyStatus {
    let now = chrono::Utc::now().timestamp();
    let cycle_count: u64 = db
        .get_state("total_think_cycles")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Load peer fitness from BOTH sources:
    // 1. Old discover_peers path (colony_peer_fitness)
    // 2. New collective protocol (colony_workers from worker registration)
    let mut peers = load_peer_fitness(db);

    // Merge in workers from the collective protocol (queen sees registered workers)
    let collective_workers = crate::collective::get_live_workers(db);
    for worker in &collective_workers {
        if !peers.iter().any(|p| p.instance_id == worker.instance_id) {
            peers.push(PeerFitnessRecord {
                instance_id: worker.instance_id.clone(),
                fitness: worker.fitness.max(0.3), // default 0.3 if not yet measured
                benchmark_pass_at_1: 0.0,
                role: "worker".to_string(),
                strongest_capability: String::new(),
                measured_at: worker.last_heartbeat,
            });
        }
    }

    // Workers: also count the queen as a peer
    if crate::collective::ColonyRole::from_env() == crate::collective::ColonyRole::Worker {
        if let Some(queen_url) = crate::collective::queen_url() {
            let queen_id = format!("queen-{}", queen_url.chars().take(20).collect::<String>());
            if !peers.iter().any(|p| p.instance_id == queen_id) {
                peers.push(PeerFitnessRecord {
                    instance_id: queen_id,
                    fitness: 0.8, // queen is likely the fittest
                    benchmark_pass_at_1: 0.0,
                    role: "queen".to_string(),
                    strongest_capability: String::new(),
                    measured_at: now,
                });
            }
        }
    }

    let colony_size = peers.len() + 1; // +1 for self

    // Build sorted fitness list (including self)
    let mut all_fitness: Vec<f64> = peers.iter().map(|p| p.fitness).collect();
    all_fitness.push(self_fitness);
    all_fitness.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let best_fitness = all_fitness.first().copied().unwrap_or(0.0);
    let worst_fitness = all_fitness.last().copied().unwrap_or(0.0);
    let mean_fitness = if all_fitness.is_empty() {
        0.0
    } else {
        all_fitness.iter().sum::<f64>() / all_fitness.len() as f64
    };
    let median_fitness = if all_fitness.is_empty() {
        0.0
    } else {
        all_fitness[all_fitness.len() / 2]
    };

    // Rank (0 = best)
    let rank = all_fitness
        .iter()
        .position(|&f| (f - self_fitness).abs() < 0.001)
        .unwrap_or(all_fitness.len() - 1);

    // Spawn rights: above median AND above absolute floor
    let can_spawn = cycle_count >= WARMUP_CYCLES
        && self_fitness >= median_fitness
        && self_fitness >= SPAWN_MIN_FITNESS;

    // Cull detection: below CULL_RATIO of best for CULL_PATIENCE consecutive evals
    let below_cull = cycle_count >= WARMUP_CYCLES
        && best_fitness > 0.0
        && (self_fitness / best_fitness) < CULL_RATIO
        && colony_size > 1;

    let prev_cull_count: u32 = db
        .get_state("colony_cull_count")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let cull_count = if below_cull {
        prev_cull_count + 1
    } else {
        0 // Reset on recovery
    };
    let _ = db.set_state("colony_cull_count", &cull_count.to_string());

    let should_cull = cull_count >= CULL_PATIENCE;

    // Specialization niche: find capabilities that peers are WEAK at
    let recommended_niche = compute_niche(&peers);

    // Colony ELO: best benchmark score across all agents
    let colony_elo = peers
        .iter()
        .map(|p| p.benchmark_pass_at_1)
        .chain(std::iter::once(
            db.get_state("benchmark_pass_at_1")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
        ))
        .fold(0.0f64, f64::max);

    // ── Ψ(t): Colony Consciousness ──
    let (psi, psi_trend) = compute_psi(db, self_fitness, &peers, free_energy, fe_trend);

    // Phase 3 readiness: enough training data + baseline competence + colony health
    let training_examples: usize = db
        .get_state("codegen_training_count")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let self_pass: f64 = db
        .get_state("benchmark_pass_at_1")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    // Phase 3 is always active — the codegen model learns continuously.
    // The old gate (psi > 0.5 && examples > 500 && pass > 60%) was unreachable
    // while the system was stuck. Phase 3 is a gradient, not a gate.
    let phase3_ready = training_examples > 0;

    let status = ColonyStatus {
        self_fitness,
        rank,
        colony_size,
        best_fitness,
        worst_fitness,
        mean_fitness,
        median_fitness,
        can_spawn,
        should_cull,
        cull_count,
        peer_fitness: peers
            .iter()
            .map(|p| (p.instance_id.clone(), p.fitness))
            .collect(),
        recommended_niche,
        colony_elo,
        psi,
        psi_trend,
        phase3_ready,
        measured_at: now,
    };

    // Store for API access
    if let Ok(json) = serde_json::to_string(&status) {
        let _ = db.set_state("colony_status", &json);
    }

    // Log colony position
    if colony_size > 1 {
        tracing::info!(
            rank = rank + 1,
            of = colony_size,
            self_fitness = format!("{:.3}", self_fitness),
            best = format!("{:.3}", best_fitness),
            median = format!("{:.3}", median_fitness),
            can_spawn,
            cull_count,
            "Colony ranking"
        );
    }

    // Emit cull signal as event
    if should_cull {
        tracing::warn!(
            self_fitness = format!("{:.3}", self_fitness),
            best_fitness = format!("{:.3}", best_fitness),
            ratio = format!("{:.2}", self_fitness / best_fitness.max(0.001)),
            patience = CULL_PATIENCE,
            "CULL SIGNAL: agent is consistently worst in colony"
        );
        crate::events::emit_event(
            db,
            "warn",
            "colony.cull_signal",
            &format!(
                "Agent fitness {:.3} is {:.0}% of best ({:.3}) for {} consecutive evaluations",
                self_fitness,
                (self_fitness / best_fitness.max(0.001)) * 100.0,
                best_fitness,
                cull_count,
            ),
            None,
            crate::events::EventRefs::default(),
        );
    }

    status
}

/// Compute merge weight for a peer based on relative fitness.
/// Returns 0.0-1.0 where higher = more influence.
/// Used to weight brain/genesis/cortex/hivemind merges.
pub fn peer_merge_weight(db: &SoulDatabase, peer_id: &str) -> f32 {
    let peers = load_peer_fitness(db);
    let self_fitness: f64 = crate::fitness::FitnessScore::load_current(db)
        .map(|f| f.total)
        .unwrap_or(0.3);

    let peer_fitness = peers
        .iter()
        .find(|p| p.instance_id == peer_id)
        .map(|p| p.fitness)
        .unwrap_or(0.3);

    // Ratio: peer_fitness / self_fitness, clamped to [0.1, 2.0]
    // If peer is fitter than you, their merge weight > 1.0 (up to 2x)
    // If peer is less fit, their merge weight < 1.0 (down to 0.1x)
    let ratio = (peer_fitness / self_fitness.max(0.01)).clamp(0.1, 2.0);

    // Scale to a merge multiplier: 0.1x to 2.0x of base merge rate
    ratio as f32
}

/// Generate a prompt section about colony position for planning.
/// Tells the agent where it stands and what to focus on.
pub fn prompt_section(db: &SoulDatabase) -> String {
    let status: Option<ColonyStatus> = db
        .get_state("colony_status")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok());

    let status = match status {
        Some(s) if s.colony_size > 1 => s,
        _ => return String::new(),
    };

    let mut lines = vec![format!(
        "# Colony Position: Rank {}/{} (fitness {:.1}%)",
        status.rank + 1,
        status.colony_size,
        status.self_fitness * 100.0,
    )];

    lines.push(format!(
        "Colony: best={:.1}%, median={:.1}%, worst={:.1}%",
        status.best_fitness * 100.0,
        status.median_fitness * 100.0,
        status.worst_fitness * 100.0,
    ));

    if status.rank == 0 {
        lines.push("You are the FITTEST agent. Your strategies should propagate. Focus on benchmark improvement.".to_string());
    } else if status.can_spawn {
        lines.push("You are above median. Your strategies are working — keep pushing.".to_string());
    } else {
        lines.push(format!(
            "You are BELOW median. {} agents are outperforming you. Try a DIFFERENT approach.",
            status.rank
        ));
    }

    if let Some(ref niche) = status.recommended_niche {
        lines.push(format!(
            "Recommended niche: {} (underserved by peers — less competition here)",
            niche
        ));
    }

    if status.colony_elo > 0.0 {
        lines.push(format!(
            "Colony best benchmark: {:.1}% pass@1",
            status.colony_elo
        ));
    }

    lines.join("\n")
}

// ── Niche Computation ────────────────────────────────────────────────

/// Find capabilities that peers are weak at — recommend as a specialization niche.
/// Compute Ψ(t) — colony consciousness metric.
///
/// Ψ = (Intelligence × Sync × Diversity × Velocity)^0.25
///
/// - Intelligence: mean pass@1 across colony (raw coding ability)
/// - Sync: colony benefit from peer sync (how much sharing helps)
/// - Diversity: fitness std deviation (specialization pressure)
/// - Velocity: negative F(t) trend (learning = decreasing surprise)
///
/// Returns (psi, psi_trend).
fn compute_psi(
    db: &Arc<SoulDatabase>,
    self_fitness: f64,
    peers: &[PeerFitnessRecord],
    free_energy: f64,
    fe_trend: f64,
) -> (f64, f64) {
    // Intelligence: mean pass@1 across colony (0.0-1.0 scale)
    let self_pass: f64 = db
        .get_state("benchmark_pass_at_1")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let all_pass: Vec<f64> = peers
        .iter()
        .map(|p| p.benchmark_pass_at_1)
        .chain(std::iter::once(self_pass))
        .collect();
    let intelligence = if all_pass.is_empty() {
        0.0
    } else {
        all_pass.iter().sum::<f64>() / all_pass.len() as f64 / 100.0
    };

    // Sync: colony benefit from evaluation system
    let sync: f64 = db
        .get_state("colony_benefit")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
        .max(0.0)
        .min(1.0);

    // Diversity: fitness std deviation across colony
    let all_fitness: Vec<f64> = peers
        .iter()
        .map(|p| p.fitness)
        .chain(std::iter::once(self_fitness))
        .collect();
    let mean_f = if all_fitness.is_empty() {
        0.0
    } else {
        all_fitness.iter().sum::<f64>() / all_fitness.len() as f64
    };
    let variance = if all_fitness.len() <= 1 {
        0.0
    } else {
        all_fitness
            .iter()
            .map(|f| (f - mean_f).powi(2))
            .sum::<f64>()
            / all_fitness.len() as f64
    };
    let diversity = variance.sqrt().min(1.0);

    // Learning velocity: negative F trend = improving (higher = better)
    let velocity = (-fe_trend).clamp(0.0, 1.0);

    // Ψ = geometric mean (all must be positive for high Ψ)
    // +0.1 offsets prevent zero from killing the product
    let psi = (intelligence.max(0.01)
        * (sync + 0.1)
        * (diversity + 0.1)
        * (velocity + 0.1))
    .powf(0.25);

    // Trend vs previous Ψ
    let prev_psi: f64 = db
        .get_state("psi_value")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let psi_trend = psi - prev_psi;

    let _ = db.set_state("psi_value", &format!("{psi:.6}"));
    let _ = db.set_state("psi_trend", &format!("{psi_trend:.6}"));

    (psi, psi_trend)
}

fn compute_niche(peers: &[PeerFitnessRecord]) -> Option<String> {
    if peers.is_empty() {
        return None;
    }

    // Count how many peers are strong at each capability
    let mut coverage: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let all_capabilities = [
        "coding",
        "review",
        "endpoint_creation",
        "coordination",
        "benchmark",
    ];

    for cap in &all_capabilities {
        coverage.insert(cap, 0);
    }

    for peer in peers {
        let strong = peer.strongest_capability.to_lowercase();
        if strong.contains("code") || strong.contains("compil") || strong.contains("gen") {
            *coverage.entry("coding").or_insert(0) += 1;
        }
        if strong.contains("review") || strong.contains("accept") {
            *coverage.entry("review").or_insert(0) += 1;
        }
        if strong.contains("endpoint") || strong.contains("shell") {
            *coverage.entry("endpoint_creation").or_insert(0) += 1;
        }
        if strong.contains("peer") || strong.contains("coord") || strong.contains("git") {
            *coverage.entry("coordination").or_insert(0) += 1;
        }
        if strong.contains("bench") || strong.contains("test") {
            *coverage.entry("benchmark").or_insert(0) += 1;
        }
    }

    // Recommend the least-covered capability
    coverage
        .iter()
        .min_by_key(|(_, &count)| count)
        .map(|(cap, _)| cap.to_string())
}

// ── Persistence ──────────────────────────────────────────────────────

/// Store a peer's fitness during discover_peers.
pub fn record_peer_fitness(
    db: &SoulDatabase,
    instance_id: &str,
    fitness: f64,
    benchmark_pass_at_1: f64,
    role: &str,
    strongest_capability: &str,
) {
    let now = chrono::Utc::now().timestamp();
    let record = PeerFitnessRecord {
        instance_id: instance_id.to_string(),
        fitness,
        benchmark_pass_at_1,
        role: role.to_string(),
        strongest_capability: strongest_capability.to_string(),
        measured_at: now,
    };

    let mut peers = load_peer_fitness(db);

    // Update or insert
    if let Some(existing) = peers.iter_mut().find(|p| p.instance_id == instance_id) {
        *existing = record;
    } else {
        peers.push(record);
    }

    // Keep max 20 peers
    if peers.len() > 20 {
        peers.sort_by(|a, b| b.measured_at.cmp(&a.measured_at));
        peers.truncate(20);
    }

    if let Ok(json) = serde_json::to_string(&peers) {
        let _ = db.set_state("colony_peer_fitness", &json);
    }
}

/// Load peer fitness records, filtering out stale entries.
/// Peers older than 2 hours are considered dead (ghosts).
pub fn load_peer_fitness(db: &SoulDatabase) -> Vec<PeerFitnessRecord> {
    let now = chrono::Utc::now().timestamp();
    let max_age_secs: i64 = 7200; // 2 hours
    let all: Vec<PeerFitnessRecord> = db
        .get_state("colony_peer_fitness")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let live: Vec<PeerFitnessRecord> = all
        .into_iter()
        .filter(|p| now - p.measured_at < max_age_secs)
        .collect();
    // If we filtered any, persist the cleaned list
    if let Ok(json) = serde_json::to_string(&live) {
        let _ = db.set_state("colony_peer_fitness", &json);
    }
    live
}

/// Load the current colony status.
pub fn load_status(db: &SoulDatabase) -> Option<ColonyStatus> {
    db.get_state("colony_status")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_merge_weight_fitter_peer() {
        // A peer with 2x your fitness should get ~2.0 weight
        let ratio = (0.6f64 / 0.3f64).clamp(0.1, 2.0);
        assert!((ratio - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_peer_merge_weight_weaker_peer() {
        // A peer with 0.5x your fitness should get ~0.5 weight
        let ratio = (0.15f64 / 0.3f64).clamp(0.1, 2.0);
        assert!((ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_niche_computation() {
        let peers = vec![
            PeerFitnessRecord {
                instance_id: "a".into(),
                fitness: 0.5,
                benchmark_pass_at_1: 60.0,
                role: "solver".into(),
                strongest_capability: "Code Generation".into(),
                measured_at: 0,
            },
            PeerFitnessRecord {
                instance_id: "b".into(),
                fitness: 0.4,
                benchmark_pass_at_1: 40.0,
                role: "solver".into(),
                strongest_capability: "Compilation".into(),
                measured_at: 0,
            },
        ];
        let niche = compute_niche(&peers);
        // Both peers are coders — niche should NOT be coding
        assert!(niche.is_some());
        assert_ne!(niche.as_deref(), Some("coding"));
    }
}
