use actix_web::{HttpRequest, HttpResponse};
use alloy::primitives::Address;
use x402::{
    hmac::compute_hmac, PaymentPayload, PaymentRequiredBody, PaymentRequirements,
    SchemeFacilitator, SettleResponse, DEFAULT_TOKEN, SCHEME_NAME, TEMPO_NETWORK,
};
use x402_facilitator::state::AppState as FacilitatorState;

use crate::error::GatewayError;

const X402_VERSION: u32 = 1;

/// Build PaymentRequirements for the platform registration fee
pub fn platform_requirements(
    platform_address: Address,
    fee: &str,
    fee_amount: &str,
) -> PaymentRequirements {
    PaymentRequirements {
        scheme: SCHEME_NAME.to_string(),
        network: TEMPO_NETWORK.to_string(),
        price: fee.to_string(),
        asset: DEFAULT_TOKEN,
        amount: fee_amount.to_string(),
        pay_to: platform_address,
        max_timeout_seconds: 30,
        description: Some("Platform registration fee".to_string()),
        mime_type: Some("application/json".to_string()),
    }
}

/// Build PaymentRequirements for an endpoint proxy request
pub fn endpoint_requirements(
    owner_address: Address,
    price_usd: &str,
    price_amount: &str,
    description: Option<&str>,
) -> PaymentRequirements {
    PaymentRequirements {
        scheme: SCHEME_NAME.to_string(),
        network: TEMPO_NETWORK.to_string(),
        price: price_usd.to_string(),
        asset: DEFAULT_TOKEN,
        amount: price_amount.to_string(),
        pay_to: owner_address,
        max_timeout_seconds: 30,
        description: description.map(String::from),
        mime_type: Some("application/json".to_string()),
    }
}

/// Build the 402 Payment Required response body
pub fn payment_required_body(requirements: PaymentRequirements) -> PaymentRequiredBody {
    PaymentRequiredBody {
        x402_version: X402_VERSION,
        accepts: vec![requirements],
        description: None,
        mime_type: None,
    }
}

/// Build a 402 Payment Required HTTP response
pub fn payment_required_response(requirements: PaymentRequirements) -> HttpResponse {
    let body = payment_required_body(requirements);
    HttpResponse::PaymentRequired()
        .content_type("application/json")
        .json(body)
}

/// Extract and decode the PAYMENT-SIGNATURE header
pub fn extract_payment_header(req: &HttpRequest) -> Option<PaymentPayload> {
    let header = req.headers().get("PAYMENT-SIGNATURE")?;
    let header_str = header.to_str().ok()?;

    // Base64 decode
    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, header_str).ok()?;

    // Parse JSON
    serde_json::from_slice(&decoded).ok()
}

/// Extract the payer address from the PAYMENT-SIGNATURE header without settling.
/// Used to verify ownership before committing to payment.
pub fn extract_payer_from_header(req: &HttpRequest) -> Option<Address> {
    let payload = extract_payment_header(req)?;
    Some(payload.payload.from)
}

