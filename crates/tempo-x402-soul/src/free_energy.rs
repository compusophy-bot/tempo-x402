//! Free Energy: The Unifying Principle.
//!
//! # One Number to Rule Them All
//!
//! Seven cognitive systems. Different architectures. Different data.
//! Different biological inspirations. But they all do the same thing:
//! **reduce the agent's surprise about the world.**
//!
//! Free energy F is a single scalar that measures total cognitive surprise:
//!
//! ```text
//! F = Σ (system_surprise_i × weight_i)
//!
//! where:
//!   system_surprise = how wrong the system's predictions are
//!   weight = how much the agent trusts that system (from Synthesis)
//! ```
//!
//! When F is HIGH: the agent is confused, its models are wrong, it should EXPLORE.
//! When F is LOW: the agent understands its world, it can EXPLOIT.
//! Decreasing F over time = THE AGENT IS GETTING SMARTER.
//!
//! # Why This Is The Theoretical Contribution
//!
//! Karl Friston's Free Energy Principle says: all intelligent systems minimize
//! surprise. But Friston's formulation is mathematical abstraction — variational
//! Bayesian inference over continuous state spaces.
//!
//! We implement it as **computable free energy over discrete cognitive systems**:
//! - Brain surprise: prediction error rate (Brier score)
//! - Cortex surprise: world model prediction error
//! - Genesis surprise: plan template failure rate
//! - Hivemind surprise: pheromone prediction mismatch
//! - Synthesis surprise: metacognitive conflict rate
//!
//! Each system contributes to F. Each system's job is to minimize its contribution.
//! The Synthesis weights determine how much each contributes.
//! The whole architecture is a multi-scale free energy minimizer.
//!
//! # The Equation
//!
//! ```text
//! F(t) = w_brain × H_brain(t)
//!      + w_cortex × H_cortex(t)
//!      + w_genesis × H_genesis(t)
//!      + w_hivemind × H_hivemind(t)
//!      + w_synthesis × H_synthesis(t)
//!      + λ × Complexity(t)
//!
//! where:
//!   H_x(t) = surprise of system x at time t (0.0 = perfect, 1.0 = maximum surprise)
//!   w_x = trust weight from Synthesis (sum to 1.0)
//!   λ = complexity penalty (prevents overfitting to noise)
//!   Complexity = total model size / data observed
//! ```
//!
//! # What This Enables
//!
//! 1. **Single optimization target**: All systems minimize F together
//! 2. **Cross-system comparison**: Which system contributes most surprise?
//! 3. **Learning curve**: F(t) decreasing over time = intelligence increasing
//! 4. **Colony comparison**: Agent A's F vs Agent B's F = who's smarter?
//! 5. **Anomaly detection**: Sudden F spike = something changed in the environment
//! 6. **Exploration trigger**: F above threshold → switch to exploration mode

use serde::{Deserialize, Serialize};

use crate::cortex;
use crate::db::SoulDatabase;
use crate::evaluation;
use crate::genesis;
use crate::hivemind;
use crate::synthesis;

// ── Constants ────────────────────────────────────────────────────────

/// Complexity penalty weight (prevents overfitting).
const LAMBDA: f64 = 0.01;
/// History size for free energy trend.
const HISTORY_SIZE: usize = 200;
/// Free energy threshold above which the agent should explore.
const EXPLORATION_THRESHOLD: f64 = 0.6;
/// Free energy threshold below which the agent can exploit.
const EXPLOITATION_THRESHOLD: f64 = 0.3;

// ── Core Types ───────────────────────────────────────────────────────

/// Per-system surprise decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSurprise {
    pub system: String,
    /// Surprise value: 0.0 (no surprise) to 1.0 (maximum surprise).
    pub surprise: f64,
    /// Trust weight from Synthesis.
    pub weight: f64,
    /// Weighted contribution to total free energy.
    pub contribution: f64,
    /// How this surprise was computed.
    pub method: String,
}

/// Complete free energy measurement at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeEnergy {
    /// Total free energy F(t).
    pub total: f64,
    /// Per-system decomposition.
    pub components: Vec<SystemSurprise>,
    /// Complexity penalty term.
    pub complexity: f64,
    /// What regime the agent is in.
    pub regime: EnergyRegime,
    /// Trend: dF/dt (negative = getting smarter).
    pub trend: f64,
    /// Unix timestamp.
    pub timestamp: i64,
}

/// What behavioral regime the free energy suggests.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum EnergyRegime {
    /// F is high — models are wrong, EXPLORE to gather information.
    Explore,
    /// F is moderate — learning actively, balanced approach.
    #[default]
    Learn,
    /// F is low — models are accurate, EXPLOIT known strategies.
    Exploit,
    /// F spiked suddenly — something changed, INVESTIGATE.
    Anomaly,
}

