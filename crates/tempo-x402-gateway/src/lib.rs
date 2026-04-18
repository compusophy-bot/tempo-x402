//! # tempo-x402-gateway
//!
//! API gateway that adds **x402 payment rails** to any HTTP endpoint.
//!
//! Register upstream APIs with a price, and clients pay per-request through the
//! gateway at `/g/{slug}/{path}`. Payments are verified and settled on-chain before
//! the request is proxied.
//!
//! ## Features
//!
//! - **Endpoint registration** with atomic slug reservation (`BEGIN IMMEDIATE`)
//! - **Embedded facilitator** &mdash; runs in-process when `FACILITATOR_PRIVATE_KEY` is set
//! - **Proxy with SSRF protection** &mdash; HTTPS-only targets, private IP blocking, DNS validation
//! - **Per-endpoint analytics** &mdash; request counts, payment counts, revenue tracking
//! - **Prometheus metrics** &mdash; `ENDPOINT_PAYMENTS` and `ENDPOINT_REVENUE` with slug labels
//! - **Pre-flight reachability check** before payment settlement (don't charge for dead targets)
//! - **Extensible database** &mdash; downstream crates (x402-node) add tables via `execute_schema()`
//!
//! ## Modules
//!
//! - [`config`] &mdash; Gateway configuration ([`config::GatewayConfig`])
//! - [`db`] &mdash; SQLite database with extensible schema
//! - [`middleware`] &mdash; Payment processing, header encoding, 402 response construction
//! - [`proxy`] &mdash; HTTP proxy with header stripping and SSRF protection
//! - [`routes`] &mdash; Endpoint registration, gateway proxy, analytics, health
//! - [`state`] &mdash; Shared application state
//! - [`validation`] &mdash; URL and SSRF validation
//! - [`metrics`] &mdash; Prometheus metrics
//! - [`facilitator`] &mdash; Embedded facilitator bootstrap
//!
//! Part of the [`tempo-x402`](https://docs.rs/tempo-x402) workspace.

pub mod config;
pub mod cors;
pub mod db;
pub mod error;
pub mod facilitator;
pub mod metrics;
pub mod middleware;
pub mod proxy;
pub mod routes;
pub mod state;
pub mod validation;

pub use config::GatewayConfig;
pub use db::Database;
pub use error::GatewayError;
pub use state::AppState;
