//! Soul chat endpoints — interactive conversation with sessions.

use super::*;

#[derive(Deserialize)]
pub(super) struct ChatRequest {
    message: String,
    #[serde(default)]
    session_id: Option<String>,
}

pub(super) async fn soul_chat(
    state: web::Data<NodeState>,
    body: web::Json<ChatRequest>,
) -> HttpResponse {
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

pub(super) async fn chat_sessions(state: web::Data<NodeState>) -> HttpResponse {
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

pub(super) async fn session_messages(
    state: web::Data<NodeState>,
    path: web::Path<String>,
) -> HttpResponse {
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
