//! WASM-compatible wallet for x402 payments.
//!
//! Provides key generation, EIP-712 signing, and payment payload construction
//! without pulling in network/transport dependencies (reqwest, tokio, rusqlite).

use alloy::primitives::{Address, FixedBytes, U256};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;
use alloy::sol;
use alloy::sol_types::SolStruct;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

// --- Constants (mirrors core crate) ---

pub const TEMPO_CHAIN_ID: u64 = 42431;
pub const TEMPO_NETWORK: &str = "eip155:42431";
pub const SCHEME_NAME: &str = "tempo-tip20";
pub const EIP712_DOMAIN_NAME: &str = "x402-tempo";
pub const EIP712_DOMAIN_VERSION: &str = "1";
pub const TOKEN_DECIMALS: u32 = 6;
pub const EXPLORER_BASE: &str = "https://explore.moderato.tempo.xyz";

/// pathUSD token address on Tempo Moderato
pub const DEFAULT_TOKEN: Address = Address::new([
    0x20, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
]);

/// Well-known Hardhat Account #0 private key, used for testnet demos only.
///
/// **NOT A SECRET** — this key is publicly documented in Hardhat's source code.
/// Any funds on this address (including testnet pathUSD) can be taken by anyone.
/// Never use this key for real assets.
pub const DEMO_PRIVATE_KEY: &str =
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

// --- EIP-712 structs (duplicated from core to avoid non-WASM deps) ---

sol! {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct PaymentAuthorization {
        address from;
        address to;
        uint256 value;
        address token;
        uint256 validAfter;
        uint256 validBefore;
        bytes32 nonce;
    }
}

// --- Payment types ---

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u32,
    pub payload: PaymentData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub price: String,
    pub asset: String,
    pub amount: String,
    #[serde(rename = "payTo")]
    pub pay_to: String,
    #[serde(rename = "maxTimeoutSeconds")]
    pub max_timeout_seconds: u64,
    pub description: Option<String>,
}

// --- EIP-712 domain ---

fn eip712_domain(token: Address) -> alloy::sol_types::Eip712Domain {
    alloy::sol_types::Eip712Domain {
        name: Some(Cow::Borrowed(EIP712_DOMAIN_NAME)),
        version: Some(Cow::Borrowed(EIP712_DOMAIN_VERSION)),
        chain_id: Some(U256::from(TEMPO_CHAIN_ID)),
        verifying_contract: Some(token),
        salt: None,
    }
}

// --- Nonce generation ---

fn random_nonce_bytes() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    // Use getrandom which works on both native and WASM
    getrandom::fill(&mut bytes).expect("getrandom failed");
    bytes
}

fn random_nonce() -> FixedBytes<32> {
    FixedBytes::from(random_nonce_bytes())
}

fn encode_signature_hex(sig: &alloy::primitives::Signature) -> String {
    let bytes = sig.as_bytes();
    format!(
        "0x{}",
        bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    )
}

// --- Key generation ---

/// Generate a random 32-byte private key and return it as a hex string with 0x prefix.
///
/// Use this to create embedded wallets where you need to store the key for later signing.
pub fn generate_random_key() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("getrandom failed");
    format!(
        "0x{}",
        bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    )
}

// --- WalletSigner ---

/// A local wallet signer for x402 payments.
///
/// Wraps a `PrivateKeySigner` and provides payment-specific signing methods.
/// Works in both native and WASM environments.
pub struct WalletSigner {
    inner: PrivateKeySigner,
}

impl WalletSigner {
    /// Create a signer from a hex-encoded private key (with or without 0x prefix).
    pub fn new(private_key: &str) -> Result<Self, String> {
        let key = private_key.strip_prefix("0x").unwrap_or(private_key);
        let signer: PrivateKeySigner = key
            .parse()
            .map_err(|e| format!("invalid private key: {e}"))?;
        Ok(Self { inner: signer })
    }

    /// Generate a new random keypair.
    pub fn random() -> Self {
        Self {
            inner: PrivateKeySigner::random(),
        }
    }

    /// Get the address of this signer.
    pub fn address(&self) -> Address {
        self.inner.address()
    }

    /// Get the address as a checksummed hex string.
    pub fn address_string(&self) -> String {
        format!("{}", self.inner.address())
    }

    /// Sign a payment authorization and return a base64-encoded PaymentPayload.
    ///
    /// `requirements` should come from a 402 response's `accepts` array.
    /// `now_secs` is the current Unix timestamp in seconds.
    pub fn sign_payment(
        &self,
        requirements: &PaymentRequirements,
        now_secs: u64,
    ) -> Result<String, String> {
        let payload = build_payment_payload(self, requirements, now_secs)?;
        let json = serde_json::to_string(&payload).map_err(|e| format!("serialize failed: {e}"))?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            json,
        ))
    }

    /// Sign a payment and return the PaymentPayload struct (not base64-encoded).
    pub fn sign_payment_payload(
        &self,
        requirements: &PaymentRequirements,
        now_secs: u64,
    ) -> Result<PaymentPayload, String> {
        build_payment_payload(self, requirements, now_secs)
    }
}

