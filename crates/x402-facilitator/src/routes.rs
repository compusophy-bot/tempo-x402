use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use x402::{PaymentPayload, PaymentRequirements, SchemeFacilitator};

use crate::metrics;
use crate::state::AppState;
use crate::webhook;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequest {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

/// Validate the HMAC header on an incoming request.
/// Returns an error response if HMAC is required but missing/invalid.
fn validate_hmac(req: &HttpRequest, body_bytes: &[u8], state: &AppState) -> Result<(), HttpResponse> {
    let secret = match &state.hmac_secret {
        Some(s) => s,
        None => return Ok(()), // No secret configured — skip HMAC (dev mode)
    };

    let header_value = req
        .headers()
        .get("X-Facilitator-Auth")
        .and_then(|v| v.to_str().ok());

    match header_value {
        Some(sig) => {
            if x402::hmac::verify_hmac(secret, body_bytes, sig) {
                Ok(())
            } else {
                tracing::warn!("HMAC verification failed — signature mismatch");
                metrics::HMAC_FAILURES.with_label_values(&["invalid"]).inc();
                Err(HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": "authentication failed"
                })))
            }
        }
        None => {
            tracing::warn!("HMAC header missing on authenticated endpoint");
            metrics::HMAC_FAILURES.with_label_values(&["missing"]).inc();
            Err(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "authentication required"
            })))
        }
    }
}

#[get("/health")]
pub async fn health(state: web::Data<AppState>) -> HttpResponse {
    match state.facilitator.health_check().await {
        Ok(block) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "service": "x402-facilitator",
            "latestBlock": block.to_string(),
        })),
        Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "degraded",
            "service": "x402-facilitator",
            "error": "RPC unreachable",
        })),
    }
}

#[get("/metrics")]
pub async fn metrics_endpoint() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(metrics::metrics_output())
}

#[get("/supported")]
pub async fn supported(state: web::Data<AppState>) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "schemes": [&state.chain_config.scheme_name],
        "networks": [&state.chain_config.network],
    }))
}

#[post("/verify")]
pub async fn verify(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    if let Err(resp) = validate_hmac(&req, &body, &state) {
        return resp;
    }

    let parsed: PaymentRequest = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "isValid": false,
                "invalidReason": "invalid request body"
            }));
        }
    };

    match state.facilitator.verify(&parsed.payment_payload, &parsed.payment_requirements).await {
        Ok(result) => {
            if result.is_valid {
                metrics::VERIFY_REQUESTS.with_label_values(&["valid"]).inc();
            } else {
                metrics::VERIFY_REQUESTS.with_label_values(&["invalid"]).inc();
                tracing::info!(
                    payer = ?result.payer,
                    reason = result.invalid_reason.as_deref().unwrap_or("unknown"),
                    "verification rejected"
                );
            }
            HttpResponse::Ok().json(result)
        }
        Err(e) => {
            metrics::VERIFY_REQUESTS.with_label_values(&["error"]).inc();
            tracing::error!(error = %e, "verification internal error");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "isValid": false,
                "invalidReason": "verification failed",
            }))
        }
    }
}

#[post("/verify-and-settle")]
pub async fn verify_and_settle(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    if let Err(resp) = validate_hmac(&req, &body, &state) {
        return resp;
    }

    let parsed: PaymentRequest = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "errorReason": "invalid request body",
                "transaction": "",
                "network": &state.chain_config.network,
            }));
        }
    };

    let start = std::time::Instant::now();

    match state.facilitator.settle(&parsed.payment_payload, &parsed.payment_requirements).await {
        Ok(result) => {
            let elapsed = start.elapsed().as_secs_f64();
            if result.success {
                metrics::SETTLE_REQUESTS.with_label_values(&["success"]).inc();
                metrics::SETTLE_LATENCY.with_label_values(&["success"]).observe(elapsed);
                tracing::info!(
                    payer = ?result.payer,
                    tx = %result.transaction,
                    "settlement completed"
                );

                // Fire webhooks
                if !state.webhook_urls.is_empty() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    webhook::fire_webhooks(
                        &state.http_client,
                        &state.webhook_urls,
                        webhook::SettlementWebhook {
                            event: "settlement.success".to_string(),
                            payer: result.payer.map(|a| format!("{a}")).unwrap_or_default(),
                            amount: parsed.payment_payload.payload.value.clone(),
                            transaction: result.transaction.clone(),
                            network: result.network.clone(),
                            timestamp: now,
                        },
                    );
                }
            } else {
                metrics::SETTLE_REQUESTS.with_label_values(&["rejected"]).inc();
                metrics::SETTLE_LATENCY.with_label_values(&["rejected"]).observe(elapsed);
                tracing::warn!(
                    payer = ?result.payer,
                    reason = result.error_reason.as_deref().unwrap_or("unknown"),
                    "settlement rejected"
                );
            }
            HttpResponse::Ok().json(result)
        }
        Err(e) => {
            let elapsed = start.elapsed().as_secs_f64();
            metrics::SETTLE_REQUESTS.with_label_values(&["error"]).inc();
            metrics::SETTLE_LATENCY.with_label_values(&["error"]).observe(elapsed);
            tracing::error!(error = %e, "settlement internal error");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "errorReason": "settlement failed",
                "transaction": "",
                "network": &state.chain_config.network,
            }))
        }
    }
}
