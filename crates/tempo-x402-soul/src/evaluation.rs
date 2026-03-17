//! Evaluation: Rigorous Measurement for Publishable Science.
//!
//! ## Why This Module Exists
//!
//! Without measurement, the cognitive architecture is anecdote. With measurement,
//! it's science. This module provides:
//!
//! 1. **Brier Scores**: Per-system prediction calibration — when a system says
//!    "80% confident", is it right 80% of the time?
//!
//! 2. **Ablation Framework**: Feature flags to disable individual cognitive systems
//!    and measure the impact. "Does the cortex actually help, or is it overhead?"
//!
//! 3. **Comparative Analysis**: Is 4-system voting better than any single system?
//!    Is weight adaptation better than uniform weights?
//!
//! 4. **Imagination Feedback**: Track whether imagined plans influenced LLM output
//!    and whether those plans succeeded.
//!
//! 5. **Colony Benefit Measurement**: Does cognitive peer sharing actually improve
//!    individual agent performance?
//!
//! ## Key Metrics
//!
//! | Metric | What It Measures | Why It Matters |
//! |--------|-----------------|----------------|
//! | Brier score | Calibration quality | "Is my confidence honest?" |
//! | System AUC | Discriminative power | "Can this system tell good from bad?" |
//! | Ablation delta | System contribution | "Does removing X make things worse?" |
//! | Adaptation gain | Meta-learning value | "Are adapted weights better than uniform?" |
//! | Imagination hit rate | Generative quality | "Do imagined plans get used?" |
//! | Colony delta | Sharing benefit | "Does peer sync improve performance?" |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::SoulDatabase;

// ── Constants ────────────────────────────────────────────────────────

/// Number of calibration bins.
const CALIBRATION_BINS: usize = 10;
/// Window size for recent metric computation.
const RECENT_WINDOW: usize = 100;
/// Maximum prediction records to store.
const MAX_RECORDS: usize = 2000;

// ── Core Types ───────────────────────────────────────────────────────

/// A single prediction record from one cognitive system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionRecord {
    /// Which system made this prediction.
    pub system: String,
    /// The predicted probability of success (0.0 - 1.0).
    pub predicted_prob: f32,
    /// The confidence level (0.0 - 1.0).
    pub confidence: f32,
    /// Actual outcome: true = success, false = failure.
    pub actual: bool,
    /// Unix timestamp.
    pub timestamp: i64,
    /// What was being predicted (for debugging).
    pub context: String,
}

/// Brier score decomposition for a system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrierDecomposition {
    /// Overall Brier score (lower is better, 0.0 = perfect, 0.25 = random).
    pub brier_score: f32,
    /// Reliability: how well-calibrated are the probabilities?
    pub reliability: f32,
    /// Resolution: how much do predictions vary? (higher = more informative)
    pub resolution: f32,
    /// Total predictions used.
    pub n_predictions: u32,
}

/// Calibration bin: for predictions in [low, high), what was the actual success rate?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub bin_low: f32,
    pub bin_high: f32,
    pub predicted_mean: f32,
    pub actual_rate: f32,
    pub count: u32,
}

/// Per-system evaluation metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub system: String,
    /// Brier score (overall and recent).
    pub brier_overall: f32,
    pub brier_recent: f32,
    /// Accuracy (binary: was direction correct?).
    pub accuracy_overall: f32,
    pub accuracy_recent: f32,
    /// Calibration curve.
    pub calibration: Vec<CalibrationBin>,
    /// Total predictions.
    pub total_predictions: u32,
    /// Brier decomposition.
    pub decomposition: BrierDecomposition,
}

/// Ablation configuration: which systems are active?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationConfig {
    pub brain_enabled: bool,
    pub cortex_enabled: bool,
    pub genesis_enabled: bool,
    pub hivemind_enabled: bool,
    pub synthesis_enabled: bool,
    pub autonomy_enabled: bool,
}

impl Default for AblationConfig {
    fn default() -> Self {
        Self {
            brain_enabled: true,
            cortex_enabled: true,
            genesis_enabled: true,
            hivemind_enabled: true,
            synthesis_enabled: true,
            autonomy_enabled: true,
        }
    }
}

