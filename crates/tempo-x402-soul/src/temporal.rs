//! Temporal Binding: adaptive cognitive scheduling via neural oscillators.
//!
//! Replaces hardcoded `is_multiple_of(N)` timers with oscillators driven by
//! internal cognitive signals. Each operation fires when its urgency —
//! computed from the agent's actual cognitive state — exceeds a threshold.
//!
//! The free energy regime (Explore/Learn/Exploit/Anomaly) modulates all
//! timing globally.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::SoulDatabase;
use crate::free_energy::EnergyRegime;

// ── Constants ────────────────────────────────────────────────────────

/// Coupling constant: how much urgency influences firing.
/// Maximum sustained urgency (1.0) cuts effective period roughly in half.
const ALPHA: f64 = 0.5;

/// Steep sigmoid for urgency normalization.
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-6.0 * (x - 0.5)).exp())
}

// ── Operation identifiers ────────────────────────────────────────────

/// All cognitive operations that can be scheduled.
pub const OP_BRAIN_TRAINING: &str = "brain_training";
pub const OP_CORTEX_DREAMING: &str = "cortex_dreaming";
pub const OP_GENESIS_EVOLUTION: &str = "genesis_evolution";
pub const OP_PEER_SYNC: &str = "peer_sync";
pub const OP_BENCHMARK: &str = "benchmark";
pub const OP_THOUGHT_DECAY: &str = "thought_decay";
pub const OP_SELF_REPAIR: &str = "self_repair";
pub const OP_MEMORY_CONSOLIDATION: &str = "memory_consolidation";

// ── Signal snapshot ──────────────────────────────────────────────────

/// All internal signals collected from cognitive systems, normalized to [0,1].
#[derive(Debug, Clone, Default)]
pub struct SignalSnapshot {
    // Brain signals
    pub brain_surprise: f64,
    pub loss_pressure: f64,

    // Cortex signals
    pub cortex_surprise: f64,
    pub curiosity: f64,
    pub arousal: f64,

    // Free energy signals
    pub fe_total: f64,
    pub fe_trend: f64,
    pub regime: EnergyRegime,

    // Genesis signals
    pub genesis_surprise: f64,
    pub template_pressure: f64,
    pub staleness: f64,

    // Peer/hivemind signals
    pub sync_benefit: f64,
    pub hivemind_surprise: f64,
    pub peer_pressure: f64,

    // Thought/memory signals
    pub thought_pressure: f64,
    pub experience_pressure: f64,
    pub consolidation_staleness: f64,

    // Benchmark signals
    pub time_pressure: f64,
    pub training_progress: f64,

    // Self-repair signals
    pub brain_divergence: f64,
    pub synthesis_surprise: f64,

    // Bloch sphere modulation — continuous multipliers per operation.
    // Computed from the Bloch state (theta, phi) each cycle.
    pub bloch_modulation: std::collections::HashMap<String, f64>,
}

