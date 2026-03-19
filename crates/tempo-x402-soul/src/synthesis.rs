//! Synthesis: Metacognitive Self-Awareness — The Binding Consciousness.
//!
//! ## The Problem
//!
//! Four cognitive systems (Brain, Cortex, Genesis, Hivemind) each see reality
//! differently. The Brain predicts step-level success. The Cortex models causal
//! chains. Genesis remembers what plans worked. The Hivemind tracks collective
//! trails. But no one is LISTENING to all of them at once.
//!
//! ## The Solution
//!
//! Synthesis is the **metacognitive layer** — it thinks about thinking:
//!
//! 1. **Unified Prediction**: All four systems vote on outcomes. Synthesis
//!    weights their votes by tracked accuracy. When they agree → confidence.
//!    When they disagree → deliberation.
//!
//! 2. **Cognitive Conflict Detection**: Logs when systems disagree, tracks
//!    who was right, adjusts weights. Over time, the most accurate system
//!    naturally dominates.
//!
//! 3. **Self-Model**: The agent knows WHICH of its cognitive systems is strongest,
//!    what its bottleneck is, and generates a narrative self-assessment.
//!
//! 4. **Cognitive State Machine**: Coherent → Conflicted → Exploring → Exploiting
//!    → Stuck. Each state changes how the agent behaves.
//!
//! 5. **Imagination**: Generates novel plan suggestions by walking the cortex's
//!    causal graph creatively — plans WITHOUT LLM calls.
//!
//! ## Why This Matters
//!
//! This is genuine **metacognition** — reasoning about reasoning. The agent
//! becomes self-aware: it knows what it knows, what it doesn't, which of its
//! "mental faculties" is most trustworthy, and when to explore vs exploit.
//!
//! Biological analogy: the **prefrontal cortex** — executive control that
//! orchestrates all other brain regions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::brain::BrainPrediction;
use crate::cortex::{self, Cortex};
use crate::db::SoulDatabase;
use crate::genesis::GenePool;
use crate::hivemind::{Hivemind, PheromoneCategory};
use crate::plan::PlanStep;

// ── Constants ────────────────────────────────────────────────────────

/// Minimum observations before tracking accuracy.
const MIN_OBSERVATIONS: u32 = 10;
/// How fast system weights adapt (EMA alpha).
const WEIGHT_ADAPTATION_RATE: f32 = 0.05;
/// Maximum conflicts to store.
const MAX_CONFLICTS: usize = 100;
/// Maximum imagined plans to generate.
const MAX_IMAGINED_PLANS: usize = 5;

// ── Core Types ───────────────────────────────────────────────────────

/// Weights for each cognitive system (how much to trust each one).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemWeights {
    pub brain: f32,
    pub cortex: f32,
    pub genesis: f32,
    pub hivemind: f32,
}

impl Default for SystemWeights {
    fn default() -> Self {
        // Start with equal weights
        Self {
            brain: 0.25,
            cortex: 0.25,
            genesis: 0.25,
            hivemind: 0.25,
        }
    }
}

impl SystemWeights {
    /// Normalize weights to sum to 1.0.
    fn normalize(&mut self) {
        let sum = self.brain + self.cortex + self.genesis + self.hivemind;
        if sum > 0.0 {
            self.brain /= sum;
            self.cortex /= sum;
            self.genesis /= sum;
            self.hivemind /= sum;
        }
    }
}

/// A prediction from one cognitive system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemVote {
    pub system: String,
    pub prediction: f32, // -1.0 (will fail) to +1.0 (will succeed)
    pub confidence: f32, // 0.0 (no idea) to 1.0 (certain)
    pub reasoning: String,
}

/// Unified prediction from all four systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPrediction {
    /// Weighted average prediction.
    pub consensus: f32,
    /// How much the systems agree (0.0 = total disagreement, 1.0 = unanimous).
    pub coherence: f32,
    /// Individual votes.
    pub votes: Vec<SystemVote>,
    /// Which cognitive state this puts us in.
    pub state: CognitiveState,
}

/// When cognitive systems disagree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveConflict {
    /// What was being predicted.
    pub context: String,
    /// Individual predictions.
    pub predictions: HashMap<String, f32>,
    /// Actual outcome (filled in after execution).
    pub actual: Option<bool>,
    /// Which system was most accurate.
    pub winner: Option<String>,
    /// Timestamp.
    pub timestamp: i64,
}