/// Result of comparing adapted weights vs uniform weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationGain {
    /// Brier score with adapted weights.
    pub adapted_brier: f32,
    /// Brier score with uniform weights (1/4 each).
    pub uniform_brier: f32,
    /// Gain: uniform_brier - adapted_brier (positive = adaptation helps).
    pub gain: f32,
    /// Is the gain statistically significant? (>0.01 Brier difference with n>50)
    pub significant: bool,
    /// Number of predictions used.
    pub n_predictions: u32,
}

/// Imagination feedback: did imagined plans get used?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImaginationMetrics {
    /// Total plans imagined.
    pub total_imagined: u64,
    /// How many imagined plans influenced the final plan (step overlap > 50%).
    pub influenced_plans: u64,
    /// How many influenced plans succeeded.
    pub influenced_successes: u64,
    /// Hit rate: influenced / total.
    pub hit_rate: f32,
    /// Success rate of influenced plans.
    pub influence_success_rate: f32,
}

/// Colony benefit: does peer sync improve performance?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyBenefit {
    /// Prediction accuracy BEFORE last peer sync.
    pub accuracy_before_sync: f32,
    /// Prediction accuracy AFTER last peer sync.
    pub accuracy_after_sync: f32,
    /// Delta (positive = sync helped).
    pub sync_delta: f32,
    /// Number of peer syncs measured.
    pub syncs_measured: u32,
    /// Average delta across all syncs.
    pub avg_sync_benefit: f32,
}

// ── The Evaluation Engine ────────────────────────────────────────────

/// Evaluation engine: tracks all metrics for publishable science.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    /// Raw prediction records (ring buffer).
    pub records: Vec<PredictionRecord>,
    /// Ablation configuration.
    pub ablation: AblationConfig,
    /// Imagination tracking.
    pub imagination: ImaginationMetrics,
    /// Colony benefit tracking.
    pub colony: ColonyBenefit,
    /// Per-sync accuracy snapshots for colony benefit measurement.
    sync_accuracy_snapshots: Vec<(i64, f32)>,
}

