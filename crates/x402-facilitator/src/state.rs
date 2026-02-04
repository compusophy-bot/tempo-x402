use alloy::providers::{
    fillers::{
        BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
        WalletFiller,
    },
    Identity, RootProvider,
};
use alloy::network::EthereumWallet;

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
    pub facilitator: x402::TempoSchemeFacilitator<WalletProvider>,
    pub hmac_secret: Option<Vec<u8>>,
    pub chain_config: x402::ChainConfig,
    pub webhook_urls: Vec<String>,
    pub http_client: reqwest::Client,
}
