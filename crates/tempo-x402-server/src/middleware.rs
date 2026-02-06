use actix_web::{HttpRequest, HttpResponse};
use base64::Engine;
use x402::{PaymentPayload, PaymentRequiredBody, PaymentRequirements, SettleResponse};

use crate::config::PaymentConfig;
use crate::metrics::{PAYMENT_ATTEMPTS, REQUESTS};

/// Check if a request is for a payment-gated route and extract the requirements.
pub fn check_payment_gate<'a>(
    req: &HttpRequest,
    config: &'a PaymentConfig,
) -> Option<&'a PaymentRequirements> {
    let method = req.method().as_str();
    let path = req.path();
    config.get_route(method, path).map(|r| &r.requirements)
}

/// Build the 402 Payment Required response body.
pub fn payment_required_body(requirements: &PaymentRequirements) -> PaymentRequiredBody {
    PaymentRequiredBody {
        x402_version: 1,
        accepts: vec![requirements.clone()],
        description: requirements.description.clone(),
        mime_type: requirements.mime_type.clone(),
    }
}

/// Decode the PAYMENT-SIGNATURE header into a PaymentPayload.
pub fn decode_payment_header(header_value: &str) -> Result<PaymentPayload, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(header_value)
        .map_err(|e| format!("invalid base64: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON payload: {e}"))
}

/// Call the facilitator's /verify-and-settle endpoint (atomic verify + settle).
/// Signs the request body with HMAC if a shared secret is configured.
pub async fn call_verify_and_settle(
    client: &reqwest::Client,
    facilitator_url: &str,
    payload: &PaymentPayload,
    requirements: &PaymentRequirements,
    hmac_secret: Option<&[u8]>,
) -> Result<SettleResponse, String> {
    let url = format!("{facilitator_url}/verify-and-settle");
    let body = serde_json::json!({
        "paymentPayload": payload,
        "paymentRequirements": requirements,
    });
    let body_bytes = serde_json::to_vec(&body).map_err(|e| format!("serialization failed: {e}"))?;

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(30));

    if let Some(secret) = hmac_secret {
        let sig = x402::hmac::compute_hmac(secret, &body_bytes);
        request = request.header("X-Facilitator-Auth", sig);
    }

    let resp = request
        .body(body_bytes)
        .send()
        .await
        .map_err(|e| format!("facilitator request failed: {e}"))?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("facilitator authentication failed".to_string());
    }

    resp.json::<SettleResponse>()
        .await
        .map_err(|e| format!("facilitator response parse failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, FixedBytes};
    use x402::TempoPaymentData;

    #[test]
    fn test_decode_valid_header() {
        let payload = PaymentPayload {
            x402_version: 1,
            payload: TempoPaymentData {
                from: Address::ZERO,
                to: Address::ZERO,
                value: "1000".to_string(),
                token: Address::ZERO,
                valid_after: 0,
                valid_before: u64::MAX,
                nonce: FixedBytes::ZERO,
                signature: "0xdead".to_string(),
            },
        };
        let json = serde_json::to_vec(&payload).unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(&json);
        let decoded = decode_payment_header(&encoded).unwrap();
        assert_eq!(decoded.x402_version, 1);
        assert_eq!(decoded.payload.value, "1000");
    }

    #[test]
    fn test_decode_invalid_base64() {
        let result = decode_payment_header("not-valid-base64!!!");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid base64"));
    }

    #[test]
    fn test_decode_invalid_json() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"this is not json");
        let result = decode_payment_header(&encoded);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid JSON"));
    }
}

/// High-level payment gate: checks header, verifies, settles.
/// Returns Ok(SettleResponse) if payment succeeded, or Err(HttpResponse) to return directly.
pub async fn require_payment(
    req: &HttpRequest,
    config: &PaymentConfig,
    http_client: &reqwest::Client,
) -> Result<SettleResponse, HttpResponse> {
    let requirements = match check_payment_gate(req, config) {
        Some(r) => r,
        None => {
            // Not gated — caller should proceed without payment
            return Err(HttpResponse::Ok().finish());
        }
    };

    let payment_header = req
        .headers()
        .get("PAYMENT-SIGNATURE")
        .and_then(|v| v.to_str().ok());

    let payment_header = match payment_header {
        Some(h) => h,
        None => {
            // Use the matched route pattern (not raw path) to prevent cardinality bombs
            let endpoint_label = req.match_pattern().unwrap_or_else(|| "unknown".to_string());
            REQUESTS
                .with_label_values(&[endpoint_label.as_str(), "402"])
                .inc();
            let body = payment_required_body(requirements);
            return Err(HttpResponse::PaymentRequired().json(body));
        }
    };

    let payload = match decode_payment_header(payment_header) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "invalid payment header");
            return Err(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "invalid payment header"
            })));
        }
    };

    tracing::info!(
        payer = %payload.payload.from,
        nonce = %format!("{:.8}", payload.payload.nonce),
        "payment attempt"
    );

    let settle_result = call_verify_and_settle(
        http_client,
        &config.facilitator_url,
        &payload,
        requirements,
        config.hmac_secret.as_deref(),
    )
    .await;

    let endpoint_label = req.match_pattern().unwrap_or_else(|| "unknown".to_string());

    match settle_result {
        Ok(ref s) if s.success => {
            PAYMENT_ATTEMPTS.with_label_values(&["success"]).inc();
            REQUESTS
                .with_label_values(&[endpoint_label.as_str(), "200"])
                .inc();
            Ok(s.clone())
        }
        Ok(s) => {
            PAYMENT_ATTEMPTS.with_label_values(&["rejected"]).inc();
            REQUESTS
                .with_label_values(&[endpoint_label.as_str(), "402"])
                .inc();
            tracing::warn!(
                payer = ?s.payer,
                reason = s.error_reason.as_deref().unwrap_or("unknown"),
                "payment rejected"
            );
            let mut body = payment_required_body(requirements);
            body.description = s.error_reason;
            Err(HttpResponse::PaymentRequired().json(body))
        }
        Err(e) => {
            PAYMENT_ATTEMPTS.with_label_values(&["error"]).inc();
            REQUESTS
                .with_label_values(&[endpoint_label.as_str(), "500"])
                .inc();
            tracing::error!(error = %e, "facilitator communication error");
            Err(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "payment processing failed"
            })))
        }
    }
}
