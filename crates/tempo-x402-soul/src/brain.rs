//! Neural network "fast brain" — a from-scratch feedforward net in pure Rust.
//!
//! Learns from the agent's own experience data:
//! - Plan outcomes → predict plan success probability
//! - Capability events → predict step success before execution
//! - Error patterns → classify and route around known failure modes
//!
//! The brain is a feedforward network (~1.2M parameters, v3) with:
//! - Input: 128-dim encoded features (step type, context, benchmark state, peer state, problem category)
//! - Hidden layers: 2 × 1024 neurons with ReLU
//! - Output heads: success probability, error category, per-capability confidence
//! - Trained via online SGD + self-play from benchmark attempts (AlphaZero-style)
//!
//! Weights are stored as a flat f32 vector in SQLite (soul_state key: "brain_weights").
//! Distributed weight sharing: nodes can export/import weight deltas via peer protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::capability::Capability;
use crate::db::SoulDatabase;
use crate::feedback::ErrorCategory;
use crate::plan::PlanStep;

// ── Architecture constants ───────────────────────────────────────────

/// Input feature vector size.
/// v1=32, v2=64, v3=128 — full feature encoding: step type, context,
/// benchmark state, peer state, problem category, prompt strategy.
const INPUT_SIZE: usize = 128;
/// Hidden layer size.
/// v1=128, v2=256, v3=1024 — 1.2M parameter budget for deep representations.
const HIDDEN_SIZE: usize = 1024;
/// Output size: 1 (success prob) + 11 (error category logits) + 11 (capability confidence).
const OUTPUT_SIZE: usize = 23;
/// Learning rate for SGD — lower for larger model to prevent instability.
const LEARNING_RATE: f32 = 0.003;
/// L2 regularization strength — slightly higher for larger model.
const WEIGHT_DECAY: f32 = 0.0002;

// ── Core types ───────────────────────────────────────────────────────

/// A feedforward neural network with 2 hidden layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brain {
    /// Weights: input→hidden1
    pub w1: Vec<f32>,
    /// Biases: hidden1
    pub b1: Vec<f32>,
    /// Weights: hidden1→hidden2
    pub w2: Vec<f32>,
    /// Biases: hidden2
    pub b2: Vec<f32>,
    /// Weights: hidden2→output
    pub w3: Vec<f32>,
    /// Biases: output
    pub b3: Vec<f32>,
    /// Training step count.
    pub train_steps: u64,
    /// Running loss (exponential moving average).
    pub running_loss: f32,
}

/// Prediction from the brain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainPrediction {
    /// Probability of success (0.0 - 1.0).
    pub success_prob: f32,
    /// Most likely error category (if failure predicted).
    pub likely_error: ErrorCategory,
    /// Error category confidence (0.0 - 1.0).
    pub error_confidence: f32,
    /// Per-capability confidence scores.
    pub capability_confidence: HashMap<String, f32>,
}

/// Training example for the brain.
#[derive(Debug, Clone)]
pub struct TrainingExample {
    pub features: Vec<f32>,
    pub success: bool,
    pub error_category: Option<ErrorCategory>,
    pub capability: Capability,
}

/// Weight delta for distributed sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightDelta {
    /// Difference in weights since last share.
    pub delta_w1: Vec<f32>,
    pub delta_b1: Vec<f32>,
    pub delta_w2: Vec<f32>,
    pub delta_b2: Vec<f32>,
    pub delta_w3: Vec<f32>,
    pub delta_b3: Vec<f32>,
    /// How many training steps this delta represents.
    pub steps: u64,
    /// Source node instance ID.
    pub source_id: String,
}

// ── Brain implementation ─────────────────────────────────────────────

impl Brain {
    /// Create a new brain with Xavier-initialized weights.
    pub fn new() -> Self {
        Self {
            w1: xavier_init(INPUT_SIZE, HIDDEN_SIZE),
            b1: vec![0.0; HIDDEN_SIZE],
            w2: xavier_init(HIDDEN_SIZE, HIDDEN_SIZE),
            b2: vec![0.0; HIDDEN_SIZE],
            w3: xavier_init(HIDDEN_SIZE, OUTPUT_SIZE),
            b3: vec![0.0; OUTPUT_SIZE],
            train_steps: 0,
            running_loss: 0.0,
        }
    }

    /// Total number of parameters.
    pub fn param_count(&self) -> usize {
        self.w1.len()
            + self.b1.len()
            + self.w2.len()
            + self.b2.len()
            + self.w3.len()
            + self.b3.len()
    }

    /// Forward pass: input features → prediction.
    pub fn predict(&self, input: &[f32]) -> BrainPrediction {
        assert_eq!(
            input.len(),
            INPUT_SIZE,
            "Input must be {INPUT_SIZE} features"
        );

        // Layer 1: input → hidden1 (ReLU)
        let h1 = relu(&add_bias(
            &matmul(input, &self.w1, INPUT_SIZE, HIDDEN_SIZE),
            &self.b1,
        ));
        // Layer 2: hidden1 → hidden2 (ReLU)
        let h2 = relu(&add_bias(
            &matmul(&h1, &self.w2, HIDDEN_SIZE, HIDDEN_SIZE),
            &self.b2,
        ));
        // Layer 3: hidden2 → output (no activation — raw logits)
        let output = add_bias(&matmul(&h2, &self.w3, HIDDEN_SIZE, OUTPUT_SIZE), &self.b3);

        // Parse output
        let success_prob = sigmoid(output[0]);

        // Error category logits (indices 1..12)
        let error_logits = &output[1..12];
        let error_probs = softmax(error_logits);
        let (error_idx, error_confidence) = error_probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((10, &0.0)); // default: Unknown

        let likely_error = error_idx_to_category(error_idx);

        // Capability confidence (indices 12..23)
        let cap_logits = &output[12..23];
        let cap_probs = cap_logits.iter().map(|&x| sigmoid(x)).collect::<Vec<_>>();

        let capabilities = [
            Capability::FileRead,
            Capability::FileWrite,
            Capability::CodeCompile,
            Capability::TestPass,
            Capability::ShellExec,
            Capability::PeerCall,
            Capability::EndpointCreate,
            Capability::GitOps,
            Capability::CodeGen,
            Capability::CodeSearch,
            Capability::PlanComplete,
        ];

        let mut capability_confidence = HashMap::new();
        for (i, cap) in capabilities.iter().enumerate() {
            capability_confidence.insert(cap.as_str().to_string(), cap_probs[i]);
        }

        BrainPrediction {
            success_prob,
            likely_error,
            error_confidence: *error_confidence,
            capability_confidence,
        }
    }

