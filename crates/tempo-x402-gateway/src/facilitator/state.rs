use alloy::network::EthereumWallet;
use alloy::providers::{
    fillers::{
        BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
    },
    Identity, RootProvider,
};

/// Concrete provider type from `ProviderBuilder::new().wallet(...).connect_http(...)`.
pub type WalletProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
>;

/// Shared application state for the facilitator server.
pub struct AppState {
    pub facilitator: x402::scheme_facilitator::TempoSchemeFacilitator<WalletProvider>,
    /// HMAC shared secret for authenticating /verify-and-settle requests.
    /// This is mandatory — the facilitator will not start without it.
    pub hmac_secret: Vec<u8>,
    pub chain_config: x402::constants::ChainConfig,
    pub webhook_urls: Vec<String>,
    pub http_client: reqwest::Client,
    /// Separate bearer token for /metrics endpoint (not the HMAC secret).
    pub metrics_token: Option<Vec<u8>>,
    /// Derived key for webhook HMAC signing (domain-separated from auth secret).
    pub webhook_hmac_key: Option<Vec<u8>>,
}
