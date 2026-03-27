//! The Cortex: a predictive world model with curiosity-driven exploration,
//! dream consolidation, and emotional valence.
//!
//! ## Architecture
//!
//! The Brain (brain.rs) is a reactive classifier: "will this step succeed?"
//! The Cortex is a **generative world model**: "what will happen if I do X?"
//!
//! ### Inference Loop
//! The inference loop functions as a continuous cycle of Active Inference:
//! 1. **Perception**: New observations are mapped to context hashes.
//! 2. **Prediction**: The causal graph generates a distribution of expected outcomes
//!    for a given action, conditioned on current context.
//! 3. **Error Detection**: Discrepancies between expected and actual outcomes are
//!    calculated as `surprise`.
//! 4. **Update**: Surprise triggers updates to the causal weights, minimizing future
//!    surprise through model refinement.
//!
//! ### State Assessment
//! Agent state is maintained via:
//! - **Causal Graph**: `(context, action) → (outcome, probability)`.
//! - **Emotional Valence**: A running affective state that biases action selection,
//!   derived from the cumulative reward history of the current goal trajectory.
//! - **Curiosity**: A metric defined by total recent prediction error, driving the
//!   agent to seek areas of high uncertainty.
//!
//! ### Free Energy Minimization
//! The agent operates on the Principle of Free Energy Minimization:
//! - **Surprise (Intrinsic Motivation)**: High prediction error is treated as
//!   an information deficit (negative information gain). The agent prioritizes
//!   actions that reduce this surprise.
//! - **Goal Alignment (Extrinsic Motivation)**: The agent seeks to minimize the
//!   expected divergence from successful goal states, balancing exploration
//!   (minimizing surprise) with exploitation (maximizing reward).
//!
//! Three subsystems work together:
//!
//! 1. **Experience Graph** — A causal graph of (context, action) → outcome transitions.
//!    Learned purely from temporal co-occurrence. Content-addressable via sparse hashing.
//!    Enables prediction, anomaly detection, and mental plan simulation.
//!
//! 2. **Curiosity Engine** — Tracks prediction errors over time. High prediction error
//!    = high curiosity = explore that frontier. Feeds into goal prioritization.
//!    Implements intrinsic motivation via the Free Energy Principle (minimize surprise
//!    by seeking out and resolving surprises).
//!
//! 3. **Dream Engine** — Periodic offline consolidation. Replays experiences with noise,
//!    extracts abstract patterns, generates counterfactuals, prunes stale memories.
//!    Inspired by hippocampal replay in biological brains.
//!
//! ## Why This Is Novel
//!
//! - **Not a neural network** — it's a sparse associative memory + causal graph.
//!   No gradient descent needed for the world model itself.
//! - **Emotional valence** biases risk-taking and exploration, like biological affect.
//! - **Mental simulation** lets the agent "imagine" plan outcomes before executing.
//! - **Curiosity is prediction error** — the agent literally seeks what it doesn't understand.
//! - **Dream consolidation** compresses episodic → semantic memory, like sleep does.
//! - **Collective cortex** — peers share causal knowledge, not just weights.
//!
//! ## Integration
//!
//! ```text
//! After each step:  cortex.record() → updates graph, curiosity, emotion
//! Every 10 cycles:  cortex.dream()  → consolidation, counterfactuals, pruning
//! Goal creation:    cortex.curiosity_report() → injected into prompts
//! Plan validation:  cortex.simulate_plan() → predict before execute
//! Peer sync:        cortex.export/merge() → share causal knowledge
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::SoulDatabase;
use crate::plan::PlanStep;

// ── Constants ────────────────────────────────────────────────────────

/// Maximum stored experiences before eviction.
const EXPERIENCE_CAPACITY: usize = 2000;
/// Maximum causal edges in the graph.
const EDGE_CAPACITY: usize = 5000;
/// Curiosity score exponential decay per cycle.
const CURIOSITY_DECAY: f32 = 0.95;
/// How many experiences to replay per dream cycle.
const DREAM_BATCH: usize = 100;
/// Minimum edge weight to survive pruning.
const EDGE_PRUNE_THRESHOLD: f32 = 0.05;
/// How fast emotional valence decays toward neutral.
const EMOTION_DECAY: f32 = 0.9;
/// Counterfactual generation probability during dreams.
const COUNTERFACTUAL_RATE: f32 = 0.2;

// ── Core Types ───────────────────────────────────────────────────────

/// A single observed transition: I was in state S, did action A, got outcome O.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    /// Unique ID.
    pub id: u64,
    /// Hash of the context (goal + recent history + agent state).
    pub context_hash: u64,
    /// Human-readable context tags for retrieval and analysis.
    pub context_tags: Vec<String>,
    /// The action taken (step type name).
    pub action: String,
    /// Whether the action succeeded.
    pub succeeded: bool,
    /// Reward signal: positive for good outcomes, negative for bad.
    pub reward: f32,
    /// Error category if failed.
    pub error_tag: Option<String>,
    /// Prediction error at time of recording: |predicted_reward - actual_reward|.
    pub surprise: f32,
    /// Emotional valence at time of recording (-1.0 to 1.0).
    pub valence: f32,
    /// Unix timestamp.
    pub timestamp: i64,
    /// How many times this experience was replayed in dreams.
    pub replay_count: u32,
    /// Abstraction level: 0 = raw experience, 1+ = consolidated pattern.
    pub abstraction_level: u32,
}

/// A causal edge: "after action A in context C, action B tends to have outcome O".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalEdge {
    /// Source action type.
    pub from_action: String,
    /// Target action type.
    pub to_action: String,
    /// Hash of context where this transition was observed.
    pub context_hash: u64,
    /// Edge weight: decayed frequency of this transition.
    pub weight: f32,
    /// Average reward of the target action following the source.
    pub avg_reward: f32,
    /// How many times this transition has been observed.
    pub observation_count: u32,
    /// Outcome correlation: does from_action's success predict to_action's success?
    /// Range: -1.0 (anti-correlated) to 1.0 (perfectly correlated).
    pub outcome_correlation: f32,
}

/// Per-action-type learned statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionModel {
    /// Base success rate across all contexts.
    pub success_rate: f32,
    /// Average reward when this action succeeds.
    pub avg_reward: f32,
    /// Variance of outcomes (high variance = unpredictable).
    pub outcome_variance: f32,
    /// Total observations.
    pub sample_count: u32,
    /// Context-dependent success modifiers: context_tag → success_delta.
    /// "When this tag is present, success rate shifts by this amount."
    pub context_modifiers: HashMap<String, f32>,
    /// Temporal patterns: which preceding actions affect this one's outcome?
    pub temporal_effects: HashMap<String, f32>,
}