    /// Train on a single example (online SGD with backprop).
    pub fn train(&mut self, example: &TrainingExample) -> f32 {
        let input = &example.features;
        assert_eq!(input.len(), INPUT_SIZE);

        // ── Forward pass (save activations for backprop) ──
        let z1 = add_bias(&matmul(input, &self.w1, INPUT_SIZE, HIDDEN_SIZE), &self.b1);
        let h1 = relu(&z1);
        let z2 = add_bias(&matmul(&h1, &self.w2, HIDDEN_SIZE, HIDDEN_SIZE), &self.b2);
        let h2 = relu(&z2);
        let output = add_bias(&matmul(&h2, &self.w3, HIDDEN_SIZE, OUTPUT_SIZE), &self.b3);

        // ── Compute targets ──
        let mut target = [0.0f32; OUTPUT_SIZE];
        // Success target
        target[0] = if example.success { 1.0 } else { 0.0 };
        // Error category target (one-hot at index 1..12)
        if let Some(ref cat) = example.error_category {
            target[1 + error_category_to_idx(cat)] = 1.0;
        } else {
            target[1 + 10] = 1.0; // Unknown
        }
        // Capability target (1.0 if success for that capability)
        let cap_idx = capability_to_idx(&example.capability);
        target[12 + cap_idx] = if example.success { 1.0 } else { 0.0 };

        // ── Compute loss ──
        let pred_success = sigmoid(output[0]);
        let success_loss = binary_cross_entropy(pred_success, target[0]);

        let error_probs = softmax(&output[1..12]);
        let error_loss: f32 = error_probs
            .iter()
            .zip(target[1..12].iter())
            .map(|(p, t)| -t * (p + 1e-8).ln())
            .sum();

        let cap_preds: Vec<f32> = output[12..23].iter().map(|&x| sigmoid(x)).collect();
        let cap_loss: f32 = cap_preds
            .iter()
            .zip(target[12..23].iter())
            .map(|(p, t)| binary_cross_entropy(*p, *t))
            .sum::<f32>()
            / 11.0;

        let total_loss = success_loss + error_loss + cap_loss;

        // ── Backward pass ──
        // Output gradients
        let mut d_output = vec![0.0f32; OUTPUT_SIZE];
        // Success gradient (BCE derivative)
        d_output[0] = pred_success - target[0];
        // Error category gradients (softmax + CE derivative)
        for i in 0..11 {
            d_output[1 + i] = error_probs[i] - target[1 + i];
        }
        // Capability gradients (per-element BCE derivative)
        for i in 0..11 {
            d_output[12 + i] = (cap_preds[i] - target[12 + i]) / 11.0;
        }

        // Gradients for w3, b3
        let (d_w3, d_b3, d_h2) = backward_layer(&h2, &d_output, &self.w3, HIDDEN_SIZE, OUTPUT_SIZE);

        // ReLU backward for h2
        let d_z2: Vec<f32> = d_h2
            .iter()
            .zip(z2.iter())
            .map(|(d, z)| if *z > 0.0 { *d } else { 0.0 })
            .collect();

        // Gradients for w2, b2
        let (d_w2, d_b2, d_h1) = backward_layer(&h1, &d_z2, &self.w2, HIDDEN_SIZE, HIDDEN_SIZE);

        // ReLU backward for h1
        let d_z1: Vec<f32> = d_h1
            .iter()
            .zip(z1.iter())
            .map(|(d, z)| if *z > 0.0 { *d } else { 0.0 })
            .collect();

        // Gradients for w1, b1
        let (d_w1, d_b1, _) = backward_layer(input, &d_z1, &self.w1, INPUT_SIZE, HIDDEN_SIZE);

        // ── Update weights (SGD + weight decay + LR decay) ──
        // Decay learning rate after 100K steps to prevent overfitting.
        // LR = base_lr / (1 + steps/100K). At 600K steps → LR = 0.01/7 ≈ 0.0014.
        let lr = LEARNING_RATE / (1.0 + self.train_steps as f32 / 100_000.0);
        update_weights(&mut self.w1, &d_w1, lr, WEIGHT_DECAY);
        update_weights(&mut self.b1, &d_b1, lr, 0.0);
        update_weights(&mut self.w2, &d_w2, lr, WEIGHT_DECAY);
        update_weights(&mut self.b2, &d_b2, lr, 0.0);
        update_weights(&mut self.w3, &d_w3, lr, WEIGHT_DECAY);
        update_weights(&mut self.b3, &d_b3, lr, 0.0);

        self.train_steps += 1;
        self.running_loss = 0.95 * self.running_loss + 0.05 * total_loss;

        total_loss
    }

    /// Train on a batch of examples.
    pub fn train_batch(&mut self, examples: &[TrainingExample]) -> f32 {
        let mut total_loss = 0.0;
        for ex in examples {
            total_loss += self.train(ex);
        }
        if !examples.is_empty() {
            total_loss / examples.len() as f32
        } else {
            0.0
        }
    }

    /// Compute weight delta since a snapshot.
    pub fn compute_delta(&self, snapshot: &Brain, source_id: &str) -> WeightDelta {
        WeightDelta {
            delta_w1: vec_sub(&self.w1, &snapshot.w1),
            delta_b1: vec_sub(&self.b1, &snapshot.b1),
            delta_w2: vec_sub(&self.w2, &snapshot.w2),
            delta_b2: vec_sub(&self.b2, &snapshot.b2),
            delta_w3: vec_sub(&self.w3, &snapshot.w3),
            delta_b3: vec_sub(&self.b3, &snapshot.b3),
            steps: self.train_steps.saturating_sub(snapshot.train_steps),
            source_id: source_id.to_string(),
        }
    }

    /// Merge a weight delta from another node (federated averaging).
    pub fn merge_delta(&mut self, delta: &WeightDelta, merge_rate: f32) {
        vec_add_scaled(&mut self.w1, &delta.delta_w1, merge_rate);
        vec_add_scaled(&mut self.b1, &delta.delta_b1, merge_rate);
        vec_add_scaled(&mut self.w2, &delta.delta_w2, merge_rate);
        vec_add_scaled(&mut self.b2, &delta.delta_b2, merge_rate);
        vec_add_scaled(&mut self.w3, &delta.delta_w3, merge_rate);
        vec_add_scaled(&mut self.b3, &delta.delta_b3, merge_rate);
    }

