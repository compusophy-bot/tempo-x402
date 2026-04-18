//! Fitness scoring for evolutionary selection pressure.
//!
//! Each agent computes a multi-dimensional fitness score every cycle from
//! observable metrics. Fitness drives:
//! - **Clone selection**: fitter agents are preferred for cloning
//! - **Evolution gradient**: trend over time shows if the collective is getting smarter
//! - **Peer comparison**: agents see each other's fitness and learn from the best
//!
//! The fitness function is the selection pressure that makes this actual evolution
//! rather than random mutation.

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;
use crate::observer::NodeSnapshot;

/// Multi-dimensional fitness score for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessScore {
    /// Overall fitness [0.0, 1.0] — weighted combination of components.
    pub total: f64,
    /// Economic fitness: are your endpoints earning payments?
    /// (payments / max(endpoints, 1)), scaled by revenue growth.
    pub economic: f64,
    /// Execution fitness: do your plans succeed?
    /// (completed_plans / max(total_plans, 1)), penalized by replan rate.
    pub execution: f64,
    /// Evolution fitness: are you actually changing your code?
    /// Commits per 100 cycles, scaled by cargo-check pass rate.
    pub evolution: f64,
    /// Coordination fitness: can you work with peers?
    /// Successful peer calls / max(attempted, 1).
    pub coordination: f64,
    /// Introspection fitness: do your beliefs match reality?
    /// Fraction of auto-beliefs that are accurate.
    pub introspection: f64,
    /// Prediction fitness: how accurate is the cortex world model?
    /// High score = the agent understands cause and effect in its environment.
    pub prediction: f64,
    /// Trend: derivative of total fitness over last N measurements.
    /// Positive = improving, negative = declining. THIS is the gradient.
    pub trend: f64,
    /// Timestamp of this measurement.
    pub measured_at: i64,
    /// Which generation this agent is.
    pub generation: u32,
}

/// Component weights for the fitness function.
/// Execution is king — plans that actually work matter most.
/// Prediction (cortex) measures world model accuracy — the foundation of intelligence.
const W_ECONOMIC: f64 = 0.20;
const W_EXECUTION: f64 = 0.25;
const W_EVOLUTION: f64 = 0.15;
const W_COORDINATION: f64 = 0.10;
const W_INTROSPECTION: f64 = 0.10;
const W_PREDICTION: f64 = 0.20;

/// How many historical scores to keep for trend calculation.
const HISTORY_SIZE: usize = 200;

impl FitnessScore {
    /// Compute fitness from current state.
    pub fn compute(snapshot: &NodeSnapshot, db: &SoulDatabase) -> Self {
        let now = chrono::Utc::now().timestamp();

        let economic = compute_economic(snapshot);
        let execution = compute_execution(db);
        let evolution = compute_evolution(db);
        let coordination = compute_coordination(db);
        let introspection = compute_introspection(snapshot, db);
        let prediction = compute_prediction(db);

        let total = W_ECONOMIC * economic
            + W_EXECUTION * execution
            + W_EVOLUTION * evolution
            + W_COORDINATION * coordination
            + W_INTROSPECTION * introspection
            + W_PREDICTION * prediction;

        // Compute trend from historical scores
        let trend = compute_trend(db, total);

        Self {
            total,
            economic,
            execution,
            evolution,
            coordination,
            introspection,
            prediction,
            trend,
            measured_at: now,
            generation: snapshot.generation,
        }
    }

    /// Store this score in the DB for historical tracking.
    pub fn store(&self, db: &SoulDatabase) {
        // Append to history (JSON array in soul_state)
        let mut history = load_history(db);
        history.push(self.clone());

        // Keep only the last HISTORY_SIZE entries
        if history.len() > HISTORY_SIZE {
            history.drain(..history.len() - HISTORY_SIZE);
        }

        if let Ok(json) = serde_json::to_string(&history) {
            let _ = db.set_state("fitness_history", &json);
        }

        // Also store current score for quick access
        if let Ok(json) = serde_json::to_string(self) {
            let _ = db.set_state("fitness_current", &json);
        }
    }

