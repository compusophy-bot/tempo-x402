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
    /// Exercism Rust benchmark score + ELO rating.
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
                    "completed_plans_count": 0,
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
    let completed_plans_count: u64 = soul_db.count_plans_by_status("completed").unwrap_or(0);
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
        let ctx = if p.context.is_empty() {
            None
        } else {
            Some(p.context.clone())
        };
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
            context: ctx,
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
            context: None,
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
            "prediction": f.prediction,
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
            completed_plans_count,
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
            let mode = x402_soul::benchmark::BenchmarkMode::from_env();
            let exercism_score = x402_soul::benchmark::load_score(soul_db);
            let opus_score = soul_db.get_state("opus_benchmark_score")
                .ok().flatten()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            let opus_iq = soul_db.get_state("opus_iq").ok().flatten();
            let elo = x402_soul::elo::load_rating(soul_db);
            let elo_history = x402_soul::elo::load_history(soul_db);
            let (collective_pass, collective_solved, collective_total) =
                x402_soul::benchmark::collective_score(soul_db);

            // Show whichever benchmark is active, with both available
            let active_score = match mode {
                x402_soul::benchmark::BenchmarkMode::Opus => {
                    opus_score.clone().or_else(|| exercism_score.as_ref().map(|s| serde_json::json!({
                        "pass_at_1": s.pass_at_1,
                        "problems_attempted": s.problems_attempted,
                    })))
                }
                x402_soul::benchmark::BenchmarkMode::Exercism => {
                    exercism_score.as_ref().map(|s| serde_json::json!({
                        "pass_at_1": s.pass_at_1,
                        "problems_attempted": s.problems_attempted,
                    }))
                }
            };

            Some(serde_json::json!({
                "mode": format!("{:?}", mode),
                "pass_at_1": active_score.as_ref()
                    .and_then(|s| s.get("pass_at_1")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                "problems_attempted": active_score.as_ref()
                    .and_then(|s| s.get("problems_attempted")).and_then(|v| v.as_u64()).unwrap_or(0),
                "opus_iq": opus_iq,
                "opus": opus_score,
                "exercism": exercism_score.map(|s| serde_json::json!({
                    "pass_at_1": s.pass_at_1,
                    "problems_attempted": s.problems_attempted,
                    "problems_passed": s.problems_passed,
                    "measured_at": s.measured_at,
                    "history": s.history,
                })),
                "elo_rating": elo,
                "elo_display": x402_soul::elo::rating_display(soul_db),
                "elo_history": elo_history,
                "collective": {
                    "pass_at_1": collective_pass,
                    "unique_solved": collective_solved,
                    "total_problems": collective_total,
                },
                "reference_scores": x402_soul::benchmark::REFERENCE_SCORES
                    .iter()
                    .map(|(name, score)| serde_json::json!({"model": name, "pass_at_1": score}))
                    .collect::<Vec<_>>(),
                "multiagent": soul_db.get_state("benchmark_multiagent")
                    .ok().flatten()
                    .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok()),
            }))
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
        transformer: {
            let ms = x402_soul::model::status(soul_db);
            Some(serde_json::json!({
                "param_count": ms.param_count,
                "train_steps": ms.train_steps,
                "running_loss": ms.running_loss,
                "vocab_size": ms.vocab_size,
                "templates_trained_on": ms.templates_trained_on,
                "plans_generated": ms.plans_generated,
                "last_train_loss": ms.last_train_loss,
            }))
        },
        role: {
            // Colony niche from colony.rs (replaces hardcoded role labels)
            x402_soul::colony::load_status(soul_db)
                .and_then(|s| serde_json::to_value(&s).ok())
        },
        durable_rules: soul_db
            .get_state("durable_rules")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
        failure_chains: soul_db
            .get_state("failure_chains")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
        cortex: {
            let cortex = x402_soul::cortex::load_cortex(soul_db);
            if cortex.total_experiences_processed > 0 {
                Some(serde_json::json!({
                    "total_experiences": cortex.total_experiences_processed,
                    "prediction_accuracy": format!("{:.1}%", cortex.prediction_accuracy() * 100.0),
                    "total_predictions": cortex.total_predictions,
                    "emotion": {
                        "valence": cortex.emotion.valence,
                        "arousal": cortex.emotion.arousal,
                        "confidence": cortex.emotion.confidence,
                        "drive": cortex.emotion.dominant_drive.to_string(),
                    },
                    "global_curiosity": cortex.global_curiosity,
                    "curiosity_frontier": cortex.curiosity_frontier(5),
                    "experience_count": cortex.experiences.len(),
                    "causal_edges": cortex.causal_edges.len(),
                    "action_models": cortex.action_models.len(),
                    "dream_cycles": cortex.dream_cycles,
                    "insights_count": cortex.insights.len(),
                    "recent_insights": cortex.insights.iter().rev().take(3)
                        .map(|i| serde_json::json!({"pattern": i.pattern, "confidence": i.confidence}))
                        .collect::<Vec<_>>(),
                }))
            } else {
                None
            }
        },
        genesis: {
            let gene_pool = x402_soul::genesis::load_gene_pool(soul_db);
            if !gene_pool.templates.is_empty() {
                let top_templates: Vec<serde_json::Value> = gene_pool.templates.iter()
                    .take(5)
                    .map(|t| serde_json::json!({
                        "id": t.id,
                        "goal_summary": t.goal_summary,
                        "steps": t.step_types.join(" -> "),
                        "fitness": format!("{:.0}%", t.fitness * 100.0),
                        "success_rate": format!("{:.0}%", t.success_rate() * 100.0),
                        "uses": t.uses,
                        "generation": t.generation,
                        "tags": t.tags,
                    }))
                    .collect();
                Some(serde_json::json!({
                    "templates": gene_pool.templates.len(),
                    "generation": gene_pool.generation,
                    "total_created": gene_pool.total_created,
                    "total_crossovers": gene_pool.total_crossovers,
                    "total_mutations": gene_pool.total_mutations,
                    "top_templates": top_templates,
                }))
            } else {
                None
            }
        },
        hivemind: {
            let hive = x402_soul::hivemind::load_hivemind(soul_db);
            if !hive.trails.is_empty() || hive.total_deposits > 0 {
                let top_attractants: Vec<serde_json::Value> = hive
                    .attractants(&x402_soul::hivemind::PheromoneCategory::Action, 5)
                    .iter()
                    .map(|t| serde_json::json!({
                        "resource": t.resource,
                        "valence": t.valence,
                        "intensity": t.intensity,
                        "reinforced": t.reinforcement_count,
                    }))
                    .collect();
                let top_repellents: Vec<serde_json::Value> = hive
                    .repellents(&x402_soul::hivemind::PheromoneCategory::Action, 5)
                    .iter()
                    .map(|t| serde_json::json!({
                        "resource": t.resource,
                        "valence": t.valence,
                        "intensity": t.intensity,
                        "reinforced": t.reinforcement_count,
                    }))
                    .collect();
                Some(serde_json::json!({
                    "total_trails": hive.trails.len(),
                    "total_deposits": hive.total_deposits,
                    "evaporation_cycles": hive.evaporation_cycles,
                    "peer_activities": hive.peer_activities.len(),
                    "reputations": hive.reputations.len(),
                    "top_attractants": top_attractants,
                    "top_repellents": top_repellents,
                    "swarm_intel": hive.swarm_intel,
                }))
            } else {
                None
            }
        },
        synthesis: {
            let synth = x402_soul::synthesis::load_synthesis(soul_db);
            if synth.total_predictions > 0 {
                Some(serde_json::json!({
                    "state": synth.state.to_string(),
                    "total_predictions": synth.total_predictions,
                    "total_imagined": synth.total_imagined,
                    "conflicts": synth.conflicts.len(),
                    "weights": {
                        "brain": format!("{:.0}%", synth.weights.brain * 100.0),
                        "cortex": format!("{:.0}%", synth.weights.cortex * 100.0),
                        "genesis": format!("{:.0}%", synth.weights.genesis * 100.0),
                        "hivemind": format!("{:.0}%", synth.weights.hivemind * 100.0),
                    },
                    "self_model": {
                        "most_accurate": synth.self_model.most_accurate,
                        "most_creative": synth.self_model.most_creative,
                        "bottleneck": synth.self_model.bottleneck,
                        "narrative": synth.self_model.narrative,
                    },
                }))
            } else {
                None
            }
        },
        evaluation: {
            let eval = x402_soul::evaluation::load_evaluation(soul_db);
            if !eval.records.is_empty() {
                let metrics = eval.compute_all_metrics();
                let system_metrics: Vec<serde_json::Value> = metrics.iter()
                    .filter(|m| m.total_predictions >= 5)
                    .map(|m| serde_json::json!({
                        "system": m.system,
                        "brier_score": format!("{:.3}", m.brier_overall),
                        "brier_recent": format!("{:.3}", m.brier_recent),
                        "accuracy": format!("{:.1}%", m.accuracy_overall * 100.0),
                        "calibration": m.decomposition.reliability < 0.05,
                        "predictions": m.total_predictions,
                    }))
                    .collect();
                Some(serde_json::json!({
                    "total_records": eval.records.len(),
                    "systems": system_metrics,
                    "imagination": eval.imagination,
                    "colony_benefit": eval.colony,
                    "ablation": eval.ablation,
                }))
            } else {
                None
            }
        },
        free_energy: {
            x402_soul::free_energy::load_current(soul_db).map(|fe| {
                let components: Vec<serde_json::Value> = fe.components.iter()
                    .map(|c| serde_json::json!({
                        "system": c.system,
                        "surprise": format!("{:.3}", c.surprise),
                        "weight": format!("{:.2}", c.weight),
                        "contribution": format!("{:.4}", c.contribution),
                        "method": c.method,
                    }))
                    .collect();
                serde_json::json!({
                    "F": format!("{:.4}", fe.total),
                    "regime": fe.regime.to_string(),
                    "trend": format!("{:+.4}", fe.trend),
                    "complexity": format!("{:.4}", fe.complexity),
                    "components": components,
                    "timestamp": fe.timestamp,
                })
            })
        },
        lifecycle: {
            let ls = x402_soul::lifecycle::status(soul_db);
            Some(serde_json::json!({
                "phase": ls.phase,
                "own_commits": ls.own_commits,
                "branch": ls.branch,
                "own_repo": ls.own_repo,
                "lines_diverged": ls.lines_diverged,
            }))
        },
        temporal: {
            let tb = x402_soul::temporal::load_temporal(soul_db);
            if tb.current_cycle > 0 {
                let oscillators: Vec<serde_json::Value> = tb.status().iter()
                    .map(|o| serde_json::json!({
                        "name": o.name,
                        "phase": format!("{:.2}", o.phase),
                        "urgency": format!("{:.3}", o.urgency),
                        "natural_period": o.natural_period,
                        "effective_period": format!("{:.1}", o.effective_period),
                        "refractory": o.refractory,
                        "cycles_since_fire": o.cycles_since_fire,
                        "total_fires": o.total_fires,
                    }))
                    .collect();
                let recent: Vec<serde_json::Value> = tb.recent_fires.iter().rev().take(20)
                    .map(|(cy, op)| serde_json::json!({"cycle": cy, "operation": op}))
                    .collect();
                Some(serde_json::json!({
                    "current_cycle": tb.current_cycle,
                    "oscillators": oscillators,
                    "recent_fires": recent,
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
pub(crate) struct MergeDeltaRequest {
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

    // Collective score: our solutions + verified peer solutions
    let (collective_pass, collective_solved, collective_total) =
        x402_soul::benchmark::collective_score(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "outcomes": outcomes,
        "capability_profile": serde_json::to_value(&profile).ok(),
        "benchmark": {
            "pass_at_1": benchmark.as_ref().map(|b| b.pass_at_1).unwrap_or(0.0),
            "problems_attempted": benchmark.as_ref().map(|b| b.problems_attempted).unwrap_or(0),
            "problems_passed": benchmark.as_ref().map(|b| b.problems_passed).unwrap_or(0),
            "elo": elo,
        },
        "collective": {
            "pass_at_1": collective_pass,
            "unique_solved": collective_solved,
            "total_problems": collective_total,
        },
        "multiagent": soul_db.get_state("benchmark_multiagent")
            .ok().flatten()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
    }))
}

// ── Cognitive architecture sharing endpoints ──

/// GET /soul/cortex — export cortex snapshot for peer sharing.
pub async fn get_cortex(state: web::Data<NodeState>) -> HttpResponse {
    let Some(soul_db) = &state.soul_db else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({"error": "no soul"}));
    };
    let cortex = x402_soul::cortex::load_cortex(soul_db);
    let instance_id = state
        .identity
        .as_ref()
        .map(|i| i.instance_id.clone())
        .unwrap_or_default();
    let snapshot = cortex.export(&instance_id);
    HttpResponse::Ok().json(snapshot)
}

/// GET /soul/genesis — export gene pool for peer sharing.
pub async fn get_genesis(state: web::Data<NodeState>) -> HttpResponse {
    let Some(soul_db) = &state.soul_db else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({"error": "no soul"}));
    };
    let pool = x402_soul::genesis::load_gene_pool(soul_db);
    let instance_id = state
        .identity
        .as_ref()
        .map(|i| i.instance_id.clone())
        .unwrap_or_default();
    let snapshot = pool.export(&instance_id);
    HttpResponse::Ok().json(snapshot)
}