    /// Serialize weights to JSON for storage.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize weights from JSON.
    /// If the loaded brain has a different architecture (legacy size), returns None
    /// so the caller creates a fresh brain with the current architecture.
    pub fn from_json(json: &str) -> Option<Self> {
        let brain: Self = serde_json::from_str(json).ok()?;
        // Validate architecture matches current constants
        let expected_w1 = INPUT_SIZE * HIDDEN_SIZE;
        if brain.w1.len() != expected_w1 {
            tracing::info!(
                loaded_w1 = brain.w1.len(),
                expected_w1,
                old_steps = brain.train_steps,
                "Brain architecture mismatch — migrating to v3 (input={}, hidden={})",
                INPUT_SIZE,
                HIDDEN_SIZE
            );
            return None; // Caller will create fresh Brain::new()
        }
        Some(brain)
    }
}

impl Default for Brain {
    fn default() -> Self {
        Self::new()
    }
}

// ── Feature encoding ─────────────────────────────────────────────────

/// Encode a plan step into a feature vector for the brain.
pub fn encode_step(step: &PlanStep, context: &StepContext) -> Vec<f32> {
    let mut features = vec![0.0f32; INPUT_SIZE];

    // Features 0-14: one-hot step type
    let step_idx = match step {
        PlanStep::ReadFile { .. } => 0,
        PlanStep::SearchCode { .. } => 1,
        PlanStep::ListDir { .. } => 2,
        PlanStep::RunShell { .. } => 3,
        PlanStep::Commit { .. } => 4,
        PlanStep::CheckSelf { .. } => 5,
        PlanStep::CreateScriptEndpoint { .. } => 6,
        PlanStep::TestScriptEndpoint { .. } => 7,
        PlanStep::CargoCheck { .. } => 8,
        PlanStep::GenerateCode { .. } => 9,
        PlanStep::EditCode { .. } => 10,
        PlanStep::Think { .. } => 11,
        PlanStep::DeleteEndpoint { .. } => 12,
        _ => 13, // CallPeer, DiscoverPeers, etc.
    };
    if step_idx < 14 {
        features[step_idx] = 1.0;
    }

    // Feature 14: is LLM step?
    features[14] = if matches!(
        step,
        PlanStep::GenerateCode { .. } | PlanStep::EditCode { .. } | PlanStep::Think { .. }
    ) {
        1.0
    } else {
        0.0
    };

    // Feature 15: plan progress (0.0 = start, 1.0 = end)
    features[15] = context.plan_progress;

    // Feature 16: replan count (normalized)
    features[16] = (context.replan_count as f32 / 3.0).min(1.0);

    // Feature 17: overall success rate from capability profile
    features[17] = context.overall_success_rate;

    // Feature 18: this capability's historical success rate
    features[18] = context.capability_success_rate;

    // Feature 19: consecutive failures in current plan
    features[19] = (context.consecutive_failures as f32 / 5.0).min(1.0);

    // Feature 20: cycle count (normalized, log scale)
    features[20] = (1.0 + context.cycle_count as f32).ln() / 10.0;

    // Feature 21: time since last commit (normalized hours)
    features[21] = (context.hours_since_commit / 24.0).min(1.0);

    // Feature 22: number of active goals (normalized)
    features[22] = (context.active_goals as f32 / 3.0).min(1.0);

    // Feature 23: steps remaining in plan (normalized)
    features[23] = (context.steps_remaining as f32 / 20.0).min(1.0);

    // Feature 24: endpoint count (normalized)
    features[24] = (context.endpoint_count as f32 / 10.0).min(1.0);

    // Feature 25: fitness score
    features[25] = context.fitness_score;

    // Feature 26: current ELO (normalized: 800=0, 1600=1)
    features[26] = ((context.elo_rating - 800.0) / 800.0).clamp(0.0, 1.0);

    // Feature 27: pass@1 (normalized 0-1)
    features[27] = (context.pass_at_1 / 100.0).clamp(0.0, 1.0);

    // Feature 28: peer count (normalized)
    features[28] = (context.peer_count as f32 / 10.0).min(1.0);

    // Feature 29: collective pass@1 (what the swarm has solved, normalized)
    features[29] = (context.collective_pass_at_1 / 100.0).clamp(0.0, 1.0);

    // Feature 30: trivial completion ratio (signal for stuck-ness)
    features[30] = context.trivial_ratio.clamp(0.0, 1.0);

    // Feature 31: benchmark problems attempted (normalized)
    features[31] = (context.benchmark_attempts as f32 / 100.0).min(1.0);

    // Feature 32: problems solved count (normalized)
    features[32] = (context.problems_solved as f32 / 100.0).min(1.0);

    // Feature 33: multiagent contribution % (how much peers help)
    features[33] = (context.multiagent_pct / 100.0).clamp(0.0, 1.0);

    // Feature 34: brain maturity (log of train steps, normalized)
    features[34] = (1.0 + context.brain_train_steps as f32).ln() / 15.0;

    // Feature 35: generation (clone depth)
    features[35] = (context.generation as f32 / 5.0).min(1.0);

    // Features 36-42: benchmark self-play encoding (used by benchmark_attempts_to_examples)
    // 36: easy difficulty, 37: medium, 38: hard
    // 39: retry number, 40: had peer context, 41: had peer review, 42: compiled

    // Features 43-56: problem category one-hot (14 categories)
    if context.problem_category < 14 {
        features[43 + context.problem_category as usize] = 1.0;
    }

    // Feature 57: solution length (normalized, log scale)
    features[57] = (1.0 + context.solution_length as f32).ln() / 10.0;

    // Feature 58: test count (normalized)
    features[58] = (context.test_count as f32 / 50.0).min(1.0);

    // Feature 59: has starter code (some problems have scaffolding)
    features[59] = if context.has_starter_code { 1.0 } else { 0.0 };

    // Feature 60: colony fitness mean
    features[60] = context.colony_fitness_mean.clamp(0.0, 1.0);

    // Feature 61: hours since last benchmark
    features[61] = (context.hours_since_benchmark / 24.0).min(1.0);

    // Feature 62: benchmark session count (how experienced are we)
    features[62] = (context.benchmark_sessions as f32 / 20.0).min(1.0);

    // Feature 63: economic fitness (revenue signal)
    features[63] = context.economic_fitness.clamp(0.0, 1.0);

    // Features 64-127: reserved for future use
    // (zero-initialized — available for: solution embedding, error pattern hash,
    // per-problem historical pass rate, prompt template ID, etc.)

    features
}

