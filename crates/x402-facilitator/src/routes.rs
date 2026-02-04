use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use x402_types::{PaymentPayload, PaymentRequirements, SchemeFacilitator, SCHEME_NAME, TEMPO_NETWORK};

use crate::state::AppState;

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
            if x402_types::hmac::verify_hmac(secret, body_bytes, sig) {
                Ok(())
            } else {
                tracing::warn!("HMAC verification failed — signature mismatch");
                Err(HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": "authentication failed"
                })))
            }
        }
        None => {
            tracing::warn!("HMAC header missing on authenticated endpoint");
            Err(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "authentication required"
            })))
        }
    }
}

#[get("/supported")]
pub async fn supported() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "schemes": [SCHEME_NAME],
        "networks": [TEMPO_NETWORK],
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
            if !result.is_valid {
                tracing::info!(
                    payer = ?result.payer,
                    reason = result.invalid_reason.as_deref().unwrap_or("unknown"),
                    "verification rejected"
                );
            }
            HttpResponse::Ok().json(result)
        }
        Err(e) => {
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
                "network": TEMPO_NETWORK,
            }));
        }
    };

    match state.facilitator.settle(&parsed.payment_payload, &parsed.payment_requirements).await {
        Ok(result) => {
            if result.success {
                tracing::info!(
                    payer = ?result.payer,
                    tx = %result.transaction,
                    "settlement completed"
                );
            } else {
                tracing::warn!(
                    payer = ?result.payer,
                    reason = result.error_reason.as_deref().unwrap_or("unknown"),
                    "settlement rejected"
                );
            }
            HttpResponse::Ok().json(result)
        }
        Err(e) => {
            tracing::error!(error = %e, "settlement internal error");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "errorReason": "settlement failed",
                "transaction": "",
                "network": TEMPO_NETWORK,
            }))
        }
    }
}