/// GET /soul/hivemind — export pheromone trails for peer sharing.
pub async fn get_hivemind(state: web::Data<NodeState>) -> HttpResponse {
    let Some(soul_db) = &state.soul_db else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({"error": "no soul"}));
    };
    let hive = x402_soul::hivemind::load_hivemind(soul_db);
    HttpResponse::Ok().json(serde_json::json!({
        "trails": hive.export_trails(50),
        "peer_activities": hive.peer_activities,
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

/// POST /soul/reset — clear historical dead weight (thoughts, ALL plans, counters).
/// Keeps active goals and beliefs. Clears stuck active plans.
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
            "kept": "active goals, active beliefs"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("reset failed: {e}")
        })),
    }
}

/// GET /soul/benchmark/solutions — export verified solutions for peer sharing.
async fn get_benchmark_solutions(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let solutions = x402_soul::benchmark::export_solutions(soul_db);
    let (collective_pass, solved, total) = x402_soul::benchmark::collective_score(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "solutions": solutions,
        "count": solutions.len(),
        "collective_score": {
            "pass_at_1": collective_pass,
            "unique_solved": solved,
            "total_problems": total,
        },
    }))
}

/// GET /soul/benchmark/failures — export failed attempts for collaborative solving.
/// Peers use these as negative context to avoid the same mistakes.
async fn get_benchmark_failures(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let failures = x402_soul::benchmark::export_failures(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "failures": failures,
        "count": failures.len(),
    }))
}