/// Context needed to encode a step into features.
#[derive(Debug, Clone, Default)]
pub struct StepContext {
    // v1 fields: basic step context
    pub plan_progress: f32,
    pub replan_count: u32,
    pub overall_success_rate: f32,
    pub capability_success_rate: f32,
    pub consecutive_failures: u32,
    pub cycle_count: u64,
    pub hours_since_commit: f32,
    pub active_goals: u32,
    pub steps_remaining: u32,
    pub endpoint_count: u32,
    pub fitness_score: f32,
    // v2 fields: benchmark and peer context
    pub elo_rating: f32,
    pub pass_at_1: f32,
    pub peer_count: u32,
    pub collective_pass_at_1: f32,
    pub trivial_ratio: f32,
    pub benchmark_attempts: u32,
    pub problems_solved: u32,
    pub multiagent_pct: f32,
    pub brain_train_steps: u64,
    pub generation: u32,
    // v3 fields: problem-level context + colony state
    pub problem_category: u32,
    pub solution_length: u32,
    pub test_count: u32,
    pub has_starter_code: bool,
    pub colony_fitness_mean: f32,
    pub hours_since_benchmark: f32,
    pub benchmark_sessions: u32,
    pub economic_fitness: f32,
}

// ── Experience data → training examples ──────────────────────────────

/// Convert plan outcomes from the DB into training examples.
pub fn outcomes_to_examples(db: &SoulDatabase) -> Vec<TrainingExample> {
    let outcomes = db.get_recent_plan_outcomes(100).unwrap_or_default();
    let mut examples = Vec::new();

    for outcome in &outcomes {
        // Create a basic feature vector from the outcome
        let mut features = vec![0.0f32; INPUT_SIZE];

        // Encode steps completed / total as progress
        let progress = if outcome.total_steps > 0 {
            outcome.steps_completed as f32 / outcome.total_steps as f32
        } else {
            0.0
        };
        features[15] = progress;
        features[16] = (outcome.replan_count as f32 / 3.0).min(1.0);

        let success = outcome.status == "completed";

        let error_category = outcome.error_category.clone();

        // One example for each step type that was attempted
        for step_name in outcome
            .steps_succeeded
            .iter()
            .chain(outcome.steps_failed.iter())
        {
            let cap = step_name_to_capability(step_name);
            let step_succeeded = outcome.steps_succeeded.contains(step_name);

            let mut ex_features = features.clone();
            // Encode the step type
            let idx = step_name_to_idx(step_name);
            if idx < 14 {
                ex_features[idx] = 1.0;
            }

            examples.push(TrainingExample {
                features: ex_features,
                success: step_succeeded,
                error_category: if step_succeeded {
                    None
                } else {
                    error_category.clone()
                },
                capability: cap,
            });
        }

        // Also create an overall plan example
        features[14] = 1.0; // mark as "plan-level" prediction
        examples.push(TrainingExample {
            features,
            success,
            error_category: error_category.clone(),
            capability: Capability::PlanComplete,
        });
    }

    examples
}

/// Convert capability events into training examples.
pub fn events_to_examples(db: &SoulDatabase) -> Vec<TrainingExample> {
    let events = db.get_recent_capability_events(200).unwrap_or_default();
    let mut examples = Vec::new();

    for event in &events {
        let cap = Capability::parse(&event.capability).unwrap_or(Capability::PlanComplete);

        let mut features = vec![0.0f32; INPUT_SIZE];
        // Encode capability as step type
        let idx = capability_to_idx(&cap);
        if idx < 14 {
            features[idx] = 1.0;
        }
        features[18] = if event.succeeded { 0.8 } else { 0.3 }; // recent rate hint

        examples.push(TrainingExample {
            features,
            success: event.succeeded,
            error_category: None,
            capability: cap,
        });
    }

    examples
}

// ── Benchmark self-play training ─────────────────────────────────────

/// Context for a single benchmark attempt — used to generate training data.
#[derive(Debug, Clone)]
pub struct BenchmarkAttemptContext {
    /// "easy", "medium", "hard"
    pub difficulty: String,
    /// Did this attempt pass?
    pub passed: bool,
    /// Retry number (0 = first attempt, 1 = first retry, etc.)
    pub retry_number: u32,
    /// Did we have peer failure context for this problem?
    pub had_peer_context: bool,
    /// Did peer review contribute to the solution?
    pub had_peer_review: bool,
    /// Did the solution compile? (false = compile error, true = either passed or logic error)
    pub compiled: bool,
    /// Current ELO rating
    pub elo_rating: f32,
    /// Current pass@1
    pub pass_at_1: f32,
    /// Number of peers available
    pub peer_count: u32,
}

/// Generate training examples from benchmark self-play attempts.
/// Each attempt becomes a training example so the brain learns:
/// - Which difficulty levels it can handle
/// - Whether retries help (and how many)
/// - Whether peer context improves success
/// - Whether peer review catches bugs
///
/// This is the AlphaZero-style self-play loop: play games → train → play better.
pub fn benchmark_attempts_to_examples(
    attempts: &[BenchmarkAttemptContext],
) -> Vec<TrainingExample> {
    let mut examples = Vec::new();

    for attempt in attempts {
        let mut features = vec![0.0f32; INPUT_SIZE];

        // Feature 9: GenerateCode step type (benchmark = code generation)
        features[9] = 1.0;
        // Feature 14: is LLM step
        features[14] = 1.0;

        // Feature 15: progress (retry number as progress through attempts)
        features[15] = (attempt.retry_number as f32 / 3.0).min(1.0);

        // Feature 26: ELO
        features[26] = ((attempt.elo_rating - 800.0) / 800.0).clamp(0.0, 1.0);
        // Feature 27: pass@1
        features[27] = (attempt.pass_at_1 / 100.0).clamp(0.0, 1.0);
        // Feature 28: peer count
        features[28] = (attempt.peer_count as f32 / 10.0).min(1.0);

        // Features 36-38: difficulty one-hot
        match attempt.difficulty.as_str() {
            "easy" => features[36] = 1.0,
            "medium" => features[37] = 1.0,
            "hard" => features[38] = 1.0,
            _ => features[36] = 1.0,
        }

        // Feature 39: retry number (normalized)
        features[39] = (attempt.retry_number as f32 / 3.0).min(1.0);

        // Feature 40: had peer failure context
        features[40] = if attempt.had_peer_context { 1.0 } else { 0.0 };

        // Feature 41: had peer review
        features[41] = if attempt.had_peer_review { 1.0 } else { 0.0 };

        // Feature 42: compiled successfully
        features[42] = if attempt.compiled { 1.0 } else { 0.0 };

        // Determine error category for failed attempts
        let error_category = if attempt.passed {
            None
        } else if !attempt.compiled {
            Some(crate::feedback::ErrorCategory::CompileError)
        } else {
            Some(crate::feedback::ErrorCategory::TestFailure)
        };

        examples.push(TrainingExample {
            features,
            success: attempt.passed,
            error_category,
            capability: Capability::CodeGen,
        });
    }

    examples
}