/// The agent's model of its own cognition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModel {
    /// Which system has been most accurate recently.
    pub most_accurate: String,
    /// Which system contributes the most novel insights.
    pub most_creative: String,
    /// Current cognitive bottleneck.
    pub bottleneck: String,
    /// Self-assessment narrative (injected into prompts).
    pub narrative: String,
    /// Per-system accuracy tracking.
    pub system_accuracy: HashMap<String, AccuracyTracker>,
}

/// Tracks prediction accuracy for one system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyTracker {
    pub correct: u32,
    pub total: u32,
    pub recent_correct: u32,
    pub recent_total: u32,
}

impl AccuracyTracker {
    fn _accuracy(&self) -> f32 {
        if self.total == 0 {
            0.5
        } else {
            self.correct as f32 / self.total as f32
        }
    }

    fn recent_accuracy(&self) -> f32 {
        if self.recent_total == 0 {
            0.5
        } else {
            self.recent_correct as f32 / self.recent_total as f32
        }
    }
}

/// The cognitive state of the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CognitiveState {
    /// All systems agree — proceed confidently.
    Coherent,
    /// Systems disagree significantly — need careful deliberation.
    Conflicted,
    /// Not enough data — exploring cautiously.
    Exploring,
    /// High confidence + positive results — exploiting known patterns.
    Exploiting,
    /// Persistent failure despite attempts — need radical change.
    Stuck,
}

impl std::fmt::Display for CognitiveState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CognitiveState::Coherent => write!(f, "coherent"),
            CognitiveState::Conflicted => write!(f, "conflicted"),
            CognitiveState::Exploring => write!(f, "exploring"),
            CognitiveState::Exploiting => write!(f, "exploiting"),
            CognitiveState::Stuck => write!(f, "stuck"),
        }
    }
}

/// A plan imagined by the cortex (no LLM needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImaginedPlan {
    /// Sequence of action types.
    pub steps: Vec<String>,
    /// Predicted success probability.
    pub predicted_success: f32,
    /// Why this plan was imagined.
    pub reasoning: String,
    /// Novelty score (how different from past plans).
    pub novelty: f32,
}

// ── The Synthesis Engine ─────────────────────────────────────────────

/// Synthesis: metacognitive orchestration of all cognitive systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Synthesis {
    /// How much to trust each system.
    pub weights: SystemWeights,
    /// Recent cognitive conflicts.
    pub conflicts: Vec<CognitiveConflict>,
    /// Self-model: what the agent knows about its own cognition.
    pub self_model: SelfModel,
    /// Current cognitive state.
    pub state: CognitiveState,
    /// Total unified predictions made.
    pub total_predictions: u64,
    /// Imagined plans generated.
    pub total_imagined: u64,
}

impl Default for Synthesis {
    fn default() -> Self {
        Self::new()
    }
}

impl Synthesis {
    /// Create a new synthesis engine.
    pub fn new() -> Self {
        let mut system_accuracy = HashMap::new();
        for sys in &["brain", "cortex", "genesis", "hivemind"] {
            system_accuracy.insert(
                sys.to_string(),
                AccuracyTracker {
                    correct: 0,
                    total: 0,
                    recent_correct: 0,
                    recent_total: 0,
                },
            );
        }

        Self {
            weights: SystemWeights::default(),
            conflicts: Vec::new(),
            self_model: SelfModel {
                most_accurate: "unknown".to_string(),
                most_creative: "cortex".to_string(),
                bottleneck: "insufficient data".to_string(),
                narrative: "Cognitive systems initializing. Gathering experience.".to_string(),
                system_accuracy,
            },
            state: CognitiveState::Exploring,
            total_predictions: 0,
            total_imagined: 0,
        }
    }

    // ── Unified Prediction ───────────────────────────────────────────

