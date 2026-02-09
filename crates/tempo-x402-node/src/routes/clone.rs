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

    // Check children limit
    let current_children = rusqlite::Connection::open(&node.db_path)
        .ok()
        .and_then(|conn| db::query_children_count(&conn).ok())
        .unwrap_or(0);

    if current_children >= node.clone_max_children {
        return Err(GatewayError::Internal(format!(
            "clone limit reached: {}/{}",
            current_children, node.clone_max_children
        )));
    }

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

    // Spawn clone
    let clone_result = agent
        .spawn_clone(&payer_address)
        .await
        .map_err(|e| GatewayError::Internal(format!("clone failed: {e}")))?;

    // Record child in DB
    let _ = db::create_child(
        &node.gateway.db,
        &clone_result.instance_id,
        Some(&clone_result.url),
        Some(&clone_result.railway_service_id),
    );

    tracing::info!(
        instance_id = %clone_result.instance_id,
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
            "instance_id": clone_result.instance_id,
            "url": clone_result.url,
            "status": "deploying",
            "transaction": settle.transaction,
        })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/clone", web::post().to(clone_instance));
}
