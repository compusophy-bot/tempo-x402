use alloy::primitives::U256;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;

use x402::{
    eip712::{encode_signature_hex, payment_domain_for_chain, random_nonce},
    ChainConfig, PaymentAuthorization, PaymentPayload, PaymentRequirements, SchemeClient,
    TempoPaymentData, X402Error,
};

/// Client-side scheme implementation: creates and signs EIP-712 payment payloads.
///
/// Use this with [`X402Client`](crate::X402Client) to make paid API requests.
pub struct TempoSchemeClient {
    signer: PrivateKeySigner,
    config: ChainConfig,
}

impl TempoSchemeClient {
    /// Create a new client with Tempo Moderato defaults.
    pub fn new(signer: PrivateKeySigner) -> Self {
        Self {
            signer,
            config: ChainConfig::default(),
        }
    }

    /// Create a new client with a custom chain configuration.
    pub fn with_chain_config(signer: PrivateKeySigner, config: ChainConfig) -> Self {
        Self { signer, config }
    }

    /// Get the address of the signer.
    pub fn address(&self) -> alloy::primitives::Address {
        self.signer.address()
    }
}

impl SchemeClient for TempoSchemeClient {
    async fn create_payment_payload(
        &self,
        x402_version: u32,
        requirements: &PaymentRequirements,
    ) -> Result<PaymentPayload, X402Error> {
        let token = requirements.asset;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| X402Error::ConfigError(format!("system time error: {e}")))?
            .as_secs();

        let valid_after = now.saturating_sub(60);
        let valid_before = now + requirements.max_timeout_seconds;

        let nonce = random_nonce();

        let value = requirements
            .amount
            .parse::<U256>()
            .map_err(|e| X402Error::InvalidPayment(format!("invalid amount: {e}")))?;

        let auth = PaymentAuthorization {
            from: self.signer.address(),
            to: requirements.pay_to,
            value,
            token,
            validAfter: U256::from(valid_after),
            validBefore: U256::from(valid_before),
            nonce,
        };

        // Sign the EIP-712 hash
        let domain = payment_domain_for_chain(&self.config, token);
        let signing_hash = alloy::sol_types::SolStruct::eip712_signing_hash(&auth, &domain);
        let sig = self
            .signer
            .sign_hash_sync(&signing_hash)
            .map_err(|e| X402Error::SignatureError(format!("signing failed: {e}")))?;

        let sig_hex = encode_signature_hex(&sig);

        let data = TempoPaymentData {
            from: self.signer.address(),
            to: requirements.pay_to,
            value: requirements.amount.clone(),
            token,
            valid_after,
            valid_before,
            nonce,
            signature: sig_hex,
        };

        Ok(PaymentPayload {
            x402_version,
            payload: data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x402::{DEFAULT_TOKEN, SCHEME_NAME, TEMPO_NETWORK};

    #[tokio::test]
    async fn test_create_payment_payload() {
        let signer = PrivateKeySigner::random();
        let client = TempoSchemeClient::new(signer.clone());

        let requirements = PaymentRequirements {
            scheme: SCHEME_NAME.to_string(),
            network: TEMPO_NETWORK.to_string(),
            price: "$0.001".to_string(),
            asset: DEFAULT_TOKEN,
            amount: "1000".to_string(),
            pay_to: alloy::primitives::Address::ZERO,
            max_timeout_seconds: 30,
            description: None,
            mime_type: None,
        };

        let payload = client
            .create_payment_payload(1, &requirements)
            .await
            .unwrap();

        assert_eq!(payload.x402_version, 1);
        assert_eq!(payload.payload.from, signer.address());
        assert_eq!(payload.payload.value, "1000");
        assert!(payload.payload.signature.starts_with("0x"));
        assert_eq!(payload.payload.signature.len(), 132); // 0x + 130 hex chars
    }

    #[test]
    fn test_address() {
        let signer = PrivateKeySigner::random();
        let client = TempoSchemeClient::new(signer.clone());
        assert_eq!(client.address(), signer.address());
    }
}
