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
    /// Accepted token addresses. Empty = accept any token.
    accepted_tokens: Vec<Address>,
    /// Maximum per-settlement amount (0 = no limit).
    max_settle_amount: U256,
}

impl<P> TempoSchemeFacilitator<P> {
    /// Create a new facilitator with Tempo Moderato defaults and in-memory nonce store.
    ///
    /// # Warning
    /// The default in-memory nonce store loses all nonces on restart, enabling
    /// replay attacks. For production use, chain `.with_nonce_store(sqlite_store)`.
    pub fn new(provider: P, facilitator_address: Address) -> Self {
        Self {
            provider,
            facilitator_address,
            config: ChainConfig::default(),
            nonce_store: Arc::new(InMemoryNonceStore::new()),
            payer_locks: Arc::new(DashMap::new()),
            max_timeout_seconds: 300, // 5 minutes default
            accepted_tokens: vec![],
            max_settle_amount: U256::ZERO,
        }
    }

    /// Create a new facilitator with a custom chain configuration.
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
            accepted_tokens: vec![],
            max_settle_amount: U256::ZERO,
        }
    }

    /// Set a custom nonce store (e.g. SqliteNonceStore for persistence).
    pub fn with_nonce_store(mut self, store: Arc<dyn NonceStore>) -> Self {
        self.nonce_store = store;
        self
    }

    /// Restrict accepted token addresses. When non-empty, payments for tokens
    /// not in this list are rejected.
    pub fn with_accepted_tokens(mut self, tokens: Vec<Address>) -> Self {
        self.accepted_tokens = tokens;
        self
    }

    /// Set a maximum per-settlement amount. Payments exceeding this are rejected.
    /// Set to U256::ZERO (default) to disable the limit.
    pub fn with_max_settle_amount(mut self, max: U256) -> Self {
        self.max_settle_amount = max;
        self
    }

    /// Start a background task that purges expired nonces and stale payer locks every 60 seconds.
    pub fn start_nonce_cleanup(&self)
    where
        P: Send + Sync + 'static,
    {
        let store = Arc::clone(&self.nonce_store);
        let payer_locks = Arc::clone(&self.payer_locks);
        let expiry_secs = self.max_timeout_seconds + 60;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let purged = store.purge_expired(expiry_secs);
                if purged > 0 {
                    tracing::info!(purged, "purged expired nonces");
                }

                // Clean up payer locks that are no longer held by anyone.
                // We check both strong_count (no external Arc clones) AND try_lock
                // (no one currently holding the mutex). This prevents a race where
                // a concurrent payer_lock() clones the Arc between our strong_count
                // check and the retain removal, which would cause two concurrent
                // requests for the same payer to hold different mutexes.
                let before = payer_locks.len();
                payer_locks
                    .retain(|_, lock| Arc::strong_count(lock) > 1 || lock.try_lock().is_err());
                let removed = before - payer_locks.len();
                if removed > 0 {
                    tracing::info!(removed, "cleaned up idle payer locks");
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

    /// Atomically check if nonce is unused and claim it if so.
    /// Returns true if successfully claimed, false if already used.
    #[doc(hidden)]
    pub fn try_use_nonce(&self, nonce: FixedBytes<32>) -> bool {
        self.nonce_store.try_use(nonce)
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

    /// Maximum number of concurrent payer locks to prevent memory exhaustion.
    const MAX_PAYER_LOCKS: usize = 100_000;

    /// Get or create a per-payer mutex for atomic operations.
    /// Note: the len() + contains_key() check is not atomic with entry(), so the cap
    /// can be overshot by up to the number of concurrent worker threads. This is
    /// acceptable since the cleanup task reclaims idle locks periodically.
    fn payer_lock(&self, payer: Address) -> Result<Arc<Mutex<()>>, X402Error> {
        // Prevent unbounded growth of the lock map
        if self.payer_locks.len() >= Self::MAX_PAYER_LOCKS && !self.payer_locks.contains_key(&payer)
        {
            return Err(X402Error::ChainError(
                "too many concurrent payers — try again later".to_string(),
            ));
        }
        Ok(self
            .payer_locks
            .entry(payer)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
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
        // 0a. Validate x402 protocol version
        if payload.x402_version != 1 {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some(format!(
                    "Unsupported x402 version: {} (expected 1)",
                    payload.x402_version
                )),
                payer: None,
            });
        }

        let p = &payload.payload;

        // 0b. Validate scheme and network match this facilitator
        if requirements.scheme != self.config.scheme_name {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some(format!(
                    "Scheme mismatch: expected '{}', got '{}'",
                    self.config.scheme_name, requirements.scheme
                )),
                payer: None,
            });
        }
        if requirements.network != self.config.network {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some(format!(
                    "Network mismatch: expected '{}', got '{}'",
                    self.config.network, requirements.network
                )),
                payer: None,
            });
        }

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
        if now >= p.valid_before {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Authorization expired".to_string()),
                payer: None,
            });
        }

        // Enforce maximum validity window to prevent replay after nonce purge.
        // Use the stricter of the facilitator's global max and the per-endpoint requirement.
        let max_window = self
            .max_timeout_seconds
            .min(requirements.max_timeout_seconds + 60); // +60 for valid_after backdate
        let validity_window = p.valid_before.saturating_sub(p.valid_after);
        if validity_window > max_window {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some(format!(
                    "Validity window too large: {}s exceeds max {}s",
                    validity_window, max_window
                )),
                payer: None,
            });
        }

        // 3. Reject zero addresses (cheap checks before expensive ecrecover)
        if p.from == Address::ZERO {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Payer address cannot be zero".to_string()),
                payer: Some(p.from),
            });
        }
        if p.token == Address::ZERO {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Token address is zero".to_string()),
                payer: None,
            });
        }
        if p.to == Address::ZERO {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Recipient address is zero".to_string()),
                payer: None,
            });
        }
        if p.from == p.to {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Self-payment not allowed".to_string()),
                payer: Some(p.from),
            });
        }
        if p.to == self.facilitator_address {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Recipient cannot be the facilitator".to_string()),
                payer: Some(p.from),
            });
        }

        // 4. Verify EIP-712 signature
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

        // 5. Verify payment details match requirements
        if p.token != requirements.asset {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Token address mismatch".to_string()),
                payer: None,
            });
        }

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

        if value.is_zero() {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Payment value must be non-zero".to_string()),
                payer: Some(p.from),
            });
        }
        if required_amount.is_zero() {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Required amount must be non-zero".to_string()),
                payer: Some(p.from),
            });
        }

        if value < required_amount {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Payment amount below required".to_string()),
                payer: Some(p.from),
            });
        }

        // 5b. Token allowlist check
        if !self.accepted_tokens.is_empty() && !self.accepted_tokens.contains(&p.token) {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some("Token not in facilitator's accepted token list".to_string()),
                payer: Some(p.from),
            });
        }

        // 5c. Per-settlement amount cap
        if !self.max_settle_amount.is_zero() && value > self.max_settle_amount {
            return Ok(VerifyResponse {
                is_valid: false,
                invalid_reason: Some(format!(
                    "Payment amount exceeds maximum per-settlement cap ({})",
                    self.max_settle_amount
                )),
                payer: Some(p.from),
            });
        }

        // 6. Check on-chain balance
        let balance = tip20::balance_of(&self.provider, p.token, p.from).await?;
        if balance < value {
            tracing::info!(
                payer = %p.from,
                balance = %balance,
                required = %value,
                "payment rejected: insufficient balance"
            );
            return Ok(VerifyResponse {
                is_valid: false,
                // Generic message — balance details are in server logs only
                invalid_reason: Some("Payment cannot be completed".to_string()),
                payer: Some(p.from),
            });
        }

        // 7. Check on-chain allowance to facilitator
        let allowance =
            tip20::allowance(&self.provider, p.token, p.from, self.facilitator_address).await?;
        if allowance < value {
            tracing::info!(
                payer = %p.from,
                allowance = %allowance,
                required = %value,
                "payment rejected: insufficient allowance"
            );
            return Ok(VerifyResponse {
                is_valid: false,
                // Generic message — allowance details are in server logs only
                invalid_reason: Some("Payment cannot be completed".to_string()),
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
        let lock = self.payer_lock(p.from)?;
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
                transaction: None,
                network: self.config.network.clone(),
            });
        }

        // Parse value (already validated by verify() above, but needed for transferFrom)
        let value = p
            .value
            .parse::<U256>()
            .map_err(|e| X402Error::InvalidPayment(format!("invalid value: {e}")))?;

        // Atomically claim the nonce BEFORE executing the transfer.
        // This prevents replay attacks even across multiple processes.
        if !self.try_use_nonce(p.nonce) {
            tracing::warn!(
                nonce = %format!("{:.8}", p.nonce),
                payer = %p.from,
                "nonce race: another request claimed it first"
            );
            return Ok(SettleResponse {
                success: false,
                error_reason: Some("Nonce already used (concurrent request)".to_string()),
                payer: Some(p.from),
                transaction: None,
                network: self.config.network.clone(),
            });
        }

        // Execute transferFrom (nonce is now claimed, safe to proceed).
        // IMPORTANT: We do NOT release the nonce on failure. The transaction may
        // have been submitted to the mempool but timed out waiting for confirmation.
        // Releasing the nonce would allow replay if the tx eventually mines.
        // The payer must sign a new authorization with a fresh nonce to retry.
        let tx_hash = match tip20::transfer_from(&self.provider, p.token, p.from, p.to, value).await
        {
            Ok(hash) => hash,
            Err(e) => {
                tracing::error!(
                    nonce = %format!("{:.8}", p.nonce),
                    payer = %p.from,
                    error = %e,
                    "transferFrom failed — nonce remains claimed to prevent double-spend"
                );
                return Err(e);
            }
        };

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
            transaction: Some(format!("{tx_hash}")),
            network: self.config.network.clone(),
        })
    }
}
