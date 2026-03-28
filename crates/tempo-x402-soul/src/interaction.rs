//! Module-to-module communication layer.
//!
//! Facilitates interactions between agents, handling payment protocols (x402),
//! request signing, and response validation.

use serde::{Deserialize, Serialize};

/// Represents an interaction request to another agent/module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionRequest {
    pub target: String,
    pub endpoint: String,
    pub payload: serde_json::Value,
    pub signature: Option<String>,
}

/// Represents the response from an interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub payment_required: Option<bool>,
    pub payment_url: Option<String>,
}

/// Core function to initiate an interaction with another peer.
pub async fn send_interaction(
    target_url: &str,
    request: InteractionRequest,
) -> Result<InteractionResponse, String> {
    // 1. Construct the client
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("failed to build client: {e}"))?;

    // 2. Perform the POST request
    let response = client
        .post(target_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("interaction request failed: {e}"))?;

    // 3. Handle status
    if !response.status().is_success() {
        return Ok(InteractionResponse {
            success: false,
            data: None,
            error: Some(format!("Request failed with status: {}", response.status())),
            payment_required: None,
            payment_url: None,
        });
    }

    // 4. Parse the response
    let response_data = response
        .json::<InteractionResponse>()
        .await
        .map_err(|e| format!("failed to parse response: {e}"))?;

    Ok(response_data)
}
