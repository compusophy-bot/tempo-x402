//! Soul endpoints — status, interactive chat with sessions, plan approval.

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
    /// HumanEval benchmark score + ELO rating.
    #[serde(skip_serializing_if = "Option::is_none")]
    benchmark: Option<serde_json::Value>,
    /// Neural brain status — parameters, training steps, loss.
    #[serde(skip_serializing_if = "Option::is_none")]
    brain: Option<serde_json::Value>,
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

async fn soul_status(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            let (tools_enabled, coding_enabled) = match &state.soul_config {
                Some(c) => (c.tools_enabled, c.coding_enabled),
                None => (false, false),
            };
            return HttpResponse::Ok().json(serde_json::json!({
                "active": false,
                "dormant": state.soul_dormant,
                "total_cycles": 0,
                "last_think_at": null,
                "mode": "observe",
                "tools_enabled": tools_enabled,
                "coding_enabled": coding_enabled,
                "cycle_health": {
                    "last_cycle_entered_code": false,
                    "total_code_entries": 0,
                    "cycles_since_last_commit": 0,
                    "failed_plans_count": 0,
                    "goals_active": 0
                },
                "recent_thoughts": []
            }));
        }
    };

    let total_cycles: u64 = soul_db
        .get_state("total_think_cycles")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let last_think_at: Option<i64> = soul_db
        .get_state("last_think_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok());

    // Show what the soul *thinks*, not what it *greps* — filter out ToolExecution
    let recent_thoughts: Vec<ThoughtEntry> = soul_db
        .recent_thoughts_by_type(
            &[
                x402_soul::ThoughtType::Decision,
                x402_soul::ThoughtType::Reasoning,
                x402_soul::ThoughtType::Observation,
                x402_soul::ThoughtType::Reflection,
                x402_soul::ThoughtType::MemoryConsolidation,
            ],
            15,
        )
        .unwrap_or_default()
        .into_iter()
        .map(|t| ThoughtEntry {
            thought_type: t.thought_type.as_str().to_string(),
            content: t.content,
            created_at: t.created_at,
        })
        .collect();

    let (tools_enabled, coding_enabled) = match &state.soul_config {
        Some(c) => (c.tools_enabled, c.coding_enabled),
        None => (false, false),
    };

    // Read cycle health metrics from soul_state
    let last_cycle_entered_code: bool = soul_db
        .get_state("last_cycle_entered_code")
        .ok()
        .flatten()
        .map(|s| s == "true")
        .unwrap_or(false);
    let total_code_entries: u64 = soul_db
        .get_state("total_code_entries")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let cycles_since_last_commit: u64 = soul_db
        .get_state("cycles_since_last_commit")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let failed_plans_count: u64 = soul_db.count_plans_by_status("failed").unwrap_or(0);
    let goals_active: u64 = soul_db
        .get_active_goals()
        .map(|g| g.len() as u64)
        .unwrap_or(0);

    // Determine displayed mode based on last cycle
    let mode = if last_cycle_entered_code {
        "code".to_string()
    } else {
        "observe".to_string()
    };

    // Fetch world model beliefs
    let beliefs: Vec<BeliefEntry> = soul_db
        .get_all_active_beliefs()
        .unwrap_or_default()
        .into_iter()
        .map(|b| BeliefEntry {
            id: b.id,
            domain: b.domain.as_str().to_string(),
            subject: b.subject,
            predicate: b.predicate,
            value: b.value,
            confidence: b.confidence.as_str().to_string(),
            confirmation_count: b.confirmation_count,
        })
        .collect();

    // Fetch active goals
    let goals: Vec<GoalEntry> = soul_db
        .get_active_goals()
        .unwrap_or_default()
        .into_iter()
        .map(|g| GoalEntry {
            id: g.id,
            description: g.description,
            status: g.status.as_str().to_string(),
            priority: g.priority,
            success_criteria: g.success_criteria,
            progress_notes: g.progress_notes,
            retry_count: g.retry_count,
            created_at: g.created_at,
            updated_at: g.updated_at,
        })
        .collect();

    // Fetch active plan
    let active_plan = soul_db.get_active_plan().ok().flatten().map(|p| {
        let current_step_type = p.steps.get(p.current_step).map(|s| s.summary());
        PlanInfo {
            id: p.id,
            goal_id: p.goal_id,
            current_step: p.current_step,
            total_steps: p.steps.len(),
            status: p.status.as_str().to_string(),
            replan_count: p.replan_count,
            current_step_type,
            goal_description: None,
            steps: None,
        }
    });

    // Fetch pending approval plan
    let pending_plan = soul_db.get_pending_approval_plan().ok().flatten().map(|p| {
        let goal_desc = soul_db
            .get_goal(&p.goal_id)
            .ok()
            .flatten()
            .map(|g| g.description);
        let step_summaries: Vec<String> = p.steps.iter().map(|s| s.summary()).collect();
        PlanInfo {
            id: p.id,
            goal_id: p.goal_id,
            current_step: p.current_step,
            total_steps: p.steps.len(),
            status: p.status.as_str().to_string(),
            replan_count: p.replan_count,
            current_step_type: None,
            goal_description: goal_desc,
            steps: Some(step_summaries),
        }
    });

    // Fetch fitness score
    let fitness = x402_soul::fitness::FitnessScore::load_current(soul_db).map(|f| {
        serde_json::json!({
            "total": f.total,
            "trend": f.trend,
            "economic": f.economic,
            "execution": f.execution,
            "evolution": f.evolution,
            "coordination": f.coordination,
            "introspection": f.introspection,
            "measured_at": f.measured_at,
        })
    });

    // Fetch capability profile
    let capability_profile = {
        let profile = x402_soul::capability::compute_profile(soul_db);
        serde_json::to_value(&profile).ok()
    };

    // Fetch recent plan outcomes (feedback loop data)
    let plan_outcomes: Vec<serde_json::Value> = soul_db
        .get_recent_plan_outcomes(10)
        .unwrap_or_default()
        .into_iter()
        .map(|o| {
            serde_json::json!({
                "goal_description": o.goal_description,
                "status": o.status,
                "lesson": o.lesson,
                "error_category": o.error_category.map(|c| c.as_str().to_string()),
                "steps_completed": o.steps_completed,
                "total_steps": o.total_steps,
                "replan_count": o.replan_count,
                "created_at": o.created_at,
            })
        })
        .collect();

    HttpResponse::Ok().json(SoulStatus {
        active: true,
        dormant: state.soul_dormant,
        total_cycles,
        last_think_at,
        mode,
        tools_enabled,
        coding_enabled,
        cycle_health: CycleHealth {
            last_cycle_entered_code,
            total_code_entries,
            cycles_since_last_commit,
            failed_plans_count,
            goals_active,
        },
        fitness,
        active_plan,
        pending_plan,
        recent_thoughts,
        beliefs,
        goals,
        capability_profile,
        plan_outcomes,
        benchmark: {
            let score = x402_soul::benchmark::load_score(soul_db);
            let elo = x402_soul::elo::load_rating(soul_db);
            let elo_history = x402_soul::elo::load_history(soul_db);
            score.map(|s| {
                serde_json::json!({
                    "pass_at_1": s.pass_at_1,
                    "problems_attempted": s.problems_attempted,
                    "problems_passed": s.problems_passed,
                    "measured_at": s.measured_at,
                    "elo_rating": elo,
                    "elo_display": x402_soul::elo::rating_display(soul_db),
                    "history": s.history,
                    "elo_history": elo_history,
                    "reference_scores": x402_soul::benchmark::REFERENCE_SCORES
                        .iter()
                        .map(|(name, score)| serde_json::json!({"model": name, "pass_at_1": score}))
                        .collect::<Vec<_>>(),
                })
            })
        },
        brain: {
            let brain = x402_soul::brain::load_brain(soul_db);
            if brain.train_steps > 0 {
                Some(serde_json::json!({
                    "parameters": brain.param_count(),
                    "train_steps": brain.train_steps,
                    "running_loss": brain.running_loss,
                }))
            } else {
                None
            }
        },
    })
}

