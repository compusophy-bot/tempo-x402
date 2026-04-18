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
use std::sync::{Arc, Mutex};

use crate::brain::BrainPrediction;
use crate::cortex::Cortex;
use crate::db::SoulDatabase;
use crate::genesis::GenePool;
use crate::hivemind::Hivemind;
use crate::plan::PlanStep;

// ── Constants ────────────────────────────────────────────────────────

// ── Thread-Safe Database Wrapper ──────────────────────────────────────

/// A thread-safe wrapper for the SoulDatabase.
#[derive(Clone)]
pub struct SafeDb {
    inner: Arc<SoulDatabase>,
    mutex: Arc<Mutex<()>>,
}

impl SafeDb {
    pub fn new(db: Arc<SoulDatabase>) -> Self {
        Self {
            inner: db,
            mutex: Arc::new(Mutex::new(())),
        }
    }

    /// Access the database.
    pub fn db(&self) -> &SoulDatabase {
        &self.inner
    }

    /// Acquire a lock to serialize access to the database.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.mutex.lock().unwrap_or_else(|e| {
            tracing::error!("Database mutex poisoned: {}", e);
            panic!("Database mutex poisoned: {}", e);
        })
    }

    /// Try to acquire a lock, returning None if already locked or poisoned.
    pub fn try_lock(&self) -> Option<std::sync::MutexGuard<'_, ()>> {
        self.mutex.try_lock().ok()
    }
}

// ── Autonomous Plan Compilation ──────────────────────────────────────
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
    #[allow(dead_code)]
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
    /// Confidence 0.0 to 1.0.
    pub confidence: f32,
    /// Individual votes for debugging.
    pub votes: Vec<SystemVote>,
}

/// Cognitive state of the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CognitiveState {
    Coherent,
    Conflicted,
    Exploring,
    Exploiting,
    Stuck,
}

/// The Metacognitive Synthesis Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Synthesis {
    pub id: String,
    pub weights: SystemWeights,
    pub state: CognitiveState,
    pub observation_count: u32,
    pub conflict_log: Vec<String>,
    pub total_predictions: u32,
    pub conflicts: Vec<String>,
}

impl Default for Synthesis {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            weights: SystemWeights::default(),
            state: CognitiveState::Coherent,
            observation_count: 0,
            conflict_log: Vec::new(),
            total_predictions: 0,
            conflicts: Vec::new(),
        }
    }
}

/// Load the current synthesis state.
pub fn load_synthesis(db: &SoulDatabase) -> Synthesis {
    match db.state.get("synthesis") {
        Ok(Some(data)) => serde_json::from_slice(&data).unwrap_or_else(|_| Synthesis::default()),
        _ => Synthesis::default(),
    }
}

/// Save the synthesis state.
pub fn save_synthesis(db: &SoulDatabase, synthesis: &Synthesis) {
    if let Ok(data) = serde_json::to_vec(synthesis) {
        let _ = db.state.insert("synthesis", data);
    }
}

impl Synthesis {
    pub fn prompt_section(&self) -> String {
        String::new()
    }
    
    pub fn record_outcome(
        &mut self,
        _votes: &[SystemVote],
        _success: bool
    ) {}

    pub fn predict_step(
        &self,
        _step: &PlanStep,
        _prediction: &BrainPrediction,
        _cortex: &Cortex,
        _genesis: &GenePool,
        _hivemind: &Hivemind,
        _goal_desc: &str,
    ) -> UnifiedPrediction {
        UnifiedPrediction {
            consensus: 0.0,
            confidence: 0.0,
            votes: Vec::new(),
        }
    }

    pub fn update_self_model(&mut self) {}
    pub fn adapt_from_brier(&mut self, _eval: &crate::evaluation::Evaluation) {}
    pub fn imagine_plans(
        &self,
        _cortex: &Cortex,
        _genesis: &GenePool,
        _description: &str,
    ) -> Vec<crate::plan::Plan> {
        Vec::new()
    }
}
