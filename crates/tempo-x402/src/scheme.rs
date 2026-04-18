//! Core trait definitions for the three-party payment model.
//!
//! - [`SchemeClient`] — client-side: creates signed payment payloads
//! - [`SchemeFacilitator`] — facilitator-side: verifies and settles payments
//! - [`SchemeServer`] — server-side: parses prices into on-chain amounts
//!
//! See [`crate::scheme_server::TempoSchemeServer`] and
//! [`crate::scheme_facilitator::TempoSchemeFacilitator`] for the Tempo implementations.

use crate::error::X402Error;
use crate::payment::{PaymentPayload, PaymentRequirements};
use crate::response::{SettleResponse, VerifyResponse};
use alloy::primitives::Address;

/// Client-side scheme: creates signed payment payloads.
pub trait SchemeClient: Send + Sync {
    /// Create a signed payment payload for the given requirements.
    fn create_payment_payload(
        &self,
        x402_version: u32,
        requirements: &PaymentRequirements,
    ) -> impl std::future::Future<Output = Result<PaymentPayload, X402Error>> + Send;
}

/// Facilitator-side scheme: verifies and settles payments.
///
/// # Security
/// The `verify()` method performs on-chain balance and allowance reads.
/// It must **never** be exposed as a standalone unauthenticated HTTP endpoint,
/// as it would allow anyone to probe arbitrary addresses' token balances
/// without paying or consuming a nonce. Always use `settle()` (which calls
/// `verify()` internally) behind authentication.
pub trait SchemeFacilitator: Send + Sync {
    /// Verify a payment payload against the requirements.
    fn verify(
        &self,
        payload: &PaymentPayload,
        requirements: &PaymentRequirements,
    ) -> impl std::future::Future<Output = Result<VerifyResponse, X402Error>> + Send;

    /// Settle a payment on-chain (re-verifies first, then executes transferFrom).
    fn settle(
        &self,
        payload: &PaymentPayload,
        requirements: &PaymentRequirements,
    ) -> impl std::future::Future<Output = Result<SettleResponse, X402Error>> + Send;
}

/// Server-side scheme: parses prices into on-chain amounts.
pub trait SchemeServer: Send + Sync {
    /// Parse a human-readable price string (e.g. "$0.001") into an amount and asset.
    fn parse_price(&self, price: &str) -> Result<(String, Address), X402Error>;
}
