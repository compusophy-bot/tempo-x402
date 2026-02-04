pub mod client;
pub mod eip712;
pub mod facilitator;
pub mod nonce_store;
pub mod server;
pub mod tip20;

use alloy::sol;

// EIP-712 struct for payment authorizations — the sol! macro auto-derives
// SolStruct which gives us eip712_signing_hash().
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

// TIP-20 (ERC-20 compatible) contract interface.
sol! {
    #[sol(rpc)]
    interface TIP20 {
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function transferFrom(address from, address to, uint256 value) external returns (bool);
        function approve(address spender, uint256 value) external returns (bool);
    }
}

pub use client::TempoSchemeClient;
pub use facilitator::TempoSchemeFacilitator;
pub use server::TempoSchemeServer;