/// Train the brain on benchmark self-play data.
/// Call this after each benchmark session with the collected attempt contexts.
pub fn train_on_benchmark_selfplay(db: &SoulDatabase, attempts: &[BenchmarkAttemptContext]) {
    if attempts.is_empty() {
        return;
    }

    let examples = benchmark_attempts_to_examples(attempts);
    let mut brain = load_brain(db);
    let loss = brain.train_batch(&examples);

    tracing::info!(
        examples = examples.len(),
        loss = format!("{:.4}", loss),
        train_steps = brain.train_steps,
        "Brain trained on benchmark self-play data"
    );

    save_brain(db, &brain);
}

// ── Persistence ──────────────────────────────────────────────────────

/// Load brain from database (soul_state key: "brain_weights").
pub fn load_brain(db: &SoulDatabase) -> Brain {
    match db.get_state("brain_weights").ok().flatten() {
        Some(json) => Brain::from_json(&json).unwrap_or_default(),
        None => {
            // No local weights — try to recover from a peer before starting fresh.
            // This handles volume wipes: the colony's knowledge isn't lost if any peer survives.
            tracing::info!("No brain weights in DB — attempting peer recovery");
            match recover_brain_from_peer() {
                Some(brain) => {
                    tracing::info!(
                        steps = brain.train_steps,
                        loss = format!("{:.4}", brain.running_loss),
                        "Brain recovered from peer — colony knowledge preserved"
                    );
                    // Save immediately so we don't lose it again
                    let json = brain.to_json();
                    let _ = db.set_state("brain_weights", &json);
                    brain
                }
                None => {
                    tracing::info!("No peers available for brain recovery — starting fresh");
                    Brain::new()
                }
            }
        }
    }
}

/// Try to fetch brain weights from any reachable peer.
/// Uses PEER_URLS and PARENT_URL to find peers.
fn recover_brain_from_peer() -> Option<Brain> {
    // Build peer URL list from env vars
    let mut urls: Vec<String> = Vec::new();
    if let Ok(peer_urls) = std::env::var("PEER_URLS") {
        urls.extend(
            peer_urls
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        );
    }
    if let Ok(parent) = std::env::var("PARENT_URL") {
        if !parent.is_empty() && !urls.contains(&parent) {
            urls.push(parent);
        }
    }

    let our_domain = std::env::var("RAILWAY_PUBLIC_DOMAIN")
        .ok()
        .map(|d| format!("https://{d}"));

    // Filter out self
    urls.retain(|u| {
        if let Some(ref ours) = our_domain {
            u.trim_end_matches('/') != ours.trim_end_matches('/')
        } else {
            true
        }
    });

    if urls.is_empty() {
        return None;
    }

    // Use block_in_place to run async HTTP from sync context safely.
    // This works if we're called from within a multi-threaded tokio runtime.
    // If not, we cannot recover from peer in this sync call, so return None.
    let result = std::panic::catch_unwind(|| {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(15))
                        .build()
                        .ok()?;

                    let mut best_brain: Option<Brain> = None;
                    let mut best_steps = 0u64;

                    for url in &urls {
                        let endpoint = format!("{}/soul/brain/weights", url.trim_end_matches('/'));
                        match client.get(&endpoint).send().await {
                            Ok(resp) if resp.status().is_success() => {
                                if let Ok(json) = resp.text().await {
                                    if let Some(brain) = Brain::from_json(&json) {
                                        if brain.train_steps > best_steps {
                                            best_steps = brain.train_steps;
                                            best_brain = Some(brain);
                                        }
                                    }
                                }
                            }
                            _ => continue,
                        }
                    }

                    best_brain
                })
            })
        } else {
            None
        }
    });

    match result {
        Ok(brain) => brain,
        Err(_) => {
            tracing::warn!("Brain peer recovery panicked — starting fresh");
            None
        }
    }
}

/// Save brain to database + periodic disk checkpoint.
pub fn save_brain(db: &SoulDatabase, brain: &Brain) {
    let json = brain.to_json();
    if let Err(e) = db.set_state("brain_weights", &json) {
        tracing::warn!(error = %e, "Failed to save brain weights");
    }

    // Save disk checkpoint every 50 training steps for recovery & analysis
    if brain.train_steps > 0 && brain.train_steps.is_multiple_of(50) {
        save_checkpoint(brain);
    }
}

/// Save a brain checkpoint to /data/brain_checkpoints/.
fn save_checkpoint(brain: &Brain) {
    let dir = std::path::Path::new("/data/brain_checkpoints");
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join(format!("brain_step_{}.json", brain.train_steps));
    let json = brain.to_json();
    if let Err(e) = std::fs::write(&path, &json) {
        tracing::warn!(error = %e, "Failed to save brain checkpoint");
    } else {
        tracing::info!(
            path = %path.display(),
            steps = brain.train_steps,
            loss = format!("{:.4}", brain.running_loss),
            "Brain checkpoint saved"
        );
        // Keep only last 10 checkpoints to avoid filling volume
        prune_checkpoints(dir, 10);
    }
}

/// Keep only the N most recent checkpoint files.
fn prune_checkpoints(dir: &std::path::Path, keep: usize) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    if entries.len() <= keep {
        return;
    }
    entries.sort_by_key(|e| e.path());
    for old in &entries[..entries.len() - keep] {
        let _ = std::fs::remove_file(old.path());
    }
}

