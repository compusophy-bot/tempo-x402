//! HMAC-SHA256 utilities for authenticating facilitator requests.
//!
//! The resource server signs outgoing requests to the facilitator with
//! [`compute_hmac`], and the facilitator verifies them with [`verify_hmac`].
//! All comparisons use constant-time operations to prevent timing attacks.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Compute HMAC-SHA256 over the given body bytes using the shared secret.
/// Returns the hex-encoded MAC.
pub fn compute_hmac(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verify an HMAC-SHA256 signature against the expected body.
/// Returns `true` if the signature is valid.
///
/// Uses constant-time comparison to prevent timing attacks.
/// The HMAC is always computed regardless of whether the hex decodes successfully,
/// preventing timing side-channels that could distinguish valid-hex from invalid-hex
/// signatures.
pub fn verify_hmac(secret: &[u8], body: &[u8], signature: &str) -> bool {
    // Always decode hex — use empty vec on failure so we still compute the MAC
    // and hit the constant-time comparison path (which will reject the length
    // mismatch in constant time via subtle::ConstantTimeEq).
    let expected = hex::decode(signature).unwrap_or_default();

    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);

    // hmac crate's verify_slice uses constant-time comparison and handles
    // length mismatches safely (always returns Err for wrong length, in
    // constant time).
    mac.verify_slice(&expected).is_ok()
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().fold(String::new(), |mut s, b| {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
            s
        })
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, ()> {
        // Use % 2 instead of is_multiple_of() for compatibility with Rust < 1.87
        #[allow(clippy::manual_is_multiple_of)]
        if s.len() % 2 != 0 || !s.is_ascii() {
            return Err(());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_roundtrip() {
        let secret = b"test-secret";
        let body = b"request body content";
        let sig = compute_hmac(secret, body);
        assert!(verify_hmac(secret, body, &sig));
    }

    #[test]
    fn test_hmac_wrong_secret() {
        let body = b"request body content";
        let sig = compute_hmac(b"secret-1", body);
        assert!(!verify_hmac(b"secret-2", body, &sig));
    }

    #[test]
    fn test_hmac_tampered_body() {
        let secret = b"test-secret";
        let sig = compute_hmac(secret, b"original");
        assert!(!verify_hmac(secret, b"tampered", &sig));
    }

    #[test]
    fn test_hmac_invalid_hex() {
        assert!(!verify_hmac(b"secret", b"body", "not-hex-zz"));
    }
}