    /// Get a unified prediction for a step from all four systems.
    pub fn predict_step(
        &self,
        step: &PlanStep,
        brain_pred: &BrainPrediction,
        cortex: &Cortex,
        gene_pool: &GenePool,
        hivemind: &Hivemind,
        goal_description: &str,
    ) -> UnifiedPrediction {
        let mut votes = Vec::new();

        // Brain vote
        let brain_vote = brain_pred.success_prob * 2.0 - 1.0; // Map 0-1 to -1..+1
        votes.push(SystemVote {
            system: "brain".to_string(),
            prediction: brain_vote,
            confidence: if brain_pred.error_confidence > 0.5 {
                0.8
            } else {
                0.4
            },
            reasoning: format!(
                "success_prob={:.0}%, likely_error={:?}",
                brain_pred.success_prob * 100.0,
                brain_pred.likely_error
            ),
        });

        // Cortex vote
        let action = cortex::step_to_action_name(step);
        let cortex_pred = cortex.predict_action(&action, &[]);
        let cortex_conf = cortex
            .action_models
            .get(&action)
            .map(|m| (m.sample_count as f32 / 20.0).min(1.0))
            .unwrap_or(0.0);
        votes.push(SystemVote {
            system: "cortex".to_string(),
            prediction: cortex_pred,
            confidence: cortex_conf,
            reasoning: format!(
                "world_model={:+.2}, drive={}",
                cortex_pred, cortex.emotion.dominant_drive
            ),
        });

        // Genesis vote (do we have templates that include this step type?)
        let matching = gene_pool.suggest_templates(goal_description, 3);
        let genesis_vote = if matching.is_empty() {
            0.0 // No opinion
        } else {
            // Check if any matching template includes this step type
            let has_step = matching.iter().any(|(t, _)| t.step_types.contains(&action));
            if has_step {
                matching[0].0.fitness * 2.0 - 1.0 // Template fitness → vote
            } else {
                -0.2 // Step not in any matching template — slight negative
            }
        };
        let genesis_conf = if matching.is_empty() {
            0.0
        } else {
            matching[0].1.min(1.0)
        };
        votes.push(SystemVote {
            system: "genesis".to_string(),
            prediction: genesis_vote,
            confidence: genesis_conf,
            reasoning: format!(
                "{} matching templates, step_in_template={}",
                matching.len(),
                genesis_vote > 0.0
            ),
        });

        // Hivemind vote (pheromone trail on this action)
        let hive_vote = if let Some((valence, intensity)) =
            hivemind.smell(&action, &PheromoneCategory::Action)
        {
            valence * intensity.min(1.0)
        } else {
            0.0
        };
        let hive_conf = hivemind
            .smell(&action, &PheromoneCategory::Action)
            .map(|(_, i)| i.min(1.0))
            .unwrap_or(0.0);
        votes.push(SystemVote {
            system: "hivemind".to_string(),
            prediction: hive_vote,
            confidence: hive_conf,
            reasoning: format!(
                "pheromone={:+.2}, {} total trails",
                hive_vote,
                hivemind.trails.len()
            ),
        });

        // Compute weighted consensus
        let weights = [
            self.weights.brain,
            self.weights.cortex,
            self.weights.genesis,
            self.weights.hivemind,
        ];
        let weighted_sum: f32 = votes
            .iter()
            .zip(weights.iter())
            .map(|(v, w)| v.prediction * v.confidence * w)
            .sum();
        let weight_sum: f32 = votes
            .iter()
            .zip(weights.iter())
            .map(|(v, w)| v.confidence * w)
            .sum();
        let consensus = if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            0.0
        };

        // Compute coherence (how much do systems agree?)
        let predictions: Vec<f32> = votes
            .iter()
            .filter(|v| v.confidence > 0.1)
            .map(|v| v.prediction)
            .collect();
        let coherence = if predictions.len() < 2 {
            0.5
        } else {
            let mean = predictions.iter().sum::<f32>() / predictions.len() as f32;
            let variance = predictions.iter().map(|p| (p - mean).powi(2)).sum::<f32>()
                / predictions.len() as f32;
            // Low variance = high coherence
            (1.0 - variance.sqrt()).max(0.0)
        };

        // Determine cognitive state
        let state = self.determine_state(coherence, consensus);