// ── Weight sharing endpoint ──

/// GET /soul/brain/weights — export brain weights for peer sharing.
pub async fn get_brain_weights(state: web::Data<NodeState>) -> HttpResponse {
    let Some(soul_db) = &state.soul_db else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({"error": "no soul"}));
    };
    let brain = x402_soul::brain::load_brain(soul_db);
    HttpResponse::Ok().json(serde_json::json!({
        "weights": brain.to_json(),
        "train_steps": brain.train_steps,
        "param_count": brain.param_count(),
    }))
}

/// POST /soul/brain/merge — merge weight delta from a peer.
#[derive(Deserialize)]
struct MergeDeltaRequest {
    delta: String,
    merge_rate: Option<f32>,
}

pub async fn merge_brain_delta(
    state: web::Data<NodeState>,
    body: web::Json<MergeDeltaRequest>,
) -> HttpResponse {
    let Some(soul_db) = &state.soul_db else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({"error": "no soul"}));
    };
    let delta: x402_soul::brain::WeightDelta = match serde_json::from_str(&body.delta) {
        Ok(d) => d,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": format!("invalid delta: {e}")}));
        }
    };
    let merge_rate = body.merge_rate.unwrap_or(0.5);
    let mut brain = x402_soul::brain::load_brain(soul_db);
    brain.merge_delta(&delta, merge_rate);
    x402_soul::brain::save_brain(soul_db, &brain);
    HttpResponse::Ok().json(serde_json::json!({
        "merged": true,
        "train_steps": brain.train_steps,
        "source": delta.source_id,
    }))
}

