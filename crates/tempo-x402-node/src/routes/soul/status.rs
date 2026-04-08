//! Soul status, diagnostics, dev-report, health, events, and history endpoints.

use super::*;

pub(super) async fn soul_status(state: web::Data<NodeState>) -> HttpResponse {
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
            let opus_score = soul_db.get_state("opus_benchmark_score")
                .ok().flatten()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            let opus_iq = soul_db.get_state("opus_iq").ok().flatten();
            let elo = x402_soul::elo::load_rating(soul_db);
            let elo_history = x402_soul::elo::load_history(soul_db);
            let (collective_pass, collective_solved, collective_total) =
                x402_soul::benchmark::collective_score(soul_db);

            let active_score = opus_score.clone();

            Some(serde_json::json!({
                "mode": "Opus",
                "pass_at_1": active_score.as_ref()
                    .and_then(|s| s.get("pass_at_1")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                "problems_attempted": active_score.as_ref()
                    .and_then(|s| s.get("problems_attempted")).and_then(|v| v.as_u64()).unwrap_or(0),
                "opus_iq": opus_iq,
                "opus": opus_score,
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
        codegen: {
            let cg = x402_soul::codegen::status(soul_db);
            Some(cg)
        },
        acceleration: {
            Some(x402_soul::acceleration::status(soul_db))
        },
        colony: {
            // Expose psi/colony data under both "role" and "colony" for convenience
            x402_soul::colony::load_status(soul_db)
                .and_then(|s| serde_json::to_value(&s).ok())
        },
        bloch: {
            Some(x402_soul::bloch::status(soul_db))
        },
        unified_model: {
            Some(x402_soul::unified_training::status(soul_db))
        },
    })
}

/// GET /soul/diagnostics — deep observability into failure patterns and stagnation risk.
/// This is the "why is execution at 15%" endpoint.
pub(super) async fn soul_diagnostics(state: web::Data<NodeState>) -> HttpResponse {
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
pub(super) fn get_volume_usage() -> serde_json::Value {
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
pub(super) fn dir_size(path: &str) -> u64 {
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

pub(super) fn format_bytes(bytes: u64) -> String {
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

/// GET /soul/dev-report — structured diagnostic for Claude Code to consume.
/// Answers: "What went wrong since last deploy? What code should I fix?"
pub(super) async fn soul_dev_report(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // 1. Repeated failure patterns — what keeps failing and why
    let outcomes = soul_db.get_recent_plan_outcomes(50).unwrap_or_default();
    let mut failure_patterns: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for o in &outcomes {
        if o.status == "failed" {
            let cat = o
                .error_category
                .as_ref()
                .map(|c| c.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            failure_patterns
                .entry(cat)
                .or_default()
                .push(o.lesson.clone());
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
        let last = elo_history
            .last()
            .and_then(|e| e.get("rating"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let prev = elo_history[elo_history.len() - 2]
            .get("rating")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        last - prev
    } else {
        0.0
    };
    let current_elo = elo_history
        .last()
        .and_then(|e| e.get("rating"))
        .and_then(|v| v.as_f64());
    let current_pass1 = elo_history
        .last()
        .and_then(|e| e.get("pass_at_1"))
        .and_then(|v| v.as_f64());

    // 7. Free energy regime — what state is the agent in?
    let fe = x402_soul::free_energy::load_current(soul_db);
    let regime = fe
        .as_ref()
        .map(|f| f.regime.to_string())
        .unwrap_or_else(|| "unknown".to_string());
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
        let weak_names: Vec<String> = weak_capabilities
            .iter()
            .filter_map(|c| {
                c.get("capability")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        recommendations.push(format!("Fix weak capabilities: {}", weak_names.join(", ")));
    }
    if !overdue_oscillators.is_empty() {
        recommendations.push("Check temporal binding — oscillators overdue".to_string());
    }
    if current_elo.unwrap_or(0.0) < 1100.0 {
        recommendations
            .push("ELO critically low — benchmark performance needs investigation".to_string());
    }
    if total_code_entries == 0 {
        recommendations.push(
            "Agent has NEVER entered code mode — check SOUL_CODING_ENABLED and INSTANCE_ID"
                .to_string(),
        );
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
pub(super) async fn soul_health(state: web::Data<NodeState>) -> HttpResponse {
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

/// GET /soul/events — queryable structured event log.
/// Supports filters: level, code, category, plan_id, resolved, since, until, limit, offset.
pub(super) async fn soul_events(
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
pub(super) struct HistoryQuery {
    since: Option<i64>,
    limit: Option<u32>,
}

pub(super) async fn soul_history(
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

/// `GET /soul/events/stream` — Server-Sent Events stream of cognitive events.
///
/// Polls the events DB every 2s for new events since the last seen timestamp.
/// Frontend subscribes with `EventSource` for real-time visualization.
pub(super) async fn soul_event_stream(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db.clone(),
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Use a channel-based stream (no futures crate needed)
    let (tx, rx) =
        tokio::sync::mpsc::channel::<Result<actix_web::web::Bytes, std::io::Error>>(32);

    tokio::spawn(async move {
        let mut last_ts = chrono::Utc::now().timestamp();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let filter = x402_soul::events::EventFilter {
                since: Some(last_ts),
                limit: 50,
                ..Default::default()
            };

            let events = soul_db.query_events(&filter).unwrap_or_default();

            let mut output = String::new();
            for event in &events {
                if event.created_at > last_ts {
                    last_ts = event.created_at;
                }
                let data = serde_json::json!({
                    "id": event.id,
                    "code": event.code,
                    "level": event.level,
                    "message": event.message,
                    "context": event.context,
                    "timestamp": event.created_at,
                });
                output.push_str(&format!("event: soul_event\ndata: {}\n\n", data));
            }

            if events.is_empty() {
                output.push_str("event: heartbeat\ndata: {}\n\n");
            }

            if tx
                .send(Ok(actix_web::web::Bytes::from(output)))
                .await
                .is_err()
            {
                break; // Client disconnected
            }
        }
    });

    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .streaming(tokio_stream::wrappers::ReceiverStream::new(rx))
}