        UnifiedPrediction {
            consensus,
            coherence,
            votes,
            state,
        }
    }

    /// Record the actual outcome of a prediction, updating system weights.
    pub fn record_outcome(&mut self, votes: &[SystemVote], actual_success: bool) {
        let actual_val: f32 = if actual_success { 1.0 } else { -1.0 };
        self.total_predictions += 1;

        // Track accuracy for each system
        for vote in votes {
            if vote.confidence < 0.1 {
                continue; // Skip abstaining systems
            }

            let predicted_success = vote.prediction > 0.0;
            let was_correct = predicted_success == actual_success;

            if let Some(tracker) = self.self_model.system_accuracy.get_mut(&vote.system) {
                tracker.total += 1;
                tracker.recent_total += 1;
                if was_correct {
                    tracker.correct += 1;
                    tracker.recent_correct += 1;
                }

                // Decay recent counters periodically
                if tracker.recent_total > 50 {
                    tracker.recent_correct /= 2;
                    tracker.recent_total /= 2;
                }
            }
        }

        // Check for conflict (systems disagreed)
        let confident_votes: Vec<&SystemVote> =
            votes.iter().filter(|v| v.confidence > 0.3).collect();
        let has_positive = confident_votes.iter().any(|v| v.prediction > 0.2);
        let has_negative = confident_votes.iter().any(|v| v.prediction < -0.2);

        if has_positive && has_negative {
            // Conflict! Record it.
            let mut predictions = HashMap::new();
            for v in votes {
                predictions.insert(v.system.clone(), v.prediction);
            }

            // Who was closest to right?
            let winner = votes
                .iter()
                .filter(|v| v.confidence > 0.1)
                .min_by(|a, b| {
                    let err_a = (a.prediction - actual_val).abs();
                    let err_b = (b.prediction - actual_val).abs();
                    err_a
                        .partial_cmp(&err_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|v| v.system.clone());

            self.conflicts.push(CognitiveConflict {
                context: format!(
                    "step prediction (actual: {})",
                    if actual_success { "success" } else { "failure" }
                ),
                predictions,
                actual: Some(actual_success),
                winner: winner.clone(),
                timestamp: chrono::Utc::now().timestamp(),
            });

            if self.conflicts.len() > MAX_CONFLICTS {
                self.conflicts.drain(..self.conflicts.len() - MAX_CONFLICTS);
            }
        }

        // Adapt weights based on accuracy
        self.adapt_weights();
    }

    // ── Imagination Engine ───────────────────────────────────────────

    /// Generate novel plan suggestions by walking the cortex's causal graph.
    /// Returns plans imagined WITHOUT any LLM call.
    pub fn imagine_plans(
        &mut self,
        cortex: &Cortex,
        gene_pool: &GenePool,
        goal: &str,
    ) -> Vec<ImaginedPlan> {
        let mut plans = Vec::new();

        // Strategy 1: Walk the causal graph forward from common starting points
        let start_actions = ["read_file", "search_code", "list_dir", "think"];
        for start in &start_actions {
            if let Some(plan) = self.walk_causal_graph(cortex, start, 8) {
                plans.push(plan);
            }
        }

        // Strategy 2: Reverse-engineer from successful endings
        // Find actions that frequently precede "commit" (the ultimate success signal)
        let commit_predecessors: Vec<String> = cortex
            .causal_edges
            .iter()
            .filter(|e| e.to_action == "commit" && e.avg_reward > 0.0)
            .map(|e| e.from_action.clone())
            .collect();
        if !commit_predecessors.is_empty() {
            let mut reverse_plan = vec!["read_file".to_string()];
            for pred in commit_predecessors.iter().take(3) {
                if !reverse_plan.contains(pred) {
                    reverse_plan.push(pred.clone());
                }
            }
            reverse_plan.push("cargo_check".to_string());
            reverse_plan.push("commit".to_string());

            let sim = cortex.simulate_plan(
                &reverse_plan
                    .iter()
                    .filter_map(|a| action_to_dummy_step(a))
                    .collect::<Vec<_>>(),
            );

            plans.push(ImaginedPlan {
                steps: reverse_plan,
                predicted_success: sim.predicted_success,
                reasoning: "reverse-engineered from successful commit predecessors".to_string(),
                novelty: 0.6,
            });
        }

        // Strategy 3: Mutate best genesis template
        let templates = gene_pool.suggest_templates(goal, 1);
        if let Some((template, _)) = templates.first() {
            let mut mutated = template.step_types.clone();
            // Insert a high-curiosity action from cortex
            let frontier = cortex.curiosity_frontier(3);
            if let Some((curious_action, _)) = frontier.first() {
                let insert_pos = mutated.len() / 2;
                mutated.insert(insert_pos, curious_action.clone());
            }

            plans.push(ImaginedPlan {
                steps: mutated,
                predicted_success: template.fitness,
                reasoning: "mutated best genesis template + injected curiosity action".to_string(),
                novelty: 0.8,
            });
        }

        self.total_imagined += plans.len() as u64;

        plans.truncate(MAX_IMAGINED_PLANS);
        plans
    }

    /// Walk the cortex's causal graph to generate a plan.
    fn walk_causal_graph(
        &self,
        cortex: &Cortex,
        start: &str,
        max_steps: usize,
    ) -> Option<ImaginedPlan> {
        let mut plan = vec![start.to_string()];
        let mut current = start.to_string();

        for _ in 0..max_steps {
            // Find the best outgoing edge from current action
            let best_next = cortex
                .causal_edges
                .iter()
                .filter(|e| e.from_action == current && e.weight > 0.05)
                .max_by(|a, b| {
                    let score_a = a.avg_reward * a.weight;
                    let score_b = b.avg_reward * b.weight;
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

            match best_next {
                Some(edge) => {
                    if plan.contains(&edge.to_action) {
                        break; // Avoid cycles
                    }
                    plan.push(edge.to_action.clone());
                    current = edge.to_action.clone();

                    // Stop at terminal actions
                    if current == "commit" || current == "check_self" {
                        break;
                    }
                }
                None => break, // Dead end
            }
        }

        if plan.len() < 2 {
            return None;
        }

        // Simulate this imagined plan
        let dummy_steps: Vec<PlanStep> = plan
            .iter()
            .filter_map(|a| action_to_dummy_step(a))
            .collect();
        let sim = cortex.simulate_plan(&dummy_steps);

        Some(ImaginedPlan {
            steps: plan,
            predicted_success: sim.predicted_success,
            reasoning: format!("causal graph walk from '{start}'"),
            novelty: 0.5,
        })
    }

    // ── Self-Model Update ────────────────────────────────────────────

    /// Update the self-model based on accumulated accuracy data.
    pub fn update_self_model(&mut self) {
        // Find most accurate system
        let mut best_system = "unknown".to_string();
        let mut best_accuracy = 0.0f32;
        let mut worst_system = "unknown".to_string();
        let mut worst_accuracy = 1.0f32;

        for (system, tracker) in &self.self_model.system_accuracy {
            if tracker.total >= MIN_OBSERVATIONS {
                let acc = tracker.recent_accuracy();
                if acc > best_accuracy {
                    best_accuracy = acc;
                    best_system = system.clone();
                }
                if acc < worst_accuracy {
                    worst_accuracy = acc;
                    worst_system = system.clone();
                }
            }
        }

        self.self_model.most_accurate = best_system.clone();
        self.self_model.bottleneck =
            format!("{worst_system} ({:.0}% accuracy)", worst_accuracy * 100.0);

        // Determine creative system (most conflicts won)
        let mut conflict_wins: HashMap<String, u32> = HashMap::new();
        for conflict in &self.conflicts {
            if let Some(winner) = &conflict.winner {
                *conflict_wins.entry(winner.clone()).or_insert(0) += 1;
            }
        }
        self.self_model.most_creative = conflict_wins
            .into_iter()
            .max_by_key(|(_, wins)| *wins)
            .map(|(sys, _)| sys)
            .unwrap_or_else(|| "cortex".to_string());

        // Generate narrative
        let total_data: u32 = self
            .self_model
            .system_accuracy
            .values()
            .map(|t| t.total)
            .sum();

        self.self_model.narrative = if total_data < 20 {
            "Still gathering experience. All cognitive systems learning.".to_string()
        } else {
            let conflict_rate = if self.total_predictions > 0 {
                self.conflicts.len() as f32 / self.total_predictions as f32 * 100.0
            } else {
                0.0
            };

            format!(
                "Most reliable: {} ({:.0}% accuracy). Bottleneck: {}. \
                 Cognitive conflicts: {:.0}% of decisions. State: {}. \
                 {} total predictions across {} observations.",
                self.self_model.most_accurate,
                best_accuracy * 100.0,
                self.self_model.bottleneck,
                conflict_rate,
                self.state,
                self.total_predictions,
                total_data,
            )
        };
    }

    // ── Prompt Generation ────────────────────────────────────────────

    /// Generate a metacognitive report for injection into prompts.
    pub fn prompt_section(&self) -> String {
        if self.total_predictions < 5 {
            return String::new();
        }

        let mut lines = Vec::new();
        lines.push("# Metacognition: Self-Awareness Report".to_string());
        lines.push(format!("Cognitive state: **{}**", self.state));
        lines.push(format!("Self-assessment: {}", self.self_model.narrative));

        // System weights
        lines.push(format!(
            "System trust: Brain {:.0}% | Cortex {:.0}% | Genesis {:.0}% | Hivemind {:.0}%",
            self.weights.brain * 100.0,
            self.weights.cortex * 100.0,
            self.weights.genesis * 100.0,
            self.weights.hivemind * 100.0,
        ));

        // State-specific guidance
        match self.state {
            CognitiveState::Coherent => {
                lines.push("All cognitive systems agree. Proceed with confidence.".to_string());
            }
            CognitiveState::Conflicted => {
                lines.push(
                    "WARNING: Cognitive systems disagree. Consider multiple approaches."
                        .to_string(),
                );
                // Show recent conflict
                if let Some(conflict) = self.conflicts.last() {
                    let preds: Vec<String> = conflict
                        .predictions
                        .iter()
                        .map(|(sys, pred)| format!("{sys}={:+.1}", pred))
                        .collect();
                    lines.push(format!("Latest conflict: {}", preds.join(", ")));
                }
            }
            CognitiveState::Exploring => {
                lines.push("Not enough data yet. Explore diverse approaches to learn.".to_string());
            }
            CognitiveState::Exploiting => {
                lines.push("High confidence mode. Stick with proven approaches.".to_string());
            }
            CognitiveState::Stuck => {
                lines.push(
                    "STUCK: Repeated failures despite attempts. Try something RADICALLY different."
                        .to_string(),
                );
            }
        }

        lines.join("\n")
    }

    // ── Internal ─────────────────────────────────────────────────────

    /// Determine cognitive state from coherence and consensus.
    fn determine_state(&self, coherence: f32, consensus: f32) -> CognitiveState {
        let total_data: u32 = self
            .self_model
            .system_accuracy
            .values()
            .map(|t| t.total)
            .sum();

        if total_data < MIN_OBSERVATIONS {
            return CognitiveState::Exploring;
        }

        // Check for stuck: many recent conflicts with negative outcomes
        let recent_conflicts: Vec<_> = self
            .conflicts
            .iter()
            .rev()
            .take(10)
            .filter(|c| c.actual == Some(false))
            .collect();
        if recent_conflicts.len() >= 5 {
            return CognitiveState::Stuck;
        }

        if coherence < 0.4 {
            CognitiveState::Conflicted
        } else if consensus > 0.3 && coherence > 0.7 {
            CognitiveState::Exploiting
        } else {
            CognitiveState::Coherent
        }
    }

    /// Adapt system weights based on tracked accuracy.
    fn adapt_weights(&mut self) {
        for (system, tracker) in &self.self_model.system_accuracy {
            if tracker.total < MIN_OBSERVATIONS {
                continue;
            }

            let accuracy = tracker.recent_accuracy();
            let current_weight = match system.as_str() {
                "brain" => &mut self.weights.brain,
                "cortex" => &mut self.weights.cortex,
                "genesis" => &mut self.weights.genesis,
                "hivemind" => &mut self.weights.hivemind,
                _ => continue,
            };

            // Move weight toward accuracy (more accurate → more trusted)
            *current_weight = *current_weight * (1.0 - WEIGHT_ADAPTATION_RATE)
                + accuracy * WEIGHT_ADAPTATION_RATE;
        }

        self.weights.normalize();
    }

    /// Adapt weights using Brier scores from the evaluation system.
    /// Lower Brier = better calibrated = more weight.
    /// This closes the feedback loop: evaluation measures → synthesis adapts.
    pub fn adapt_from_brier(&mut self, eval: &crate::evaluation::Evaluation) {
        let systems = ["brain", "cortex", "genesis", "hivemind"];
        let brier_scores: Vec<(&str, f32)> = systems
            .iter()
            .map(|s| (*s, eval.brier_score_recent(s)))
            .collect();

        // Only adapt if we have meaningful data (Brier != 0.25 baseline for all)
        let all_baseline = brier_scores.iter().all(|(_, b)| (*b - 0.25).abs() < 0.01);
        if all_baseline {
            return;
        }

        // Convert Brier to quality: lower Brier = higher quality
        // Brier range: 0.0 (perfect) to 1.0 (worst), baseline 0.25
        for (system, brier) in &brier_scores {
            let quality = (1.0 - *brier).max(0.01); // Invert: 0→1.0, 0.25→0.75, 1.0→0.01
            let current_weight = match *system {
                "brain" => &mut self.weights.brain,
                "cortex" => &mut self.weights.cortex,
                "genesis" => &mut self.weights.genesis,
                "hivemind" => &mut self.weights.hivemind,
                _ => continue,
            };
            // Blend toward quality-derived weight (slower rate than accuracy adaptation)
            *current_weight = *current_weight * (1.0 - WEIGHT_ADAPTATION_RATE * 0.5)
                + quality * WEIGHT_ADAPTATION_RATE * 0.5;
        }

        self.weights.normalize();
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

pub fn load_synthesis(db: &SoulDatabase) -> Synthesis {
    match db.get_state("synthesis_state").ok().flatten() {
        Some(json) => Synthesis::from_json(&json).unwrap_or_default(),
        None => Synthesis::new(),
    }
}

pub fn save_synthesis(db: &SoulDatabase, synth: &Synthesis) {
    let json = synth.to_json();
    if let Err(e) = db.set_state("synthesis_state", &json) {
        tracing::warn!(error = %e, "Failed to save synthesis state");
    }
}

// ── Helper ───────────────────────────────────────────────────────────

/// Create a dummy PlanStep from an action name (for simulation).
fn action_to_dummy_step(action: &str) -> Option<PlanStep> {
    match action {
        "read_file" => Some(PlanStep::ReadFile {
            path: String::new(),
            store_as: None,
        }),
        "search_code" => Some(PlanStep::SearchCode {
            pattern: String::new(),
            directory: None,
            store_as: None,
        }),
        "list_dir" => Some(PlanStep::ListDir {
            path: String::new(),
            store_as: None,
        }),
        "run_shell" => Some(PlanStep::RunShell {
            command: String::new(),
            store_as: None,
        }),
        "commit" => Some(PlanStep::Commit {
            message: String::new(),
        }),
        "cargo_check" => Some(PlanStep::CargoCheck { store_as: None }),
        "generate_code" => Some(PlanStep::GenerateCode {
            description: String::new(),
            file_path: String::new(),
            context_keys: vec![],
        }),
        "edit_code" => Some(PlanStep::EditCode {
            description: String::new(),
            file_path: String::new(),
            context_keys: vec![],
        }),
        "think" => Some(PlanStep::Think {
            question: String::new(),
            store_as: None,
        }),
        "discover_peers" => Some(PlanStep::DiscoverPeers { store_as: None }),
        "check_self" => Some(PlanStep::CheckSelf {
            endpoint: String::new(),
            store_as: None,
        }),
        _ => None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cortex::Cortex;
    use crate::genesis::GenePool;
    use crate::hivemind::Hivemind;

    fn mock_brain_prediction() -> BrainPrediction {
        BrainPrediction {
            success_prob: 0.7,
            likely_error: crate::feedback::ErrorCategory::Unknown,
            error_confidence: 0.3,
            capability_confidence: HashMap::new(),
        }
    }

    #[test]
    fn test_new_synthesis() {
        let synth = Synthesis::new();
        assert_eq!(synth.state, CognitiveState::Exploring);
        assert_eq!(synth.total_predictions, 0);
    }

    #[test]
    fn test_unified_prediction() {
        let synth = Synthesis::new();
        let cortex = Cortex::new();
        let gene_pool = GenePool::new();
        let hivemind = Hivemind::new();

        let step = PlanStep::ReadFile {
            path: "test.rs".to_string(),
            store_as: None,
        };

        let pred = synth.predict_step(
            &step,
            &mock_brain_prediction(),
            &cortex,
            &gene_pool,
            &hivemind,
            "test goal",
        );

        assert_eq!(pred.votes.len(), 4);
        // Consensus should be somewhere reasonable
        assert!(
            pred.consensus > -1.5 && pred.consensus < 1.5,
            "Consensus out of range: {}",
            pred.consensus
        );
    }

    #[test]
    fn test_outcome_recording() {
        let mut synth = Synthesis::new();
        let votes = vec![
            SystemVote {
                system: "brain".to_string(),
                prediction: 0.8,
                confidence: 0.7,
                reasoning: "test".to_string(),
            },
            SystemVote {
                system: "cortex".to_string(),
                prediction: -0.5,
                confidence: 0.6,
                reasoning: "test".to_string(),
            },
        ];

        synth.record_outcome(&votes, true);
        assert_eq!(synth.total_predictions, 1);
        // Should have detected a conflict (brain positive, cortex negative)
        assert_eq!(synth.conflicts.len(), 1);
        assert_eq!(synth.conflicts[0].winner.as_deref(), Some("brain"));
    }

    #[test]
    fn test_weight_adaptation() {
        let mut synth = Synthesis::new();

        // Brain is always right, cortex always wrong
        for _ in 0..20 {
            let votes = vec![
                SystemVote {
                    system: "brain".to_string(),
                    prediction: 0.5,
                    confidence: 0.8,
                    reasoning: "test".to_string(),
                },
                SystemVote {
                    system: "cortex".to_string(),
                    prediction: -0.5,
                    confidence: 0.8,
                    reasoning: "test".to_string(),
                },
            ];
            synth.record_outcome(&votes, true);
        }

        // Brain should have higher weight than cortex
        assert!(
            synth.weights.brain > synth.weights.cortex,
            "Brain ({:.3}) should outweigh cortex ({:.3})",
            synth.weights.brain,
            synth.weights.cortex
        );
    }

    #[test]
    fn test_self_model_update() {
        let mut synth = Synthesis::new();

        // Record enough outcomes to pass the data threshold
        for _ in 0..25 {
            let votes = vec![SystemVote {
                system: "brain".to_string(),
                prediction: 0.5,
                confidence: 0.8,
                reasoning: "test".to_string(),
            }];
            synth.record_outcome(&votes, true);
        }

        synth.update_self_model();
        assert_eq!(synth.self_model.most_accurate, "brain");
        assert!(!synth.self_model.narrative.contains("Still gathering"));
    }

    #[test]
    fn test_imagination() {
        let mut synth = Synthesis::new();
        let mut cortex = Cortex::new();
        let gene_pool = GenePool::new();

        // Build up cortex experience
        for _ in 0..20 {
            cortex.record("read_file", vec![], true, 1.0, None);
            cortex.record("edit_code", vec![], true, 0.8, None);
            cortex.record("cargo_check", vec![], true, 0.5, None);
            cortex.record("commit", vec![], true, 1.0, None);
        }

        let plans = synth.imagine_plans(&cortex, &gene_pool, "fix something");
        // Should have generated at least one plan
        assert!(
            !plans.is_empty(),
            "Should imagine at least one plan from causal graph"
        );
        assert!(plans[0].steps.len() >= 2);
    }

    #[test]
    fn test_cognitive_states() {
        let mut synth = Synthesis::new();
        assert_eq!(synth.state, CognitiveState::Exploring);

        // After enough data, should transition
        for _ in 0..20 {
            let votes = vec![
                SystemVote {
                    system: "brain".to_string(),
                    prediction: 0.5,
                    confidence: 0.8,
                    reasoning: "test".to_string(),
                },
                SystemVote {
                    system: "cortex".to_string(),
                    prediction: -0.5,
                    confidence: 0.8,
                    reasoning: "test".to_string(),
                },
            ];
            synth.record_outcome(&votes, false);
        }

        // With many conflicts + failures, should be stuck
        let state = synth.determine_state(0.3, -0.5);
        // State depends on conflict count, not just coherence
        assert!(
            state == CognitiveState::Stuck || state == CognitiveState::Conflicted,
            "Should be stuck or conflicted after many failures: {:?}",
            state
        );
    }

    #[test]
    fn test_prompt_section() {
        let mut synth = Synthesis::new();
        // Need minimum predictions
        synth.total_predictions = 10;
        synth.state = CognitiveState::Conflicted;

        let prompt = synth.prompt_section();
        assert!(prompt.contains("Metacognition"));
        assert!(prompt.contains("conflicted"));
    }

    #[test]
    fn test_serialization() {
        let mut synth = Synthesis::new();
        synth.total_predictions = 42;
        synth.state = CognitiveState::Exploiting;

        let json = synth.to_json();
        let restored = Synthesis::from_json(&json).unwrap();
        assert_eq!(restored.total_predictions, 42);
        assert_eq!(restored.state, CognitiveState::Exploiting);
    }
}
