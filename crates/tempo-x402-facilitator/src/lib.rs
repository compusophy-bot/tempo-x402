//! x402 facilitator — verifies EIP-712 payment signatures and settles on-chain.
//!
//! The facilitator receives HMAC-authenticated requests from resource servers,
//! verifies payment signatures, and executes `transferFrom` on the TIP-20 contract.
//! Settlement logic lives in the core [`x402`] crate; this crate provides the
//! HTTP server, state management, and webhook notifications.
//!
//! # Modules
//!
//! - [`routes`] — HTTP endpoints (health, supported, verify-and-settle, metrics)
//! - [`state`] — Shared [`AppState`](state::AppState) (also used by `x402-gateway` for embedded mode)
//! - [`webhook`] — SSRF-protected webhook notifications on settlement
//! - [`metrics`] — Prometheus metrics for settlement operations

pub mod bootstrap;
pub mod metrics;
pub mod routes;
pub mod state;
pub mod webhook;
