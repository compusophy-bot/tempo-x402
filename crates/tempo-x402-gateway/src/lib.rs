//! x402 API gateway — proxy with per-request payment rails.
//!
//! Register API endpoints with a price, and clients pay per-request via
//! `/g/{slug}/{path}`. Includes SSRF protection, atomic slug reservation,
//! per-endpoint analytics, and optional embedded facilitator mode.
//!
//! # Modules
//!
//! - [`config`] — Gateway configuration ([`GatewayConfig`])
//! - [`db`] — SQLite database ([`Database`]) with extensible schema
//! - [`error`] — Gateway error types ([`GatewayError`])
//! - [`middleware`] — Payment processing, header encoding, and 402 response construction
//! - [`proxy`] — HTTP proxy with header stripping and SSRF protection
//! - [`routes`] — HTTP endpoints (registration, gateway proxy, analytics, health)
//! - [`state`] — Shared application state ([`AppState`])
//! - [`validation`] — URL and SSRF validation
//! - [`metrics`] — Prometheus metrics for endpoint payments and revenue

pub mod config;
pub mod cors;
pub mod db;
pub mod error;
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