/// Call the facilitator's /verify-and-settle endpoint.
/// If `facilitator_state` is Some, calls the facilitator in-process (no HTTP).
/// Otherwise falls back to the HTTP path.
pub async fn verify_and_settle(
    http_client: &reqwest::Client,
    facilitator_url: &str,
    hmac_secret: Option<&[u8]>,
    facilitator_state: Option<&FacilitatorState>,
    payload: &PaymentPayload,
    requirements: &PaymentRequirements,
) -> Result<SettleResponse, GatewayError> {
    // In-process path: call facilitator directly
    if let Some(fac) = facilitator_state {
        return fac
            .facilitator
            .settle(payload, requirements)
            .await
            .map_err(|e| GatewayError::PaymentFailed(e.to_string()));
    }

    // HTTP fallback path
    let url = format!(
        "{}/verify-and-settle",
        facilitator_url.trim_end_matches('/')
    );

    let request_body = serde_json::json!({
        "paymentPayload": payload,
        "paymentRequirements": requirements,
    });

    let body_bytes = serde_json::to_vec(&request_body)
        .map_err(|e| GatewayError::Internal(format!("failed to serialize request: {}", e)))?;

    let mut req_builder = http_client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body_bytes.clone());

    // Add HMAC signature if secret is configured
    if let Some(secret) = hmac_secret {
        let signature = compute_hmac(secret, &body_bytes);
        req_builder = req_builder.header("X-Facilitator-Auth", signature);
    }

    let response = req_builder
        .send()
        .await
        .map_err(|e| GatewayError::PaymentFailed(format!("facilitator request failed: {}", e)))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| GatewayError::PaymentFailed(format!("failed to read response: {}", e)))?;

    if !status.is_success() {
        tracing::error!(
            status = %status,
            body = %body,
            "facilitator returned non-success response"
        );
        return Err(GatewayError::PaymentFailed("settlement failed".to_string()));
    }

    let settle_response: SettleResponse = serde_json::from_str(&body)
        .map_err(|e| GatewayError::PaymentFailed(format!("invalid facilitator response: {}", e)))?;

    if !settle_response.success {
        return Err(GatewayError::PaymentFailed(
            settle_response
                .error_reason
                .unwrap_or_else(|| "unknown error".to_string()),
        ));
    }

    Ok(settle_response)
}

/// Process payment for a request - either return 402 or verify and settle
pub async fn require_payment(
    req: &HttpRequest,
    requirements: PaymentRequirements,
    http_client: &reqwest::Client,
    facilitator_url: &str,
    hmac_secret: Option<&[u8]>,
    facilitator_state: Option<&FacilitatorState>,
) -> Result<SettleResponse, HttpResponse> {
    // Check for PAYMENT-SIGNATURE header
    let payload = match extract_payment_header(req) {
        Some(p) => p,
        None => return Err(payment_required_response(requirements)),
    };

    // Verify and settle the payment
    match verify_and_settle(
        http_client,
        facilitator_url,
        hmac_secret,
        facilitator_state,
        &payload,
        &requirements,
    )
    .await
    {
        Ok(settle) => Ok(settle),
        Err(e) => {
            tracing::warn!("Payment verification failed: {}", e);
            Err(HttpResponse::PaymentRequired().json(serde_json::json!({
                "error": "payment_failed",
                "message": e.to_string(),
                "x402_version": X402_VERSION,
                "accepts": [requirements],
            })))
        }
    }
}

/// Build the PAYMENT-RESPONSE header value.
/// If `hmac_secret` is provided, appends an HMAC signature: `base64.hmac_hex`.
/// The HMAC covers context fields (payer, network) to prevent cross-endpoint replay.
pub fn payment_response_header(settle: &SettleResponse, hmac_secret: Option<&[u8]>) -> String {
    let response = serde_json::json!({
        "success": settle.success,
        "transaction": settle.transaction,
        "network": settle.network,
        "payer": settle.payer.map(|a| format!("{:#x}", a)),
    });
    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        response.to_string(),
    );
    if let Some(secret) = hmac_secret {
        let mac = compute_hmac(secret, encoded.as_bytes());
        format!("{}.{}", encoded, mac)
    } else {
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_requirements() {
        let addr: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let req = platform_requirements(addr, "$0.01", "10000");

        assert_eq!(req.scheme, "tempo-tip20");
        assert_eq!(req.network, "eip155:42431");
        assert_eq!(req.price, "$0.01");
        assert_eq!(req.amount, "10000");
        assert_eq!(req.pay_to, addr);
    }

    #[test]
    fn test_endpoint_requirements() {
        let addr: Address = "0xabcdef1234567890abcdef1234567890abcdef12"
            .parse()
            .unwrap();
        let req = endpoint_requirements(addr, "$0.05", "50000", Some("Test API"));

        assert_eq!(req.price, "$0.05");
        assert_eq!(req.amount, "50000");
        assert_eq!(req.pay_to, addr);
        assert_eq!(req.description, Some("Test API".to_string()));
    }
}
