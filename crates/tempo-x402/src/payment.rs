use alloy::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

/// Core payment data that gets signed via EIP-712.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoPaymentData {
    pub from: Address,
    pub to: Address,
    pub value: String,
    pub token: Address,
    pub valid_after: u64,
    pub valid_before: u64,
    pub nonce: FixedBytes<32>,
    pub signature: String,
}

/// Wire-format payment payload (sent in PAYMENT-SIGNATURE header, base64-encoded JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u32,
    pub payload: TempoPaymentData,
}

/// A single entry in the `accepts` array of a 402 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub price: String,
    pub asset: Address,
    pub amount: String,
    pub pay_to: Address,
    pub max_timeout_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// The 402 response body returned by the resource server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequiredBody {
    pub x402_version: u32,
    pub accepts: Vec<PaymentRequirements>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}
