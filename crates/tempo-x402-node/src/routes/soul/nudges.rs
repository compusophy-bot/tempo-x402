//! Nudge and goal abandonment endpoints.

use super::*;

#[derive(Deserialize)]
pub(super) struct NudgeRequest {
    message: String,
    priority: Option<u32>,
}

pub(super) async fn soul_nudge(
    state: web::Data<NodeState>,
    body: web::Json<NudgeRequest>,
) -> HttpResponse {
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

pub(super) async fn soul_nudges(state: web::Data<NodeState>) -> HttpResponse {
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
pub(super) async fn abandon_all_goals(state: web::Data<NodeState>) -> HttpResponse {
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
pub(super) struct AbandonGoalRequest {
    goal_id: String,
}

pub(super) async fn abandon_goal(
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
