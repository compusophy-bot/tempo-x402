use alloy::primitives::Address;

/// Tempo Moderato chain ID.
pub const TEMPO_CHAIN_ID: u64 = 42431;

/// CAIP-2 network identifier for Tempo Moderato.
pub const TEMPO_NETWORK: &str = "eip155:42431";

/// x402 scheme name for TIP-20 payments on Tempo.
pub const SCHEME_NAME: &str = "tempo-tip20";

/// pathUSD token address on Tempo Moderato testnet.
pub const DEFAULT_TOKEN: Address = Address::new([
    0x20, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00,
]);

/// pathUSD has 6 decimal places.
pub const TOKEN_DECIMALS: u32 = 6;

/// Default RPC endpoint for Tempo Moderato.
pub const RPC_URL: &str = "https://rpc.moderato.tempo.xyz";

/// Block explorer base URL.
pub const EXPLORER_BASE: &str = "https://explore.moderato.tempo.xyz";