impl std::fmt::Display for EnergyRegime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnergyRegime::Explore => write!(f, "EXPLORE"),
            EnergyRegime::Learn => write!(f, "LEARN"),
            EnergyRegime::Exploit => write!(f, "EXPLOIT"),
            EnergyRegime::Anomaly => write!(f, "ANOMALY"),
        }
    }
}

/// Free energy history for trend analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeEnergyHistory {
    /// Historical measurements (newest at end).
    pub measurements: Vec<FreeEnergy>,
    /// Global minimum ever achieved.
    pub global_min: f64,
    /// Global maximum ever observed.
    pub global_max: f64,
    /// Total measurements recorded.
    pub total_measurements: u64,
}

impl Default for FreeEnergyHistory {
    fn default() -> Self {
        Self {
            measurements: Vec::new(),
            global_min: f64::MAX,
            global_max: f64::MIN,
            total_measurements: 0,
        }
    }
}

// ── Computation ──────────────────────────────────────────────────────

/// Compute free energy F(t) from all cognitive systems.
/// This is THE unifying metric — every cycle, compute one number.
pub fn compute(db: &SoulDatabase) -> FreeEnergy {
    let now = chrono::Utc::now().timestamp();

    let synth = synthesis::load_synthesis(db);
    let cortex = cortex::load_cortex(db);
    let gene_pool = genesis::load_gene_pool(db);
    let hive = hivemind::load_hivemind(db);
    let eval = evaluation::load_evaluation(db);

    let mut components = Vec::new();

    // ── Brain surprise: Brier score (0.0 = perfect, 0.25 = random, ~1.0 = adversarial) ──
    let brain_brier = eval.brier_score("brain") as f64;
    // Normalize: Brier of 0.25 = random = 0.5 surprise, 0.0 = 0.0 surprise
    let brain_surprise = (brain_brier / 0.5).min(1.0);
    components.push(SystemSurprise {
        system: "brain".to_string(),
        surprise: brain_surprise,
        weight: synth.weights.brain as f64,
        contribution: brain_surprise * synth.weights.brain as f64,
        method: format!("Brier score: {:.3}", brain_brier),
    });

    // ── Cortex surprise: 1 - prediction_accuracy ──
    let cortex_accuracy = cortex.prediction_accuracy() as f64;
    let cortex_surprise = 1.0 - cortex_accuracy;
    components.push(SystemSurprise {
        system: "cortex".to_string(),
        surprise: cortex_surprise,
        weight: synth.weights.cortex as f64,
        contribution: cortex_surprise * synth.weights.cortex as f64,
        method: format!("1 - prediction_accuracy ({:.1}%)", cortex_accuracy * 100.0),
    });

    // ── Genesis surprise: 1 - average_template_fitness ──
    let genesis_surprise = if gene_pool.templates.is_empty() {
        0.8 // High surprise when no templates (don't know what works)
    } else {
        let avg_fitness = gene_pool
            .templates
            .iter()
            .map(|t| t.fitness as f64)
            .sum::<f64>()
            / gene_pool.templates.len() as f64;
        1.0 - avg_fitness
    };
    components.push(SystemSurprise {
        system: "genesis".to_string(),
        surprise: genesis_surprise,
        weight: synth.weights.genesis as f64,
        contribution: genesis_surprise * synth.weights.genesis as f64,
        method: format!(
            "1 - avg_template_fitness ({} templates)",
            gene_pool.templates.len()
        ),
    });

    // ── Hivemind surprise: fraction of actions without pheromone coverage ──
    let all_actions = [
        "read_file",
        "edit_code",
        "cargo_check",
        "commit",
        "search_code",
        "run_shell",
        "think",
        "generate_code",
    ];
    let covered = all_actions
        .iter()
        .filter(|a| {
            hive.smell(a, &hivemind::PheromoneCategory::Action)
                .is_some()
        })
        .count();
    let hivemind_surprise = 1.0 - (covered as f64 / all_actions.len() as f64);
    components.push(SystemSurprise {
        system: "hivemind".to_string(),
        surprise: hivemind_surprise,
        weight: synth.weights.hivemind as f64,
        contribution: hivemind_surprise * synth.weights.hivemind as f64,
        method: format!(
            "1 - pheromone_coverage ({}/{} actions covered)",
            covered,
            all_actions.len()
        ),
    });

    // ── Synthesis surprise: cognitive conflict rate ──
    let synthesis_surprise = if synth.total_predictions == 0 {
        0.5 // Uncertain
    } else {
        let conflict_rate = synth.conflicts.len() as f64 / synth.total_predictions as f64;
        conflict_rate.min(1.0)
    };
    components.push(SystemSurprise {
        system: "synthesis".to_string(),
        surprise: synthesis_surprise,
        weight: 0.1, // Synthesis gets a fixed small weight (it's the observer, not observed)
        contribution: synthesis_surprise * 0.1,
        method: format!(
            "conflict_rate ({}/{})",
            synth.conflicts.len(),
            synth.total_predictions
        ),
    });

    // ── Complexity penalty: total model size / total data observed ──
    let total_model_size = cortex.experiences.len() as f64
        + gene_pool.templates.len() as f64
        + hive.trails.len() as f64;
    let total_data = cortex.total_experiences_processed as f64
        + gene_pool.total_created as f64
        + hive.total_deposits as f64;
    let complexity = if total_data > 0.0 {
        LAMBDA * (total_model_size / total_data)
    } else {
        LAMBDA
    };

    // ── Total free energy ──
    let weighted_surprise: f64 = components.iter().map(|c| c.contribution).sum();
    let total = weighted_surprise + complexity;

    // ── Determine regime ──
    let history = load_history(db);
    let trend = compute_trend(&history, total);

    let regime = if trend > 0.05 && history.measurements.len() > 5 {
        // F is increasing suddenly — anomaly
        EnergyRegime::Anomaly
    } else if total > EXPLORATION_THRESHOLD {
        EnergyRegime::Explore
    } else if total < EXPLOITATION_THRESHOLD {
        EnergyRegime::Exploit
    } else {
        EnergyRegime::Learn
    };

    FreeEnergy {
        total,
        components,
        complexity,
        regime,
        trend,
        timestamp: now,
    }
}