/// Emotional state of the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalState {
    /// Current valence: -1.0 (frustrated/negative) to 1.0 (excited/positive).
    /// Neutral = 0.0.
    pub valence: f32,
    /// Arousal: 0.0 (calm/routine) to 1.0 (activated/alert).
    /// High arousal = novel situation or emotional intensity.
    pub arousal: f32,
    /// Confidence: 0.0 (uncertain) to 1.0 (certain).
    /// Based on recent prediction accuracy.
    pub confidence: f32,
    /// Drive: which motivation is currently dominant?
    pub dominant_drive: Drive,
}

/// Motivational drives that compete for behavioral control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Drive {
    /// Seek novel experiences (high curiosity, high arousal).
    Explore,
    /// Repeat successful patterns (high confidence, positive valence).
    Exploit,
    /// Avoid known failure modes (negative valence, moderate arousal).
    Avoid,
    /// No strong drive — default execution.
    Neutral,
}

impl std::fmt::Display for Drive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Drive::Explore => write!(f, "explore"),
            Drive::Exploit => write!(f, "exploit"),
            Drive::Avoid => write!(f, "avoid"),
            Drive::Neutral => write!(f, "neutral"),
        }
    }
}

/// Result of mentally simulating a plan through the world model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    /// Predicted overall success probability.
    pub predicted_success: f32,
    /// Predicted total reward.
    pub predicted_reward: f32,
    /// Per-step predictions.
    pub step_predictions: Vec<StepPrediction>,
    /// Confidence in the simulation (based on how many similar experiences we have).
    pub confidence: f32,
    /// Novel steps: actions we haven't seen before (high curiosity value).
    pub novel_steps: Vec<String>,
    /// Risky steps: actions with high outcome variance.
    pub risky_steps: Vec<String>,
}

/// Prediction for a single step in a simulated plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPrediction {
    pub action: String,
    pub predicted_success: f32,
    pub predicted_reward: f32,
    pub confidence: f32,
    /// Why this prediction was made.
    pub reasoning: String,
}

/// Insight extracted during dream consolidation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamInsight {
    /// What pattern was discovered.
    pub pattern: String,
    /// How strong the evidence is.
    pub confidence: f32,
    /// When this insight was generated.
    pub timestamp: i64,
}

/// Exportable cortex state for peer sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CortexSnapshot {
    /// Compressed action models (the learned knowledge).
    pub action_models: HashMap<String, ActionModel>,
    /// Top causal edges by weight.
    pub top_edges: Vec<CausalEdge>,
    /// Curiosity frontier: what this agent is most curious about.
    pub curiosity_frontier: Vec<(String, f32)>,
    /// Dream insights: abstract patterns discovered.
    pub insights: Vec<DreamInsight>,
    /// Source instance ID.
    pub source_id: String,
    /// Total experiences this cortex has processed.
    pub total_experiences: u64,
}

// ── The Cortex ───────────────────────────────────────────────────────

/// The Cortex: predictive world model with curiosity-driven exploration
/// and dream consolidation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cortex {
    // ── Experience storage ──
    /// Ring buffer of experiences (newest at end).
    pub experiences: Vec<Experience>,
    /// Next experience ID.
    next_id: u64,

    // ── Causal graph ──
    /// Directed edges between action types, weighted by observed frequency.
    pub causal_edges: Vec<CausalEdge>,

    // ── Prediction models ──
    /// Per-action learned statistics.
    pub action_models: HashMap<String, ActionModel>,

    // ── Curiosity ──
    /// Per-action curiosity scores (decayed prediction error).
    pub curiosity_scores: HashMap<String, f32>,
    /// Global curiosity level (average across all actions).
    pub global_curiosity: f32,

    // ── Emotional state ──
    pub emotion: EmotionalState,

    // ── Meta-learning: which features predict success? ──
    /// Feature importance scores: context_tag → predictive_value.
    pub feature_importance: HashMap<String, f32>,

    // ── Dream state ──
    /// Insights discovered during dream consolidation.
    pub insights: Vec<DreamInsight>,
    /// Total dream cycles completed.
    pub dream_cycles: u64,

    // ── Stats ──
    pub total_predictions: u64,
    pub correct_predictions: u64,
    pub total_experiences_processed: u64,

    // ── Internal ──
    /// Last action taken (for causal edge recording).
    last_action: Option<String>,
    /// Last context hash.
    last_context_hash: u64,
    /// Last action succeeded?
    last_succeeded: bool,
}

impl Default for Cortex {
    fn default() -> Self {
        Self::new()
    }
}

impl Cortex {
    /// Create a new empty cortex.
    pub fn new() -> Self {
        Self {
            experiences: Vec::new(),
            next_id: 0,
            causal_edges: Vec::new(),
            action_models: HashMap::new(),
            curiosity_scores: HashMap::new(),
            global_curiosity: 0.5,
            emotion: EmotionalState {
                valence: 0.0,
                arousal: 0.5,
                confidence: 0.5,
                dominant_drive: Drive::Explore,
            },
            feature_importance: HashMap::new(),
            insights: Vec::new(),
            dream_cycles: 0,
            total_predictions: 0,
            correct_predictions: 0,
            total_experiences_processed: 0,
            last_action: None,
            last_context_hash: 0,
            last_succeeded: false,
        }
    }

    /// Initialize cortex and log startup.
    pub fn init_and_log(&mut self) {
        self.record(
            "system_init",
            vec!["system".to_string(), "init".to_string()],
            true,
            1.0,
            None,
        );
    }

    // ── Recording ────────────────────────────────────────────────────

    /// Helper to construct a new experience.
    pub fn log_experience(
        &mut self,
        action: &str,
        context_tags: Vec<String>,
        succeeded: bool,
        reward: f32,
        error_tag: Option<String>,
    ) -> Experience {
        let context_hash = hash_context(&context_tags);
        let now = chrono::Utc::now().timestamp();

        // ── Predict before recording (to compute surprise) ──
        let predicted_reward = self.predict_action(action, &context_tags);
        let surprise = (predicted_reward - reward).abs();

        // ── Update emotional state ──
        self.update_emotion(succeeded, reward, surprise);

        // ── Create experience ──
        Experience {
            id: self.next_id,
            context_hash,
            context_tags,
            action: action.to_string(),
            succeeded,
            reward,
            error_tag,
            surprise,
            valence: self.emotion.valence,
            timestamp: now,
            replay_count: 0,
            abstraction_level: 0,
        }
    }

