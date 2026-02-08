//! Shared security utilities for the x402 payment protocol.
//!
//! This module provides constant-time comparison and other cryptographic
//! helpers used across multiple crates. All implementations use the `subtle`
//! crate for timing-attack resistance.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Constant-time byte comparison that does not leak input lengths or content.
///
/// Both inputs are hashed to fixed-length SHA-256 digests before comparison,
/// so timing reveals neither the content nor the length of either input.
/// The final comparison uses `subtle::ConstantTimeEq` for guaranteed
/// constant-time behavior.
///
/// # Use cases
/// - Bearer token validation for `/metrics` endpoints
/// - Any secret comparison where timing attacks are a concern
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let ha = Sha256::digest(a);
    let hb = Sha256::digest(b);
    ha.ct_eq(&hb).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_inputs_match() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn different_inputs_do_not_match() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn different_length_inputs_do_not_match() {
        assert!(!constant_time_eq(b"short", b"much longer string"));
    }

    #[test]
    fn empty_inputs_match() {
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn empty_vs_nonempty_do_not_match() {
        assert!(!constant_time_eq(b"", b"notempty"));
    }
}
