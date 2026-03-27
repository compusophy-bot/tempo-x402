//! Cartridge manifest — metadata for a deployed WASM cartridge.

use serde::{Deserialize, Serialize};

/// ABI version. Increment when host function signatures change.
pub const ABI_VERSION: u32 = 1;

/// Metadata for a deployed WASM cartridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeManifest {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_price")]
    pub price_usd: String,
    #[serde(default = "default_amount")]
    pub price_amount: String,
    #[serde(default)]
    pub owner_address: String,
    /// Link to source repo (e.g. GitHub URL).
    #[serde(default)]
    pub source_repo: Option<String>,
    /// SHA-256 hash of the .wasm binary.
    pub wasm_hash: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_version() -> String {
    "0.1.0".to_string()
}
fn default_price() -> String {
    "$0.001".to_string()
}
fn default_amount() -> String {
    "1000".to_string()
}
fn default_active() -> bool {
    true
}

/// Result of executing a cartridge.
#[derive(Debug, Clone, Serialize)]
pub struct CartridgeResult {
    pub status: u16,
    pub body: String,
    pub content_type: String,
    pub duration_ms: u64,
}

/// Request context passed to a cartridge invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// Payment info (if request was paid).
    #[serde(default)]
    pub payment: Option<PaymentContext>,
}

/// Payment context from a settled x402 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentContext {
    pub payer: String,
    pub amount: String,
    pub tx_hash: String,
}