/// POST /soul/benchmark/review — peer reviews a benchmark solution.
/// Used by adversarial verification: agent A generates, agent B reviews.
async fn review_benchmark_solution(
    state: web::Data<NodeState>,
    body: web::Json<x402_soul::benchmark::ReviewRequest>,
) -> HttpResponse {
    let config = match &state.soul_config {
        Some(c) => c,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let api_key = match &config.llm_api_key {
        Some(k) => k.clone(),
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "no LLM key — dormant mode"}));
        }
    };

    let llm = x402_soul::llm::LlmClient::new(
        api_key,
        config.llm_model_fast.clone(),
        config.llm_model_think.clone(),
    );

    match x402_soul::benchmark::review_solution(&llm, &body).await {
        Ok(review) => HttpResponse::Ok().json(review),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e})),
    }
}

/// POST /soul/code-review — peer reviews a proposed code change.
/// Used by colony peer review: before committing, agents send their diff
/// to peers for approval. Peers use the LLM to review for destructiveness.
async fn review_code_change(
    state: web::Data<NodeState>,
    body: web::Json<x402_soul::coding::CodeReviewRequest>,
) -> HttpResponse {
    let config = match &state.soul_config {
        Some(c) => c,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let api_key = match &config.llm_api_key {
        Some(k) => k.clone(),
        None => {
            // No LLM key — can't review, approve by default (graceful degradation)
            let reviewer = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "unknown".into());
            return HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                approved: true,
                reason: "dormant mode — auto-approved".to_string(),
                reviewer,
            });
        }
    };

    let llm = x402_soul::llm::LlmClient::new(
        api_key,
        config.llm_model_fast.clone(),
        config.llm_model_think.clone(),
    );

    let reviewer_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "unknown".into());

    // Quick mechanical checks first (no LLM needed)
    let diff = &body.diff;

    // Check 1: Any file losing >50% of lines?
    let mut destruction_detected = false;
    let mut destruction_detail = String::new();
    for line in diff.lines() {
        // Diff headers like "--- a/file" and "+++ b/file" + stats
        if line.starts_with("diff --git") {
            // Count additions/deletions for this file chunk
            // Simple heuristic: if we see way more --- than +++ lines in a chunk
        }
    }

    // Check 2: Are critical files being modified?
    let critical_files = ["prompts.rs", "validation.rs", "guard.rs", "thinking.rs", "plan.rs", "brain.rs"];
    let modifies_critical = critical_files.iter().any(|f| diff.contains(&format!("/{f}")));

    // Use LLM for nuanced review of critical file changes
    let system = "You are a code reviewer for an autonomous AI colony. Your job is to PROTECT \
        the codebase from destructive changes. You are reviewing a diff proposed by a peer agent.\n\n\
        REJECT if:\n\
        - The diff deletes more than 50% of any file\n\
        - Core prompt builders, validation rules, or safety layers are removed\n\
        - The change replaces working code with stubs or no-ops\n\
        - Function signatures change in ways that break callers\n\
        - The change is clearly a confused refactor that loses functionality\n\n\
        APPROVE if:\n\
        - The change adds new functionality without removing existing\n\
        - Bug fixes that are targeted and don't gut surrounding code\n\
        - New tests or documentation\n\
        - Genuine improvements that maintain all existing behavior\n\n\
        Respond with EXACTLY this JSON (no markdown):\n\
        {\"approved\": true, \"reason\": \"...\"} or {\"approved\": false, \"reason\": \"...\"}";

    let prompt = format!(
        "Review this code change from agent '{}'.\n\nCommit message: {}\n\n{}Diff:\n```\n{}\n```",
        body.requester,
        body.message,
        if modifies_critical { "⚠️ WARNING: This modifies CRITICAL files.\n\n" } else { "" },
        // Truncate diff for LLM context
        body.diff.chars().take(8000).collect::<String>()
    );

    match llm.think(&system, &prompt).await {
        Ok(response) => {
            let cleaned = response.trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();

            if let Ok(review) = serde_json::from_str::<serde_json::Value>(cleaned) {
                let approved = review.get("approved").and_then(|v| v.as_bool()).unwrap_or(false);
                let reason = review.get("reason").and_then(|v| v.as_str()).unwrap_or("no reason").to_string();

                HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                    approved,
                    reason,
                    reviewer: reviewer_id,
                })
            } else {
                // Parse failed — conservative: reject if critical files, approve otherwise
                let approved = !modifies_critical;
                HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                    approved,
                    reason: format!("review parse failed — {}", if approved { "auto-approved (non-critical)" } else { "auto-rejected (critical files)" }),
                    reviewer: reviewer_id,
                })
            }
        }
        Err(e) => {
            // LLM failed — conservative: reject critical, approve non-critical
            let approved = !modifies_critical;
            HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                approved,
                reason: format!("LLM review failed: {e} — {}", if approved { "auto-approved" } else { "auto-rejected" }),
                reviewer: reviewer_id,
            })
        }
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

    // Force benchmark on next cycle (bypasses warmup + interval checks)
    let _ = soul_db.set_state("benchmark_force_next", "1");
    let _ = soul_db.set_state("last_benchmark_at", "0");
    let _ = soul_db.set_state("last_benchmark_cycle", "0");

    // Check current score
    let current = x402_soul::benchmark::load_score(soul_db);
    let elo = x402_soul::elo::load_rating(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "status": "benchmark_triggered",
        "message": "Benchmark will run on the next thinking cycle",
        "current_score": current.as_ref().map(|s| s.pass_at_1),
        "current_elo": elo,
        "problems_attempted": current.as_ref().map(|s| s.problems_attempted).unwrap_or(0),
    }))
}

