//! # tempo-x402-soul
//!
//! Autonomous **agentic soul** for x402 nodes.
//!
//! Runs a plan-driven execution loop powered by Gemini:
//! observe &rarr; create goals &rarr; plan steps &rarr; execute &rarr; reflect &rarr; repeat.
//!
//! ## Architecture
//!
//! ```text
//! Every N seconds:
//!   observe → read nudges → check stagnation → get/create plan
//!   → execute next step → advance plan → housekeeping → sleep
//! ```
//!
//! Steps that **don't** call LLM: `ReadFile`, `SearchCode`, `ListDir`, `RunShell`,
//! `Commit`, `CheckSelf`, `CreateScriptEndpoint`, `CargoCheck`.
//!
//! Steps that **do** call LLM: `GenerateCode`, `EditCode`, `Think`.
//!
//! ## Capabilities
//!
//! - **Plan-driven execution** &mdash; goals decompose into deterministic step sequences
//! - **Feedback loop** &mdash; structured plan outcomes, error classification, lessons fed back into prompts
//! - **Capability tracking** &mdash; per-skill success rates, profile-guided planning
//! - **Neuroplastic memory** &mdash; salience scoring, tiered decay
//! - **World model** &mdash; structured beliefs about self, endpoints, codebase, strategy
//! - **Coding agent** &mdash; read, write, edit files, run shell, git commit/push/PR
//! - **Script endpoints** &mdash; create instant bash-based HTTP endpoints
//! - **Peer coordination** &mdash; discover siblings, call paid services, exchange catalogs
//! - **Fitness evolution** &mdash; 5-component fitness score with trend gradient
//! - **Interactive chat** &mdash; session-based conversation with plan context injection
//!
//! Without a `GEMINI_API_KEY`, the soul runs in **dormant mode**: it still observes
//! and records snapshots, but skips all LLM calls.
//!
//! ## Key modules
//!
//! - [`thinking`] &mdash; main plan-driven loop
//! - [`plan`] &mdash; plan types, step execution, plan status
//! - [`prompts`] &mdash; focused prompt builders (goal creation, planning, code generation, replan, reflection)
//! - [`feedback`] &mdash; structured plan outcomes, error classification, lesson extraction
//! - [`capability`] &mdash; per-skill success rate tracking and profile-guided planning
//! - [`tools`] &mdash; tool executor: shell, file ops, git, endpoints, peers
//! - [`git`] &mdash; branch-per-VM git workflow with fork support
//! - [`coding`] &mdash; pre-commit validation pipeline (cargo check &rarr; test &rarr; commit)
//! - [`db`] &mdash; SQLite: thoughts, goals, plans, nudges, beliefs, chat sessions
//! - [`fitness`] &mdash; 5-component fitness scoring with trend gradient
//! - [`chat`] &mdash; session-based interactive chat with plan context
//! - [`neuroplastic`] &mdash; salience scoring, memory decay
//!
//! Part of the [`tempo-x402`](https://docs.rs/tempo-x402) workspace.

pub mod acceleration;
pub mod autonomy;
pub mod benchmark;
pub mod bloch;
pub mod brain;
pub mod cognitive_cartridge;
pub mod capability;
pub mod chat;
pub mod code_quality;
pub mod codegen;
pub mod coding;
pub mod collective;
pub mod colony;
pub mod computer_use;
pub mod config;
pub mod cortex;
pub mod db;
pub mod elo;
pub mod error;
pub mod evaluation;
pub mod events;
pub mod feedback;
pub mod fitness;
pub mod free_energy;
pub mod genesis;
pub mod git;
pub mod guard;
pub mod hivemind;
pub mod housekeeping;
pub mod lifecycle;
pub mod llm;
pub mod memory;
pub mod mode;
pub mod model;
pub mod moe;
pub mod neuroplastic;
pub mod normalize;
pub mod observer;
pub mod opus_bench;
pub mod persistent_memory;
pub mod plan;
pub mod prompts;
pub mod synthesis;
pub mod temporal;
pub mod thinking;
pub mod toon;
pub mod tool_decl;
pub mod tool_registry;
pub mod tools;
pub mod validation;
pub mod world_model;