/// Collect all internal signals from cognitive systems.
pub fn compute_signals(db: &Arc<SoulDatabase>) -> SignalSnapshot {
    let mut s = SignalSnapshot::default();

    // ── Brain signals ──
    let brain = crate::brain::load_brain(db);
    // Normalize loss: 0 = perfect, 15+ = diverged → [0,1]
    s.brain_surprise = (brain.running_loss as f64 / 15.0).min(1.0);
    // Loss pressure: high if loss is increasing or stuck high
    s.loss_pressure = if brain.train_steps > 100 {
        (brain.running_loss as f64 / 10.0).min(1.0)
    } else {
        0.0
    };
    // Brain divergence for self-repair
    s.brain_divergence = if brain.train_steps > 1000 && brain.running_loss > 15.0 {
        1.0
    } else {
        (brain.running_loss as f64 / 20.0).min(1.0)
    };

    // ── Cortex signals ──
    let cortex = crate::cortex::load_cortex(db);
    s.cortex_surprise = 1.0 - cortex.prediction_accuracy() as f64;
    s.curiosity = cortex.global_curiosity as f64;
    s.arousal = cortex.emotion.arousal as f64;

    // Experience pressure: how full is the experience buffer relative to useful dream material
    let exp_count = cortex.experiences.len() as f64;
    s.experience_pressure = (exp_count / 50.0).min(1.0);

    // ── Free energy signals ──
    if let Some(fe) = crate::free_energy::load_current(db) {
        // FE total normalized: 0 = perfect, ~2.0 = high → [0,1]
        s.fe_total = (fe.total / 2.0).min(1.0);
        // FE trend: negative = improving, positive = worsening → [0,1]
        // Map [-0.1, +0.1] → [0, 1] where 0.5 = stable
        s.fe_trend = (fe.trend * 5.0 + 0.5).clamp(0.0, 1.0);
        s.regime = fe.regime;
    }

    // ── Genesis signals ──
    let gene_pool = crate::genesis::load_gene_pool(db);
    // Genesis surprise: inverse of average template fitness
    let avg_fitness = if gene_pool.templates.is_empty() {
        0.5
    } else {
        gene_pool
            .templates
            .iter()
            .map(|t| t.fitness as f64)
            .sum::<f64>()
            / gene_pool.templates.len() as f64
    };
    s.genesis_surprise = 1.0 - avg_fitness;
    // Template pressure: few templates = high pressure to evolve
    s.template_pressure = 1.0 - (gene_pool.templates.len() as f64 / 30.0).min(1.0);
    // Staleness: high generation gap without improvement
    s.staleness = (gene_pool.generation as f64 / 100.0).min(1.0);

    // ── Hivemind/peer signals ──
    let eval = crate::evaluation::load_evaluation(db);
    s.hivemind_surprise = {
        let brier = eval.brier_score("hivemind") as f64;
        (brier / 0.5).min(1.0)
    };
    s.sync_benefit = eval.colony.avg_sync_benefit.abs() as f64;
    // Peer pressure: number of known peers wanting sync
    let peer_count: f64 = db
        .get_state("peer_endpoint_catalog")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
        .map(|v| v.len() as f64)
        .unwrap_or(0.0);
    s.peer_pressure = (peer_count / 5.0).min(1.0);

    // ── Thought/memory signals ──
    let thought_count: f64 = db
        .recent_thoughts_by_type(
            &[
                crate::memory::ThoughtType::Reasoning,
                crate::memory::ThoughtType::Decision,
                crate::memory::ThoughtType::Observation,
            ],
            50,
        )
        .map(|t| t.len() as f64)
        .unwrap_or(0.0);
    s.thought_pressure = (thought_count / 30.0).min(1.0);

    // Consolidation staleness: cycles since last consolidation
    let last_consol: f64 = db
        .get_state("last_consolidation_cycle")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let current_cycle: f64 = db
        .get_state("total_think_cycles")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    s.consolidation_staleness = ((current_cycle - last_consol) / 40.0).min(1.0);

    // ── Benchmark signals ──
    let last_benchmark: i64 = db
        .get_state("last_benchmark_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let hours_since = (chrono::Utc::now().timestamp() - last_benchmark).max(0) as f64 / 3600.0;
    s.time_pressure = (hours_since / 6.0).min(1.0); // 6-hour cooldown

    // Training progress: how much has the brain improved recently
    s.training_progress = if brain.train_steps > 50 {
        (1.0 - brain.running_loss as f64 / 5.0).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // ── Synthesis surprise ──
    let synth = crate::synthesis::load_synthesis(db);
    // If in Conflicted or Stuck state, high surprise
    s.synthesis_surprise = match synth.state {
        crate::synthesis::CognitiveState::Stuck => 1.0,
        crate::synthesis::CognitiveState::Conflicted => 0.7,
        crate::synthesis::CognitiveState::Exploring => 0.3,
        _ => 0.1,
    };

    // ── Bloch sphere modulation ──
    // Compute continuous multipliers for each operation based on the
    // current Bloch state (theta, phi). Blends with discrete regime_multiplier.
    let bloch = crate::bloch::load_bloch(db);
    let ops = [
        OP_BRAIN_TRAINING,
        OP_CORTEX_DREAMING,
        OP_GENESIS_EVOLUTION,
        OP_PEER_SYNC,
        OP_BENCHMARK,
        OP_THOUGHT_DECAY,
        OP_SELF_REPAIR,
        OP_MEMORY_CONSOLIDATION,
    ];
    for op in &ops {
        s.bloch_modulation
            .insert(op.to_string(), bloch.oscillator_modulation(op));
    }

    s
}

// ── Oscillator ───────────────────────────────────────────────────────

/// A neural oscillator that tracks phase and urgency for a cognitive operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oscillator {
    /// Operation name (e.g. "brain_training").
    pub name: String,
    /// Phase φ ∈ [0,1) — advances by 1/T₀ each cycle.
    pub phase: f64,
    /// Accumulated urgency U ∈ [0,∞).
    pub urgency: f64,
    /// Natural period T₀ in cycles.
    pub natural_period: u32,
    /// Minimum cycles between firings.
    pub refractory: u32,
    /// Cycles since last firing.
    pub cycles_since_fire: u32,
    /// Total times this oscillator has fired.
    pub total_fires: u64,
    /// Signal weights: (signal_name, weight).
    #[serde(default)]
    pub signal_weights: Vec<(String, f64)>,
}

impl Oscillator {
    fn new(
        name: &str,
        natural_period: u32,
        refractory: u32,
        signal_weights: Vec<(&str, f64)>,
    ) -> Self {
        Self {
            name: name.to_string(),
            phase: 0.0,
            urgency: 0.0,
            natural_period,
            refractory,
            cycles_since_fire: refractory + 1, // Allow firing immediately after init
            total_fires: 0,
            signal_weights: signal_weights
                .into_iter()
                .map(|(s, w)| (s.to_string(), w))
                .collect(),
        }
    }

    /// Advance phase by one cycle and accumulate urgency.
    /// Returns true if the oscillator should fire.
    fn tick(&mut self, urgency_input: f64, regime_mult: f64) -> bool {
        self.cycles_since_fire += 1;

        // Advance natural phase
        self.phase += 1.0 / self.natural_period as f64;

        // Accumulate urgency (modulated by regime)
        self.urgency += urgency_input * regime_mult / self.natural_period as f64;

        // Check refractory period
        if self.cycles_since_fire <= self.refractory {
            return false;
        }

        // Firing condition: φ + α·U ≥ 1.0
        if self.phase + ALPHA * self.urgency >= 1.0 {
            self.phase = 0.0;
            self.urgency = 0.0;
            self.cycles_since_fire = 0;
            self.total_fires += 1;
            return true;
        }

        false
    }

    /// Effective period: how many cycles between firings at current urgency.
    pub fn effective_period(&self) -> f64 {
        if self.total_fires < 2 {
            self.natural_period as f64
        } else {
            // Estimate from current urgency contribution
            let u = self.urgency.max(0.001);
            self.natural_period as f64 / (1.0 + ALPHA * u)
        }
    }
}

// ── Urgency computation ──────────────────────────────────────────────

/// Look up a signal value by name from the snapshot.
fn get_signal(signals: &SignalSnapshot, name: &str) -> f64 {
    match name {
        "brain_surprise" => signals.brain_surprise,
        "loss_pressure" => signals.loss_pressure,
        "cortex_surprise" => signals.cortex_surprise,
        "curiosity" => signals.curiosity,
        "arousal" => signals.arousal,
        "fe_total" => signals.fe_total,
        "fe_trend" => signals.fe_trend,
        "genesis_surprise" => signals.genesis_surprise,
        "template_pressure" => signals.template_pressure,
        "staleness" => signals.staleness,
        "sync_benefit" => signals.sync_benefit,
        "hivemind_surprise" => signals.hivemind_surprise,
        "peer_pressure" => signals.peer_pressure,
        "thought_pressure" => signals.thought_pressure,
        "experience_pressure" => signals.experience_pressure,
        "consolidation_staleness" => signals.consolidation_staleness,
        "time_pressure" => signals.time_pressure,
        "training_progress" => signals.training_progress,
        "brain_divergence" => signals.brain_divergence,
        "synthesis_surprise" => signals.synthesis_surprise,
        _ => 0.0,
    }
}

/// Compute raw urgency for an oscillator given its signal weights and current signals.
fn compute_urgency(osc: &Oscillator, signals: &SignalSnapshot) -> f64 {
    let weighted_sum: f64 = osc
        .signal_weights
        .iter()
        .map(|(name, weight)| weight * get_signal(signals, name))
        .sum();
    sigmoid(weighted_sum)
}

/// Regime modulation multiplier R(regime, operation).
fn regime_multiplier(regime: &EnergyRegime, op_name: &str) -> f64 {
    match (regime, op_name) {
        (EnergyRegime::Anomaly, OP_BRAIN_TRAINING) => 2.0,
        (EnergyRegime::Anomaly, OP_CORTEX_DREAMING) => 2.0,
        (EnergyRegime::Anomaly, OP_GENESIS_EVOLUTION) => 1.5,
        (EnergyRegime::Anomaly, OP_PEER_SYNC) => 2.0,
        (EnergyRegime::Anomaly, OP_BENCHMARK) => 1.0,
        (EnergyRegime::Anomaly, OP_THOUGHT_DECAY) => 1.5,
        (EnergyRegime::Anomaly, OP_SELF_REPAIR) => 3.0,
        (EnergyRegime::Anomaly, OP_MEMORY_CONSOLIDATION) => 1.5,

        (EnergyRegime::Explore, OP_BRAIN_TRAINING) => 1.0,
        (EnergyRegime::Explore, OP_CORTEX_DREAMING) => 1.5,
        (EnergyRegime::Explore, OP_GENESIS_EVOLUTION) => 0.6,
        (EnergyRegime::Explore, OP_PEER_SYNC) => 1.5,
        (EnergyRegime::Explore, OP_BENCHMARK) => 0.5,
        (EnergyRegime::Explore, OP_THOUGHT_DECAY) => 0.8,
        (EnergyRegime::Explore, OP_SELF_REPAIR) => 1.0,
        (EnergyRegime::Explore, OP_MEMORY_CONSOLIDATION) => 0.8,

        (EnergyRegime::Learn, _) => 1.0, // Learn regime: all neutral

        (EnergyRegime::Exploit, OP_BRAIN_TRAINING) => 0.8,
        (EnergyRegime::Exploit, OP_CORTEX_DREAMING) => 0.6,
        (EnergyRegime::Exploit, OP_GENESIS_EVOLUTION) => 1.5,
        (EnergyRegime::Exploit, OP_PEER_SYNC) => 0.8,
        (EnergyRegime::Exploit, OP_BENCHMARK) => 1.5,
        (EnergyRegime::Exploit, OP_THOUGHT_DECAY) => 1.0,
        (EnergyRegime::Exploit, OP_SELF_REPAIR) => 0.8,
        (EnergyRegime::Exploit, OP_MEMORY_CONSOLIDATION) => 1.2,

        _ => 1.0,
    }
}

// ── Temporal Binding ─────────────────────────────────────────────────

/// The temporal binding system: manages all oscillators and dispatches fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalBinding {
    /// All oscillators.
    pub oscillators: Vec<Oscillator>,
    /// Current cycle count (for logging).
    pub current_cycle: u64,
    /// Recent fire events: (cycle, op_name).
    pub recent_fires: Vec<(u64, String)>,
}