// ── Experience sharing endpoints ──

/// GET /soul/lessons — export lessons (plan outcomes + capability profile) for peer sharing.
/// This is the key collective-intelligence endpoint: peers fetch each other's hard-won
/// experience so the swarm learns faster than any individual.
pub async fn get_lessons(state: web::Data<NodeState>) -> HttpResponse {
    let Some(soul_db) = &state.soul_db else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({"error": "no soul"}));
    };

    // Recent plan outcomes with lessons
    let outcomes: Vec<serde_json::Value> = soul_db
        .get_recent_plan_outcomes(20)
        .unwrap_or_default()
        .into_iter()
        .map(|o| {
            serde_json::json!({
                "goal": o.goal_description,
                "status": o.status,
                "error_category": o.error_category,
                "lesson": o.lesson,
                "steps_succeeded": o.steps_succeeded,
                "steps_failed": o.steps_failed,
            })
        })
        .collect();

    // Capability profile — what this agent is good/bad at
    let profile = x402_soul::capability::compute_profile(soul_db);

    // Benchmark score if available
    let benchmark = x402_soul::benchmark::load_score(soul_db);
    let elo = x402_soul::elo::load_rating(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "outcomes": outcomes,
        "capability_profile": serde_json::to_value(&profile).ok(),
        "benchmark": {
            "pass_at_1": benchmark.as_ref().map(|b| b.pass_at_1).unwrap_or(0.0),
            "problems_attempted": benchmark.as_ref().map(|b| b.problems_attempted).unwrap_or(0),
            "problems_passed": benchmark.as_ref().map(|b| b.problems_passed).unwrap_or(0),
            "elo": elo,
        },
    }))
}

// ── Chat endpoints ──

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    #[serde(default)]
    session_id: Option<String>,
}

