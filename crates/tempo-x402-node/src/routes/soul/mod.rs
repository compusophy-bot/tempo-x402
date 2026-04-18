//! Soul endpoints — status, interactive chat with sessions, plan approval.

mod admin;
mod benchmark;
mod brain;
mod chat;
mod cognition;
mod colony_routes;
mod diagnostics;
mod lifecycle;
mod nudges;
mod plans;
mod status;

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::state::NodeState;

#[derive(Serialize, Deserialize, Clone)]
pub struct CycleHealth {
    pub last_cycle_entered_code: bool,
    pub total_code_entries: u64,
    pub cycles_since_last_commit: u64,
    pub completed_plans_count: u64,
    pub failed_plans_count: u64,
    pub goals_active: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlanInfo {
    pub id: String,
    pub status: String,
    pub created_at: i64,
    pub goal_id: Option<String>,
    pub current_step: String,
    pub total_steps: usize,
    pub replan_count: u32,
    pub current_step_type: String,
    pub goal_description: Option<String>,
    pub steps: Option<Vec<String>>,
    pub context: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ThoughtEntry {
    pub thought_type: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BeliefEntry {
    pub id: String,
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub value: String,
    pub confidence: String,
    pub confirmation_count: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GoalEntry {
    pub id: String,
    pub description: String,
    pub status: String,
    pub priority: u32,
    pub success_criteria: String,
    pub progress_notes: String,
    pub retry_count: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/soul/status", web::get().to(status::soul_status))
        .route("/soul/chat", web::post().to(chat::soul_chat))
        .route("/soul/chat/stream", web::post().to(chat::soul_chat_stream))
        .route("/soul/chat/sessions", web::get().to(chat::chat_sessions))
        .route(
            "/soul/chat/sessions/{id}",
            web::get().to(chat::session_messages),
        )
        .route("/soul/plan/approve", web::post().to(plans::plan_approve))
        .route("/soul/plan/reject", web::post().to(plans::plan_reject))
        .route("/soul/plan/pending", web::get().to(plans::plan_pending))
        .route("/soul/nudge", web::post().to(nudges::soul_nudge))
        .route("/soul/nudges", web::get().to(nudges::soul_nudges))
        .route(
            "/soul/goals/abandon-all",
            web::post().to(nudges::abandon_all_goals),
        )
        .route("/soul/goals/abandon", web::post().to(nudges::abandon_goal))
        .route("/soul/reset", web::post().to(lifecycle::soul_reset))
        .route(
            "/soul/cognitive-reset",
            web::post().to(lifecycle::cognitive_reset),
        )
        .route(
            "/soul/admin/reward",
            web::post().to(lifecycle::admin_reward),
        )
        .route(
            "/soul/admin/penalty",
            web::post().to(lifecycle::admin_penalty),
        )
        .route(
            "/soul/brain/weights",
            web::get().to(brain::get_brain_weights),
        )
        .route(
            "/soul/brain/merge",
            web::post().to(brain::merge_brain_delta),
        )
        .route("/soul/lessons", web::get().to(brain::get_lessons))
        .route("/soul/diagnostics", web::get().to(diagnostics::diagnostics))
        .route("/soul/introspection_summary", web::get().to(diagnostics::introspection_summary))
        .route(
            "/soul/benchmark",
            web::post().to(benchmark::trigger_benchmark),
        );
}

#[derive(Serialize)]
struct SoulStatus {
    active: bool,
    dormant: bool,
    total_cycles: u64,
    last_think_at: Option<i64>,
    mode: String,
    tools_enabled: bool,
    coding_enabled: bool,
    /// Cycle health metrics for observability.
    cycle_health: CycleHealth,
    /// Active plan info.
    #[serde(skip_serializing_if = "Option::is_none")]
    active_plan: Option<PlanInfo>,
    /// Pending approval plan info.
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_plan: Option<PlanInfo>,
    recent_thoughts: Vec<ThoughtEntry>,
    /// Fitness score — evolutionary selection pressure.
    #[serde(skip_serializing_if = "Option::is_none")]
    fitness: Option<serde_json::Value>,
    /// Active beliefs from the world model.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    beliefs: Vec<BeliefEntry>,
    /// Active goals driving multi-cycle behavior.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    goals: Vec<GoalEntry>,
    /// Capability profile — success rates per skill.
    #[serde(skip_serializing_if = "Option::is_none")]
    capability_profile: Option<serde_json::Value>,
    /// Recent plan outcomes — feedback loop data.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    plan_outcomes: Vec<serde_json::Value>,
    /// Opus IQ benchmark score + ELO rating.
    #[serde(skip_serializing_if = "Option::is_none")]
    benchmark: Option<serde_json::Value>,
    /// Neural brain status — parameters, training steps, loss.
    #[serde(skip_serializing_if = "Option::is_none")]
    brain: Option<serde_json::Value>,
    /// Plan transformer — 284K parameter sequence model.
    #[serde(skip_serializing_if = "Option::is_none")]
    transformer: Option<serde_json::Value>,
    /// Colony status — rank, niche, peer fitness.
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<serde_json::Value>,
    /// Durable behavioral rules — mechanically enforced from past failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    durable_rules: Option<serde_json::Value>,
    /// Recent failure chains — causal analysis of why steps fail.
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_chains: Option<serde_json::Value>,
    /// Cortex: predictive world model state — emotion, curiosity, prediction accuracy.
    #[serde(skip_serializing_if = "Option::is_none")]
    cortex: Option<serde_json::Value>,
    /// Genesis: evolved plan template gene pool.
    #[serde(skip_serializing_if = "Option::is_none")]
    genesis: Option<serde_json::Value>,
    /// Hivemind: stigmergic swarm intelligence.
    #[serde(skip_serializing_if = "Option::is_none")]
    hivemind: Option<serde_json::Value>,
    /// Synthesis: metacognitive self-awareness.
    #[serde(skip_serializing_if = "Option::is_none")]
    synthesis: Option<serde_json::Value>,
    /// Evaluation: rigorous measurement (Brier scores, calibration, ablation).
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluation: Option<serde_json::Value>,
    /// Free energy: the unifying metric (lower = smarter).
    #[serde(skip_serializing_if = "Option::is_none")]
    free_energy: Option<serde_json::Value>,
    /// Lifecycle: Fork → Branch → Birth differentiation phase.
    #[serde(skip_serializing_if = "Option::is_none")]
    lifecycle: Option<serde_json::Value>,
    /// Temporal binding: adaptive cognitive scheduling via neural oscillators.
    #[serde(skip_serializing_if = "Option::is_none")]
    temporal: Option<serde_json::Value>,
    /// Code generation model: 50M param Rust code generator (Phase 3).
    #[serde(skip_serializing_if = "Option::is_none")]
    codegen: Option<serde_json::Value>,
}