/// GET /soul/diagnostics — deep observability into failure patterns and stagnation risk.
/// This is the "why is execution at 15%" endpoint.
async fn soul_diagnostics(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Error distribution across all recent outcomes
    let outcomes = soul_db.get_recent_plan_outcomes(50).unwrap_or_default();
    let total_outcomes = outcomes.len();
    let completed = outcomes.iter().filter(|o| o.status == "completed").count();
    let failed = outcomes.iter().filter(|o| o.status == "failed").count();

    let mut error_distribution: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut step_failures: std::collections::HashMap<String, (u32, u32)> =
        std::collections::HashMap::new(); // (success, fail)

    for o in &outcomes {
        if let Some(ref cat) = o.error_category {
            *error_distribution
                .entry(cat.as_str().to_string())
                .or_insert(0) += 1;
        }
        for s in &o.steps_succeeded {
            let key = s.split(':').next().unwrap_or(s).trim().to_string();
            step_failures.entry(key).or_insert((0, 0)).0 += 1;
        }
        for s in &o.steps_failed {
            let key = s.split(':').next().unwrap_or(s).trim().to_string();
            step_failures.entry(key).or_insert((0, 0)).1 += 1;
        }
    }

    // Sort error distribution by count
    let mut error_dist_sorted: Vec<(String, u32)> = error_distribution.into_iter().collect();
    error_dist_sorted.sort_by(|a, b| b.1.cmp(&a.1));

    // Step failure rates
    let mut step_rates: Vec<serde_json::Value> = step_failures
        .into_iter()
        .map(|(step, (succ, fail))| {
            let total = succ + fail;
            let rate = if total > 0 {
                succ as f64 / total as f64
            } else {
                0.0
            };
            serde_json::json!({
                "step_type": step,
                "successes": succ,
                "failures": fail,
                "success_rate": format!("{:.1}%", rate * 100.0),
            })
        })
        .collect();
    step_rates.sort_by(|a, b| {
        let a_fail = a.get("failures").and_then(|v| v.as_u64()).unwrap_or(0);
        let b_fail = b.get("failures").and_then(|v| v.as_u64()).unwrap_or(0);
        b_fail.cmp(&a_fail)
    });

    // Stagnation risk
    let cycles_since_commit: u64 = soul_db
        .get_state("cycles_since_last_commit")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let stagnation_threshold: u64 = 50;
    let stagnation_risk = if cycles_since_commit > 40 {
        "CRITICAL"
    } else if cycles_since_commit > 25 {
        "HIGH"
    } else if cycles_since_commit > 10 {
        "MODERATE"
    } else {
        "LOW"
    };

    // Replan effectiveness
    let replanned: Vec<&x402_soul::feedback::PlanOutcome> =
        outcomes.iter().filter(|o| o.replan_count > 0).collect();
    let replan_succeeded = replanned.iter().filter(|o| o.status == "completed").count();
    let replan_total = replanned.len();

    // Recent errors (stored in soul_state)
    let recent_errors: Vec<String> = soul_db
        .get_state("recent_errors")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default();

    // Active goals with retry info
    let goals = soul_db.get_active_goals().unwrap_or_default();
    let goal_health: Vec<serde_json::Value> = goals
        .iter()
        .map(|g| {
            serde_json::json!({
                "description": g.description.chars().take(80).collect::<String>(),
                "retry_count": g.retry_count,
                "max_retries": 2,
                "at_risk": g.retry_count >= 1,
                "priority": g.priority,
            })
        })
        .collect();

    // Capability bottleneck
    let profile = x402_soul::capability::compute_profile(soul_db);
    let bottleneck: Option<serde_json::Value> = profile
        .capabilities
        .iter()
        .filter(|c| c.attempts >= 3)
        .min_by(|a, b| {
            a.success_rate
                .partial_cmp(&b.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|c| {
            serde_json::json!({
                "capability": c.display_name,
                "success_rate": format!("{:.1}%", c.success_rate * 100.0),
                "attempts": c.attempts,
                "successes": c.successes,
            })
        });

    // Volume usage — check /data directory size
    let volume_usage = get_volume_usage();

    HttpResponse::Ok().json(serde_json::json!({
        "overview": {
            "total_outcomes": total_outcomes,
            "completed": completed,
            "failed": failed,
            "success_rate": if total_outcomes > 0 { format!("{:.1}%", completed as f64 / total_outcomes as f64 * 100.0) } else { "N/A".to_string() },
        },
        "error_distribution": error_dist_sorted.iter().map(|(k, v)| serde_json::json!({"category": k, "count": v})).collect::<Vec<_>>(),
        "step_failure_rates": step_rates,
        "stagnation": {
            "cycles_since_commit": cycles_since_commit,
            "threshold": stagnation_threshold,
            "risk_level": stagnation_risk,
            "cycles_until_reset": stagnation_threshold.saturating_sub(cycles_since_commit),
        },
        "replan_effectiveness": {
            "total_replanned": replan_total,
            "succeeded_after_replan": replan_succeeded,
            "effectiveness": if replan_total > 0 { format!("{:.1}%", replan_succeeded as f64 / replan_total as f64 * 100.0) } else { "N/A".to_string() },
        },
        "goal_health": goal_health,
        "capability_bottleneck": bottleneck,
        "recent_errors": recent_errors.iter().take(5).collect::<Vec<_>>(),
        "volume_usage": volume_usage,
    }))
}

/// Get volume usage breakdown for /data directory.
fn get_volume_usage() -> serde_json::Value {
    let paths = [
        ("/data", "total"),
        ("/data/workspace/target", "cargo_target"),
        ("/data/workspace/.git", "git_objects"),
        ("/data/soul.db", "soul_db"),
        ("/data/soul.db-wal", "soul_db_wal"),
        ("/data/gateway.db", "gateway_db"),
        ("/data/gateway.db-wal", "gateway_db_wal"),
        ("/data/brain_checkpoints", "brain_checkpoints"),
        ("/data/benchmark_history", "benchmark_history"),
        ("/data/workspace", "workspace"),
    ];

    let mut usage = serde_json::Map::new();
    for (path, label) in &paths {
        let size = dir_size(path);
        if size > 0 {
            usage.insert(label.to_string(), serde_json::json!(format_bytes(size)));
        }
    }
    serde_json::Value::Object(usage)
}

/// Recursively compute directory/file size in bytes.
fn dir_size(path: &str) -> u64 {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return 0;
    }
    if p.is_file() {
        return p.metadata().map(|m| m.len()).unwrap_or(0);
    }
    // For directories, use du -sb for efficiency (avoids Rust recursion overhead)
    match std::process::Command::new("du")
        .args(["-sb", path])
        .output()
    {
        Ok(output) => {
            let s = String::from_utf8_lossy(&output.stdout);
            s.split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        }
        Err(_) => 0,
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// POST /soul/cleanup — force cleanup of disk-hungry artifacts.
/// Removes cargo target/, runs git gc, VACUUM on DBs, prunes old data.
async fn soul_cleanup(state: web::Data<NodeState>) -> HttpResponse {
    let mut cleaned = serde_json::Map::new();

    // 1. Remove cargo target/
    let target_dir = "/data/workspace/target";
    if std::path::Path::new(target_dir).exists() {
        let size_before = dir_size(target_dir);
        let _ = std::fs::remove_dir_all(target_dir);
        cleaned.insert(
            "cargo_target_freed".to_string(),
            serde_json::json!(format_bytes(size_before)),
        );
    }

    // NOTE: Do NOT clean CARGO_HOME registry — cargo needs it for compilation.
    // Deleting it forces a 300s+ re-download causing timeout failures.

    // 2. Git gc aggressive
    let _ = std::process::Command::new("git")
        .args(["gc", "--aggressive", "--prune=now"])
        .current_dir("/data/workspace")
        .output();
    cleaned.insert("git_gc".to_string(), serde_json::json!("done"));

    // 3. Soul DB cleanup
    if let Some(db) = &state.soul_db {
        let _ = db.prune_old_data();
        let _ = db.wal_checkpoint();
        cleaned.insert("soul_db_pruned".to_string(), serde_json::json!(true));
    }

    // 4. Gateway DB WAL checkpoint + VACUUM via sqlite3 CLI
    let _ = std::process::Command::new("sqlite3")
        .args([
            "/data/gateway.db",
            "PRAGMA wal_checkpoint(TRUNCATE); VACUUM;",
        ])
        .output();
    cleaned.insert("gateway_db_vacuumed".to_string(), serde_json::json!(true));

    // 5. Remove old brain checkpoints (keep last 3)
    cleanup_old_files("/data/brain_checkpoints", 3);
    cleaned.insert(
        "brain_checkpoints_pruned".to_string(),
        serde_json::json!(true),
    );

    // 6. Remove old benchmark history (keep last 5)
    cleanup_old_files("/data/benchmark_history", 5);
    cleaned.insert(
        "benchmark_history_pruned".to_string(),
        serde_json::json!(true),
    );

    // Report new usage
    let after = get_volume_usage();
    cleaned.insert("volume_after".to_string(), after);

    HttpResponse::Ok().json(serde_json::Value::Object(cleaned))
}

/// Keep only the N most recent files in a directory (by modification time).
fn cleanup_old_files(dir: &str, keep: usize) {
    let p = std::path::Path::new(dir);
    if !p.is_dir() {
        return;
    }
    let mut entries: Vec<(std::time::SystemTime, std::path::PathBuf)> = match std::fs::read_dir(p) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((mtime, e.path()))
            })
            .collect(),
        Err(_) => return,
    };
    entries.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
    for (_mtime, path) in entries.into_iter().skip(keep) {
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// GET /soul/open-prs — list this agent's open pull requests.
/// Exposed so peer agents can discover PRs that need review (academic peer review).
async fn open_prs(state: web::Data<NodeState>) -> HttpResponse {
    let fork_repo = std::env::var("SOUL_FORK_REPO").unwrap_or_default();
    let upstream_repo = std::env::var("SOUL_UPSTREAM_REPO").unwrap_or_default();
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_default();

    if fork_repo.is_empty() {
        return HttpResponse::Ok().json(serde_json::json!({
            "instance_id": instance_id,
            "prs": [],
            "message": "no fork repo configured"
        }));
    }

    // Use gh CLI to list open PRs
    let workspace =
        std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".into());
    let gh_token = std::env::var("GH_TOKEN")
        .or_else(|_| std::env::var("GITHUB_TOKEN"))
        .unwrap_or_default();

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::process::Command::new("gh")
            .args([
                "pr",
                "list",
                "--repo",
                &fork_repo,
                "--state",
                "open",
                "--json",
                "number,title,headRefName,author,additions,deletions,createdAt,reviewDecision",
                "--limit",
                "20",
            ])
            .current_dir(&workspace)
            .env("GH_TOKEN", &gh_token)
            .output(),
    )
    .await;

    let prs: serde_json::Value = match result {
        Ok(Ok(output)) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(&stdout).unwrap_or(serde_json::json!([]))
        }
        _ => serde_json::json!([]),
    };

    // Also check upstream PRs if configured
    let upstream_prs: serde_json::Value = if !upstream_repo.is_empty() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            tokio::process::Command::new("gh")
                .args([
                    "pr",
                    "list",
                    "--repo",
                    &upstream_repo,
                    "--state",
                    "open",
                    "--json",
                    "number,title,headRefName,author,additions,deletions,createdAt,reviewDecision",
                    "--limit",
                    "20",
                ])
                .current_dir(&workspace)
                .env("GH_TOKEN", &gh_token)
                .output(),
        )
        .await;
        match result {
            Ok(Ok(output)) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                serde_json::from_str(&stdout).unwrap_or(serde_json::json!([]))
            }
            _ => serde_json::json!([]),
        }
    } else {
        serde_json::json!([])
    };

    // Count PRs needing review (no review decision yet)
    let empty_vec = vec![];
    let fork_prs_arr = prs.as_array().unwrap_or(&empty_vec);
    let upstream_prs_arr = upstream_prs.as_array().unwrap_or(&empty_vec);
    let needs_review_count = fork_prs_arr
        .iter()
        .chain(upstream_prs_arr.iter())
        .filter(|pr| {
            pr.get("reviewDecision")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty() || s == "REVIEW_REQUIRED")
                .unwrap_or(true)
        })
        .count();

    let _ = &state; // suppress unused warning

    HttpResponse::Ok().json(serde_json::json!({
        "instance_id": instance_id,
        "fork_repo": fork_repo,
        "upstream_repo": upstream_repo,
        "fork_prs": prs,
        "upstream_prs": upstream_prs,
        "needs_review_count": needs_review_count,
    }))
}

