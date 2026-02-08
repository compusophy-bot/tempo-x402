use alloy::primitives::Address;

/// Tempo Moderato chain ID.
pub const TEMPO_CHAIN_ID: u64 = 42431;

/// CAIP-2 network identifier for Tempo Moderato.
pub const TEMPO_NETWORK: &str = "eip155:42431";

/// x402 scheme name for TIP-20 payments on Tempo.
pub const SCHEME_NAME: &str = "tempo-tip20";

/// pathUSD token address on Tempo Moderato testnet.
pub const DEFAULT_TOKEN: Address = Address::new([
    0x20, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
]);

/// pathUSD has 6 decimal places.
pub const TOKEN_DECIMALS: u32 = 6;

/// Default RPC endpoint for Tempo Moderato.
pub const RPC_URL: &str = "https://rpc.moderato.tempo.xyz";

/// Block explorer base URL.
pub const EXPLORER_BASE: &str = "https://explore.moderato.tempo.xyz";

/// Runtime chain configuration. Decouples scheme implementations from
/// compile-time constants, enabling multi-chain support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub network: String,
    pub scheme_name: String,
    pub default_token: Address,
    pub token_decimals: u32,
    pub rpc_url: String,
    pub explorer_base: String,
    pub eip712_domain_name: String,
    pub eip712_domain_version: String,
}

impl Default for ChainConfig {
    /// Defaults to Tempo Moderato configuration.
    fn default() -> Self {
        Self {
            chain_id: TEMPO_CHAIN_ID,
            network: TEMPO_NETWORK.to_string(),
            scheme_name: SCHEME_NAME.to_string(),
            default_token: DEFAULT_TOKEN,
            token_decimals: TOKEN_DECIMALS,
            rpc_url: RPC_URL.to_string(),
            explorer_base: EXPLORER_BASE.to_string(),
            eip712_domain_name: "x402-tempo".to_string(),
            eip712_domain_version: "1".to_string(),
        }
    }
}