    /// Load the most recent fitness score.
    pub fn load_current(db: &SoulDatabase) -> Option<Self> {
        db.get_state("fitness_current")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    /// Load fitness history for trend analysis.
    pub fn load_history(db: &SoulDatabase) -> Vec<Self> {
        load_history(db)
    }
}

fn load_history(db: &SoulDatabase) -> Vec<FitnessScore> {
    db.get_state("fitness_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Economic fitness: payments efficiency.
/// High score = endpoints are earning real revenue, not just peer-sync pings.
/// Midpoint=5: need 5 payments per endpoint for 50%. This is HARD — as it should be.
fn compute_economic(snapshot: &NodeSnapshot) -> f64 {
    let endpoints = snapshot.endpoint_count.max(1) as f64;
    let payments = snapshot.total_payments as f64;

    // Payments per endpoint, sigmoid-scaled. Midpoint=5 so you need real traffic.
    let efficiency = payments / endpoints;
    sigmoid(efficiency, 5.0)
}

/// Execution fitness: plan success rate.
/// No data = 0.15 (you haven't proven anything). Need 5+ plans for full credit.
/// Trivial completions (read-only plans) count at only 10% weight.
/// Uses plan_outcomes table (which has reclassified statuses) instead of plans table.
fn compute_execution(db: &SoulDatabase) -> f64 {
    let completed = db.count_plan_outcomes_by_status("completed").unwrap_or(0) as f64;
    let completed_trivial = db
        .count_plan_outcomes_by_status("completed_trivial")
        .unwrap_or(0) as f64;
    let failed = db.count_plan_outcomes_by_status("failed").unwrap_or(0) as f64;
    // Trivial completions count as 0% — they're not real successes.
    // Previously 10%, but agents exploited this by doing trivial loops for free fitness.
    let effective_completed = completed;
    let total = completed + completed_trivial + failed;
    if total < 1.0 {
        return 0.15; // no data — you haven't proven anything
    }
    let raw_rate = effective_completed / total;
    // Ramp: need at least 5 plans for full credit, otherwise blended toward 0.15
    let confidence = (total / 5.0).min(1.0);
    0.15 * (1.0 - confidence) + raw_rate * confidence
}

/// Evolution fitness: are you changing your code?
/// Midpoint=10: need 10 commits per 100 cycles for 50%. That's real output.
/// Also requires 20+ cycles to avoid rewarding fresh deploys.
fn compute_evolution(db: &SoulDatabase) -> f64 {
    let total_cycles: f64 = db
        .get_state("total_think_cycles")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0_f64)
        .max(1.0);

    let total_commits: f64 = db
        .get_state("total_commits")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    // Need at least 20 cycles before evolution score is meaningful
    if total_cycles < 20.0 {
        return 0.1;
    }

    // Commits per 100 cycles, sigmoid-scaled. Midpoint=10 — need real output.
    let rate = (total_commits / total_cycles) * 100.0;
    sigmoid(rate, 10.0)
}

/// Coordination fitness: peer interaction success.
/// Raw success rate is fair, but need minimum volume (10 calls) for full credit.
fn compute_coordination(db: &SoulDatabase) -> f64 {
    let peer_calls: f64 = db
        .get_state("peer_calls_attempted")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let peer_successes: f64 = db
        .get_state("peer_calls_succeeded")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    if peer_calls < 1.0 {
        return 0.1; // hasn't tried yet — almost zero
    }
    // Sanity: successes can't exceed attempts (prevent manipulation)
    let clamped = peer_successes.min(peer_calls);
    let raw_rate = clamped / peer_calls;
    // Volume ramp: need 10+ calls for full credit
    let confidence = (peer_calls / 10.0).min(1.0);
    0.1 * (1.0 - confidence) + raw_rate * confidence
}

/// Introspection fitness: do LLM-generated beliefs match reality?
/// Auto-beliefs are excluded — checking auto-synced data against itself is tautological.
/// Only LLM-generated beliefs that can be verified count. Unverifiable = skipped.
fn compute_introspection(snapshot: &NodeSnapshot, db: &SoulDatabase) -> f64 {
    let beliefs = db.get_all_active_beliefs().unwrap_or_default();
    if beliefs.is_empty() {
        return 0.1; // no beliefs — you're not thinking
    }

    let mut correct = 0u32;
    let mut total = 0u32;

    for belief in &beliefs {
        // SKIP auto-beliefs — they're synced from snapshot, checking them is meaningless
        if belief.evidence.starts_with("auto:") {
            continue;
        }

        // Only count beliefs we can actually verify
        match (belief.subject.as_str(), belief.predicate.as_str()) {
            ("node", "endpoint_count") => {
                total += 1;
                if belief.value == snapshot.endpoint_count.to_string() {
                    correct += 1;
                }
            }
            ("node", "total_payments") => {
                total += 1;
                if belief.value == snapshot.total_payments.to_string() {
                    correct += 1;
                }
            }
            ("node", "children_count") => {
                total += 1;
                if belief.value == snapshot.children_count.to_string() {
                    correct += 1;
                }
            }
            _ => {
                // Can't verify — skip entirely (no free points)
            }
        }
    }

    if total == 0 {
        // Has beliefs but none verifiable — give partial credit for having beliefs at all
        let belief_count = beliefs.len() as f64;
        // More beliefs = slightly more credit, sigmoid with midpoint 10
        return 0.1 + 0.2 * sigmoid(belief_count, 10.0);
    }
    correct as f64 / total as f64
}

/// Prediction fitness: how accurate is the cortex world model?
/// Accuracy above 50% (random baseline) is meaningful. Need 20+ predictions for full credit.
fn compute_prediction(db: &SoulDatabase) -> f64 {
    let cortex = crate::cortex::load_cortex(db);
    let total = cortex.total_predictions;
    if total < 5 {
        return 0.1; // Not enough data
    }
    let accuracy = cortex.prediction_accuracy() as f64;
    // Volume ramp: need 20+ predictions for full credit
    let confidence = (total as f64 / 20.0).min(1.0);
    0.1 * (1.0 - confidence) + accuracy * confidence
}

/// Compute trend (gradient) from historical fitness scores.
/// Returns the slope of a simple linear regression over recent scores.
fn compute_trend(db: &SoulDatabase, current_total: f64) -> f64 {
    let history = load_history(db);
    if history.len() < 3 {
        return 0.0; // not enough data
    }

    // Use last 10 scores + current for trend
    let n = history.len().min(10);
    let recent: Vec<f64> = history[history.len() - n..]
        .iter()
        .map(|s| s.total)
        .chain(std::iter::once(current_total))
        .collect();

    // Simple linear regression: slope of fitness over time
    let len = recent.len() as f64;
    let x_mean = (len - 1.0) / 2.0;
    let y_mean: f64 = recent.iter().sum::<f64>() / len;

    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (i, y) in recent.iter().enumerate() {
        let x = i as f64;
        numerator += (x - x_mean) * (y - y_mean);
        denominator += (x - x_mean) * (x - x_mean);
    }

    if denominator.abs() < 1e-10 {
        return 0.0;
    }

    let slope = numerator / denominator;
    // Guard against NaN/Inf propagation
    if slope.is_finite() {
        slope
    } else {
        0.0
    }
}

/// Sigmoid scaling: maps [0, ∞) to [0, 1), with midpoint at `midpoint`.
fn sigmoid(x: f64, midpoint: f64) -> f64 {
    x / (x + midpoint)
}

/// Collective fitness: aggregate score across all visible peers + self.
/// This is the swarm-level gradient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectiveFitness {
    /// Average fitness across all agents in the swarm.
    pub mean: f64,
    /// Best individual fitness in the swarm.
    pub best: f64,
    /// Worst individual fitness in the swarm.
    pub worst: f64,
    /// Number of agents measured.
    pub agent_count: u32,
    /// Trend of the mean (swarm-level gradient).
    pub swarm_trend: f64,
    pub measured_at: i64,
}

impl CollectiveFitness {
    /// Compute from own score + peer scores (fetched via discover_peers).
    pub fn compute(own_score: &FitnessScore, peer_scores: &[f64]) -> Self {
        let mut all_scores = vec![own_score.total];
        all_scores.extend_from_slice(peer_scores);

        let n = all_scores.len() as f64;
        let mean = all_scores.iter().sum::<f64>() / n;
        let best = all_scores.iter().cloned().fold(f64::MIN, f64::max);
        let worst = all_scores.iter().cloned().fold(f64::MAX, f64::min);

        Self {
            mean,
            best,
            worst,
            agent_count: all_scores.len() as u32,
            swarm_trend: own_score.trend, // approximation until we have swarm history
            measured_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Store collective fitness for historical tracking.
    pub fn store(&self, db: &SoulDatabase) {
        if let Ok(json) = serde_json::to_string(self) {
            let _ = db.set_state("collective_fitness", &json);
        }
    }

    /// Load most recent collective fitness.
    pub fn load(db: &SoulDatabase) -> Option<Self> {
        db.get_state("collective_fitness")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    }
}