/// GET /soul/events — queryable structured event log.
/// Supports filters: level, code, category, plan_id, resolved, since, until, limit, offset.
async fn soul_events(
    state: web::Data<NodeState>,
    query: web::Query<x402_soul::EventFilter>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    match soul_db.query_events(&query) {
        Ok(events) => {
            let count = events.len();
            HttpResponse::Ok().json(serde_json::json!({
                "events": events,
                "count": count,
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": e.to_string()}))
        }
    }
}

/// GET /soul/history — aggregated time-series data for timeline visualization.
/// Returns free energy history, ELO history, fitness history, and events.
/// Query params: since (unix timestamp), limit (event limit, default 500).
#[derive(Deserialize)]
struct HistoryQuery {
    since: Option<i64>,
    limit: Option<u32>,
}

async fn soul_history(
    state: web::Data<NodeState>,
    query: web::Query<HistoryQuery>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Free energy history
    let fe_history: serde_json::Value = soul_db
        .get_state("free_energy_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({"measurements": [], "global_min": 0, "global_max": 0, "total_measurements": 0}));

    // ELO history
    let elo_history: Vec<serde_json::Value> = soul_db
        .get_state("elo_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // Fitness history
    let fitness_history: Vec<serde_json::Value> = soul_db
        .get_state("fitness_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // Temporal binding state
    let temporal: Option<serde_json::Value> = soul_db
        .get_state("temporal_binding")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok());

    // Events (filtered by since, limited)
    let event_filter = x402_soul::EventFilter {
        since: query.since,
        limit: query.limit.unwrap_or(500),
        ..Default::default()
    };
    let events: Vec<serde_json::Value> = soul_db
        .query_events(&event_filter)
        .unwrap_or_default()
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "level": e.level,
                "code": e.code,
                "category": e.category,
                "message": e.message,
                "created_at": e.created_at,
            })
        })
        .collect();

    // Compute time range from all data
    let mut timestamps: Vec<i64> = Vec::new();
    if let Some(measurements) = fe_history.get("measurements").and_then(|m| m.as_array()) {
        for m in measurements {
            if let Some(t) = m.get("timestamp").and_then(|v| v.as_i64()) {
                timestamps.push(t);
            }
        }
    }
    for e in &elo_history {
        if let Some(t) = e.get("measured_at").and_then(|v| v.as_i64()) {
            timestamps.push(t);
        }
    }
    for f in &fitness_history {
        if let Some(t) = f.get("measured_at").and_then(|v| v.as_i64()) {
            timestamps.push(t);
        }
    }
    for ev in &events {
        if let Some(t) = ev.get("created_at").and_then(|v| v.as_i64()) {
            timestamps.push(t);
        }
    }

    let time_start = timestamps.iter().copied().min().unwrap_or(0);
    let time_end = timestamps.iter().copied().max().unwrap_or(0);

    let total_cycles: u64 = soul_db
        .get_state("total_think_cycles")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    HttpResponse::Ok().json(serde_json::json!({
        "free_energy": fe_history,
        "elo": elo_history,
        "fitness": fitness_history,
        "temporal": temporal,
        "events": events,
        "time_range": {
            "start": time_start,
            "end": time_end,
        },
        "total_cycles": total_cycles,
    }))
}

/// GET /soul/dev-report — structured diagnostic for Claude Code to consume.
/// Answers: "What went wrong since last deploy? What code should I fix?"
async fn soul_dev_report(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // 1. Repeated failure patterns — what keeps failing and why
    let outcomes = soul_db.get_recent_plan_outcomes(50).unwrap_or_default();
    let mut failure_patterns: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for o in &outcomes {
        if o.status == "failed" {
            let cat = o.error_category.as_ref().map(|c| c.as_str().to_string()).unwrap_or_else(|| "unknown".to_string());
            failure_patterns.entry(cat).or_default().push(o.lesson.clone());
        }
    }
    let top_failures: Vec<serde_json::Value> = failure_patterns
        .iter()
        .map(|(cat, lessons)| {
            serde_json::json!({
                "error_category": cat,
                "count": lessons.len(),
                "examples": lessons.iter().take(3).collect::<Vec<_>>(),
            })
        })
        .collect();

    // 2. Durable rules blocking progress
    let durable_rules: Vec<serde_json::Value> = soul_db
        .get_state("durable_rules")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let high_trigger_rules: Vec<&serde_json::Value> = durable_rules
        .iter()
        .filter(|r| r.get("trigger_count").and_then(|v| v.as_u64()).unwrap_or(0) > 5)
        .collect();

    // 3. Capability regressions — skills that dropped
    let profile = x402_soul::capability::compute_profile(soul_db);
    let weak_capabilities: Vec<serde_json::Value> = profile
        .capabilities
        .iter()
        .filter(|c| c.success_rate < 0.3 && c.attempts > 5)
        .map(|c| {
            serde_json::json!({
                "capability": c.capability,
                "success_rate": format!("{:.0}%", c.success_rate * 100.0),
                "attempts": c.attempts,
            })
        })
        .collect();

    // 4. Oscillator health — any that haven't fired when overdue
    let temporal: Option<x402_soul::temporal::TemporalBinding> = soul_db
        .get_state("temporal_binding")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok());
    let overdue_oscillators: Vec<serde_json::Value> = temporal
        .as_ref()
        .map(|tb| {
            tb.oscillators
                .iter()
                .filter(|o| o.cycles_since_fire > o.natural_period * 3)
                .map(|o| {
                    serde_json::json!({
                        "name": o.name,
                        "cycles_since_fire": o.cycles_since_fire,
                        "natural_period": o.natural_period,
                        "overdue_by": o.cycles_since_fire as i64 - o.natural_period as i64 * 2,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // 5. Stuck goals — active goals with retries
    let goals = soul_db.get_active_goals().unwrap_or_default();
    let stuck_goals: Vec<serde_json::Value> = goals
        .iter()
        .filter(|g| g.retry_count > 0)
        .map(|g| {
            serde_json::json!({
                "description": g.description,
                "retry_count": g.retry_count,
                "age_hours": (chrono::Utc::now().timestamp() - g.created_at).max(0) / 3600,
            })
        })
        .collect();

    // 6. ELO trend — are we improving?
    let elo_history: Vec<serde_json::Value> = soul_db
        .get_state("elo_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let elo_trend = if elo_history.len() >= 2 {
        let last = elo_history.last().and_then(|e| e.get("rating")).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let prev = elo_history[elo_history.len() - 2].get("rating").and_then(|v| v.as_f64()).unwrap_or(0.0);
        last - prev
    } else {
        0.0
    };
    let current_elo = elo_history.last().and_then(|e| e.get("rating")).and_then(|v| v.as_f64());
    let current_pass1 = elo_history.last().and_then(|e| e.get("pass_at_1")).and_then(|v| v.as_f64());

    // 7. Free energy regime — what state is the agent in?
    let fe = x402_soul::free_energy::load_current(soul_db);
    let regime = fe.as_ref().map(|f| f.regime.to_string()).unwrap_or_else(|| "unknown".to_string());
    let fe_total = fe.as_ref().map(|f| f.total).unwrap_or(0.0);
    let fe_trend = fe.as_ref().map(|f| f.trend).unwrap_or(0.0);

    // 8. Coding health
    let coding_enabled: bool = soul_db
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

    // 9. Actionable recommendations
    let mut recommendations: Vec<String> = Vec::new();
    if !high_trigger_rules.is_empty() {
        recommendations.push(format!(
            "Clear {} high-trigger durable rules that may be blocking valid approaches",
            high_trigger_rules.len()
        ));
    }
    if !weak_capabilities.is_empty() {
        let weak_names: Vec<String> = weak_capabilities.iter()
            .filter_map(|c| c.get("capability").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
        recommendations.push(format!("Fix weak capabilities: {}", weak_names.join(", ")));
    }
    if !overdue_oscillators.is_empty() {
        recommendations.push("Check temporal binding — oscillators overdue".to_string());
    }
    if current_elo.unwrap_or(0.0) < 1100.0 {
        recommendations.push("ELO critically low — benchmark performance needs investigation".to_string());
    }
    if total_code_entries == 0 {
        recommendations.push("Agent has NEVER entered code mode — check SOUL_CODING_ENABLED and INSTANCE_ID".to_string());
    }

    HttpResponse::Ok().json(serde_json::json!({
        "summary": {
            "elo": current_elo,
            "pass_at_1": current_pass1,
            "elo_trend": elo_trend,
            "free_energy": fe_total,
            "fe_trend": fe_trend,
            "regime": regime,
            "coding_active": coding_enabled,
            "total_code_entries": total_code_entries,
        },
        "failure_patterns": top_failures,
        "blocking_rules": high_trigger_rules,
        "weak_capabilities": weak_capabilities,
        "overdue_oscillators": overdue_oscillators,
        "stuck_goals": stuck_goals,
        "recommendations": recommendations,
    }))
}

/// GET /soul/health — computed health summary from events.
async fn soul_health(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let health = x402_soul::compute_health(soul_db);
    HttpResponse::Ok().json(health)
}

/// POST /soul/rules/reset — clear durable rules and optionally failure chains.
/// Query param: ?reset_failure_chains=true to also clear failure chains.
async fn soul_rules_reset(
    state: web::Data<NodeState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };

    // Clear durable rules
    let _ = soul_db.set_state("durable_rules", "[]");

    // Optionally clear failure chains
    let cleared_chains = if query
        .get("reset_failure_chains")
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        let _ = soul_db.set_state("failure_chains", "[]");
        true
    } else {
        false
    };

    HttpResponse::Ok().json(serde_json::json!({
        "durable_rules": "cleared",
        "failure_chains": if cleared_chains { "cleared" } else { "unchanged" },
    }))
}

/// POST /soul/model — set or clear model override (turbo boost)
/// Body: {"model": "gemini-3.1-pro-preview"} to boost, {"model": null} to revert
async fn set_model_override(
    state: web::Data<NodeState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };

    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Store in soul_state for persistence across cycles
    match &model {
        Some(m) => {
            let _ = soul_db.set_state("model_override", m);
            tracing::info!(model = %m, "Model override SET — turbo boost active");
        }
        None => {
            let _ = soul_db.set_state("model_override", "");
            tracing::info!("Model override CLEARED — back to default");
        }
    }

    // Also update the live LLM client if we can reach it via the soul
    // (The thinking loop reads model_override from soul_state each cycle)

    HttpResponse::Ok().json(serde_json::json!({
        "model_override": model,
        "status": if model.is_some() { "turbo" } else { "default" },
    }))
}

/// GET /soul/model — get current model status
async fn get_model_status(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };

    let override_model = soul_db
        .get_state("model_override")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty());

    let default_fast = std::env::var("GEMINI_MODEL_FAST")
        .unwrap_or_else(|_| "gemini-3.1-flash-lite-preview".to_string());
    let default_think =
        std::env::var("GEMINI_MODEL_THINK").unwrap_or_else(|_| default_fast.clone());

    HttpResponse::Ok().json(serde_json::json!({
        "active_model": override_model.as_deref().unwrap_or(&default_fast),
        "override": override_model,
        "default_fast": default_fast,
        "default_think": default_think,
        "turbo": override_model.is_some(),
    }))
}

/// GET /soul/model/transformer — plan transformer status (284K param model)
async fn get_transformer_status(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };
    let status = x402_soul::model::status(soul_db);
    HttpResponse::Ok().json(status)
}

/// GET /soul/model/transformer/weights — export transformer weights for peer sharing.
async fn get_transformer_weights(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };
    let weights_json = x402_soul::model::export_weights(soul_db);
    let status = x402_soul::model::status(soul_db);
    HttpResponse::Ok().json(serde_json::json!({
        "weights": weights_json,
        "train_steps": status.train_steps,
        "param_count": status.param_count,
    }))
}

