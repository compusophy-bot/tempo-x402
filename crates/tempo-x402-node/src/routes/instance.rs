use actix_web::{web, HttpResponse};

use crate::db;
use crate::state::NodeState;

/// GET /instance/info — returns identity, children, version, uptime, clone availability
pub async fn info(state: web::Data<NodeState>) -> HttpResponse {
    let identity_info = state.identity.as_ref().map(|id| {
        serde_json::json!({
            "address": format!("{:#x}", id.address),
            "instance_id": id.instance_id,
            "parent_url": id.parent_url,
            "parent_address": id.parent_address.map(|a| format!("{:#x}", a)),
            "created_at": id.created_at.to_rfc3339(),
        })
    });

    // Open a read connection to query children
    let children = rusqlite::Connection::open(&state.db_path)
        .ok()
        .and_then(|conn| db::query_children(&conn).ok())
        .unwrap_or_default();

    let uptime_secs = (chrono::Utc::now() - state.started_at).num_seconds();

    let clone_available = state.agent.is_some()
        && state.clone_price.is_some()
        && (children.len() as u32) < state.clone_max_children;

    HttpResponse::Ok().json(serde_json::json!({
        "identity": identity_info,
        "children": children,
        "children_count": children.len(),
        "clone_available": clone_available,
        "clone_price": state.clone_price,
        "clone_max_children": state.clone_max_children,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": uptime_secs,
    }))
}

/// POST /instance/register — child callback, updates children table
pub async fn register(
    body: web::Json<serde_json::Value>,
    state: web::Data<NodeState>,
) -> HttpResponse {
    let instance_id = match body.get("instance_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "missing instance_id"
            }));
        }
    };

    let address = body.get("address").and_then(|v| v.as_str()).unwrap_or("");
    let url = body.get("url").and_then(|v| v.as_str());

    match db::update_child(
        &state.gateway.db,
        instance_id,
        Some(address),
        url,
        Some("running"),
    ) {
        Ok(()) => {
            tracing::info!(
                instance_id = %instance_id,
                address = %address,
                "Child instance registered"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "registered",
            }))
        }
        Err(e) => {
            tracing::warn!(
                instance_id = %instance_id,
                error = %e,
                "Failed to update child record"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "acknowledged",
            }))
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/instance")
            .route("/info", web::get().to(info))
            .route("/register", web::post().to(register)),
    );
}
