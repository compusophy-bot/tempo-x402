//! Bootstrap an embedded facilitator instance.
//!
//! Used by both the gateway and node binaries to initialize an in-process
//! facilitator without running a separate HTTP server.

use std::sync::Arc;

use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

use crate::state::AppState;
use crate::webhook;

/// Configuration for bootstrapping an embedded facilitator.
pub struct BootstrapConfig<'a> {
    /// The facilitator's private key (hex-encoded).
    pub private_key: &'a str,
    /// RPC URL for the Tempo chain.
    pub rpc_url: &'a str,
    /// Path to the SQLite nonce database.
    pub nonce_db_path: &'a str,
    /// HMAC shared secret (required for embedded facilitator).
    pub hmac_secret: Vec<u8>,
    /// Webhook URLs for settlement notifications.
    pub webhook_urls: Vec<String>,
    /// Metrics bearer token (as raw bytes).
    pub metrics_token: Option<Vec<u8>>,
}

/// Bootstrap an embedded facilitator instance.
///
/// Parses the private key, opens the SQLite nonce store (refuses to start with
/// in-memory fallback), validates webhook URLs, derives the webhook HMAC key,
/// and constructs the shared [`AppState`].
///
/// # Panics
///
/// Calls `std::process::exit(1)` if the SQLite nonce store cannot be opened
/// (in-memory fallback is a security risk) or if webhook URLs are invalid.
pub fn bootstrap_embedded_facilitator(config: BootstrapConfig<'_>) -> Arc<AppState> {
    tracing::info!("Embedded facilitator: bootstrapping in-process");

    let signer: PrivateKeySigner = config
        .private_key
        .parse()
        .expect("invalid FACILITATOR_PRIVATE_KEY");
    let facilitator_address = signer.address();

    let provider = ProviderBuilder::new()
        .wallet(alloy::network::EthereumWallet::from(signer))
        .connect_http(config.rpc_url.parse().expect("invalid RPC_URL"));

    // Set up nonce storage — SQLite is mandatory for replay protection
    let nonce_store: Arc<dyn x402::nonce_store::NonceStore> =
        match x402::nonce_store::SqliteNonceStore::open(config.nonce_db_path) {
            Ok(store) => {
                tracing::info!("Nonce store: SQLite at {}", config.nonce_db_path);
                Arc::new(store)
            }
            Err(e) => {
                // CRITICAL: Do not fall back to in-memory. In-memory nonces are lost
                // on restart, enabling replay of any recently-settled payment.
                tracing::error!(
                    "Failed to open SQLite nonce store at {}: {}",
                    config.nonce_db_path,
                    e
                );
                tracing::error!(
                    "Refusing to start — in-memory fallback would enable replay attacks on restart"
                );
                std::process::exit(1);
            }
        };

    let facilitator =
        x402::scheme_facilitator::TempoSchemeFacilitator::new(provider, facilitator_address)
            .with_nonce_store(nonce_store);

    facilitator.start_nonce_cleanup();

    tracing::info!("Embedded facilitator address: {facilitator_address}");

    if !config.webhook_urls.is_empty() {
        tracing::info!("Webhook URLs configured: {}", config.webhook_urls.len());
        if let Err(e) = webhook::validate_webhook_urls(&config.webhook_urls) {
            tracing::error!("Invalid webhook configuration: {e}");
            std::process::exit(1);
        }
    }

    // Derive domain-separated webhook HMAC key
    let webhook_hmac_key =
        Some(x402::hmac::compute_hmac(&config.hmac_secret, b"x402-webhook-hmac").into_bytes());

    Arc::new(AppState {
        facilitator,
        hmac_secret: config.hmac_secret,
        chain_config: x402::constants::ChainConfig::default(),
        webhook_urls: config.webhook_urls,
        http_client: webhook::webhook_client(),
        metrics_token: config.metrics_token,
        webhook_hmac_key,
    })
}
