use actix_web::{web, HttpRequest, HttpResponse};

use crate::db;
use crate::state::NodeState;
use x402_gateway::error::GatewayError;
use x402_gateway::middleware::{payment_response_header, require_payment};

/// POST /clone — x402-gated clone operation
pub async fn clone_instance(
    req: HttpRequest,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    let agent = node
        .agent
        .as_ref()
        .ok_or_else(|| GatewayError::Internal("cloning not configured".to_string()))?;

    let clone_price = node
        .clone_price
        .as_deref()
        .ok_or_else(|| GatewayError::Internal("clone price not set".to_string()))?;

    let clone_price_amount = node
        .clone_price_amount
        .as_deref()
        .ok_or_else(|| GatewayError::Internal("clone price amount not set".to_string()))?;

    // Build payment requirements
    let requirements = x402::PaymentRequirements {
        scheme: x402::SCHEME_NAME.to_string(),
        network: x402::TEMPO_NETWORK.to_string(),
        price: clone_price.to_string(),
        asset: x402::DEFAULT_TOKEN,
        amount: clone_price_amount.to_string(),
        pay_to: node.gateway.config.platform_address,
        max_timeout_seconds: 60,
        description: Some("Clone instance fee".to_string()),
        mime_type: Some("application/json".to_string()),
    };

    // Early 402 if no payment header
    if x402_gateway::middleware::extract_payment_header(&req).is_none() {
        return Ok(x402_gateway::middleware::payment_required_response(
            requirements,
        ));
    }

    // Verify and settle payment
    let settle = match require_payment(
        &req,
        requirements,
        &node.gateway.http_client,
        &node.gateway.config.facilitator_url,
        node.gateway.config.hmac_secret.as_deref(),
        node.gateway.facilitator.as_deref(),
    )
    .await
    {
        Ok(s) => s,
        Err(http_response) => return Ok(http_response),
    };

    let payer_address = settle
        .payer
        .map(|a| format!("{:#x}", a))
        .unwrap_or_default();

    // Generate instance ID up front so we can reserve the DB slot before Railway calls
    let instance_id = uuid::Uuid::new_v4().to_string();

    // 1. Reserve slot in DB BEFORE any Railway API calls (atomic limit check)
    match db::reserve_child_slot(&node.gateway.db, node.clone_max_children, &instance_id) {
        Ok(true) => {
            tracing::info!(instance_id = %instance_id, "Child slot reserved");
        }
        Ok(false) => {
            return Ok(HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "clone limit reached",
            })));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to reserve child slot in DB");
            return Err(GatewayError::Internal(
                "failed to reserve clone slot".to_string(),
            ));
        }
    }

    // 2. Spawn clone on Railway (with retry + cleanup-on-failure)
    let clone_result = match agent.spawn_clone(&instance_id, &payer_address).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(
                instance_id = %instance_id,
                error = %e,
                "Clone orchestration failed"
            );
            // Mark the reserved slot as failed
            if let Err(db_err) = db::mark_child_failed(&node.gateway.db, &instance_id) {
                tracing::error!(error = %db_err, "Failed to mark child as failed in DB");
            }
            return Err(GatewayError::Internal("clone operation failed".to_string()));
        }
    };

    // 3. Update the reserved slot with Railway details
    if let Err(e) = db::update_child_deployment(
        &node.gateway.db,
        &instance_id,
        &clone_result.url,
        &clone_result.railway_service_id,
        "deploying",
    ) {
        // Railway resources exist but DB update failed — log but still return success
        // since the child is at least tracked from the reservation step
        tracing::error!(
            instance_id = %instance_id,
            error = %e,
            "Failed to update child deployment details in DB"
        );
    }

    tracing::info!(
        instance_id = %instance_id,
        url = %clone_result.url,
        payer = %payer_address,
        "Clone spawned successfully"
    );

    Ok(HttpResponse::Created()
        .insert_header((
            "PAYMENT-RESPONSE",
            payment_response_header(&settle, node.gateway.config.hmac_secret.as_deref()),
        ))
        .json(serde_json::json!({
            "success": true,
            "instance_id": instance_id,
            "url": clone_result.url,
            "status": "deploying",
            "transaction": settle.transaction,
        })))
}

/// GET /clone/{instance_id}/status — check clone deployment status
pub async fn clone_status(
    path: web::Path<String>,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    let instance_id = path.into_inner();

    match db::get_child_by_instance_id(&node.gateway.db, &instance_id) {
        Ok(Some(child)) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "instance_id": child.instance_id,
            "status": child.status,
            "url": child.url,
            "created_at": child.created_at,
        }))),
        Ok(None) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "clone not found",
        }))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to query clone status");
            Err(GatewayError::Internal(
                "failed to query clone status".to_string(),
            ))
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/clone", web::post().to(clone_instance))
        .route("/clone/{instance_id}/status", web::get().to(clone_status));
}