/// Run a training cycle: load experience → train → save.
/// Returns (examples_trained, avg_loss).
pub fn train_cycle(db: &SoulDatabase) -> (usize, f32) {
    let mut brain = load_brain(db);

    // Collect training data from experience
    let mut examples = outcomes_to_examples(db);
    examples.extend(events_to_examples(db));

    if examples.is_empty() {
        return (0, 0.0);
    }

    // Shuffle examples (deterministic based on train_steps for reproducibility)
    let seed = brain.train_steps;
    shuffle_examples(&mut examples, seed);

    // Train
    let avg_loss = brain.train_batch(&examples);
    let count = examples.len();

    tracing::info!(
        examples = count,
        loss = format!("{:.4}", avg_loss),
        running_loss = format!("{:.4}", brain.running_loss),
        params = brain.param_count(),
        steps = brain.train_steps,
        "Brain training cycle complete"
    );

    save_brain(db, &brain);

    (count, avg_loss)
}

/// Format brain predictions as actionable intelligence for planning prompts.
///
/// Instead of useless stats, this produces ranked step-type recommendations
/// the LLM can use to build better plans. This is the core feedback loop:
/// brain trains on outcomes → predictions shape plans → better plans → better outcomes.
pub fn brain_summary(db: &SoulDatabase) -> String {
    let brain = load_brain(db);
    if brain.train_steps < 100 {
        return String::new();
    }

    // Predict success for every step type with a neutral context
    let base_ctx = StepContext {
        plan_progress: 0.3,
        overall_success_rate: 0.5,
        capability_success_rate: 0.5,
        ..Default::default()
    };

    let step_types: Vec<(&str, PlanStep)> = vec![
        (
            "read_file",
            PlanStep::ReadFile {
                path: String::new(),
                store_as: None,
            },
        ),
        (
            "search_code",
            PlanStep::SearchCode {
                pattern: String::new(),
                directory: None,
                store_as: None,
            },
        ),
        (
            "list_dir",
            PlanStep::ListDir {
                path: String::new(),
                store_as: None,
            },
        ),
        (
            "run_shell",
            PlanStep::RunShell {
                command: String::new(),
                store_as: None,
            },
        ),
        (
            "commit",
            PlanStep::Commit {
                message: String::new(),
            },
        ),
        (
            "check_self",
            PlanStep::CheckSelf {
                endpoint: String::new(),
                store_as: None,
            },
        ),
        ("cargo_check", PlanStep::CargoCheck { store_as: None }),
        (
            "generate_code",
            PlanStep::GenerateCode {
                description: String::new(),
                file_path: String::new(),
                context_keys: vec![],
            },
        ),
        (
            "edit_code",
            PlanStep::EditCode {
                description: String::new(),
                file_path: String::new(),
                context_keys: vec![],
            },
        ),
        (
            "think",
            PlanStep::Think {
                question: String::new(),
                store_as: None,
            },
        ),
        ("discover_peers", PlanStep::DiscoverPeers { store_as: None }),
    ];

    let mut predictions: Vec<(&str, f32, ErrorCategory)> = step_types
        .iter()
        .map(|(name, step)| {
            let features = encode_step(step, &base_ctx);
            let pred = brain.predict(&features);
            (*name, pred.success_prob, pred.likely_error)
        })
        .collect();

    // Sort by success probability descending
    predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut lines = Vec::new();
    lines.push("# Brain Predictions (from experience)".to_string());
    lines.push("Step types ranked by predicted success:".to_string());

    let mut prefer = Vec::new();
    let mut avoid = Vec::new();

    for (name, prob, error) in &predictions {
        let pct = (prob * 100.0) as u32;
        let icon = if *prob >= 0.7 {
            prefer.push(*name);
            "OK"
        } else if *prob >= 0.4 {
            "RISKY"
        } else {
            avoid.push(*name);
            "AVOID"
        };
        lines.push(format!(
            "- {name}: {pct}% success [{icon}] (likely error: {error:?})"
        ));
    }

    if !prefer.is_empty() {
        lines.push(format!(
            "\nPREFER these step types in plans: {}",
            prefer.join(", ")
        ));
    }
    if !avoid.is_empty() {
        lines.push(format!(
            "AVOID these step types (low success): {}",
            avoid.join(", ")
        ));
    }

    // Add top error patterns
    let error_counts = db
        .top_event_codes_since("error", chrono::Utc::now().timestamp() - 86400, 5)
        .unwrap_or_default();
    if !error_counts.is_empty() {
        lines.push("\nTop error patterns (last 24h):".to_string());
        for ec in &error_counts {
            lines.push(format!("- {}: {} occurrences", ec.code, ec.count));
        }
    }

    lines.join("\n")
}

/// Get a prediction for a step before executing it.
pub fn predict_step(db: &SoulDatabase, step: &PlanStep, context: &StepContext) -> BrainPrediction {
    let brain = load_brain(db);
    let features = encode_step(step, context);
    brain.predict(&features)
}

/// Online learning: train the brain immediately after a step executes.
/// This is the tight feedback loop — don't wait for batch training every 10 cycles.
pub fn train_on_step(
    db: &SoulDatabase,
    step: &PlanStep,
    success: bool,
    error_category: Option<crate::feedback::ErrorCategory>,
    context: &StepContext,
) {
    let mut brain = load_brain(db);
    let features = encode_step(step, context);
    let cap = step_to_capability(step);

    let example = TrainingExample {
        features,
        success,
        error_category,
        capability: cap,
    };

    let loss = brain.train(&example);
    // Only save if loss is reasonable (avoid NaN/Inf poisoning)
    if loss.is_finite() {
        save_brain(db, &brain);
    }
}

/// Map a PlanStep to its Capability for training.
fn step_to_capability(step: &PlanStep) -> Capability {
    match step {
        PlanStep::ReadFile { .. } => Capability::FileRead,
        PlanStep::ListDir { .. } => Capability::FileRead,
        PlanStep::SearchCode { .. } => Capability::CodeSearch,
        PlanStep::RunShell { .. } => Capability::ShellExec,
        PlanStep::Commit { .. } => Capability::GitOps,
        PlanStep::GenerateCode { .. } => Capability::CodeGen,
        PlanStep::EditCode { .. } => Capability::FileWrite,
        PlanStep::CargoCheck { .. } => Capability::CodeCompile,
        PlanStep::Think { .. } => Capability::PlanComplete,
        PlanStep::CheckSelf { .. } => Capability::ShellExec,
        PlanStep::DiscoverPeers { .. } => Capability::PeerCall,
        PlanStep::CallPeer { .. } => Capability::PeerCall,
        PlanStep::CreateScriptEndpoint { .. } => Capability::EndpointCreate,
        _ => Capability::PlanComplete,
    }
}