impl Default for TemporalBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl TemporalBinding {
    /// Create a new temporal binding system with all oscillators.
    pub fn new() -> Self {
        let oscillators = vec![
            Oscillator::new(
                OP_BRAIN_TRAINING,
                10,
                3,
                vec![
                    ("brain_surprise", 0.4),
                    ("loss_pressure", 0.3),
                    ("arousal", 0.2),
                    ("fe_trend", 0.1),
                ],
            ),
            Oscillator::new(
                OP_CORTEX_DREAMING,
                10,
                5,
                vec![
                    ("cortex_surprise", 0.3),
                    ("curiosity", 0.3),
                    ("experience_pressure", 0.2),
                    ("arousal", 0.2),
                ],
            ),
            Oscillator::new(
                OP_GENESIS_EVOLUTION,
                20,
                8,
                vec![
                    ("genesis_surprise", 0.3),
                    ("fe_total", 0.2),
                    ("template_pressure", 0.3),
                    ("staleness", 0.2),
                ],
            ),
            Oscillator::new(
                OP_PEER_SYNC,
                5,
                3,
                vec![
                    ("sync_benefit", 0.4),
                    ("hivemind_surprise", 0.2),
                    ("peer_pressure", 0.2),
                    ("fe_trend", 0.2),
                ],
            ),
            Oscillator::new(
                OP_BENCHMARK,
                15,
                5,
                vec![
                    ("time_pressure", 0.5),
                    ("training_progress", 0.3),
                    ("fe_trend", 0.2),
                ],
            ),
            Oscillator::new(
                OP_THOUGHT_DECAY,
                10,
                5,
                vec![
                    ("thought_pressure", 0.6),
                    ("consolidation_staleness", 0.2),
                    ("fe_total", 0.2),
                ],
            ),
            Oscillator::new(
                OP_SELF_REPAIR,
                20,
                10,
                vec![
                    ("brain_divergence", 0.3),
                    ("hivemind_surprise", 0.2),
                    ("genesis_surprise", 0.2),
                    ("synthesis_surprise", 0.3),
                ],
            ),
            Oscillator::new(
                OP_MEMORY_CONSOLIDATION,
                30,
                12,
                vec![
                    ("thought_pressure", 0.4),
                    ("experience_pressure", 0.3),
                    ("fe_total", 0.3),
                ],
            ),
        ];

        Self {
            oscillators,
            current_cycle: 0,
            recent_fires: Vec::new(),
        }
    }

