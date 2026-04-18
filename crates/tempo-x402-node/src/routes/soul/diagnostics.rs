//! Diagnostics cartridge: exposing fitness and economic metrics.

use actix_web::{web, HttpResponse};
use serde_json::json;
use crate::state::NodeState;
use x402_soul::synthesis;
use chrono::Utc;

pub(super) async fn diagnostics(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => return HttpResponse::ServiceUnavailable().json(json!({"error": "soul_db not initialized"})),
    };

    let synth = synthesis::load_synthesis(soul_db);
    
    // 1. Fitness Metrics: Derived from goal success and prediction error minimization.
    let total_plans: u64 = soul_db.count_plans_by_status("completed").unwrap_or(0) 
                         + soul_db.count_plans_by_status("failed").unwrap_or(0);
    let completed_plans: u64 = soul_db.count_plans_by_status("completed").unwrap_or(0);
    let success_rate = if total_plans > 0 {
        completed_plans as f64 / total_plans as f64
    } else {
        0.0
    };

    let active_goals = soul_db.get_active_goals().unwrap_or_default().len();

    // 2. Economic Metrics: Derived from reputation/settlements (if available) 
    // and internal node economy state.
    let reputation: u64 = soul_db
        .get_state("reputation_score")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // 3. Memory Metrics: Derived from synthesis metadata
    let observation_count = synth.observation_count;
    let total_predictions = synth.total_predictions;

    // 4. System Stats
    let uptime = Utc::now().signed_duration_since(state.started_at);
    let error_count: u64 = soul_db.get_state("error_count").ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(0);

    HttpResponse::Ok().json(json!({
        "fitness": {
            "success_rate": success_rate,
            "total_plans": total_plans,
            "completed_plans": completed_plans,
            "active_goals": active_goals,
            "prediction_error_rate": soul_db.get_state("avg_surprise").ok().flatten().unwrap_or_else(|| "0".to_string()),
            "cognitive_state": format!("{:?}", synth.state)
        },
        "memory": {
            "observations": observation_count,
            "predictions": total_predictions,
            "conflicts_recorded": synth.conflicts.len()
        },
        "economy": {
            "reputation": reputation,
            "earned_tokens": "0".to_string(),
            "clone_price": state.clone_price,
            "is_node_competitive": success_rate > 0.5 && reputation > 10
        },
        "system": {
            "uptime_seconds": uptime.num_seconds(),
            "error_count": error_count
        }
    }))
}

pub(super) async fn introspection_summary(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => return HttpResponse::ServiceUnavailable().json(json!({"error": "soul_db not initialized"})),
    };

    let completed_plans: u64 = soul_db.count_plans_by_status("completed").unwrap_or(0);
    let failed_plans: u64 = soul_db.count_plans_by_status("failed").unwrap_or(0);
    let total_plans = completed_plans + failed_plans;

    // Simple Elo-like health metric: success_rate * (1 + log(total_plans + 1))
    let win_rate = if total_plans > 0 {
        completed_plans as f64 / total_plans as f64
    } else {
        0.0
    };
    
    let elo_proxy = (win_rate * 1000.0) + (total_plans as f64).ln() * 50.0;
    
    let error_count: u64 = soul_db.get_state("error_count").ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(0);
    let health_score = if error_count > 0 {
        (100.0 / (error_count as f64 + 1.0)).max(0.0)
    } else {
        100.0
    };

    HttpResponse::Ok().json(json!({
        "elo_proxy": elo_proxy,
        "health_score": health_score,
        "metrics": {
            "completed_plans": completed_plans,
            "failed_plans": failed_plans,
            "error_count": error_count
        }
    }))
}