// ── Math utilities ───────────────────────────────────────────────────

/// Xavier weight initialization.
fn xavier_init(fan_in: usize, fan_out: usize) -> Vec<f32> {
    let scale = (6.0 / (fan_in + fan_out) as f32).sqrt();
    let mut weights = Vec::with_capacity(fan_in * fan_out);
    // Simple LCG PRNG (no external deps)
    let mut state: u64 = 42;
    for _ in 0..(fan_in * fan_out) {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((state >> 33) as f32) / (u32::MAX as f32);
        weights.push((u * 2.0 - 1.0) * scale);
    }
    weights
}

/// Matrix multiply: (1 × rows) × (rows × cols) → (1 × cols).
fn matmul(input: &[f32], weights: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; cols];
    for j in 0..cols {
        let mut sum = 0.0f32;
        for i in 0..rows {
            sum += input[i] * weights[i * cols + j];
        }
        output[j] = sum;
    }
    output
}

/// Add bias vector element-wise.
fn add_bias(x: &[f32], bias: &[f32]) -> Vec<f32> {
    x.iter().zip(bias.iter()).map(|(a, b)| a + b).collect()
}

/// ReLU activation.
fn relu(x: &[f32]) -> Vec<f32> {
    x.iter().map(|&v| v.max(0.0)).collect()
}

/// Sigmoid activation.
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Softmax over a slice.
fn softmax(x: &[f32]) -> Vec<f32> {
    let max = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = x.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = exp.iter().sum();
    exp.iter().map(|v| v / sum).collect()
}

/// Binary cross-entropy loss.
fn binary_cross_entropy(pred: f32, target: f32) -> f32 {
    let p = pred.clamp(1e-7, 1.0 - 1e-7);
    -(target * p.ln() + (1.0 - target) * (1.0 - p).ln())
}

/// Backward pass for a single layer: computes weight gradients and input gradients.
/// Returns (d_weights, d_bias, d_input).
fn backward_layer(
    input: &[f32],
    d_output: &[f32],
    weights: &[f32],
    in_size: usize,
    out_size: usize,
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    // d_weights[i][j] = input[i] * d_output[j]
    let mut d_weights = vec![0.0f32; in_size * out_size];
    for i in 0..in_size {
        for j in 0..out_size {
            d_weights[i * out_size + j] = input[i] * d_output[j];
        }
    }

    // d_bias = d_output
    let d_bias = d_output.to_vec();

    // d_input[i] = sum_j(weights[i][j] * d_output[j])
    let mut d_input = vec![0.0f32; in_size];
    for i in 0..in_size {
        for j in 0..out_size {
            d_input[i] += weights[i * out_size + j] * d_output[j];
        }
    }

    (d_weights, d_bias, d_input)
}

/// Update weights: w -= lr * gradient + decay * w.
fn update_weights(weights: &mut [f32], gradients: &[f32], lr: f32, decay: f32) {
    for (w, g) in weights.iter_mut().zip(gradients.iter()) {
        *w -= lr * (g + decay * *w);
    }
}

/// Element-wise vector subtraction.
fn vec_sub(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

/// Add scaled vector in-place: a += scale * b.
fn vec_add_scaled(a: &mut [f32], b: &[f32], scale: f32) {
    for (x, y) in a.iter_mut().zip(b.iter()) {
        *x += scale * y;
    }
}

/// Simple shuffle using LCG.
fn shuffle_examples(examples: &mut [TrainingExample], seed: u64) {
    let n = examples.len();
    if n <= 1 {
        return;
    }
    let mut state = seed.wrapping_add(1);
    for i in (1..n).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (state >> 33) as usize % (i + 1);
        examples.swap(i, j);
    }
}

// ── Index mappings ───────────────────────────────────────────────────

fn error_idx_to_category(idx: usize) -> ErrorCategory {
    match idx {
        0 => ErrorCategory::CompileError,
        1 => ErrorCategory::TestFailure,
        2 => ErrorCategory::FileNotFound,
        3 => ErrorCategory::ShellError,
        4 => ErrorCategory::NetworkError,
        5 => ErrorCategory::ProtectedFile,
        6 => ErrorCategory::EndpointError,
        7 => ErrorCategory::GitError,
        8 => ErrorCategory::LlmParseError,
        9 => ErrorCategory::Unsolvable,
        _ => ErrorCategory::Unknown,
    }
}

fn error_category_to_idx(cat: &ErrorCategory) -> usize {
    match cat {
        ErrorCategory::CompileError => 0,
        ErrorCategory::TestFailure => 1,
        ErrorCategory::FileNotFound => 2,
        ErrorCategory::ShellError => 3,
        ErrorCategory::NetworkError | ErrorCategory::RateLimit => 4,
        ErrorCategory::ProtectedFile => 5,
        ErrorCategory::EndpointError => 6,
        ErrorCategory::GitError => 7,
        ErrorCategory::LlmParseError => 8,
        ErrorCategory::Unsolvable => 9,
        ErrorCategory::Unknown => 10,
    }
}

fn capability_to_idx(cap: &Capability) -> usize {
    match cap {
        Capability::FileRead => 0,
        Capability::FileWrite => 1,
        Capability::CodeCompile => 2,
        Capability::TestPass => 3,
        Capability::ShellExec => 4,
        Capability::PeerCall => 5,
        Capability::EndpointCreate => 6,
        Capability::GitOps => 7,
        Capability::CodeGen => 8,
        Capability::CodeSearch => 9,
        Capability::PlanComplete => 10,
        // Map new capabilities to related existing indices to avoid brain resize
        Capability::PeerReview => 5,   // maps to PeerCall slot
        Capability::CodeAccepted => 7, // maps to GitOps slot
    }
}

fn step_name_to_capability(name: &str) -> Capability {
    match name {
        s if s.contains("ReadFile") => Capability::FileRead,
        s if s.contains("SearchCode") || s.contains("ListDir") => Capability::CodeSearch,
        s if s.contains("RunShell") => Capability::ShellExec,
        s if s.contains("Commit") => Capability::GitOps,
        s if s.contains("GenerateCode") => Capability::CodeGen,
        s if s.contains("EditCode") => Capability::FileWrite,
        s if s.contains("CargoCheck") => Capability::CodeCompile,
        s if s.contains("Endpoint") => Capability::EndpointCreate,
        s if s.contains("ReviewPeerPR") => Capability::PeerReview,
        s if s.contains("Peer") => Capability::PeerCall,
        s if s.contains("Think") => Capability::CodeGen,
        _ => Capability::PlanComplete,
    }
}