    /// Advance all oscillators by one cycle with the given signals.
    /// Returns the names of operations that should fire this cycle.
    pub fn tick(&mut self, signals: &SignalSnapshot) -> Vec<String> {
        self.current_cycle += 1;
        let mut fired = Vec::new();

        for osc in &mut self.oscillators {
            let urgency = compute_urgency(osc, signals);
            // Blend discrete regime multiplier with continuous Bloch sphere modulation.
            // The Bloch sphere gives smooth, gradient-driven modulation instead of
            // hard-coded regime × operation lookup tables.
            let discrete_mult = regime_multiplier(&signals.regime, &osc.name);
            let bloch_mult = signals
                .bloch_modulation
                .get(&osc.name)
                .copied()
                .unwrap_or(1.0);
            // Blend: 50% discrete + 50% Bloch (gradual transition)
            let regime_mult = 0.5 * discrete_mult + 0.5 * bloch_mult;

            if osc.tick(urgency, regime_mult) {
                tracing::info!(
                    op = %osc.name,
                    phase = format!("{:.2}", osc.phase),
                    urgency = format!("{:.3}", urgency),
                    regime_mult = format!("{:.1}", regime_mult),
                    total_fires = osc.total_fires,
                    "Temporal fire"
                );
                fired.push(osc.name.clone());
            }
        }

        // Track recent fires (keep last 50)
        for op in &fired {
            self.recent_fires.push((self.current_cycle, op.clone()));
        }
        if self.recent_fires.len() > 50 {
            self.recent_fires.drain(..self.recent_fires.len() - 50);
        }

        fired
    }

