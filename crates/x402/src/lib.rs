// Core types and traits
pub mod constants;
pub mod error;
pub mod hmac;
pub mod payment;
pub mod response;
pub mod scheme;

// Tempo blockchain implementation
pub mod eip712;
pub mod nonce_store;
pub mod scheme_client;
pub mod scheme_facilitator;
pub mod scheme_server;
pub mod tip20;

// HTTP client
pub mod http_client;

use alloy::sol;

// EIP-712 struct for payment authorizations -- the sol! macro auto-derives
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

// Re-exports
pub use constants::ChainConfig;
pub use constants::*;
pub use error::X402Error;
pub use payment::*;
pub use response::*;
pub use scheme::*;

pub use scheme_client::TempoSchemeClient;
pub use scheme_facilitator::TempoSchemeFacilitator;
pub use scheme_server::TempoSchemeServer;

pub use http_client::{encode_payment, X402Client};
