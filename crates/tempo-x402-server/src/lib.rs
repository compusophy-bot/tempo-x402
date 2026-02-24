//! x402 resource server — gates HTTP endpoints behind 402 payments.
//!
//! Provides middleware that intercepts requests to protected routes, returns
//! HTTP 402 with [`PaymentRequirements`](x402::payment::PaymentRequirements),
//! and settles payments via the facilitator before granting access.
//!
//! # Modules
//!
//! - [`config`] — Payment configuration and route registration ([`PaymentConfigBuilder`](config::PaymentConfigBuilder))
//! - [`middleware`] — Payment gate middleware ([`require_payment`](middleware::require_payment))
//! - [`metrics`] — Prometheus metrics for request and payment tracking

pub mod config;
pub mod metrics;
pub mod middleware;

pub use config::{PaymentConfig, PaymentConfigBuilder, PaymentGateConfig, RoutePaymentConfig};
pub use middleware::{
    call_verify_and_settle, check_payment_gate, decode_payment_header, payment_required_body,
    require_payment,
};