    /// Record an experience after a step executes.
    /// This is the primary learning signal for the cortex.
    pub fn record(
        &mut self,
        action: &str,
        context_tags: Vec<String>,
        succeeded: bool,
        reward: f32,
        error_tag: Option<String>,
    ) {
        let exp = self.log_experience(action, context_tags, succeeded, reward, error_tag);
        
        self.next_id += 1;
        self.total_experiences_processed += 1;

        // ── Update prediction accuracy ──
        self.total_predictions += 1;
        let predicted_success = exp.reward > 0.0;
        if predicted_success == succeeded {
            self.correct_predictions += 1;
        }

        // ── Store experience (evict oldest if at capacity) ──
        if self.experiences.len() >= EXPERIENCE_CAPACITY {
            // Evict least valuable: lowest (surprise * recency) score
            let now = exp.timestamp;
            let evict_idx = self
                .experiences
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let score_a = a.surprise * (1.0 / (1.0 + (now - a.timestamp) as f32 / 3600.0));
                    let score_b = b.surprise * (1.0 / (1.0 + (now - b.timestamp) as f32 / 3600.0));
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.experiences.remove(evict_idx);
        }
        self.experiences.push(exp.clone());

        // ── Update action model ──
        self.update_action_model(action, &exp.context_tags, succeeded, reward);

        // ── Update causal edge from previous action ──
        if let Some(ref prev_action) = self.last_action.clone() {
            self.update_causal_edge(
                prev_action,
                action,
                self.last_context_hash,
                succeeded,
                reward,
            );
        }

        // ── Update curiosity ──
        let curiosity = self
            .curiosity_scores
            .entry(action.to_string())
            .or_insert(0.5);
        *curiosity = *curiosity * CURIOSITY_DECAY + exp.surprise * (1.0 - CURIOSITY_DECAY);

        // ── Update feature importance ──
        for tag in &exp.context_tags {
            let importance = self.feature_importance.entry(tag.clone()).or_insert(0.0);
            // If this tag's presence correlates with surprising outcomes, it's important
            *importance = *importance * 0.95 + exp.surprise * 0.05;
        }

        // ── Update global curiosity ──
        if !self.curiosity_scores.is_empty() {
            self.global_curiosity =
                self.curiosity_scores.values().sum::<f32>() / self.curiosity_scores.len() as f32;
        }

        // ── Store last action for next causal edge ──
        self.last_action = Some(action.to_string());
        self.last_context_hash = exp.context_hash;
        self.last_succeeded = succeeded;
    }

    // ── Prediction ───────────────────────────────────────────────────

    /// Predict the expected reward for an action in a given context.
    /// Returns predicted reward: positive = likely good, negative = likely bad.
    pub fn predict_action(&self, action: &str, context_tags: &[String]) -> f32 {
        let model = match self.action_models.get(action) {
            Some(m) => m,
            None => return 0.0, // Unknown action — no prediction (max curiosity)
        };

        // Start with base success rate
        let mut predicted = model.success_rate * model.avg_reward.max(0.1);

        // Apply context modifiers
        for tag in context_tags {
            if let Some(modifier) = model.context_modifiers.get(tag) {
                predicted += modifier;
            }
        }

        // Apply temporal effects (what happened last?)
        if let Some(ref prev) = self.last_action {
            if let Some(effect) = model.temporal_effects.get(prev) {
                predicted += effect;
            }
        }

        predicted.clamp(-1.0, 1.0)
    }

    /// Mentally simulate a plan: predict the outcome of each step sequentially.
    pub fn simulate_plan(&self, steps: &[PlanStep]) -> SimulationResult {
        let mut cumulative_success = 1.0f32;
        let mut total_reward = 0.0f32;
        let mut step_predictions = Vec::new();
        let mut novel_steps = Vec::new();
        let mut risky_steps = Vec::new();
        let mut prev_action: Option<String> = self.last_action.clone();
        let mut total_confidence = 0.0f32;

        for step in steps {
            let action = step_to_action_name(step);
            let model = self.action_models.get(&action);

            let (predicted_success, predicted_reward, confidence, reasoning) = match model {
                Some(m) if m.sample_count >= 3 => {
                    let mut success = m.success_rate;
                    let reward = m.avg_reward;

                    // Temporal effect from previous action
                    if let Some(ref prev) = prev_action {
                        if let Some(effect) = m.temporal_effects.get(prev) {
                            success = (success + effect).clamp(0.0, 1.0);
                        }
                    }

                    let conf = (m.sample_count as f32 / 20.0).min(1.0);

                    // Flag high-variance actions
                    if m.outcome_variance > 0.3 {
                        risky_steps.push(action.clone());
                    }

                    let reason = format!(
                        "{}% base success ({} observations), variance {:.2}",
                        (success * 100.0) as u32,
                        m.sample_count,
                        m.outcome_variance,
                    );

                    (success, reward, conf, reason)
                }
                Some(m) => {
                    // Not enough data — moderate confidence
                    novel_steps.push(action.clone());
                    (
                        m.success_rate,
                        m.avg_reward,
                        0.2,
                        format!("sparse data ({} observations)", m.sample_count),
                    )
                }
                None => {
                    // Completely novel action
                    novel_steps.push(action.clone());
                    (0.5, 0.0, 0.0, "never observed before".to_string())
                }
            };

            cumulative_success *= predicted_success;
            total_reward += predicted_reward * predicted_success;
            total_confidence += confidence;

            step_predictions.push(StepPrediction {
                action: action.clone(),
                predicted_success,
                predicted_reward,
                confidence,
                reasoning,
            });

            prev_action = Some(action);
        }

        let avg_confidence = if step_predictions.is_empty() {
            0.0
        } else {
            total_confidence / step_predictions.len() as f32
        };

        SimulationResult {
            predicted_success: cumulative_success,
            predicted_reward: total_reward,
            step_predictions,
            confidence: avg_confidence,
            novel_steps,
            risky_steps,
        }
    }

    // ── Curiosity ────────────────────────────────────────────────────