async fn soul_chat(state: web::Data<NodeState>, body: web::Json<ChatRequest>) -> HttpResponse {
    // Validate soul is active
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "soul is not active"
            }));
        }
    };

    // Validate not dormant
    if state.soul_dormant {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "soul is dormant (no LLM API key)"
        }));
    }

    // Validate message length
    let message = body.message.trim();
    if message.is_empty() || message.len() > 4096 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "message must be 1-4096 characters"
        }));
    }

    // Get config and observer
    let config = match &state.soul_config {
        Some(c) => c,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "soul config not available"
            }));
        }
    };

    let observer = match &state.soul_observer {
        Some(o) => o,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "soul observer not available"
            }));
        }
    };

    match x402_soul::handle_chat(
        message,
        body.session_id.as_deref(),
        config,
        soul_db,
        observer,
    )
    .await
    {
        Ok(reply) => HttpResponse::Ok().json(serde_json::json!({
            "reply": reply.reply,
            "tool_executions": reply.tool_executions,
            "thought_ids": reply.thought_ids,
            "session_id": reply.session_id,
        })),
        Err(e) => {
            tracing::warn!(error = %e, "Soul chat failed");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("chat failed: {e}")
            }))
        }
    }
}

// ── Session endpoints ──

async fn chat_sessions(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::Ok().json(serde_json::json!([]));
        }
    };

    match soul_db.list_sessions(20) {
        Ok(sessions) => HttpResponse::Ok().json(sessions),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list sessions");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to list sessions: {e}")
            }))
        }
    }
}

async fn session_messages(state: web::Data<NodeState>, path: web::Path<String>) -> HttpResponse {
    let session_id = path.into_inner();
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "soul is not active"
            }));
        }
    };

    match soul_db.get_session_messages(&session_id, 50) {
        Ok(messages) => HttpResponse::Ok().json(messages),
        Err(e) => {
            tracing::warn!(error = %e, session_id = %session_id, "Failed to get session messages");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to get messages: {e}")
            }))
        }
    }
}

// ── Plan approval endpoints ──

#[derive(Deserialize)]
struct PlanApproveRequest {
    plan_id: String,
}

#[derive(Deserialize)]
struct PlanRejectRequest {
    plan_id: String,
    #[serde(default)]
    reason: Option<String>,
}

async fn plan_approve(
    state: web::Data<NodeState>,
    body: web::Json<PlanApproveRequest>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "soul is not active"
            }));
        }
    };

    match soul_db.approve_plan(&body.plan_id) {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "status": "approved",
            "plan_id": body.plan_id,
        })),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "no pending plan with that ID"
        })),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to approve plan");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to approve plan: {e}")
            }))
        }
    }
}

async fn plan_reject(
    state: web::Data<NodeState>,
    body: web::Json<PlanRejectRequest>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "soul is not active"
            }));
        }
    };

    match soul_db.reject_plan(&body.plan_id) {
        Ok(true) => {
            // Insert a nudge with the rejection reason
            if let Some(reason) = &body.reason {
                let _ = soul_db.insert_nudge("user", &format!("Plan rejected: {}", reason), 5);
            }
            HttpResponse::Ok().json(serde_json::json!({
                "status": "rejected",
                "plan_id": body.plan_id,
            }))
        }
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "no pending plan with that ID"
        })),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to reject plan");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to reject plan: {e}")
            }))
        }
    }
}

async fn plan_pending(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::Ok().json(serde_json::json!(null));
        }
    };

    match soul_db.get_pending_approval_plan() {
        Ok(Some(plan)) => {
            let goal_desc = soul_db
                .get_goal(&plan.goal_id)
                .ok()
                .flatten()
                .map(|g| g.description);
            let step_summaries: Vec<String> = plan.steps.iter().map(|s| s.summary()).collect();
            HttpResponse::Ok().json(serde_json::json!({
                "id": plan.id,
                "goal_id": plan.goal_id,
                "goal_description": goal_desc,
                "steps": step_summaries,
                "total_steps": plan.steps.len(),
                "created_at": plan.created_at,
            }))
        }
        Ok(None) => HttpResponse::Ok().json(serde_json::json!(null)),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get pending plan");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to get pending plan: {e}")
            }))
        }
    }
}

// ── Nudge endpoints ──

#[derive(Deserialize)]
struct NudgeRequest {
    message: String,
    priority: Option<u32>,
}

