//! API utilities for making paid requests and gateway interactions

#![allow(dead_code)]

use crate::{PaymentRequirements, SettleResponse};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

// Gateway and facilitator URLs
const GATEWAY_URL: &str = "https://x402-gateway-production-5018.up.railway.app";

/// 402 Payment Required response body
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentRequiredBody {
    pub x402_version: u32,
    pub accepts: Vec<PaymentRequirements>,
    pub error: String,
}

/// Payment payload to send in PAYMENT-SIGNATURE header
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentPayload {
    pub x402_version: u32,
    pub payload: PaymentData,
}

/// Payment data for EIP-712 signing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentData {
    pub from: String,
    pub to: String,
    pub value: String,
    pub token: String,
    pub valid_after: u64,
    pub valid_before: u64,
    pub nonce: String,
    pub signature: String,
}

/// Make a paid request to the gateway demo endpoint
pub async fn make_paid_request() -> Result<(String, Option<SettleResponse>), String> {
    // For demo, we'll call the gateway's health endpoint which is free
    // In a real app, you'd call a registered endpoint that requires payment

    let resp = Request::get(&format!("{}/health", GATEWAY_URL))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read body: {}", e))?;

    // Parse settlement info from header if present
    let settle = resp
        .headers()
        .get("payment-response")
        .and_then(|s| serde_json::from_str::<SettleResponse>(&s).ok());

    Ok((body, settle))
}

/// Register an endpoint on the gateway
pub async fn register_endpoint(
    slug: &str,
    target_url: &str,
    price: &str,
) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "slug": slug,
        "target_url": target_url,
        "price": price
    });

    let resp = Request::post(&format!("{}/register", GATEWAY_URL))
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap())
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if resp.status() == 402 {
        // Would need to handle payment here
        return Err("Payment required - connect wallet to pay registration fee".to_string());
    }

    if !resp.ok() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Registration failed: {}", err));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// List all registered endpoints
pub async fn list_endpoints() -> Result<Vec<serde_json::Value>, String> {
    let resp = Request::get(&format!("{}/endpoints", GATEWAY_URL))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let endpoints = data["endpoints"].as_array().cloned().unwrap_or_default();

    Ok(endpoints)
}

/// Get details of a specific endpoint
pub async fn get_endpoint(slug: &str) -> Result<serde_json::Value, String> {
    let resp = Request::get(&format!("{}/endpoints/{}", GATEWAY_URL, slug))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if resp.status() == 404 {
        return Err("Endpoint not found".to_string());
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Call a proxied endpoint through the gateway
pub async fn call_endpoint(
    slug: &str,
    path: &str,
) -> Result<(String, Option<SettleResponse>), String> {
    let url = if path.is_empty() {
        format!("{}/g/{}", GATEWAY_URL, slug)
    } else {
        format!("{}/g/{}/{}", GATEWAY_URL, slug, path)
    };

    let resp = Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if resp.status() == 402 {
        // Would need to sign and retry with payment
        return Err("Payment required - implement payment flow".to_string());
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read body: {}", e))?;

    let settle = resp
        .headers()
        .get("payment-response")
        .and_then(|s| serde_json::from_str::<SettleResponse>(&s).ok());

    Ok((body, settle))
}

/// Generate random nonce (32 bytes as hex string)
fn random_nonce() -> String {
    use js_sys::Math;
    let mut bytes = [0u8; 32];
    for byte in bytes.iter_mut() {
        *byte = (Math::random() * 256.0) as u8;
    }
    format!("0x{}", hex::encode(&bytes))
}

/// Simple hex encoding
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
