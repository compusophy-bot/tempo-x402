use std::sync::Arc;

use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::Provider;
use dashmap::DashMap;
use tokio::sync::Mutex;

use crate::{
    ChainConfig, PaymentPayload, PaymentRequirements, SchemeFacilitator, SettleResponse,
    VerifyResponse, X402Error,
};

use crate::eip712::verify_signature_for_chain;
use crate::nonce_store::{InMemoryNonceStore, NonceStore};
use crate::tip20;
use crate::PaymentAuthorization;

/// Facilitator-side scheme implementation: verifies signatures and settles on-chain.
pub struct TempoSchemeFacilitator<P> {
    provider: P,
    facilitator_address: Address,
    config: ChainConfig,
    /// Pluggable nonce store for replay protection.
    nonce_store: Arc<dyn NonceStore>,
    /// Per-payer mutex for atomic verify+settle (prevents TOCTOU).
    payer_locks: Arc<DashMap<Address, Arc<Mutex<()>>>>,
    /// Maximum payment timeout in seconds (used for nonce expiry).
    max_timeout_seconds: u64,
}

impl<P> TempoSchemeFacilitator<P> {
    pub fn new(provider: P, facilitator_address: Address) -> Self {
        Self {
            provider,
            facilitator_address,
            config: ChainConfig::default(),
            nonce_store: Arc::new(InMemoryNonceStore::new()),
            payer_locks: Arc::new(DashMap::new()),
            max_timeout_seconds: 300, // 5 minutes default
        }
    }

    pub fn with_chain_config(
        provider: P,
        facilitator_address: Address,
        config: ChainConfig,
    ) -> Self {
        Self {
            provider,
            facilitator_address,
            config,
            nonce_store: Arc::new(InMemoryNonceStore::new()),
            payer_locks: Arc::new(DashMap::new()),
            max_timeout_seconds: 300,
        }
    }

    /// Set a custom nonce store (e.g. SqliteNonceStore for persistence).
    pub fn with_nonce_store(mut self, store: Arc<dyn NonceStore>) -> Self {
        self.nonce_store = store;
        self
    }