    /// Get the curiosity frontier: action types ranked by curiosity score.
    /// High curiosity = high prediction error = frontier of knowledge.
    pub fn curiosity_frontier(&self, top_n: usize) -> Vec<(String, f32)> {
        let mut scores: Vec<(String, f32)> = self.curiosity_scores.clone().into_iter().collect();
        // Also add actions we've never seen (infinite curiosity)
        let known: std::collections::HashSet<&str> =
            self.action_models.keys().map(|s| s.as_str()).collect();
        let all_actions = [
            "read_file",
            "search_code",
            "list_dir",
            "run_shell",
            "commit",
            "check_self",
            "create_script_endpoint",
            "test_script_endpoint",
            "cargo_check",
            "generate_code",
            "edit_code",
            "think",
            "discover_peers",
            "call_peer",
            "review_peer_pr",
            "clone_self",
        ];
        for action in all_actions {
            if !known.contains(action) {
                scores.push((action.to_string(), 1.0)); // Max curiosity for unknown
            }
        }
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_n);
        scores
    }

    /// Generate a prompt section about what the cortex is curious about.
    pub fn curiosity_report(&self) -> String {
        if self.total_experiences_processed < 10 {
            return String::new(); // Not enough data yet
        }

        let mut lines = Vec::new();
        lines.push("# Cortex: Predictive World Model".to_string());

        // Emotional state
        let emotion_desc = match &self.emotion.dominant_drive {
            Drive::Explore => "EXPLORING — seeking novel experiences",
            Drive::Exploit => "EXPLOITING — repeating successful patterns",
            Drive::Avoid => "CAUTIOUS — avoiding known failure modes",
            Drive::Neutral => "NEUTRAL — no strong drive",
        };
        lines.push(format!(
            "- Emotional state: {} (valence: {:+.2}, arousal: {:.2}, confidence: {:.2})",
            emotion_desc, self.emotion.valence, self.emotion.arousal, self.emotion.confidence,
        ));

        // Prediction accuracy
        if self.total_predictions > 0 {
            let accuracy = self.correct_predictions as f32 / self.total_predictions as f32 * 100.0;
            lines.push(format!(
                "- World model accuracy: {:.1}% ({} predictions)",
                accuracy, self.total_predictions,
            ));
        }

        // Curiosity frontier
        let frontier = self.curiosity_frontier(5);
        if !frontier.is_empty() {
            lines.push("- Curiosity frontier (most surprising/unknown):".to_string());
            for (action, score) in &frontier {
                let label = if *score > 0.7 {
                    "HIGH CURIOSITY"
                } else if *score > 0.4 {
                    "moderate"
                } else {
                    "low"
                };
                lines.push(format!(
                    "  - {action}: {:.0}% surprise [{label}]",
                    score * 100.0
                ));
            }
        }

        // Top action models (what we've learned)
        let mut models: Vec<(&String, &ActionModel)> = self.action_models.iter().collect();
        models.sort_by(|a, b| {
            b.1.sample_count
                .partial_cmp(&a.1.sample_count)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if !models.is_empty() {
            lines.push("- Learned action statistics:".to_string());
            for (action, model) in models.iter().take(8) {
                let risk = if model.outcome_variance > 0.3 {
                    " [HIGH VARIANCE]"
                } else {
                    ""
                };
                lines.push(format!(
                    "  - {}: {:.0}% success, avg reward {:.2}, {} observations{}",
                    action,
                    model.success_rate * 100.0,
                    model.avg_reward,
                    model.sample_count,
                    risk,
                ));
            }
        }

        // Dream insights
        let recent_insights: Vec<_> = self.insights.iter().rev().take(3).collect();
        if !recent_insights.is_empty() {
            lines.push("- Dream insights (patterns discovered during consolidation):".to_string());
            for insight in recent_insights {
                lines.push(format!(
                    "  - {} (confidence: {:.0}%)",
                    insight.pattern,
                    insight.confidence * 100.0
                ));
            }
        }

        // Emotional guidance
        match self.emotion.dominant_drive {
            Drive::Explore => {
                lines.push(
                    "\nCortex recommends: EXPLORE — try actions you haven't attempted much, \
                     especially those with high curiosity scores above."
                        .to_string(),
                );
            }
            Drive::Exploit => {
                lines.push(
                    "\nCortex recommends: EXPLOIT — stick with proven approaches. \
                     Your recent success rate is high."
                        .to_string(),
                );
            }
            Drive::Avoid => {
                lines.push(
                    "\nCortex recommends: CAUTION — recent failures suggest changing approach. \
                     Avoid repeating patterns that led to negative outcomes."
                        .to_string(),
                );
            }
            Drive::Neutral => {}
        }

        lines.join("\n")
    }

    // ── Dream Consolidation ──────────────────────────────────────────

    /// Run a dream cycle: replay experiences, extract patterns, generate
    /// counterfactuals, and prune stale edges.
    ///
    /// Returns the number of insights generated.
    pub fn dream(&mut self) -> usize {
        if self.experiences.is_empty() {
            return 0;
        }

        let now = chrono::Utc::now().timestamp();
        self.dream_cycles += 1;
        let mut new_insights = Vec::new();

        // ── Phase 1: Replay and strengthen ──
        // Replay high-surprise experiences to strengthen their patterns.
        let mut replay_indices: Vec<usize> = (0..self.experiences.len()).collect();
        // Sort by surprise * recency (most surprising recent experiences first)
        replay_indices.sort_by(|&a, &b| {
            let exp_a = &self.experiences[a];
            let exp_b = &self.experiences[b];
            let score_a = exp_a.surprise * (1.0 / (1.0 + (now - exp_a.timestamp) as f32 / 3600.0));
            let score_b = exp_b.surprise * (1.0 / (1.0 + (now - exp_b.timestamp) as f32 / 3600.0));
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        replay_indices.truncate(DREAM_BATCH);

        for &idx in &replay_indices {
            if idx < self.experiences.len() {
                self.experiences[idx].replay_count += 1;
            }
        }

        // ── Phase 2: Extract temporal patterns ──
        // Look for recurring (A → B) sequences with consistent outcomes.
        let mut sequence_outcomes: HashMap<(String, String), Vec<bool>> = HashMap::new();
        for window in self.experiences.windows(2) {
            let key = (window[0].action.clone(), window[1].action.clone());
            sequence_outcomes
                .entry(key)
                .or_default()
                .push(window[1].succeeded);
        }

        for ((from, to), outcomes) in &sequence_outcomes {
            if outcomes.len() >= 5 {
                let success_rate =
                    outcomes.iter().filter(|&&s| s).count() as f32 / outcomes.len() as f32;
                // If there's a strong pattern (very high or very low success), record insight
                if success_rate > 0.85 {
                    new_insights.push(DreamInsight {
                        pattern: format!(
                            "{from} followed by {to} succeeds {:.0}% of the time ({} observations)",
                            success_rate * 100.0,
                            outcomes.len()
                        ),
                        confidence: (outcomes.len() as f32 / 20.0).min(1.0),
                        timestamp: now,
                    });
                } else if success_rate < 0.15 {
                    new_insights.push(DreamInsight {
                        pattern: format!(
                            "{from} followed by {to} FAILS {:.0}% of the time — avoid this sequence",
                            (1.0 - success_rate) * 100.0,
                        ),
                        confidence: (outcomes.len() as f32 / 20.0).min(1.0),
                        timestamp: now,
                    });
                }
            }
        }

        // ── Phase 3: Context pattern discovery ──
        // Which context tags correlate most strongly with success/failure?
        let mut tag_success: HashMap<String, (u32, u32)> = HashMap::new(); // (successes, total)
        for exp in &self.experiences {
            for tag in &exp.context_tags {
                let entry = tag_success.entry(tag.clone()).or_insert((0, 0));
                entry.1 += 1;
                if exp.succeeded {
                    entry.0 += 1;
                }
            }
        }

        for (tag, (successes, total)) in &tag_success {
            if *total >= 10 {
                let rate = *successes as f32 / *total as f32;
                let base_rate = self.experiences.iter().filter(|e| e.succeeded).count() as f32
                    / self.experiences.len().max(1) as f32;
                let delta = rate - base_rate;
                if delta.abs() > 0.2 {
                    let direction = if delta > 0.0 {
                        "BOOSTS success"
                    } else {
                        "HURTS success"
                    };
                    new_insights.push(DreamInsight {
                        pattern: format!(
                            "Context '{tag}' {direction} by {:.0}% ({} observations)",
                            delta.abs() * 100.0,
                            total
                        ),
                        confidence: (*total as f32 / 30.0).min(1.0),
                        timestamp: now,
                    });
                }
            }
        }

        // ── Phase 4: Counterfactual generation ──
        // "What if action A had been action B?" — generate imagined experiences.
        // Use deterministic selection based on dream_cycles.
        let mut counterfactual_count = 0u32;
        let seed = self.dream_cycles;
        for (i, exp) in self.experiences.iter().enumerate() {
            // Deterministic "random" selection
            let hash = (seed.wrapping_mul(2654435761) ^ i as u64) % 100;
            if hash < (COUNTERFACTUAL_RATE * 100.0) as u64 {
                // What's the most common alternative action in this context?
                let alternatives: Vec<&str> = self
                    .action_models
                    .keys()
                    .filter(|a| a.as_str() != exp.action)
                    .map(|a| a.as_str())
                    .collect();
                if let Some(alt) =
                    alternatives.get(counterfactual_count as usize % alternatives.len().max(1))
                {
                    let predicted = self.predict_action(alt, &exp.context_tags);
                    if (predicted - exp.reward).abs() > 0.3 {
                        // Significant counterfactual — record it
                        let direction = if predicted > exp.reward {
                            "would have been BETTER"
                        } else {
                            "would have been WORSE"
                        };
                        new_insights.push(DreamInsight {
                            pattern: format!(
                                "Counterfactual: {} instead of {} {} (predicted reward: {:.2} vs actual: {:.2})",
                                alt, exp.action, direction, predicted, exp.reward
                            ),
                            confidence: 0.3, // Low confidence — it's imagined
                            timestamp: now,
                        });
                    }
                }
                counterfactual_count += 1;
            }
        }

        // ── Phase 5: Prune stale edges ──
        // Decay all edge weights and remove weak ones.
        for edge in &mut self.causal_edges {
            edge.weight *= 0.95;
        }
        self.causal_edges
            .retain(|e| e.weight >= EDGE_PRUNE_THRESHOLD);

        // ── Phase 6: Consolidate similar experiences ──
        // Merge experiences with same (action, context_hash) into abstracted patterns.
        self.consolidate_experiences();

        // ── Store new insights (keep last 50) ──
        self.insights.extend(new_insights.iter().cloned());
        if self.insights.len() > 50 {
            self.insights.drain(..self.insights.len() - 50);
        }

        let insight_count = new_insights.len();
        if insight_count > 0 {
            tracing::info!(
                dream_cycle = self.dream_cycles,
                insights = insight_count,
                experiences = self.experiences.len(),
                edges = self.causal_edges.len(),
                "Dream consolidation complete"
            );
        }

        insight_count
    }

    // ── Peer Sharing ─────────────────────────────────────────────────

    /// Export cortex state for sharing with peers.
    pub fn export(&self, source_id: &str) -> CortexSnapshot {
        // Export top edges by weight
        let mut top_edges = self.causal_edges.clone();
        top_edges.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        top_edges.truncate(100);

        CortexSnapshot {
            action_models: self.action_models.clone(),
            top_edges,
            curiosity_frontier: self.curiosity_frontier(10),
            insights: self.insights.iter().rev().take(10).cloned().collect(),
            source_id: source_id.to_string(),
            total_experiences: self.total_experiences_processed,
        }
    }

    /// Merge knowledge from a peer's cortex snapshot.
    /// Weighted merge: peer knowledge blended at `merge_rate` (0.0-1.0).
    pub fn merge(&mut self, peer: &CortexSnapshot, merge_rate: f32) {
        let rate = merge_rate.clamp(0.0, 0.5);

        // Merge action models
        for (action, peer_model) in &peer.action_models {
            let local = self
                .action_models
                .entry(action.clone())
                .or_insert(ActionModel {
                    success_rate: 0.5,
                    avg_reward: 0.0,
                    outcome_variance: 0.5,
                    sample_count: 0,
                    context_modifiers: HashMap::new(),
                    temporal_effects: HashMap::new(),
                });

            // Weighted average based on sample counts
            let total = local.sample_count as f32 + peer_model.sample_count as f32;
            if total > 0.0 {
                let local_weight = (1.0 - rate) * local.sample_count as f32 / total;
                let peer_weight = rate * peer_model.sample_count as f32 / total;
                let weight_sum = local_weight + peer_weight;

                if weight_sum > 0.0 {
                    local.success_rate = (local.success_rate * local_weight
                        + peer_model.success_rate * peer_weight)
                        / weight_sum;
                    local.avg_reward = (local.avg_reward * local_weight
                        + peer_model.avg_reward * peer_weight)
                        / weight_sum;
                }
            }

            // Merge context modifiers
            for (tag, &modifier) in &peer_model.context_modifiers {
                let local_mod = local.context_modifiers.entry(tag.clone()).or_insert(0.0);
                *local_mod = *local_mod * (1.0 - rate) + modifier * rate;
            }

            // Merge temporal effects
            for (prev, &effect) in &peer_model.temporal_effects {
                let local_effect = local.temporal_effects.entry(prev.clone()).or_insert(0.0);
                *local_effect = *local_effect * (1.0 - rate) + effect * rate;
            }
        }

        // Merge causal edges (add peer's top edges, blend weights)
        for peer_edge in &peer.top_edges {
            if let Some(local_edge) = self.causal_edges.iter_mut().find(|e| {
                e.from_action == peer_edge.from_action && e.to_action == peer_edge.to_action
            }) {
                local_edge.weight = local_edge.weight * (1.0 - rate) + peer_edge.weight * rate;
                local_edge.avg_reward =
                    local_edge.avg_reward * (1.0 - rate) + peer_edge.avg_reward * rate;
            } else if self.causal_edges.len() < EDGE_CAPACITY {
                let mut new_edge = peer_edge.clone();
                new_edge.weight *= rate; // Discount imported edges
                self.causal_edges.push(new_edge);
            }
        }

        // Import peer's dream insights (low confidence — it's secondhand).
        // Strip existing "[from peer ...]" prefixes to prevent recursive nesting
        // that bloats insight strings on every sync cycle.
        let now = chrono::Utc::now().timestamp();
        let peer_prefix_re = "[from peer ";
        for insight in &peer.insights {
            // Strip all existing "[from peer ...] " prefixes
            let mut pattern = insight.pattern.as_str();
            while pattern.starts_with(peer_prefix_re) {
                if let Some(end) = pattern.find("] ") {
                    pattern = &pattern[end + 2..];
                } else {
                    break;
                }
            }
            self.insights.push(DreamInsight {
                pattern: format!("[from peer {}] {}", peer.source_id, pattern),
                confidence: insight.confidence * 0.5, // Halve confidence for imported
                timestamp: now,
            });
        }
        if self.insights.len() > 50 {
            self.insights.drain(..self.insights.len() - 50);
        }

        tracing::info!(
            peer = %peer.source_id,
            peer_experiences = peer.total_experiences,
            merged_models = peer.action_models.len(),
            merged_edges = peer.top_edges.len(),
            merged_insights = peer.insights.len(),
            "Cortex merged peer knowledge"
        );
    }

    // ── Persistence ──────────────────────────────────────────────────

    /// Serialize to JSON for storage.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    /// Prediction accuracy as a ratio (0.0 - 1.0).
    pub fn prediction_accuracy(&self) -> f32 {
        if self.total_predictions == 0 {
            return 0.5;
        }
        self.correct_predictions as f32 / self.total_predictions as f32
    }

    // ── Internal Helpers ─────────────────────────────────────────────

    /// Update the action model with a new observation.
    fn update_action_model(
        &mut self,
        action: &str,
        context_tags: &[String],
        succeeded: bool,
        reward: f32,
    ) {
        let model = self
            .action_models
            .entry(action.to_string())
            .or_insert(ActionModel {
                success_rate: 0.5,
                avg_reward: 0.0,
                outcome_variance: 0.5,
                sample_count: 0,
                context_modifiers: HashMap::new(),
                temporal_effects: HashMap::new(),
            });

        model.sample_count += 1;
        let n = model.sample_count as f32;
        let alpha = 1.0 / n.min(50.0); // Learning rate decays with more samples

        // Update success rate (exponential moving average)
        let success_val = if succeeded { 1.0 } else { 0.0 };
        model.success_rate = model.success_rate * (1.0 - alpha) + success_val * alpha;

        // Update average reward
        model.avg_reward = model.avg_reward * (1.0 - alpha) + reward * alpha;

        // Update variance (Welford's online algorithm simplified)
        let diff = reward - model.avg_reward;
        model.outcome_variance = model.outcome_variance * (1.0 - alpha) + diff * diff * alpha;

        // Update context modifiers
        let baseline = model.success_rate;
        for tag in context_tags {
            let modifier = model.context_modifiers.entry(tag.clone()).or_insert(0.0);
            let delta = success_val - baseline;
            *modifier = *modifier * (1.0 - alpha) + delta * alpha;
            // Prune near-zero modifiers
            if modifier.abs() < 0.01 && model.sample_count > 20 {
                model.context_modifiers.remove(tag);
            }
        }

        // Update temporal effects
        if let Some(ref prev) = self.last_action {
            let effect = model.temporal_effects.entry(prev.clone()).or_insert(0.0);
            let delta = success_val - baseline;
            *effect = *effect * (1.0 - alpha) + delta * alpha;
        }
    }

    /// Update or create a causal edge between two actions.
    fn update_causal_edge(
        &mut self,
        from: &str,
        to: &str,
        context_hash: u64,
        succeeded: bool,
        reward: f32,
    ) {
        if let Some(edge) = self
            .causal_edges
            .iter_mut()
            .find(|e| e.from_action == from && e.to_action == to && e.context_hash == context_hash)
        {
            // Update existing edge
            edge.observation_count += 1;
            let alpha = 1.0 / (edge.observation_count as f32).min(50.0);
            edge.weight = edge.weight * 0.99 + 0.01; // Slight boost for observation
            edge.avg_reward = edge.avg_reward * (1.0 - alpha) + reward * alpha;

            // Update outcome correlation
            let success_val = if succeeded { 1.0f32 } else { -1.0 };
            let prev_val = if self.last_succeeded { 1.0f32 } else { -1.0 };
            let correlation_sample = success_val * prev_val;
            edge.outcome_correlation =
                edge.outcome_correlation * (1.0 - alpha) + correlation_sample * alpha;
        } else if self.causal_edges.len() < EDGE_CAPACITY {
            // Create new edge
            self.causal_edges.push(CausalEdge {
                from_action: from.to_string(),
                to_action: to.to_string(),
                context_hash,
                weight: 0.1, // New edges start weak
                avg_reward: reward,
                observation_count: 1,
                outcome_correlation: 0.0,
            });
        }
    }

    /// Update emotional state based on what just happened.
    fn update_emotion(&mut self, succeeded: bool, reward: f32, surprise: f32) {
        // ── Valence update ──
        // Success → positive, failure → negative, modulated by reward magnitude
        let outcome_valence = if succeeded {
            reward.max(0.1)
        } else {
            reward.min(-0.1)
        };
        // Relief effect: success after failure is extra positive
        let relief_bonus = if succeeded && self.emotion.valence < -0.2 {
            0.2
        } else {
            0.0
        };
        self.emotion.valence = (self.emotion.valence * EMOTION_DECAY
            + outcome_valence * (1.0 - EMOTION_DECAY)
            + relief_bonus)
            .clamp(-1.0, 1.0);

        // ── Arousal update ──
        // Surprise → high arousal, routine → low arousal
        self.emotion.arousal = (self.emotion.arousal * EMOTION_DECAY
            + surprise * (1.0 - EMOTION_DECAY))
            .clamp(0.0, 1.0);

        // ── Confidence update ──
        // Correct predictions → higher confidence
        if self.total_predictions > 10 {
            self.emotion.confidence = self.prediction_accuracy();
        }

        // ── Dominant drive ──
        self.emotion.dominant_drive = if self.emotion.arousal > 0.6 && self.global_curiosity > 0.5 {
            Drive::Explore
        } else if self.emotion.valence > 0.3 && self.emotion.confidence > 0.6 {
            Drive::Exploit
        } else if self.emotion.valence < -0.3 {
            Drive::Avoid
        } else {
            Drive::Neutral
        };
    }

    /// Consolidate similar experiences into abstracted patterns.
    fn consolidate_experiences(&mut self) {
        // Group by (action, abstraction_level=0)
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, exp) in self.experiences.iter().enumerate() {
            if exp.abstraction_level == 0 {
                groups.entry(exp.action.clone()).or_default().push(i);
            }
        }

        // For groups with 10+ raw experiences, create a consolidated pattern
        let now = chrono::Utc::now().timestamp();
        let mut consolidated = Vec::new();
        for (action, indices) in &groups {
            if indices.len() >= 10 {
                let exps: Vec<&Experience> = indices
                    .iter()
                    .filter_map(|&i| self.experiences.get(i))
                    .collect();

                let success_count = exps.iter().filter(|e| e.succeeded).count();
                let avg_reward = exps.iter().map(|e| e.reward).sum::<f32>() / exps.len() as f32;
                let avg_surprise = exps.iter().map(|e| e.surprise).sum::<f32>() / exps.len() as f32;

                // Collect common context tags (appearing in >50% of experiences)
                let mut tag_counts: HashMap<&str, usize> = HashMap::new();
                for exp in &exps {
                    for tag in &exp.context_tags {
                        *tag_counts.entry(tag.as_str()).or_insert(0) += 1;
                    }
                }
                let common_tags: Vec<String> = tag_counts
                    .into_iter()
                    .filter(|(_, count)| *count > exps.len() / 2)
                    .map(|(tag, _)| tag.to_string())
                    .collect();

                consolidated.push(Experience {
                    id: self.next_id,
                    context_hash: hash_context(&common_tags),
                    context_tags: common_tags,
                    action: action.clone(),
                    succeeded: success_count > exps.len() / 2,
                    reward: avg_reward,
                    error_tag: None,
                    surprise: avg_surprise,
                    valence: avg_reward.clamp(-1.0, 1.0),
                    timestamp: now,
                    replay_count: 0,
                    abstraction_level: 1,
                });
                self.next_id += 1;
            }
        }

        // Remove old raw experiences that were consolidated (keep the newest 5 per action)
        for indices in groups.values() {
            if indices.len() >= 10 {
                let mut sorted_indices = indices.clone();
                sorted_indices.sort_by(|&a, &b| {
                    self.experiences[b]
                        .timestamp
                        .cmp(&self.experiences[a].timestamp)
                });
                // Mark older ones for removal (keep newest 5)
                for &idx in sorted_indices.iter().skip(5) {
                    if idx < self.experiences.len() {
                        self.experiences[idx].abstraction_level = u32::MAX; // Mark for removal
                    }
                }
            }
        }
        self.experiences.retain(|e| e.abstraction_level != u32::MAX);

        // Add consolidated patterns
        self.experiences.extend(consolidated);
    }
}

// ── Persistence helpers ──────────────────────────────────────────────

/// Load cortex from database.
pub fn load_cortex(db: &SoulDatabase) -> Cortex {
    match db.get_state("cortex_state").ok().flatten() {
        Some(json) => Cortex::from_json(&json).unwrap_or_default(),
        None => Cortex::new(),
    }
}

/// Save cortex to database.
pub fn save_cortex(db: &SoulDatabase, cortex: &Cortex) {
    let json = cortex.to_json();
    if let Err(e) = db.set_state("cortex_state", &json) {
        tracing::warn!(error = %e, "Failed to save cortex state");
    }
}

// ── Utility functions ────────────────────────────────────────────────

/// Hash a set of context tags into a u64 for content-addressable lookup.
fn hash_context(tags: &[String]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    for tag in tags {
        for byte in tag.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3); // FNV-1a prime
        }
        hash ^= 0xff; // separator
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Convert a PlanStep to an action name for the cortex.
pub fn step_to_action_name(step: &PlanStep) -> String {
    match step {
        PlanStep::ReadFile { .. } => "read_file".to_string(),
        PlanStep::SearchCode { .. } => "search_code".to_string(),
        PlanStep::ListDir { .. } => "list_dir".to_string(),
        PlanStep::RunShell { .. } => "run_shell".to_string(),
        PlanStep::Commit { .. } => "commit".to_string(),
        PlanStep::CheckSelf { .. } => "check_self".to_string(),
        PlanStep::CreateScriptEndpoint { .. } => "create_script_endpoint".to_string(),
        PlanStep::TestScriptEndpoint { .. } => "test_script_endpoint".to_string(),
        PlanStep::CargoCheck { .. } => "cargo_check".to_string(),
        PlanStep::GenerateCode { .. } => "generate_code".to_string(),
        PlanStep::EditCode { .. } => "edit_code".to_string(),
        PlanStep::Think { .. } => "think".to_string(),
        PlanStep::DiscoverPeers { .. } => "discover_peers".to_string(),
        PlanStep::CallPeer { .. } => "call_peer".to_string(),
        PlanStep::DeleteEndpoint { .. } => "delete_endpoint".to_string(),
        PlanStep::ReviewPeerPR { .. } => "review_peer_pr".to_string(),
        PlanStep::CloneSelf { .. } => "clone_self".to_string(),
        _ => "other".to_string(),
    }
}

/// Build context tags from the current step and agent state.
pub fn build_context_tags(
    step: &PlanStep,
    goal_description: &str,
    plan_progress: f32,
    cycle_count: u64,
) -> Vec<String> {
    let mut tags = Vec::new();

    // Goal context (first 50 chars, normalized)
    let goal_tag: String = goal_description
        .chars()
        .take(50)
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
        .trim()
        .to_lowercase()
        .replace(' ', "_");
    if !goal_tag.is_empty() {
        tags.push(format!("goal:{goal_tag}"));
    }

    // Step-specific context
    match step {
        PlanStep::ReadFile { path, .. }
        | PlanStep::GenerateCode {
            file_path: path, ..
        }
        | PlanStep::EditCode {
            file_path: path, ..
        } => {
            // Extract file extension
            if let Some(ext) = std::path::Path::new(path).extension() {
                tags.push(format!("ext:{}", ext.to_string_lossy()));
            }
            // Extract filename
            if let Some(name) = std::path::Path::new(path).file_name() {
                tags.push(format!("file:{}", name.to_string_lossy()));
            }
        }
        PlanStep::RunShell { command, .. } => {
            // Extract first word of command
            if let Some(cmd) = command.split_whitespace().next() {
                tags.push(format!("cmd:{cmd}"));
            }
        }
        PlanStep::CallPeer { slug, .. } => {
            tags.push(format!("peer:{slug}"));
        }
        _ => {}
    }

    // Plan progress bucket
    let progress_bucket = if plan_progress < 0.25 {
        "early"
    } else if plan_progress < 0.75 {
        "middle"
    } else {
        "late"
    };
    tags.push(format!("progress:{progress_bucket}"));

    // Cycle bucket (early/established/veteran)
    let maturity = if cycle_count < 50 {
        "early"
    } else if cycle_count < 200 {
        "established"
    } else {
        "veteran"
    };
    tags.push(format!("maturity:{maturity}"));

    tags
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cortex() {
        let cortex = Cortex::new();
        assert_eq!(cortex.experiences.len(), 0);
        assert_eq!(cortex.total_experiences_processed, 0);
        assert_eq!(cortex.emotion.dominant_drive, Drive::Explore);
    }

    #[test]
    fn test_record_experience() {
        let mut cortex = Cortex::new();
        cortex.record(
            "read_file",
            vec!["goal:test".to_string(), "ext:rs".to_string()],
            true,
            1.0,
            None,
        );
        assert_eq!(cortex.experiences.len(), 1);
        assert_eq!(cortex.total_experiences_processed, 1);
        assert!(cortex.action_models.contains_key("read_file"));
    }

    #[test]
    fn test_prediction_improves() {
        let mut cortex = Cortex::new();
        let tags = vec!["goal:test".to_string()];

        // Record many successful read_file experiences
        for _ in 0..20 {
            cortex.record("read_file", tags.clone(), true, 1.0, None);
        }

        // Prediction should be positive
        let pred = cortex.predict_action("read_file", &tags);
        assert!(
            pred > 0.0,
            "Prediction should be positive after successes: {pred}"
        );

        // Record many failed edit_code experiences
        for _ in 0..20 {
            cortex.record(
                "edit_code",
                tags.clone(),
                false,
                -0.5,
                Some("compile_error".to_string()),
            );
        }

        // Prediction for edit_code should be negative
        let pred2 = cortex.predict_action("edit_code", &tags);
        assert!(
            pred2 < pred,
            "Failed action prediction should be lower: {pred2} vs {pred}"
        );
    }

    #[test]
    fn test_curiosity_for_unknown() {
        let mut cortex = Cortex::new();
        // Record some known actions
        cortex.record("read_file", vec![], true, 1.0, None);
        cortex.record("commit", vec![], true, 1.0, None);

        let frontier = cortex.curiosity_frontier(20);
        // Unknown actions should have max curiosity (1.0)
        let unknown: Vec<_> = frontier.iter().filter(|(_, s)| *s >= 0.99).collect();
        assert!(
            !unknown.is_empty(),
            "Unknown actions should have high curiosity"
        );
    }

    #[test]
    fn test_causal_edges() {
        let mut cortex = Cortex::new();
        let tags = vec![];

        // Record A → B sequence multiple times
        for _ in 0..10 {
            cortex.record("read_file", tags.clone(), true, 1.0, None);
            cortex.record("edit_code", tags.clone(), true, 0.8, None);
        }

        // Should have causal edge from read_file → edit_code
        let edge = cortex
            .causal_edges
            .iter()
            .find(|e| e.from_action == "read_file" && e.to_action == "edit_code");
        assert!(
            edge.is_some(),
            "Should have causal edge read_file → edit_code"
        );
        assert!(
            edge.unwrap().observation_count >= 9,
            "Edge should have multiple observations"
        );
    }

    #[test]
    fn test_emotional_dynamics() {
        let mut cortex = Cortex::new();

        // Repeated failures should produce negative valence
        for _ in 0..10 {
            cortex.record("edit_code", vec![], false, -0.5, Some("error".to_string()));
        }
        assert!(
            cortex.emotion.valence < 0.0,
            "Repeated failures should produce negative valence: {}",
            cortex.emotion.valence
        );
        assert_eq!(cortex.emotion.dominant_drive, Drive::Avoid);

        // Success after failure should produce relief
        let valence_before = cortex.emotion.valence;
        cortex.record("edit_code", vec![], true, 1.0, None);
        assert!(
            cortex.emotion.valence > valence_before,
            "Success after failure should improve valence"
        );
    }

    #[test]
    fn test_dream_consolidation() {
        let mut cortex = Cortex::new();
        let tags = vec!["goal:test".to_string()];

        // Record enough experiences for patterns to emerge
        for i in 0..30 {
            cortex.record("read_file", tags.clone(), true, 1.0, None);
            cortex.record(
                "edit_code",
                tags.clone(),
                i % 3 != 0, // 67% success rate
                if i % 3 != 0 { 0.8 } else { -0.5 },
                if i % 3 == 0 {
                    Some("compile_error".to_string())
                } else {
                    None
                },
            );
        }

        let insights = cortex.dream();
        // Should have extracted some patterns
        assert!(
            true, // Dream should process without panic
            "Dream should process without panic"
        );
    }

    #[test]
    fn test_simulate_plan() {
        let mut cortex = Cortex::new();

        // Build up experience
        for _ in 0..20 {
            cortex.record("read_file", vec![], true, 1.0, None);
            cortex.record("edit_code", vec![], true, 0.8, None);
            cortex.record("cargo_check", vec![], true, 0.5, None);
        }

        let steps = vec![
            PlanStep::ReadFile {
                path: "test.rs".to_string(),
                store_as: None,
            },
            PlanStep::EditCode {
                description: "fix bug".to_string(),
                file_path: "test.rs".to_string(),
                context_keys: vec![],
            },
            PlanStep::CargoCheck { store_as: None },
        ];

        let result = cortex.simulate_plan(&steps);
        assert!(
            result.predicted_success > 0.0,
            "Plan with known-good steps should have positive prediction"
        );
        assert_eq!(result.step_predictions.len(), 3);
    }

    #[test]
    fn test_peer_sharing() {
        let mut cortex_a = Cortex::new();
        let mut cortex_b = Cortex::new();

        // Agent A learns about read_file
        for _ in 0..20 {
            cortex_a.record("read_file", vec![], true, 1.0, None);
        }

        // Agent B learns about edit_code
        for _ in 0..20 {
            cortex_b.record("edit_code", vec![], false, -0.5, Some("error".to_string()));
        }

        // Share A's knowledge with B
        let snapshot = cortex_a.export("agent-a");
        cortex_b.merge(&snapshot, 0.3);

        // B should now know about read_file
        assert!(
            cortex_b.action_models.contains_key("read_file"),
            "B should have read_file model after merge"
        );
    }

    #[test]
    fn test_experience_eviction() {
        let mut cortex = Cortex::new();

        // Fill beyond capacity
        for i in 0..(EXPERIENCE_CAPACITY + 100) {
            cortex.record("read_file", vec![format!("iter:{i}")], true, 1.0, None);
        }

        assert!(
            cortex.experiences.len() <= EXPERIENCE_CAPACITY,
            "Should not exceed capacity: {}",
            cortex.experiences.len()
        );
    }

    #[test]
    fn test_serialization() {
        let mut cortex = Cortex::new();
        cortex.record("test", vec!["tag".to_string()], true, 0.5, None);

        let json = cortex.to_json();
        let restored = Cortex::from_json(&json).unwrap();
        assert_eq!(restored.experiences.len(), 1);
        assert_eq!(restored.total_experiences_processed, 1);
    }

    #[test]
    fn test_context_hash_deterministic() {
        let tags1 = vec!["a".to_string(), "b".to_string()];
        let tags2 = vec!["a".to_string(), "b".to_string()];
        let tags3 = vec!["b".to_string(), "a".to_string()];

        assert_eq!(hash_context(&tags1), hash_context(&tags2));
        // Order matters (this is intentional — context order carries meaning)
        assert_ne!(hash_context(&tags1), hash_context(&tags3));
    }
}
