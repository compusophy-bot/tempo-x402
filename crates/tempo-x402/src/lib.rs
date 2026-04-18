//! # tempo-x402
//!
//! **HTTP 402 Payment Required** for the Tempo blockchain.
//!
//! Implements pay-per-request API monetization using [EIP-712](https://eips.ethereum.org/EIPS/eip-712)
//! signed authorizations and TIP-20 (ERC-20 compatible) token transfers on the
//! [Tempo](https://tempo.xyz) chain. One header, one on-chain transfer, zero custodial risk.
//!
//! ## How it works
//!
//! 1. Client requests a protected endpoint
//! 2. Server responds **402** with pricing (token, amount, recipient)
//! 3. Client signs an EIP-712 [`PaymentAuthorization`], retries with `PAYMENT-SIGNATURE` header
//! 4. Facilitator atomically verifies the signature, checks balance/allowance/nonce,
//!    and calls `transferFrom` on-chain
//! 5. Server returns the content + transaction hash
//!
//! The facilitator holds no user funds &mdash; it only has token approval to call
//! `transferFrom` on behalf of clients who explicitly approved it.
//!
//! ## Architecture
//!
//! Three-party model:
//!
//! - **Client** ([`client::X402Client`]) &mdash; signs payment authorizations, handles the 402 retry flow
//! - **Server** ([`scheme_server::TempoSchemeServer`]) &mdash; gates endpoints, returns 402 with pricing
//! - **Facilitator** ([`scheme_facilitator::TempoSchemeFacilitator`]) &mdash; verifies signatures and settles on-chain
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`constants`] | Chain configuration (ID `42431`), token address, well-known addresses |
//! | [`eip712`] | EIP-712 typed-data signing, signature verification, nonce generation |
//! | [`wallet`] | WASM-compatible wallet: key generation, EIP-712 signing, payment payloads |
//! | [`client`] | Client SDK &mdash; handles 402 flow automatically |
//! | [`scheme`] | Core trait definitions ([`scheme::SchemeClient`], [`scheme::SchemeFacilitator`], [`scheme::SchemeServer`]) |
//! | [`scheme_server`] | Server implementation: price parsing and payment requirements |
//! | [`scheme_facilitator`] | Facilitator implementation: signature verification and on-chain settlement |
//! | [`tip20`] | On-chain TIP-20 token operations (balance, allowance, transfer, approve) |
//! | [`nonce_store`] | Replay protection backends (in-memory and persistent SQLite) |
//! | [`payment`] | Payment data structures (payloads, requirements, 402 response body) |
//! | [`response`] | Facilitator response types (verify/settle results) |
//! | [`hmac`] | HMAC-SHA256 for facilitator request authentication |
//! | [`security`] | Constant-time comparison utilities |
//! | [`network`] | SSRF protection: private IP detection, DNS validation |
//! | [`facilitator_client`] | HTTP client for calling a remote facilitator |
//! | [`error`] | Error types for all x402 operations |
//!
//! ## Quick start
//!
//! Parse a price and generate payment requirements:
//!
//! ```
//! use x402::scheme::SchemeServer;
//! use x402::scheme_server::TempoSchemeServer;
//!
//! let server = TempoSchemeServer::default();
//! let (amount, asset) = server.parse_price("$0.001").unwrap();
//! assert_eq!(amount, "1000"); // 1000 micro-tokens (6 decimals)
//! ```
//!
//! Generate a wallet and sign a payment:
//!
//! ```
//! use x402::wallet::{generate_random_key, WalletSigner};
//!
//! let key = generate_random_key();
//! let signer = WalletSigner::new(&key).unwrap();
//! let address = signer.address();
//! ```
//!
//! ## Workspace crates
//!
//! | Crate | Purpose |
//! |-------|---------|
//! | **tempo-x402** (this crate) | Core library |
//! | [`tempo-x402-gateway`](https://docs.rs/tempo-x402-gateway) | API gateway + embedded facilitator |
//! | [`tempo-x402-identity`](https://docs.rs/tempo-x402-identity) | Agent identity: wallet, faucet, ERC-8004 |
//! | [`tempo-x402-soul`](https://docs.rs/tempo-x402-soul) | Autonomous cognition: plans, memory, coding agent |
//! | [`tempo-x402-node`](https://docs.rs/tempo-x402-node) | Self-deploying node with clone orchestration |
//!
//! ## Feature flags
//!
//! - **`full`** (default) &mdash; all features: async runtime, SQLite nonce store, HTTP client
//! - **`wasm`** &mdash; WASM-compatible subset: types, EIP-712 signing, wallet (no tokio/rusqlite)
//! - **`demo`** &mdash; includes a demo private key for testing
//!
//! ## Network
//!
//! - **Chain**: Tempo Moderato, Chain ID `42431`
//! - **Token**: pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals)
//! - **RPC**: `https://rpc.moderato.tempo.xyz`