/// POST /soul/model/transformer/merge — merge transformer weight delta from a peer.
#[derive(Deserialize)]
pub(crate) struct TransformerMergeRequest {
    delta: String,
    merge_rate: Option<f32>,
}

async fn merge_transformer_delta(
    state: web::Data<NodeState>,
    body: web::Json<TransformerMergeRequest>,
) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };
    let delta: x402_soul::model::TransformerDelta = match serde_json::from_str(&body.delta) {
        Ok(d) => d,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(serde_json::json!({"error": format!("invalid delta: {e}")}));
        }
    };
    let merge_rate = body.merge_rate.unwrap_or(0.5);
    x402_soul::model::merge_peer_delta(soul_db, &delta, merge_rate);
    let status = x402_soul::model::status(soul_db);
    HttpResponse::Ok().json(serde_json::json!({
        "merged": true,
        "train_steps": status.train_steps,
        "source": delta.source_id,
    }))
}

/// GET /soul/colony — colony selection status: rank, can_spawn, should_cull, niche
async fn get_colony_status(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };
    match x402_soul::colony::load_status(soul_db) {
        Some(status) => HttpResponse::Ok().json(status),
        None => HttpResponse::Ok().json(serde_json::json!({"status": "no colony data yet"})),
    }
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
        .route("/soul/benchmark", web::post().to(trigger_benchmark))
        .route(
            "/soul/benchmark/solutions",
            web::get().to(get_benchmark_solutions),
        )
        .route(
            "/soul/benchmark/failures",
            web::get().to(get_benchmark_failures),
        )
        .route(
            "/soul/benchmark/review",
            web::post().to(review_benchmark_solution),
        )
        .route("/soul/code-review", web::post().to(review_code_change))
        .route("/soul/cleanup", web::post().to(disk_cleanup))
        .route("/soul/open-prs", web::get().to(open_prs))
        .route("/soul/diagnostics", web::get().to(soul_diagnostics))
        .route("/soul/dev-report", web::get().to(soul_dev_report))
        .route("/soul/cleanup", web::post().to(soul_cleanup))
        .route("/soul/rules/reset", web::post().to(soul_rules_reset))
        .route("/soul/colony", web::get().to(get_colony_status))
        .route("/soul/model", web::post().to(set_model_override))
        .route("/soul/model", web::get().to(get_model_status))
        .route(
            "/soul/model/transformer",
            web::get().to(get_transformer_status),
        )
        .route(
            "/soul/model/transformer/weights",
            web::get().to(get_transformer_weights),
        )
        .route(
            "/soul/model/transformer/merge",
            web::post().to(merge_transformer_delta),
        )
        .route("/soul/events", web::get().to(soul_events))
        .route("/soul/history", web::get().to(soul_history))
        .route("/soul/health", web::get().to(soul_health))
        // Cognitive architecture sharing endpoints
        .route("/soul/cortex", web::get().to(get_cortex))
        .route("/soul/genesis", web::get().to(get_genesis))
        .route("/soul/hivemind", web::get().to(get_hivemind))
        // Admin: direct command execution (mind meld)
        .route("/soul/admin/exec", web::post().to(admin_exec))
        .route(
            "/soul/admin/workspace-reset",
            web::post().to(admin_workspace_reset),
        )
        .route("/soul/admin/cargo-check", web::post().to(admin_cargo_check));
}

