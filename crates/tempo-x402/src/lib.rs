//! x402 payment protocol for the Tempo blockchain.
//!
//! Implements HTTP 402 pay-per-request using EIP-712 signed authorizations
//! and TIP-20 (ERC-20 compatible) token transfers on the Tempo chain.
//!
//! # Three-party model
//!
//! - **Client** — signs payment authorizations (see `tempo-x402-client` crate)
//! - **Server** ([`TempoSchemeServer`]) — gates endpoints, returns 402 with pricing
//! - **Facilitator** ([`TempoSchemeFacilitator`]) — verifies signatures and settles on-chain
//!
//! # Quick example (server)
//!
//! ```
//! use x402::{TempoSchemeServer, SchemeServer};
//!
//! let server = TempoSchemeServer::default();
//! let (amount, asset) = server.parse_price("$0.001").unwrap();
//! assert_eq!(amount, "1000"); // 1000 micro-tokens
//! ```
//!
//! For making paid requests, use the `tempo-x402-client` crate.

// Core types and traits
pub mod constants;
pub mod error;
pub mod hmac;
pub mod payment;
pub mod response;
pub mod scheme;
pub mod security;

// Tempo blockchain implementation
pub mod eip712;
pub mod nonce_store;
pub mod scheme_facilitator;
pub mod scheme_server;
pub mod tip20;

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

pub use scheme_facilitator::TempoSchemeFacilitator;
pub use scheme_server::TempoSchemeServer;
