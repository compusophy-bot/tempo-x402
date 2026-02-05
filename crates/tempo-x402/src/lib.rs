//! x402 payment protocol for the Tempo blockchain.
//!
//! Implements HTTP 402 pay-per-request using EIP-712 signed authorizations
//! and TIP-20 (ERC-20 compatible) token transfers on the Tempo chain.
//!
//! # Three-party model
//!
//! - **Client** ([`TempoSchemeClient`]) — signs payment authorizations
//! - **Server** ([`TempoSchemeServer`]) — gates endpoints, returns 402 with pricing
//! - **Facilitator** ([`TempoSchemeFacilitator`]) — verifies signatures and settles on-chain
//!
//! # Quick example (client)
//!
//! ```no_run
//! use alloy::signers::local::PrivateKeySigner;
//! use x402::{TempoSchemeClient, X402Client};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let signer: PrivateKeySigner = "0xYOUR_KEY".parse().unwrap();
//! let client = X402Client::new(TempoSchemeClient::new(signer));
//!
//! let (resp, settlement) = client
//!     .fetch("https://api.example.com/data", reqwest::Method::GET)
//!     .await
//!     .unwrap();
//! # }
//! ```

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

// EIP-712 struct for payment authorizations.
// The sol! macro derives SolStruct which provides eip712_signing_hash().
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

// TIP-20 (ERC-20 compatible) contract interface for on-chain token operations.
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