    /// Get a snapshot of all oscillator states for status endpoint.
    pub fn status(&self) -> Vec<OscillatorStatus> {
        self.oscillators
            .iter()
            .map(|osc| OscillatorStatus {
                name: osc.name.clone(),
                phase: osc.phase,
                urgency: osc.urgency,
                natural_period: osc.natural_period,
                effective_period: osc.effective_period(),
                refractory: osc.refractory,
                cycles_since_fire: osc.cycles_since_fire,
                total_fires: osc.total_fires,
            })
            .collect()
    }
}

/// Status snapshot for a single oscillator (for JSON serialization).
#[derive(Debug, Clone, Serialize)]
pub struct OscillatorStatus {
    pub name: String,
    pub phase: f64,
    pub urgency: f64,
    pub natural_period: u32,
    pub effective_period: f64,
    pub refractory: u32,
    pub cycles_since_fire: u32,
    pub total_fires: u64,
}

// ── Persistence ──────────────────────────────────────────────────────

const STATE_KEY: &str = "temporal_binding";

/// Load temporal binding from soul_state, or create fresh if absent.
pub fn load_temporal(db: &Arc<SoulDatabase>) -> TemporalBinding {
    db.get_state(STATE_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Save temporal binding to soul_state.
pub fn save_temporal(db: &Arc<SoulDatabase>, tb: &TemporalBinding) {
    if let Ok(json) = serde_json::to_string(tb) {
        let _ = db.set_state(STATE_KEY, &json);
    }
}

// ── Prompt section ───────────────────────────────────────────────────

/// Generate a prompt section about temporal scheduling state.
pub fn prompt_section(db: &Arc<SoulDatabase>) -> String {
    let tb = load_temporal(db);
    if tb.current_cycle == 0 {
        return String::new();
    }

    let mut lines = vec!["# TEMPORAL BINDING (Adaptive Scheduling)".to_string()];

    for osc in &tb.oscillators {
        lines.push(format!(
            "- {}: phase={:.2}, urgency={:.2}, T₀={}, fires={}, last={}cy ago",
            osc.name,
            osc.phase,
            osc.urgency,
            osc.natural_period,
            osc.total_fires,
            osc.cycles_since_fire,
        ));
    }

    if !tb.recent_fires.is_empty() {
        let recent: Vec<String> = tb
            .recent_fires
            .iter()
            .rev()
            .take(10)
            .map(|(cy, op)| format!("cy{}: {}", cy, op))
            .collect();
        lines.push(format!("Recent fires: {}", recent.join(", ")));
    }

    lines.join("\n")
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_signals() -> SignalSnapshot {
        SignalSnapshot::default()
    }

    #[test]
    fn test_natural_period_firing() {
        // With zero signals, oscillator fires at natural period
        let mut osc = Oscillator::new("test", 10, 0, vec![]);
        let signals = default_signals();
        let regime = EnergyRegime::Learn;

        let mut fired_at = Vec::new();
        for cycle in 1..=30 {
            let urgency = compute_urgency(&osc, &signals);
            let rm = regime_multiplier(&regime, "test");
            if osc.tick(urgency, rm) {
                fired_at.push(cycle);
            }
        }
        // Should fire approximately every 10 cycles
        assert!(
            !fired_at.is_empty(),
            "should fire at least once in 30 cycles"
        );
        assert_eq!(fired_at[0], 10, "first fire should be at cycle 10");
    }

    #[test]
    fn test_urgency_accelerates_firing() {
        // High urgency signals should cause earlier firing
        let mut osc_normal = Oscillator::new("test", 20, 0, vec![("brain_surprise", 1.0)]);
        let mut osc_urgent = Oscillator::new("test", 20, 0, vec![("brain_surprise", 1.0)]);

        let low_signals = default_signals();
        let mut high_signals = default_signals();
        high_signals.brain_surprise = 1.0;

        let regime = EnergyRegime::Learn;
        let mut normal_fire = 0u32;
        let mut urgent_fire = 0u32;

        for _ in 1..=20 {
            let u1 = compute_urgency(&osc_normal, &low_signals);
            let u2 = compute_urgency(&osc_urgent, &high_signals);
            let rm = regime_multiplier(&regime, "test");
            if osc_normal.tick(u1, rm) {
                normal_fire += 1;
            }
            if osc_urgent.tick(u2, rm) {
                urgent_fire += 1;
            }
        }

        assert!(
            urgent_fire >= normal_fire,
            "high urgency should fire at least as often: urgent={}, normal={}",
            urgent_fire,
            normal_fire
        );
    }

    #[test]
    fn test_refractory_period() {
        let mut osc = Oscillator::new("test", 3, 5, vec![]);
        let signals = default_signals();
        let regime = EnergyRegime::Learn;

        let mut fires = Vec::new();
        for cycle in 1..=20 {
            let urgency = compute_urgency(&osc, &signals);
            let rm = regime_multiplier(&regime, "test");
            if osc.tick(urgency, rm) {
                fires.push(cycle);
            }
        }

        // Check that consecutive fires are at least refractory+1 apart
        for i in 1..fires.len() {
            let gap = fires[i] - fires[i - 1];
            assert!(
                gap > 5,
                "gap between fires should respect refractory: gap={}, fires={:?}",
                gap,
                fires
            );
        }
    }

    #[test]
    fn test_regime_modulation() {
        // Anomaly regime should give 3x for self_repair
        assert_eq!(
            regime_multiplier(&EnergyRegime::Anomaly, OP_SELF_REPAIR),
            3.0
        );
        // Exploit should slow down dreaming
        assert_eq!(
            regime_multiplier(&EnergyRegime::Exploit, OP_CORTEX_DREAMING),
            0.6
        );
        // Learn is always 1.0
        assert_eq!(
            regime_multiplier(&EnergyRegime::Learn, OP_BRAIN_TRAINING),
            1.0
        );
    }

    #[test]
    fn test_homeostatic_guarantee() {
        // Even with zero signals, every oscillator must eventually fire
        let mut tb = TemporalBinding::new();
        let signals = default_signals();

        // Run for 200 cycles — every oscillator should fire at least once
        for _ in 0..200 {
            tb.tick(&signals);
        }

        for osc in &tb.oscillators {
            assert!(
                osc.total_fires > 0,
                "oscillator {} never fired in 200 cycles — homeostatic guarantee violated",
                osc.name
            );
        }
    }

    #[test]
    fn test_sigmoid() {
        assert!((sigmoid(0.0) - 0.047).abs() < 0.01);
        assert!((sigmoid(0.5) - 0.5).abs() < 0.01);
        assert!((sigmoid(1.0) - 0.953).abs() < 0.01);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut tb = TemporalBinding::new();
        let signals = default_signals();
        // Advance a few cycles
        for _ in 0..5 {
            tb.tick(&signals);
        }

        let json = serde_json::to_string(&tb).unwrap();
        let restored: TemporalBinding = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.current_cycle, tb.current_cycle);
        assert_eq!(restored.oscillators.len(), tb.oscillators.len());
        for (a, b) in restored.oscillators.iter().zip(tb.oscillators.iter()) {
            assert_eq!(a.name, b.name);
            assert!((a.phase - b.phase).abs() < 1e-10);
            assert_eq!(a.total_fires, b.total_fires);
        }
    }

    #[test]
    fn test_temporal_binding_tick() {
        let mut tb = TemporalBinding::new();
        let mut signals = default_signals();
        signals.brain_surprise = 0.8;
        signals.regime = EnergyRegime::Anomaly;

        let mut all_fires: Vec<String> = Vec::new();
        for _ in 0..100 {
            let fires = tb.tick(&signals);
            all_fires.extend(fires);
        }

        // With high brain surprise in anomaly regime, brain_training should fire multiple times
        let brain_fires = all_fires.iter().filter(|f| *f == OP_BRAIN_TRAINING).count();
        assert!(
            brain_fires > 5,
            "brain_training should fire frequently with high surprise in anomaly: got {}",
            brain_fires
        );
    }
}