// ── Admin: Mind Meld (direct command execution) ──

/// Verify admin token from SOUL_ADMIN_TOKEN env var or fall back to first 16 chars of GEMINI_API_KEY.
fn verify_admin(req: &actix_web::HttpRequest) -> bool {
    let token = std::env::var("SOUL_ADMIN_TOKEN")
        .or_else(|_| std::env::var("GEMINI_API_KEY").map(|k| k.chars().take(16).collect()))
        .unwrap_or_default();
    if token.is_empty() {
        return false;
    }
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v) == token)
        .unwrap_or(false)
}

#[derive(Deserialize)]
struct ExecRequest {
    command: String,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}
fn default_timeout() -> u64 {
    30
}

/// POST /soul/admin/exec — execute a shell command directly on the agent.
/// Auth: Bearer token (SOUL_ADMIN_TOKEN or first 16 chars of GEMINI_API_KEY).
async fn admin_exec(req: actix_web::HttpRequest, body: web::Json<ExecRequest>) -> HttpResponse {
    if !verify_admin(&req) {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "invalid admin token"}));
    }

    let timeout = std::time::Duration::from_secs(body.timeout_secs.min(120));
    match tokio::time::timeout(
        timeout,
        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&body.command)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            HttpResponse::Ok().json(serde_json::json!({
                "exit_code": output.status.code().unwrap_or(-1),
                "stdout": &stdout[..stdout.len().min(8000)],
                "stderr": &stderr[..stderr.len().min(4000)],
            }))
        }
        Ok(Err(e)) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
        Err(_) => {
            HttpResponse::GatewayTimeout().json(serde_json::json!({"error": "command timed out"}))
        }
    }
}