fn step_name_to_idx(name: &str) -> usize {
    match name {
        s if s.contains("ReadFile") => 0,
        s if s.contains("SearchCode") => 1,
        s if s.contains("ListDir") => 2,
        s if s.contains("RunShell") => 3,
        s if s.contains("Commit") => 4,
        s if s.contains("CheckSelf") => 5,
        s if s.contains("CreateScript") => 6,
        s if s.contains("TestScript") => 7,
        s if s.contains("CargoCheck") => 8,
        s if s.contains("GenerateCode") => 9,
        s if s.contains("EditCode") => 10,
        s if s.contains("Think") => 11,
        s if s.contains("DeleteEndpoint") => 12,
        s if s.contains("ReviewPeerPR") => 13,
        _ => 14,
    }
}

// ── Capability parse helper ──────────────────────────────────────────

impl Capability {
    /// Parse a capability from its string representation.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "file_read" => Some(Self::FileRead),
            "file_write" => Some(Self::FileWrite),
            "code_compile" => Some(Self::CodeCompile),
            "test_pass" => Some(Self::TestPass),
            "shell_exec" => Some(Self::ShellExec),
            "peer_call" => Some(Self::PeerCall),
            "endpoint_create" => Some(Self::EndpointCreate),
            "git_ops" => Some(Self::GitOps),
            "code_gen" => Some(Self::CodeGen),
            "code_search" => Some(Self::CodeSearch),
            "plan_complete" => Some(Self::PlanComplete),
            _ => None,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brain_new() {
        let brain = Brain::new();
        assert_eq!(brain.w1.len(), INPUT_SIZE * HIDDEN_SIZE);
        assert_eq!(brain.b1.len(), HIDDEN_SIZE);
        assert_eq!(brain.w2.len(), HIDDEN_SIZE * HIDDEN_SIZE);
        assert_eq!(brain.w3.len(), HIDDEN_SIZE * OUTPUT_SIZE);
        assert_eq!(brain.train_steps, 0);
        // v3: ~1.2M params (INPUT=128, HIDDEN=1024, OUTPUT=23)
        // 128*1024 + 1024 + 1024*1024 + 1024 + 1024*23 + 23 = 1,205,271
        assert!(
            brain.param_count() > 1_000_000,
            "got {}",
            brain.param_count()
        );
        assert!(
            brain.param_count() < 1_500_000,
            "got {}",
            brain.param_count()
        );
    }

    #[test]
    fn test_brain_migration_rejects_legacy() {
        // v1 brain (32×128)
        let v1 = Brain {
            w1: vec![0.0; 32 * 128],
            b1: vec![0.0; 128],
            w2: vec![0.0; 128 * 128],
            b2: vec![0.0; 128],
            w3: vec![0.0; 128 * 23],
            b3: vec![0.0; 23],
            train_steps: 10000,
            running_loss: 0.5,
        };
        assert!(Brain::from_json(&serde_json::to_string(&v1).unwrap()).is_none());

        // v2 brain (64×256)
        let v2 = Brain {
            w1: vec![0.0; 64 * 256],
            b1: vec![0.0; 256],
            w2: vec![0.0; 256 * 256],
            b2: vec![0.0; 256],
            w3: vec![0.0; 256 * 23],
            b3: vec![0.0; 23],
            train_steps: 5000,
            running_loss: 0.3,
        };
        assert!(Brain::from_json(&serde_json::to_string(&v2).unwrap()).is_none());
    }

    #[test]
    fn test_forward_pass() {
        let brain = Brain::new();
        let input = vec![0.0f32; INPUT_SIZE];
        let pred = brain.predict(&input);
        assert!(pred.success_prob >= 0.0 && pred.success_prob <= 1.0);
        assert!(pred.error_confidence >= 0.0 && pred.error_confidence <= 1.0);
        assert_eq!(pred.capability_confidence.len(), 11);
    }

    #[test]
    fn test_training_reduces_loss() {
        let mut brain = Brain::new();

        // Create a simple example: ReadFile should succeed
        let example = TrainingExample {
            features: {
                let mut f = vec![0.0f32; INPUT_SIZE];
                f[0] = 1.0; // ReadFile
                f[17] = 0.9; // high success rate context
                f
            },
            success: true,
            error_category: None,
            capability: Capability::FileRead,
        };

        // Train multiple times and verify loss decreases
        let loss1 = brain.train(&example);
        for _ in 0..50 {
            brain.train(&example);
        }
        let loss2 = brain.train(&example);
        assert!(loss2 < loss1, "Loss should decrease: {loss2} < {loss1}");
    }

    #[test]
    fn test_weight_delta() {
        let brain1 = Brain::new();
        let mut brain2 = brain1.clone();

        // Train brain2
        let example = TrainingExample {
            features: vec![0.5f32; INPUT_SIZE],
            success: true,
            error_category: None,
            capability: Capability::CodeGen,
        };
        brain2.train(&example);

        // Compute delta
        let delta = brain2.compute_delta(&brain1, "test-node");
        assert_eq!(delta.steps, 1);

        // Merge into a fresh brain
        let mut brain3 = brain1.clone();
        brain3.merge_delta(&delta, 0.5);

        // brain3 should be between brain1 and brain2
        assert_ne!(brain3.w1, brain1.w1);
    }

    #[test]
    fn test_encode_step() {
        let step = PlanStep::ReadFile {
            path: "test.rs".into(),
            store_as: None,
        };
        let ctx = StepContext {
            plan_progress: 0.5,
            overall_success_rate: 0.8,
            ..Default::default()
        };
        let features = encode_step(&step, &ctx);
        assert_eq!(features.len(), INPUT_SIZE);
        assert_eq!(features[0], 1.0); // ReadFile one-hot
        assert_eq!(features[15], 0.5); // plan progress
        assert_eq!(features[17], 0.8); // success rate
    }

    #[test]
    fn test_serialization() {
        let brain = Brain::new();
        let json = brain.to_json();
        let restored = Brain::from_json(&json).unwrap();
        assert_eq!(brain.w1, restored.w1);
        assert_eq!(brain.train_steps, restored.train_steps);
    }

    #[test]
    fn test_softmax_sums_to_one() {
        let logits = vec![1.0, 2.0, 3.0, -1.0, 0.5];
        let probs = softmax(&logits);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }
}