/// Build a signed PaymentPayload from wallet signer and payment requirements.
pub fn build_payment_payload(
    signer: &WalletSigner,
    requirements: &PaymentRequirements,
    now_secs: u64,
) -> Result<PaymentPayload, String> {
    // Parse addresses
    let token: Address = requirements
        .asset
        .parse()
        .map_err(|e| format!("invalid asset address: {e}"))?;
    let pay_to: Address = requirements
        .pay_to
        .parse()
        .map_err(|e| format!("invalid payTo address: {e}"))?;

    // Parse amount
    let value: U256 = requirements
        .amount
        .parse()
        .map_err(|e| format!("invalid amount: {e}"))?;

    // Time window: 60s before now to max_timeout_seconds after now
    let valid_after = now_secs.saturating_sub(60);
    let valid_before = now_secs.saturating_add(requirements.max_timeout_seconds);

    let nonce = random_nonce();

    let auth = PaymentAuthorization {
        from: signer.address(),
        to: pay_to,
        value,
        token,
        validAfter: U256::from(valid_after),
        validBefore: U256::from(valid_before),
        nonce,
    };

    // EIP-712 sign
    let domain = eip712_domain(token);
    let signing_hash = auth.eip712_signing_hash(&domain);
    let sig = signer
        .inner
        .sign_hash_sync(&signing_hash)
        .map_err(|e| format!("signing failed: {e}"))?;
    let sig_hex = encode_signature_hex(&sig);

    Ok(PaymentPayload {
        x402_version: 1,
        payload: PaymentData {
            from: format!("{}", signer.address()),
            to: format!("{}", pay_to),
            value: requirements.amount.clone(),
            token: format!("{}", token),
            valid_after,
            valid_before,
            nonce: format!("0x{}", alloy::hex::encode(nonce)),
            signature: sig_hex,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_signer_random() {
        let w = WalletSigner::random();
        assert_ne!(w.address(), Address::ZERO);
    }

    #[test]
    fn test_wallet_signer_from_key() {
        let w = WalletSigner::new(DEMO_PRIVATE_KEY).unwrap();
        assert_ne!(w.address(), Address::ZERO);
        assert_eq!(w.address_string().len(), 42); // 0x + 40 hex
    }

    #[test]
    fn test_sign_payment() {
        let w = WalletSigner::random();
        let req = PaymentRequirements {
            scheme: SCHEME_NAME.to_string(),
            network: TEMPO_NETWORK.to_string(),
            price: "$0.001".to_string(),
            asset: format!("{}", DEFAULT_TOKEN),
            amount: "1000".to_string(),
            pay_to: format!("{}", Address::ZERO),
            max_timeout_seconds: 30,
            description: None,
        };

        let b64 = w.sign_payment(&req, 1700000000).unwrap();
        assert!(!b64.is_empty());

        // Decode and verify structure
        let json = String::from_utf8(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64).unwrap(),
        )
        .unwrap();
        let payload: PaymentPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload.x402_version, 1);
        assert_eq!(payload.payload.value, "1000");
        assert!(payload.payload.signature.starts_with("0x"));
        assert_eq!(payload.payload.signature.len(), 132); // 0x + 130 hex
    }

    /// Cross-validate wallet constants against the core crate to prevent silent EIP-712 mismatches.
    #[test]
    fn test_constants_match_core_crate() {
        assert_eq!(
            TEMPO_CHAIN_ID,
            x402::TEMPO_CHAIN_ID,
            "TEMPO_CHAIN_ID mismatch"
        );
        assert_eq!(TEMPO_NETWORK, x402::TEMPO_NETWORK, "TEMPO_NETWORK mismatch");
        assert_eq!(SCHEME_NAME, x402::SCHEME_NAME, "SCHEME_NAME mismatch");
        assert_eq!(
            TOKEN_DECIMALS,
            x402::TOKEN_DECIMALS,
            "TOKEN_DECIMALS mismatch"
        );
        assert_eq!(
            format!("{}", DEFAULT_TOKEN),
            format!("{}", x402::DEFAULT_TOKEN),
            "DEFAULT_TOKEN mismatch"
        );
        let core_config = x402::ChainConfig::default();
        assert_eq!(
            EIP712_DOMAIN_NAME, core_config.eip712_domain_name,
            "EIP712_DOMAIN_NAME mismatch"
        );
        assert_eq!(
            EIP712_DOMAIN_VERSION, core_config.eip712_domain_version,
            "EIP712_DOMAIN_VERSION mismatch"
        );
    }

    #[test]
    fn test_build_payment_payload() {
        let w = WalletSigner::random();
        let req = PaymentRequirements {
            scheme: SCHEME_NAME.to_string(),
            network: TEMPO_NETWORK.to_string(),
            price: "$0.001".to_string(),
            asset: format!("{}", DEFAULT_TOKEN),
            amount: "1000".to_string(),
            pay_to: format!("{}", Address::ZERO),
            max_timeout_seconds: 30,
            description: None,
        };

        let payload = build_payment_payload(&w, &req, 1700000000).unwrap();
        assert_eq!(payload.payload.from, w.address_string());
        assert_eq!(payload.payload.valid_after, 1700000000 - 60);
        assert_eq!(payload.payload.valid_before, 1700000000 + 30);
    }
}