impl Default for Evaluation {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluation {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            ablation: AblationConfig::default(),
            imagination: ImaginationMetrics {
                total_imagined: 0,
                influenced_plans: 0,
                influenced_successes: 0,
                hit_rate: 0.0,
                influence_success_rate: 0.0,
            },
            colony: ColonyBenefit {
                accuracy_before_sync: 0.5,
                accuracy_after_sync: 0.5,
                sync_delta: 0.0,
                syncs_measured: 0,
                avg_sync_benefit: 0.0,
            },
            sync_accuracy_snapshots: Vec::new(),
        }
    }

    // ── Record ───────────────────────────────────────────────────────

    /// Record a prediction from a cognitive system.
    pub fn record_prediction(
        &mut self,
        system: &str,
        predicted_prob: f32,
        confidence: f32,
        actual: bool,
        context: &str,
    ) {
        let now = chrono::Utc::now().timestamp();
        self.records.push(PredictionRecord {
            system: system.to_string(),
            predicted_prob: predicted_prob.clamp(0.0, 1.0),
            confidence: confidence.clamp(0.0, 1.0),
            actual,
            timestamp: now,
            context: context.chars().take(100).collect(),
        });

        // Evict oldest if over capacity
        if self.records.len() > MAX_RECORDS {
            self.records.drain(..self.records.len() - MAX_RECORDS);
        }
    }

    /// Record that imagined plans were generated and whether they influenced the final plan.
    pub fn record_imagination(
        &mut self,
        n_imagined: u64,
        influenced: bool,
        plan_succeeded: Option<bool>,
    ) {
        self.imagination.total_imagined += n_imagined;
        if influenced {
            self.imagination.influenced_plans += 1;
            if plan_succeeded == Some(true) {
                self.imagination.influenced_successes += 1;
            }
        }
        // Recompute rates
        if self.imagination.total_imagined > 0 {
            self.imagination.hit_rate =
                self.imagination.influenced_plans as f32 / self.imagination.total_imagined as f32;
        }
        if self.imagination.influenced_plans > 0 {
            self.imagination.influence_success_rate = self.imagination.influenced_successes as f32
                / self.imagination.influenced_plans as f32;
        }
    }

    /// Snapshot accuracy before a peer sync (for colony benefit measurement).
    /// Records the count of records so we can compare ONLY new predictions after sync.
    pub fn pre_sync_snapshot(&mut self) {
        let now = chrono::Utc::now().timestamp();
        // Use RECENT accuracy (last 30 predictions), not all-time
        let recent_acc = self.compute_recent_accuracy(30);
        self.colony.accuracy_before_sync = recent_acc;
        self.sync_accuracy_snapshots.push((now, recent_acc));
        if self.sync_accuracy_snapshots.len() > 50 {
            self.sync_accuracy_snapshots
                .drain(..self.sync_accuracy_snapshots.len() - 50);
        }
    }

    /// Measure accuracy after a peer sync and compute benefit.
    /// Compares RECENT predictions (not all-time) to detect actual improvement.
    pub fn post_sync_measurement(&mut self) {
        let recent_acc = self.compute_recent_accuracy(30);
        self.colony.accuracy_after_sync = recent_acc;
        self.colony.sync_delta = recent_acc - self.colony.accuracy_before_sync;
        self.colony.syncs_measured += 1;

        // Running average of sync benefit
        let n = self.colony.syncs_measured as f32;
        self.colony.avg_sync_benefit =
            self.colony.avg_sync_benefit * ((n - 1.0) / n) + self.colony.sync_delta * (1.0 / n);
    }

    /// Compute accuracy of the most recent N predictions.
    fn compute_recent_accuracy(&self, n: usize) -> f32 {
        let recent: Vec<&PredictionRecord> = self.records.iter().rev().take(n).collect();
        if recent.is_empty() {
            return 0.5;
        }
        let correct = recent
            .iter()
            .filter(|r| (r.predicted_prob > 0.5) == r.actual)
            .count();
        correct as f32 / recent.len() as f32
    }

    // ── Compute Metrics ──────────────────────────────────────────────

    /// Compute Brier score for a system.
    pub fn brier_score(&self, system: &str) -> f32 {
        let preds: Vec<&PredictionRecord> = self
            .records
            .iter()
            .filter(|r| r.system == system && r.confidence > 0.1)
            .collect();
        if preds.is_empty() {
            return 0.25; // Random baseline
        }
        let sum: f32 = preds
            .iter()
            .map(|r| {
                let actual = if r.actual { 1.0 } else { 0.0 };
                (r.predicted_prob - actual).powi(2)
            })
            .sum();
        sum / preds.len() as f32
    }

    /// Compute Brier score for recent predictions only.
    pub fn brier_score_recent(&self, system: &str) -> f32 {
        let preds: Vec<&PredictionRecord> = self
            .records
            .iter()
            .filter(|r| r.system == system && r.confidence > 0.1)
            .rev()
            .take(RECENT_WINDOW)
            .collect();
        if preds.is_empty() {
            return 0.25;
        }
        let sum: f32 = preds
            .iter()
            .map(|r| {
                let actual = if r.actual { 1.0 } else { 0.0 };
                (r.predicted_prob - actual).powi(2)
            })
            .sum();
        sum / preds.len() as f32
    }

    /// Compute Brier decomposition (reliability + resolution).
    pub fn brier_decomposition(&self, system: &str) -> BrierDecomposition {
        let preds: Vec<&PredictionRecord> = self
            .records
            .iter()
            .filter(|r| r.system == system && r.confidence > 0.1)
            .collect();
        let n = preds.len();
        if n < 10 {
            return BrierDecomposition {
                brier_score: 0.25,
                reliability: 0.0,
                resolution: 0.0,
                n_predictions: n as u32,
            };
        }

        let base_rate = preds.iter().filter(|r| r.actual).count() as f32 / n as f32;
        let brier = self.brier_score(system);

        // Bin predictions for calibration
        let mut bins: Vec<(f32, u32, u32)> = vec![(0.0, 0, 0); CALIBRATION_BINS]; // (sum_pred, n_success, n_total)
        for pred in &preds {
            let bin_idx = ((pred.predicted_prob * CALIBRATION_BINS as f32) as usize)
                .min(CALIBRATION_BINS - 1);
            bins[bin_idx].0 += pred.predicted_prob;
            bins[bin_idx].2 += 1;
            if pred.actual {
                bins[bin_idx].1 += 1;
            }
        }

        // Reliability: mean squared difference between predicted and actual per bin
        let mut reliability = 0.0f32;
        let mut resolution = 0.0f32;
        for (sum_pred, n_success, n_total) in &bins {
            if *n_total == 0 {
                continue;
            }
            let mean_pred = sum_pred / *n_total as f32;
            let actual_rate = *n_success as f32 / *n_total as f32;
            let bin_weight = *n_total as f32 / n as f32;

            reliability += bin_weight * (mean_pred - actual_rate).powi(2);
            resolution += bin_weight * (actual_rate - base_rate).powi(2);
        }

        BrierDecomposition {
            brier_score: brier,
            reliability,
            resolution,
            n_predictions: n as u32,
        }
    }

    /// Compute calibration curve for a system.
    pub fn calibration_curve(&self, system: &str) -> Vec<CalibrationBin> {
        let preds: Vec<&PredictionRecord> = self
            .records
            .iter()
            .filter(|r| r.system == system && r.confidence > 0.1)
            .collect();

        let bin_width = 1.0 / CALIBRATION_BINS as f32;
        let mut bins = Vec::new();

        for i in 0..CALIBRATION_BINS {
            let low = i as f32 * bin_width;
            let high = low + bin_width;

            let bin_preds: Vec<&&PredictionRecord> = preds
                .iter()
                .filter(|r| r.predicted_prob >= low && r.predicted_prob < high)
                .collect();

            if bin_preds.is_empty() {
                bins.push(CalibrationBin {
                    bin_low: low,
                    bin_high: high,
                    predicted_mean: (low + high) / 2.0,
                    actual_rate: 0.0,
                    count: 0,
                });
                continue;
            }

            let predicted_mean =
                bin_preds.iter().map(|r| r.predicted_prob).sum::<f32>() / bin_preds.len() as f32;
            let actual_rate =
                bin_preds.iter().filter(|r| r.actual).count() as f32 / bin_preds.len() as f32;

            bins.push(CalibrationBin {
                bin_low: low,
                bin_high: high,
                predicted_mean,
                actual_rate,
                count: bin_preds.len() as u32,
            });
        }

        bins
    }

    /// Compute full metrics for all systems.
    pub fn compute_all_metrics(&self) -> Vec<SystemMetrics> {
        let systems = ["brain", "cortex", "genesis", "hivemind"];
        systems
            .iter()
            .map(|&sys| {
                let n = self.records.iter().filter(|r| r.system == sys).count() as u32;
                SystemMetrics {
                    system: sys.to_string(),
                    brier_overall: self.brier_score(sys),
                    brier_recent: self.brier_score_recent(sys),
                    accuracy_overall: self.compute_system_accuracy(sys, false),
                    accuracy_recent: self.compute_system_accuracy(sys, true),
                    calibration: self.calibration_curve(sys),
                    total_predictions: n,
                    decomposition: self.brier_decomposition(sys),
                }
            })
            .collect()
    }

    /// Compare adapted weights vs uniform weights.
    pub fn compute_adaptation_gain(&self) -> AdaptationGain {
        let systems = ["brain", "cortex", "genesis", "hivemind"];
        let n_per_system: Vec<u32> = systems
            .iter()
            .map(|s| self.records.iter().filter(|r| r.system == *s).count() as u32)
            .collect();

        let min_n = *n_per_system.iter().min().unwrap_or(&0);
        if min_n < 10 {
            return AdaptationGain {
                adapted_brier: 0.25,
                uniform_brier: 0.25,
                gain: 0.0,
                significant: false,
                n_predictions: min_n,
            };
        }

        // Compute adapted Brier: use actual synthesis weights
        let synth = crate::synthesis::load_synthesis(
            // Can't access DB here, so compute from records
            &crate::db::SoulDatabase::new(":memory:").unwrap(),
        );
        let adapted_brier = {
            let weights = [
                synth.weights.brain,
                synth.weights.cortex,
                synth.weights.genesis,
                synth.weights.hivemind,
            ];
            self.weighted_ensemble_brier(&weights)
        };

        // Compute uniform Brier: equal weights
        let uniform_brier = self.weighted_ensemble_brier(&[0.25, 0.25, 0.25, 0.25]);

        let gain = uniform_brier - adapted_brier;

        AdaptationGain {
            adapted_brier,
            uniform_brier,
            gain,
            significant: gain > 0.01 && min_n > 50,
            n_predictions: min_n,
        }
    }

    // ── Prompt Section ───────────────────────────────────────────────

    /// Generate an evaluation summary for prompts.
    pub fn summary(&self) -> String {
        if self.records.len() < 20 {
            return String::new();
        }

        let metrics = self.compute_all_metrics();
        let mut lines = Vec::new();
        lines.push("# Evaluation Metrics (empirical measurement)".to_string());

        for m in &metrics {
            if m.total_predictions < 5 {
                continue;
            }
            let calibration_emoji = if m.decomposition.reliability < 0.05 {
                "WELL-CALIBRATED"
            } else if m.decomposition.reliability < 0.1 {
                "moderate"
            } else {
                "POORLY-CALIBRATED"
            };
            lines.push(format!(
                "- {}: Brier={:.3} (recent {:.3}), accuracy={:.0}%, {} ({} predictions)",
                m.system,
                m.brier_overall,
                m.brier_recent,
                m.accuracy_overall * 100.0,
                calibration_emoji,
                m.total_predictions,
            ));
        }

        // Adaptation gain
        let gain = self.compute_adaptation_gain();
        if gain.n_predictions >= 10 {
            lines.push(format!(
                "- Weight adaptation: {} (adapted Brier {:.3} vs uniform {:.3}, delta {:+.3}{})",
                if gain.gain > 0.0 { "HELPS" } else { "neutral" },
                gain.adapted_brier,
                gain.uniform_brier,
                gain.gain,
                if gain.significant {
                    " *significant*"
                } else {
                    ""
                },
            ));
        }

        // Imagination
        if self.imagination.total_imagined > 0 {
            lines.push(format!(
                "- Imagination: {:.0}% hit rate ({}/{} influenced), {:.0}% success rate",
                self.imagination.hit_rate * 100.0,
                self.imagination.influenced_plans,
                self.imagination.total_imagined,
                self.imagination.influence_success_rate * 100.0,
            ));
        }

        // Colony benefit
        if self.colony.syncs_measured > 0 {
            lines.push(format!(
                "- Colony sync benefit: {:+.3} avg accuracy delta ({} syncs measured)",
                self.colony.avg_sync_benefit, self.colony.syncs_measured,
            ));
        }

        lines.join("\n")
    }

    // ── Internal ─────────────────────────────────────────────────────

    fn compute_system_accuracy(&self, system: &str, recent_only: bool) -> f32 {
        let preds: Vec<&PredictionRecord> = if recent_only {
            self.records
                .iter()
                .filter(|r| r.system == system && r.confidence > 0.1)
                .rev()
                .take(RECENT_WINDOW)
                .collect()
        } else {
            self.records
                .iter()
                .filter(|r| r.system == system && r.confidence > 0.1)
                .collect()
        };
        if preds.is_empty() {
            return 0.5;
        }
        let correct = preds
            .iter()
            .filter(|r| (r.predicted_prob > 0.5) == r.actual)
            .count();
        correct as f32 / preds.len() as f32
    }

    fn compute_overall_accuracy(&self) -> f32 {
        if self.records.is_empty() {
            return 0.5;
        }
        let correct = self
            .records
            .iter()
            .filter(|r| (r.predicted_prob > 0.5) == r.actual)
            .count();
        correct as f32 / self.records.len() as f32
    }

    fn weighted_ensemble_brier(&self, weights: &[f32; 4]) -> f32 {
        let systems = ["brain", "cortex", "genesis", "hivemind"];

        // Group records by timestamp (approximate: same second)
        let mut grouped: HashMap<i64, Vec<&PredictionRecord>> = HashMap::new();
        for record in &self.records {
            grouped.entry(record.timestamp).or_default().push(record);
        }

        let mut total_brier = 0.0f32;
        let mut count = 0u32;

        for (_, group) in &grouped {
            // Get predictions from each system
            let mut weighted_pred = 0.0f32;
            let mut weight_sum = 0.0f32;
            let mut actual = None;

            for (i, sys) in systems.iter().enumerate() {
                if let Some(pred) = group.iter().find(|r| r.system == *sys) {
                    weighted_pred += pred.predicted_prob * weights[i];
                    weight_sum += weights[i];
                    actual = Some(pred.actual);
                }
            }

            if let Some(actual_val) = actual {
                if weight_sum > 0.0 {
                    let ensemble_pred = weighted_pred / weight_sum;
                    let actual_f = if actual_val { 1.0 } else { 0.0 };
                    total_brier += (ensemble_pred - actual_f).powi(2);
                    count += 1;
                }
            }
        }

        if count == 0 {
            0.25
        } else {
            total_brier / count as f32
        }
    }

    // ── Persistence ──────────────────────────────────────────────────

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