pub use chat::{handle_chat, ChatReply};
pub use config::SoulConfig;
pub use db::{ChatMessage, ChatSession, Nudge, SoulDatabase};
pub use error::SoulError;
pub use events::{compute_health, emit_event, EventFilter, EventRefs, HealthSummary, SoulEvent};
pub use memory::{Thought, ThoughtType};
pub use observer::{NodeObserver, NodeSnapshot};
pub use plan::{Plan, PlanStatus, PlanStep};
pub use thinking::ThinkingLoop;
pub use tools::ToolExecutor;
pub use world_model::{Goal, GoalStatus};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

/// The Soul: owns the database and thinking loop, spawns as a background task.
pub struct Soul {
    db: Arc<SoulDatabase>,
    config: SoulConfig,
    /// Cartridge engine for cognitive cartridge execution (Phase 4).
    cartridge_engine: Option<std::sync::Arc<x402_cartridge::CartridgeEngine>>,
}

impl Soul {
    /// Create a new Soul from config. Opens the soul database.
    pub fn new(config: SoulConfig) -> Result<Self, SoulError> {
        let db = Arc::new(SoulDatabase::new(&config.db_path)?);
        Ok(Self {
            db,
            config,
            cartridge_engine: None,
        })
    }

    /// Set the cartridge engine for cognitive cartridges (Phase 4).
    pub fn with_cartridge_engine(
        mut self,
        engine: std::sync::Arc<x402_cartridge::CartridgeEngine>,
    ) -> Self {
        self.cartridge_engine = Some(engine);
        self
    }

    /// Spawn the thinking loop as a background tokio task.
    /// The `alive` flag is set to `true` while the soul is running, `false` during restart.
    /// The loop automatically restarts after panics (with a 30s cooldown).
    pub fn spawn(self, observer: Arc<dyn NodeObserver>, alive: Arc<AtomicBool>) -> JoinHandle<()> {
        let alive_for_task = alive;
        let config = self.config;
        let db = self.db;
        let cartridge_engine = self.cartridge_engine;

        let handle = tokio::spawn(async move {
            let mut consecutive_panics: u32 = 0;
            loop {
                alive_for_task.store(true, Ordering::Relaxed);
                let alive_for_loop = alive_for_task.clone();
                let mut loop_instance = ThinkingLoop::new(config.clone(), db.clone(), observer.clone());
                if let Some(ref engine) = cartridge_engine {
                    loop_instance.set_cartridge_engine(engine.clone());
                }

                // Spawn the thinking loop in an inner task so panics become JoinErrors
                let inner = tokio::spawn(async move {
                    loop_instance.run(alive_for_loop).await;
                });

                match inner.await {
                    Ok(()) => break, // clean exit (shouldn't happen — run() loops forever)
                    Err(e) if e.is_panic() => {
                        alive_for_task.store(false, Ordering::Relaxed);
                        consecutive_panics += 1;
                        tracing::error!(
                            "Soul thinking loop panicked ({consecutive_panics}/5) — restarting in 30s: {:?}",
                            e
                        );

                        // Crash-loop breaker: if we panic 5 times in a row, fail the
                        // active plan so we don't loop forever on poisoned state.
                        if consecutive_panics >= 5 {
                            tracing::error!(
                                "Crash loop detected ({consecutive_panics} consecutive panics) — failing active plan"
                            );
                            if let Ok(Some(mut plan)) = db.get_active_plan() {
                                plan.status = crate::plan::PlanStatus::Failed;
                                let _ = db.update_plan(&plan);
                                tracing::warn!("Force-failed stuck plan {}", plan.id);
                            }
                            consecutive_panics = 0;
                        }

                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        // loop continues → fresh ThinkingLoop created
                    }
                    Err(e) => {
                        alive_for_task.store(false, Ordering::Relaxed);
                        tracing::error!("Soul thinking loop failed: {:?}", e);
                        break;
                    }
                }
            }
        });

        handle
    }

    /// Get a reference to the soul database (for external queries).
    pub fn database(&self) -> &Arc<SoulDatabase> {
        &self.db
    }
}
