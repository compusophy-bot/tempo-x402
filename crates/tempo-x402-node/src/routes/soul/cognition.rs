//! Cognitive architecture sharing endpoints — cortex, genesis, hivemind, colony.

use super::*;

/// GET /soul/cortex — export cortex snapshot for peer sharing.
pub(super) async fn get_cortex(state: web::Data<NodeState>) -> HttpResponse {
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
pub(super) async fn get_genesis(state: web::Data<NodeState>) -> HttpResponse {
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
pub(super) async fn get_hivemind(state: web::Data<NodeState>) -> HttpResponse {
    let Some(soul_db) = &state.soul_db else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({"error": "no soul"}));
    };
    let hive = x402_soul::hivemind::load_hivemind(soul_db);
    HttpResponse::Ok().json(serde_json::json!({
        "trails": hive.export_trails(50),
        "peer_activities": hive.peer_activities,
    }))
}

/// GET /soul/colony — colony selection status: rank, can_spawn, should_cull, niche
pub(super) async fn get_colony_status(state: web::Data<NodeState>) -> HttpResponse {
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