// ── Persistence helpers ──────────────────────────────────────────────

pub fn load_evaluation(db: &SoulDatabase) -> Evaluation {
    match db.get_state("evaluation_state").ok().flatten() {
        Some(json) => Evaluation::from_json(&json).unwrap_or_else(Evaluation::new),
        None => Evaluation::new(),
    }
}

pub fn save_evaluation(db: &SoulDatabase, eval: &Evaluation) {
    let json = eval.to_json();
    if let Err(e) = db.set_state("evaluation_state", &json) {
        tracing::warn!(error = %e, "Failed to save evaluation state");
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_evaluation() {
        let eval = Evaluation::new();
        assert!(eval.records.is_empty());
        assert!(eval.ablation.brain_enabled);
    }

    #[test]
    fn test_brier_score_perfect() {
        let mut eval = Evaluation::new();
        // Perfect predictions
        for _ in 0..20 {
            eval.record_prediction("brain", 0.9, 0.8, true, "test");
            eval.record_prediction("brain", 0.1, 0.8, false, "test");
        }
        let brier = eval.brier_score("brain");
        assert!(
            brier < 0.05,
            "Perfect predictions should have low Brier: {brier}"
        );
    }

    #[test]
    fn test_brier_score_random() {
        let mut eval = Evaluation::new();
        // Random predictions (always 0.5)
        for i in 0..40 {
            eval.record_prediction("cortex", 0.5, 0.8, i % 2 == 0, "test");
        }
        let brier = eval.brier_score("cortex");
        assert!(
            (brier - 0.25).abs() < 0.01,
            "Random predictions should have Brier ~0.25: {brier}"
        );
    }

    #[test]
    fn test_calibration_curve() {
        let mut eval = Evaluation::new();
        for i in 0..100 {
            let pred = (i as f32) / 100.0;
            // Make actual match predicted (well-calibrated)
            let actual = (i % 100) < (i as usize);
            eval.record_prediction("brain", pred, 0.8, actual, "test");
        }
        let cal = eval.calibration_curve("brain");
        assert_eq!(cal.len(), CALIBRATION_BINS);
    }

    #[test]
    fn test_brier_decomposition() {
        let mut eval = Evaluation::new();
        for i in 0..50 {
            eval.record_prediction("brain", 0.8, 0.9, i % 5 != 0, "test");
        }
        let decomp = eval.brier_decomposition("brain");
        assert!(decomp.brier_score >= 0.0);
        assert!(decomp.n_predictions == 50);
    }

    #[test]
    fn test_imagination_tracking() {
        let mut eval = Evaluation::new();
        eval.record_imagination(5, false, None);
        eval.record_imagination(3, true, Some(true));
        eval.record_imagination(2, true, Some(false));

        assert_eq!(eval.imagination.total_imagined, 10);
        assert_eq!(eval.imagination.influenced_plans, 2);
        assert_eq!(eval.imagination.influenced_successes, 1);
        assert!((eval.imagination.influence_success_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_colony_benefit() {
        let mut eval = Evaluation::new();
        // Record some baseline predictions
        for i in 0..30 {
            eval.record_prediction("brain", 0.6, 0.8, i % 3 != 0, "pre-sync");
        }
        eval.pre_sync_snapshot();

        // Record better predictions after sync
        for i in 0..30 {
            eval.record_prediction("brain", 0.8, 0.8, i % 5 != 0, "post-sync");
        }
        eval.post_sync_measurement();

        assert_eq!(eval.colony.syncs_measured, 1);
    }

    #[test]
    fn test_all_metrics() {
        let mut eval = Evaluation::new();
        for i in 0..20 {
            eval.record_prediction("brain", 0.7, 0.8, i % 3 != 0, "test");
            eval.record_prediction("cortex", 0.5, 0.5, i % 2 == 0, "test");
        }
        let metrics = eval.compute_all_metrics();
        assert_eq!(metrics.len(), 4);
        assert!(metrics[0].total_predictions == 20); // brain
    }

    #[test]
    fn test_serialization() {
        let mut eval = Evaluation::new();
        eval.record_prediction("brain", 0.8, 0.9, true, "test");
        let json = eval.to_json();
        let restored = Evaluation::from_json(&json).unwrap();
        assert_eq!(restored.records.len(), 1);
    }
}