    /// Start a background task that purges expired nonces every 60 seconds.
    pub fn start_nonce_cleanup(&self)
    where
        P: Send + Sync + 'static,
    {
        let store = Arc::clone(&self.nonce_store);
        let expiry_secs = self.max_timeout_seconds + 60;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let purged = store.purge_expired(expiry_secs);
                if purged > 0 {
                    tracing::info!(purged, "purged expired nonces");
                }
            }
        });
    }

    /// Check if a nonce has already been used.
    #[doc(hidden)]
    pub fn is_nonce_used(&self, nonce: &FixedBytes<32>) -> bool {
        self.nonce_store.is_used(nonce)
    }

    /// Record a nonce as used.
    #[doc(hidden)]
    pub fn record_nonce(&self, nonce: FixedBytes<32>) {
        self.nonce_store.record(nonce);
    }

    /// Check RPC connectivity by fetching the latest block number.
    pub async fn health_check(&self) -> Result<u64, X402Error>
    where
        P: Provider + Send + Sync,
    {
        self.provider
            .get_block_number()
            .await
            .map_err(|e| X402Error::ChainError(format!("health check failed: {e}")))
    }

    /// Get or create a per-payer mutex for atomic operations.
    fn payer_lock(&self, payer: Address) -> Arc<Mutex<()>> {
        self.payer_locks
            .entry(payer)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

impl<P> SchemeFacilitator for TempoSchemeFacilitator<P>
where
    P: Provider + Send + Sync,
{
    async fn verify(
        &self,
        payload: &PaymentPayload,
        requirements: &PaymentRequirements,
    ) -> Result<VerifyResponse, X402Error> {
        let p = &payload.payload;

        // 1. Check nonce replay
        if self.is_nonce_used(&p.nonce) {
            tracing::warn!(
                nonce = %format!("{:.8}", p.nonce),
                payer = %p.from,
                "replayed nonce rejected"
            );
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Nonce already used".to_string()),
                payer: Some(p.from),
            });
        }

        // 2. Check time window
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| X402Error::ConfigError(format!("system time error: {e}")))?
            .as_secs();

        if now < p.valid_after {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Authorization not yet valid".to_string()),
                payer: None,
            });
        }
        if now > p.valid_before {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Authorization expired".to_string()),
                payer: None,
            });
        }

        // 3. Verify EIP-712 signature
        let value = p
            .value
            .parse::<U256>()
            .map_err(|e| X402Error::InvalidPayment(format!("invalid value: {e}")))?;

        let auth = PaymentAuthorization {
            from: p.from,
            to: p.to,
            value,
            token: p.token,
            validAfter: U256::from(p.valid_after),
            validBefore: U256::from(p.valid_before),
            nonce: p.nonce,
        };

        let sig_bytes = alloy::hex::decode(p.signature.strip_prefix("0x").unwrap_or(&p.signature))
            .map_err(|e| X402Error::SignatureError(format!("invalid hex signature: {e}")))?;

        let recovered = verify_signature_for_chain(&auth, &sig_bytes, &self.config)?;
        if recovered != p.from {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Invalid signature".to_string()),
                payer: None,
            });
        }

        // 4. Verify payment details match requirements
        if p.to != requirements.pay_to {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Recipient mismatch".to_string()),
                payer: None,
            });
        }

        let required_amount = requirements
            .amount
            .parse::<U256>()
            .map_err(|e| X402Error::InvalidPayment(format!("invalid required amount: {e}")))?;
        if value < required_amount {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Payment amount below required".to_string()),
                payer: Some(p.from),
            });
        }

        // 5. Check on-chain balance
        let balance = tip20::balance_of(&self.provider, p.token, p.from).await?;
        if balance < value {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Insufficient balance".to_string()),
                payer: Some(p.from),
            });
        }

        // 6. Check on-chain allowance to facilitator
        let allowance =
            tip20::allowance(&self.provider, p.token, p.from, self.facilitator_address).await?;
        if allowance < value {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Insufficient allowance -- run approve first".to_string()),
                payer: Some(p.from),
            });
        }

        tracing::info!(
            payer = %p.from,
            amount = %value,
            nonce = %format!("{:.8}", p.nonce),
            "payment verification succeeded"
        );

        Ok(VerifyResponse {
            is_valid: true,
            invalid_reason: None,
            payer: Some(p.from),
        })
    }

    async fn settle(
        &self,
        payload: &PaymentPayload,
        requirements: &PaymentRequirements,
    ) -> Result<SettleResponse, X402Error> {
        let p = &payload.payload;

        // Acquire per-payer lock to prevent TOCTOU
        let lock = self.payer_lock(p.from);
        let _guard = lock.lock().await;

        // Re-verify before settling (under the lock)
        let check = self.verify(payload, requirements).await?;
        if !check.is_valid {
            tracing::warn!(
                payer = %p.from,
                reason = check.invalid_reason.as_deref().unwrap_or("unknown"),
                "settlement rejected after re-verification"
            );
            return Ok(SettleResponse {
                success: false,
                error_reason: check.invalid_reason,
                payer: check.payer,
                transaction: String::new(),
                network: self.config.network.clone(),
            });
        }

        let value = p
            .value
            .parse::<U256>()
            .map_err(|e| X402Error::InvalidPayment(format!("invalid value: {e}")))?;

        // Execute transferFrom
        let tx_hash = tip20::transfer_from(&self.provider, p.token, p.from, p.to, value).await?;

        // Record nonce AFTER successful settlement
        self.record_nonce(p.nonce);

        tracing::info!(
            payer = %p.from,
            amount = %value,
            nonce = %format!("{:.8}", p.nonce),
            tx = %tx_hash,
            "payment settled successfully"
        );

        Ok(SettleResponse {
            success: true,
            error_reason: None,
            payer: Some(p.from),
            transaction: format!("{tx_hash}"),
            network: self.config.network.clone(),
        })
    }
}