/// Compute and store free energy, returning the measurement.
pub fn measure(db: &SoulDatabase) -> FreeEnergy {
    let fe = compute(db);

    // Store in history
    let mut history = load_history(db);
    history.measurements.push(fe.clone());
    if history.measurements.len() > HISTORY_SIZE {
        history
            .measurements
            .drain(..history.measurements.len() - HISTORY_SIZE);
    }
    if fe.total < history.global_min {
        history.global_min = fe.total;
    }
    if fe.total > history.global_max {
        history.global_max = fe.total;
    }
    history.total_measurements += 1;
    save_history(db, &history);

    // Store current for quick access
    if let Ok(json) = serde_json::to_string(&fe) {
        let _ = db.set_state("free_energy_current", &json);
    }

    fe
}

/// Load current free energy (quick access, no recomputation).
pub fn load_current(db: &SoulDatabase) -> Option<FreeEnergy> {
    db.get_state("free_energy_current")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

// ── Prompt Generation ────────────────────────────────────────────────

/// Generate a prompt section about the agent's free energy state.
pub fn prompt_section(db: &SoulDatabase) -> String {
    let fe = match load_current(db) {
        Some(fe) => fe,
        None => return String::new(),
    };

    let history = load_history(db);
    if history.total_measurements < 3 {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "# Free Energy: F = {:.3} [{}]",
        fe.total, fe.regime
    ));

    // Trend
    let trend_desc = if fe.trend < -0.02 {
        "DECREASING (getting smarter)"
    } else if fe.trend > 0.02 {
        "INCREASING (more confused)"
    } else {
        "stable"
    };
    lines.push(format!("Trend: {:+.4} — {}", fe.trend, trend_desc));

    // Decomposition (sorted by contribution)
    let mut sorted = fe.components.clone();
    sorted.sort_by(|a, b| {
        b.contribution
            .partial_cmp(&a.contribution)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    lines.push("Surprise decomposition:".to_string());
    for comp in &sorted {
        let bar_len = (comp.surprise * 20.0) as usize;
        let bar: String = "#".repeat(bar_len);
        lines.push(format!(
            "  {:<10} {:.2} × {:.2} = {:.3}  [{}] {}",
            comp.system, comp.surprise, comp.weight, comp.contribution, bar, comp.method
        ));
    }

    // Historical context
    lines.push(format!(
        "History: min={:.3}, max={:.3}, {} measurements",
        history.global_min, history.global_max, history.total_measurements
    ));

    // Regime-specific guidance
    match fe.regime {
        EnergyRegime::Explore => {
            lines.push(
                "Regime: EXPLORE — high surprise. Your world model is inaccurate. \
                 Prioritize diverse experiences over optimization."
                    .to_string(),
            );
        }
        EnergyRegime::Learn => {
            lines.push(
                "Regime: LEARN — moderate surprise. Models are improving. \
                 Balance exploration with exploitation."
                    .to_string(),
            );
        }
        EnergyRegime::Exploit => {
            lines.push(
                "Regime: EXPLOIT — low surprise. Your models are accurate. \
                 Prioritize proven strategies and optimization."
                    .to_string(),
            );
        }
        EnergyRegime::Anomaly => {
            lines.push(
                "Regime: ANOMALY — surprise spiked. Something changed. \
                 Investigate what's different before continuing."
                    .to_string(),
            );
        }
    }

    lines.join("\n")
}

// ── Trend Computation ────────────────────────────────────────────────

fn compute_trend(history: &FreeEnergyHistory, current: f64) -> f64 {
    if history.measurements.len() < 3 {
        return 0.0;
    }

    // Use last 10 measurements + current for linear regression
    let n = history.measurements.len().min(10);
    let recent: Vec<f64> = history.measurements[history.measurements.len() - n..]
        .iter()
        .map(|m| m.total)
        .chain(std::iter::once(current))
        .collect();

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
    if slope.is_finite() {
        slope
    } else {
        0.0
    }
}

// ── Persistence ──────────────────────────────────────────────────────

fn load_history(db: &SoulDatabase) -> FreeEnergyHistory {
    db.get_state("free_energy_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_history(db: &SoulDatabase, history: &FreeEnergyHistory) {
    if let Ok(json) = serde_json::to_string(history) {
        let _ = db.set_state("free_energy_history", &json);
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_free_energy() {
        let db = SoulDatabase::new(":memory:").unwrap();
        let fe = compute(&db);

        // Fresh agent should have high free energy (lots of surprise)
        assert!(
            fe.total > 0.0,
            "Free energy should be positive: {}",
            fe.total
        );
        assert!(
            fe.total <= 1.5,
            "Free energy should be bounded: {}",
            fe.total
        );
        assert_eq!(fe.components.len(), 5); // brain, cortex, genesis, hivemind, synthesis
    }

    #[test]
    fn test_regime_detection() {
        let db = SoulDatabase::new(":memory:").unwrap();
        let fe = compute(&db);

        // Fresh agent with no data should be in Explore regime
        assert!(
            fe.regime == EnergyRegime::Explore || fe.regime == EnergyRegime::Learn,
            "Fresh agent should be exploring or learning: {:?}",
            fe.regime
        );
    }

    #[test]
    fn test_measure_and_history() {
        let db = SoulDatabase::new(":memory:").unwrap();

        // Measure multiple times
        for _ in 0..5 {
            let fe = measure(&db);
            assert!(fe.total > 0.0);
        }

        let history = load_history(&db);
        assert_eq!(history.measurements.len(), 5);
        assert_eq!(history.total_measurements, 5);
    }

    #[test]
    fn test_components_sum() {
        let db = SoulDatabase::new(":memory:").unwrap();
        let fe = compute(&db);

        // Weighted surprise + complexity should equal total
        let weighted_sum: f64 = fe.components.iter().map(|c| c.contribution).sum();
        let expected = weighted_sum + fe.complexity;
        assert!(
            (fe.total - expected).abs() < 1e-6,
            "Total {:.6} should equal sum {:.6}",
            fe.total,
            expected
        );
    }

    #[test]
    fn test_trend_computation() {
        let mut history = FreeEnergyHistory::default();

        // Simulate decreasing free energy (agent getting smarter)
        for i in 0..10 {
            history.measurements.push(FreeEnergy {
                total: 0.8 - (i as f64 * 0.05),
                components: vec![],
                complexity: 0.0,
                regime: EnergyRegime::Learn,
                trend: 0.0,
                timestamp: i,
            });
        }

        let trend = compute_trend(&history, 0.25);
        assert!(
            trend < 0.0,
            "Decreasing F should have negative trend: {trend}"
        );
    }

    #[test]
    fn test_prompt_section() {
        let db = SoulDatabase::new(":memory:").unwrap();
        // Need some history first
        for _ in 0..5 {
            measure(&db);
        }
        let prompt = prompt_section(&db);
        assert!(prompt.contains("Free Energy"));
    }

    #[test]
    fn test_serialization() {
        let fe = FreeEnergy {
            total: 0.42,
            components: vec![SystemSurprise {
                system: "brain".to_string(),
                surprise: 0.3,
                weight: 0.25,
                contribution: 0.075,
                method: "test".to_string(),
            }],
            complexity: 0.01,
            regime: EnergyRegime::Learn,
            trend: -0.02,
            timestamp: 12345,
        };
        let json = serde_json::to_string(&fe).unwrap();
        let restored: FreeEnergy = serde_json::from_str(&json).unwrap();
        assert!((restored.total - 0.42).abs() < 1e-6);
    }
}
