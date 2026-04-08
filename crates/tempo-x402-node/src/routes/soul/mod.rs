//! Soul endpoints — status, interactive chat with sessions, plan approval.

mod admin;
mod benchmark;
mod brain;
mod chat;
mod cognition;
mod colony_routes;
mod lifecycle;
mod nudges;
mod plans;
mod status;

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::state::NodeState;

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
    /// Learning acceleration α — second derivative of intelligence.
    #[serde(skip_serializing_if = "Option::is_none")]
    acceleration: Option<serde_json::Value>,
    /// Colony consciousness: Psi, colony size, phase3 readiness (alias of role for convenience).
    #[serde(skip_serializing_if = "Option::is_none")]
    colony: Option<serde_json::Value>,
    /// Bloch sphere: continuous cognitive state (theta, phi) on S².
    #[serde(skip_serializing_if = "Option::is_none")]
    bloch: Option<serde_json::Value>,
    /// Unified model: shared encoder with fast/slow heads.
    #[serde(skip_serializing_if = "Option::is_none")]
    unified_model: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct PlanInfo {
    id: String,
    goal_id: String,
    current_step: usize,
    total_steps: usize,
    status: String,
    replan_count: u32,
    current_step_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    goal_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<Vec<String>>,
    /// Accumulated step results (store_as keys from completed steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize)]
struct GoalEntry {
    id: String,
    description: String,
    status: String,
    priority: u32,
    success_criteria: String,
    progress_notes: String,
    retry_count: u32,
    created_at: i64,
    updated_at: i64,
}

#[derive(Serialize)]
struct BeliefEntry {
    id: String,
    domain: String,
    subject: String,
    predicate: String,
    value: String,
    confidence: String,
    confirmation_count: u32,
}

#[derive(Serialize)]
struct CycleHealth {
    last_cycle_entered_code: bool,
    total_code_entries: u64,
    cycles_since_last_commit: u64,
    completed_plans_count: u64,
    failed_plans_count: u64,
    goals_active: u64,
}

#[derive(Serialize)]
struct ThoughtEntry {
    #[serde(rename = "type")]
    thought_type: String,
    content: String,
    created_at: i64,
}

/// `GET /soul/system` — node-side system metrics (CPU, RAM, disk).
/// No external crates — reads /proc directly (Linux only, graceful fallback).
async fn system_metrics() -> HttpResponse {
    let cpu = read_cpu_usage();
    let (mem_used_mb, mem_total_mb) = read_memory();
    let (disk_used_mb, disk_total_mb, disk_pct) = read_disk("/data");

    HttpResponse::Ok().json(serde_json::json!({
        "cpu_pct": cpu,
        "mem_used_mb": mem_used_mb,
        "mem_total_mb": mem_total_mb,
        "mem_pct": if mem_total_mb > 0 { (mem_used_mb as f64 / mem_total_mb as f64 * 100.0).round() } else { 0.0 },
        "disk_used_mb": disk_used_mb,
        "disk_total_mb": disk_total_mb,
        "disk_pct": disk_pct,
    }))
}

fn read_cpu_usage() -> f64 {
    // Read /proc/loadavg — 1-min load average
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
        .map(|load| (load * 100.0 / num_cpus().max(1) as f64).round().min(100.0))
        .unwrap_or(0.0)
}

fn num_cpus() -> usize {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .map(|s| s.matches("processor").count())
        .unwrap_or(1)
        .max(1)
}

fn read_memory() -> (u64, u64) {
    let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total_kb = 0u64;
    let mut available_kb = 0u64;
    for line in meminfo.lines() {
        if let Some(val) = line.strip_prefix("MemTotal:") {
            total_kb = val.trim().split_whitespace().next()
                .and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("MemAvailable:") {
            available_kb = val.trim().split_whitespace().next()
                .and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }
    let used_mb = (total_kb.saturating_sub(available_kb)) / 1024;
    let total_mb = total_kb / 1024;
    (used_mb, total_mb)
}

fn read_disk(path: &str) -> (u64, u64, f64) {
    let output = std::process::Command::new("df")
        .args(["-m", path])
        .output()
        .ok();
    if let Some(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = stdout.lines().nth(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let total: u64 = parts[1].parse().unwrap_or(0);
                let used: u64 = parts[2].parse().unwrap_or(0);
                let pct: f64 = parts[4].trim_end_matches('%').parse().unwrap_or(0.0);
                return (used, total, pct);
            }
        }
    }
    (0, 0, 0.0)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/soul/system", web::get().to(system_metrics))
        .route("/soul/status", web::get().to(status::soul_status))
        .route("/soul/chat", web::post().to(chat::soul_chat))
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
        .route(
            "/soul/benchmark",
            web::post().to(benchmark::trigger_benchmark),
        )
        .route(
            "/soul/benchmark/solutions",
            web::get().to(benchmark::get_benchmark_solutions),
        )
        .route(
            "/soul/benchmark/failures",
            web::get().to(benchmark::get_benchmark_failures),
        )
        .route(
            "/soul/benchmark/review",
            web::post().to(benchmark::review_benchmark_solution),
        )
        .route(
            "/soul/code-review",
            web::post().to(benchmark::review_code_change),
        )
        .route("/soul/cleanup", web::post().to(lifecycle::disk_cleanup))
        .route("/soul/open-prs", web::get().to(lifecycle::open_prs))
        .route("/soul/diagnostics", web::get().to(status::soul_diagnostics))
        .route("/soul/dev-report", web::get().to(status::soul_dev_report))
        .route("/soul/cleanup", web::post().to(lifecycle::soul_cleanup))
        .route(
            "/soul/rules/reset",
            web::post().to(lifecycle::soul_rules_reset),
        )
        .route("/soul/colony", web::get().to(cognition::get_colony_status))
        .route("/soul/model", web::post().to(brain::set_model_override))
        .route("/soul/model", web::get().to(brain::get_model_status))
        .route(
            "/soul/model/transformer",
            web::get().to(brain::get_transformer_status),
        )
        .route(
            "/soul/model/transformer/weights",
            web::get().to(brain::get_transformer_weights),
        )
        .route(
            "/soul/model/transformer/merge",
            web::post().to(brain::merge_transformer_delta),
        )
        .route("/soul/events", web::get().to(status::soul_events))
        .route(
            "/soul/events/stream",
            web::get().to(status::soul_event_stream),
        )
        .route("/soul/history", web::get().to(status::soul_history))
        .route("/soul/health", web::get().to(status::soul_health))
        // Cognitive architecture sharing endpoints
        .route("/soul/cortex", web::get().to(cognition::get_cortex))
        .route("/soul/genesis", web::get().to(cognition::get_genesis))
        .route("/soul/hivemind", web::get().to(cognition::get_hivemind))
        // Admin: direct command execution (mind meld)
        .route("/soul/admin/exec", web::post().to(admin::admin_exec))
        .route(
            "/soul/admin/workspace-reset",
            web::post().to(admin::admin_workspace_reset),
        )
        .route(
            "/soul/admin/cargo-check",
            web::post().to(admin::admin_cargo_check),
        )
        // Studio: file browsing for the IDE
        .route("/soul/admin/ls", web::get().to(admin::admin_ls))
        .route("/soul/admin/cat", web::get().to(admin::admin_cat))
        // Colony collective consciousness endpoints
        .route(
            "/soul/colony/register",
            web::post().to(colony_routes::colony_register),
        )
        .route(
            "/soul/colony/peers",
            web::get().to(colony_routes::colony_peers),
        )
        .route(
            "/soul/colony/benchmark/assignment",
            web::get().to(colony_routes::colony_benchmark_assignment),
        )
        .route(
            "/soul/colony/benchmark/result",
            web::post().to(colony_routes::colony_benchmark_result),
        )
        .route(
            "/soul/colony/train",
            web::post().to(colony_routes::colony_train),
        )
        .route(
            "/soul/colony/work",
            web::post().to(colony_routes::colony_work),
        )
        .route(
            "/soul/colony/report",
            web::post().to(colony_routes::colony_report),
        );
}
