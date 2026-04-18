//! HTTP client for calling a remote facilitator's `/verify-and-settle` endpoint.
//!
//! Used by both the resource server and gateway when the facilitator runs
//! as a separate process (not embedded in-process).

use crate::payment::{PaymentPayload, PaymentRequirements};
use crate::response::SettleResponse;

/// Call a remote facilitator's `/verify-and-settle` endpoint.
///
/// Serializes the payment payload and requirements as JSON, optionally signs
/// the request body with HMAC, and returns the parsed [`SettleResponse`].
pub async fn call_verify_and_settle(
    client: &reqwest::Client,
    facilitator_url: &str,
    payload: &PaymentPayload,
    requirements: &PaymentRequirements,
    hmac_secret: Option<&[u8]>,
) -> Result<SettleResponse, String> {
    let url = format!(
        "{}/verify-and-settle",
        facilitator_url.trim_end_matches('/')
    );
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
        let sig = crate::hmac::compute_hmac(secret, &body_bytes);
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