async fn soul_nudge(state: web::Data<NodeState>, body: web::Json<NudgeRequest>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "soul is not active"
            }));
        }
    };

    let message = body.message.trim();
    if message.is_empty() || message.len() > 2048 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "message must be 1-2048 characters"
        }));
    }

    // User nudges get highest priority (5) by default
    let priority = body.priority.unwrap_or(5).min(5);

    match soul_db.insert_nudge("user", message, priority) {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({
            "id": id,
            "status": "queued"
        })),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to insert nudge");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to queue nudge: {e}")
            }))
        }
    }
}

async fn soul_nudges(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::Ok().json(serde_json::json!([]));
        }
    };

    match soul_db.get_unprocessed_nudges(20) {
        Ok(nudges) => HttpResponse::Ok().json(nudges),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch nudges");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to fetch nudges: {e}")
            }))
        }
    }
}

/// POST /soul/goals/abandon-all — abandon all active goals (emergency reset).
async fn abandon_all_goals(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    match soul_db.abandon_all_active_goals() {
        Ok(count) => HttpResponse::Ok().json(serde_json::json!({
            "abandoned": count,
            "status": "ok"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to abandon goals: {e}")
        })),
    }
}

/// POST /soul/goals/abandon — abandon a single goal by ID.
#[derive(Deserialize)]
struct AbandonGoalRequest {
    goal_id: String,
}

async fn abandon_goal(
    state: web::Data<NodeState>,
    body: web::Json<AbandonGoalRequest>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    match soul_db.update_goal(
        &body.goal_id,
        Some("abandoned"),
        None,
        Some(chrono::Utc::now().timestamp()),
    ) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "goal_id": body.goal_id,
            "status": "abandoned"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to abandon goal: {e}")
        })),
    }
}

/// POST /soul/reset — clear historical dead weight (thoughts, failed plans, counters).
/// Keeps active goals, beliefs, and active plans.
async fn soul_reset(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    match soul_db.reset_history() {
        Ok((thoughts, plans, nudges)) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "deleted": {
                "thoughts": thoughts,
                "plans": plans,
                "nudges": nudges,
            },
            "kept": "active goals, active beliefs, active plans"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("reset failed: {e}")
        })),
    }
}

/// POST /soul/benchmark — request a benchmark run on the next cycle.
/// Sets a flag that the thinking loop checks.
async fn trigger_benchmark(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Clear cooldown so benchmark triggers on next eligible cycle
    let _ = soul_db.set_state("last_benchmark_at", "0");
    let _ = soul_db.set_state("last_benchmark_cycle", "0");

    // Check current score
    let current = x402_soul::benchmark::load_score(soul_db);
    let elo = x402_soul::elo::load_rating(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "status": "benchmark_requested",
        "message": "Benchmark will run on the next cycle that is divisible by the interval (default: 100)",
        "current_score": current.as_ref().map(|s| s.pass_at_1),
        "current_elo": elo,
        "problems_attempted": current.as_ref().map(|s| s.problems_attempted).unwrap_or(0),
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/soul/status", web::get().to(soul_status))
        .route("/soul/chat", web::post().to(soul_chat))
        .route("/soul/chat/sessions", web::get().to(chat_sessions))
        .route("/soul/chat/sessions/{id}", web::get().to(session_messages))
        .route("/soul/plan/approve", web::post().to(plan_approve))
        .route("/soul/plan/reject", web::post().to(plan_reject))
        .route("/soul/plan/pending", web::get().to(plan_pending))
        .route("/soul/nudge", web::post().to(soul_nudge))
        .route("/soul/nudges", web::get().to(soul_nudges))
        .route("/soul/goals/abandon-all", web::post().to(abandon_all_goals))
        .route("/soul/goals/abandon", web::post().to(abandon_goal))
        .route("/soul/reset", web::post().to(soul_reset))
        .route("/soul/brain/weights", web::get().to(get_brain_weights))
        .route("/soul/brain/merge", web::post().to(merge_brain_delta))
        .route("/soul/lessons", web::get().to(get_lessons))
        .route("/soul/benchmark", web::post().to(trigger_benchmark));
}