// ---------------------------------------------------------------------------
// Public modules — organized by layer
// ---------------------------------------------------------------------------

/// Chain configuration, token addresses, and well-known constants.
pub mod constants;

/// Error types for x402 operations.
pub mod error;

/// Payment data structures exchanged between client, server, and facilitator.
pub mod payment;

/// Facilitator response types returned after verify/settle operations.
pub mod response;

/// Core trait definitions for the three-party payment model.
pub mod scheme;

/// EIP-712 typed-data signing, signature verification, and nonce generation.
pub mod eip712;

/// WASM-compatible wallet: key generation, EIP-712 signing, payment payloads.
pub mod wallet;

/// TIP-20 (ERC-20 compatible) on-chain token operations.
#[cfg(feature = "full")]
pub mod tip20;

/// Replay protection via nonce tracking (in-memory and persistent SQLite backends).
#[cfg(feature = "full")]
pub mod nonce_store;

/// HMAC-SHA256 utilities for authenticating facilitator requests.
pub mod hmac;

/// Constant-time comparison utilities for timing-attack resistance.
pub mod security;

/// Network validation utilities (private IP detection for SSRF protection).
#[cfg(feature = "full")]
pub mod network;

/// [`scheme::SchemeFacilitator`] implementation: signature verification and on-chain settlement.
#[cfg(feature = "full")]
pub mod scheme_facilitator;

/// [`scheme::SchemeServer`] implementation: price parsing and payment requirements.
#[cfg(feature = "full")]
pub mod scheme_server;

/// HTTP client for calling a remote facilitator's `/verify-and-settle` endpoint.
#[cfg(feature = "full")]
pub mod facilitator_client;

/// Client SDK for making paid API requests (handles 402 flow automatically).
#[cfg(feature = "full")]
pub mod client;

// ---------------------------------------------------------------------------
// Solidity type bindings (generated by alloy sol! macro)
// ---------------------------------------------------------------------------

use alloy::sol;

// EIP-712 struct for payment authorizations.
//
// The `sol!` macro derives `alloy::sol_types::SolStruct` which provides
// `eip712_signing_hash()`. This struct is the on-chain representation of a
// payment authorization that clients sign and facilitators verify.
//
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
//
// Used by the `tip20` module functions to interact with the pathUSD token contract.
#[cfg(feature = "full")]
sol! {
    #[sol(rpc)]
    interface TIP20 {
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function transferFrom(address from, address to, uint256 value) external returns (bool);
        function approve(address spender, uint256 value) external returns (bool);
    }
}

// ---------------------------------------------------------------------------
// Convenience re-exports — key types available at crate root
// ---------------------------------------------------------------------------

pub use constants::ChainConfig;
pub use error::X402Error;
#[cfg(feature = "full")]
pub use scheme_facilitator::TempoSchemeFacilitator;
#[cfg(feature = "full")]
pub use scheme_server::TempoSchemeServer;
pub use wallet::{
    build_payment_payload, generate_random_key, recover_message_signer, WalletSigner,
};
