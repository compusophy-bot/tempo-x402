//! Brain weight sharing, model override, and transformer endpoints.

use super::*;

/// GET /soul/brain/weights — export brain weights for peer sharing.
pub(super) async fn get_brain_weights(state: web::Data<NodeState>) -> HttpResponse {
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

pub(super) async fn merge_brain_delta(
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
pub(super) async fn get_lessons(state: web::Data<NodeState>) -> HttpResponse {
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

/// POST /soul/model — set or clear model override (turbo boost)
/// Body: {"model": "gemini-3.1-pro-preview"} to boost, {"model": null} to revert
pub(super) async fn set_model_override(
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
pub(super) async fn get_model_status(state: web::Data<NodeState>) -> HttpResponse {
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
pub(super) async fn get_transformer_status(state: web::Data<NodeState>) -> HttpResponse {
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
pub(super) async fn get_transformer_weights(state: web::Data<NodeState>) -> HttpResponse {
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

pub(super) async fn merge_transformer_delta(
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