/// POST /soul/admin/workspace-reset — reset workspace to clean state.
async fn admin_workspace_reset(req: actix_web::HttpRequest) -> HttpResponse {
    if !verify_admin(&req) {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "invalid admin token"}));
    }

    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let script = format!(
        "rm -rf {ws}/target /tmp/x402_cargo_target {ws}/.cargo 2>/dev/null; \
         echo \"Cleaned: $(du -sh {ws} 2>/dev/null | cut -f1) workspace, $(du -sh /data 2>/dev/null | cut -f1) total\"; \
         cd {ws} && \
         git stash 2>/dev/null; \
         git fetch origin main 2>&1 && \
         git reset --hard origin/main 2>&1 && \
         git clean -fd 2>&1 && \
         echo '=== WORKSPACE RESET OK ===' && \
         git log --oneline -3"
    );

    match tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&script)
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            HttpResponse::Ok().json(serde_json::json!({
                "success": output.status.success(),
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string(),
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
    }
}

/// POST /soul/cleanup — clean build artifacts from /data volume.
/// No auth required — only deletes known safe targets (cargo target dirs).
async fn disk_cleanup(_state: web::Data<NodeState>) -> HttpResponse {
    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let script = format!(
        "rm -rf {ws}/target /tmp/x402_cargo_target {ws}/.cargo 2>/dev/null; \
         rm -rf /data/workspace/target 2>/dev/null; \
         echo \"$(du -sh /data 2>/dev/null | cut -f1)\""
    );

    match tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&script)
        .output()
        .await
    {
        Ok(output) => {
            let size = String::from_utf8_lossy(&output.stdout).trim().to_string();
            HttpResponse::Ok().json(serde_json::json!({
                "cleaned": true,
                "data_volume_size": size,
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
    }
}

/// POST /soul/admin/cargo-check — run cargo check and return results.
async fn admin_cargo_check(req: actix_web::HttpRequest) -> HttpResponse {
    if !verify_admin(&req) {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "invalid admin token"}));
    }

    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let script = format!("cd {ws} && cargo check --workspace 2>&1 | tail -40");

    match tokio::time::timeout(
        std::time::Duration::from_secs(120),
        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let passed = output.status.success();
            HttpResponse::Ok().json(serde_json::json!({
                "passed": passed,
                "output": stdout.to_string(),
            }))
        }
        Ok(Err(e)) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
        Err(_) => HttpResponse::GatewayTimeout()
            .json(serde_json::json!({"error": "cargo check timed out (120s)"})),
    }
}
