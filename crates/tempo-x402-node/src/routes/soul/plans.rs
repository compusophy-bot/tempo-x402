//! Plan approval endpoints — approve, reject, and query pending plans.

use super::*;

#[derive(Deserialize)]
pub(super) struct PlanApproveRequest {
    plan_id: String,
}

#[derive(Deserialize)]
pub(super) struct PlanRejectRequest {
    plan_id: String,
    #[serde(default)]
    reason: Option<String>,
}

pub(super) async fn plan_approve(
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

pub(super) async fn plan_reject(
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

pub(super) async fn plan_pending(state: web::Data<NodeState>) -> HttpResponse {
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
