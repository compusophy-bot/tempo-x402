//! EIP-712 typed-data signing, signature verification, and nonce generation.
//!
//! Provides functions for:
//! - Building EIP-712 domains ([`payment_domain`], [`payment_domain_for_chain`])
//! - Computing signing hashes ([`signing_hash`], [`signing_hash_for_chain`])
//! - Verifying signatures with EIP-2 malleability protection ([`verify_signature`], [`verify_signature_for_chain`])
//! - Generating cryptographically secure random nonces ([`random_nonce`])
//! - Encoding signatures to hex ([`encode_signature_hex`])

use alloy::primitives::{Address, FixedBytes, Signature, B256, U256};
use alloy::sol_types::SolStruct;

use crate::PaymentAuthorization;
use crate::{ChainConfig, X402Error};

/// Build the EIP-712 domain for a given chain config and token address.
pub fn payment_domain_for_chain(
    config: &ChainConfig,
    token: Address,
) -> alloy::sol_types::Eip712Domain {
    alloy::sol_types::Eip712Domain {
        name: Some(std::borrow::Cow::Owned(config.eip712_domain_name.clone())),
        version: Some(std::borrow::Cow::Owned(
            config.eip712_domain_version.clone(),
        )),
        chain_id: Some(U256::from(config.chain_id)),
        verifying_contract: Some(token),
        salt: None,
    }
}

/// Build the EIP-712 domain for a given token address (Tempo defaults).
pub fn payment_domain(token: Address) -> alloy::sol_types::Eip712Domain {
    payment_domain_for_chain(&ChainConfig::default(), token)
}

/// Compute the EIP-712 signing hash for a given chain config.
pub fn signing_hash_for_chain(auth: &PaymentAuthorization, config: &ChainConfig) -> B256 {
    let domain = payment_domain_for_chain(config, auth.token);
    auth.eip712_signing_hash(&domain)
}

/// Compute the EIP-712 signing hash (Tempo defaults).
pub fn signing_hash(auth: &PaymentAuthorization) -> B256 {
    signing_hash_for_chain(auth, &ChainConfig::default())
}

/// secp256k1 curve order N / 2 — signatures with s > this are malleable (EIP-2).
const SECP256K1_N_DIV_2: U256 = U256::from_limbs([
    0xBFD25E8CD0364140,
    0xBAAEDCE6AF48A03B,
    0xFFFFFFFFFFFFFFFE,
    0x7FFFFFFFFFFFFFFF,
]);

/// Verify an EIP-712 signature for a given chain config.
/// Rejects high-s signatures to prevent malleability (EIP-2).
pub fn verify_signature_for_chain(
    auth: &PaymentAuthorization,
    signature_bytes: &[u8],
    config: &ChainConfig,
) -> Result<Address, X402Error> {
    // F-03: Validate signature length before parsing
    if signature_bytes.len() != 65 {
        return Err(X402Error::SignatureError(format!(
            "signature must be 65 bytes, got {}",
            signature_bytes.len()
        )));
    }

    let sig = Signature::from_raw(signature_bytes)
        .map_err(|e| X402Error::SignatureError(format!("invalid signature: {e}")))?;

    // V-value validation: alloy's Signature::from_raw() normalizes the v/parity byte
    // from the 65th byte of the signature. It accepts v ∈ {0, 1, 27, 28} and normalizes
    // to a boolean parity. Invalid v values cause from_raw() to return Err above.
    // No additional v-value check is needed.

    // F-01: Reject high-s signatures (EIP-2 malleability protection)
    if sig.s() > SECP256K1_N_DIV_2 {
        return Err(X402Error::SignatureError(
            "high-s signature rejected (EIP-2 malleability)".to_string(),
        ));
    }

    let hash = signing_hash_for_chain(auth, config);
    sig.recover_address_from_prehash(&hash)
        .map_err(|e| X402Error::SignatureError(format!("recovery failed: {e}")))
}

/// Verify an EIP-712 signature and return the recovered signer address (Tempo defaults).
pub fn verify_signature(
    auth: &PaymentAuthorization,
    signature_bytes: &[u8],
) -> Result<Address, X402Error> {
    verify_signature_for_chain(auth, signature_bytes, &ChainConfig::default())
}

/// Generate a random 32-byte nonce (keccak256 of 32 random bytes).
/// Uses `rand::fill` which delegates to the OS CSPRNG (cryptographically secure).
pub fn random_nonce() -> FixedBytes<32> {
    use alloy::primitives::keccak256;
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes); // CSPRNG via ThreadRng -> OsRng
    keccak256(bytes)
}

/// Encode a Signature to a hex string with 0x prefix (65 bytes -> 0x + 130 hex).
/// Uses Electrum notation: v = 27 or 28 in the last byte.
pub fn encode_signature_hex(sig: &Signature) -> String {
    let bytes = sig.as_bytes();
    format!("0x{}", alloy::hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, FixedBytes, U256};
    use alloy::signers::local::PrivateKeySigner;
    use alloy::signers::SignerSync;

    #[test]
    fn test_sign_and_verify_roundtrip() {
        let signer: PrivateKeySigner = PrivateKeySigner::random();
        let addr = signer.address();

        let auth = PaymentAuthorization {
            from: addr,
            to: Address::ZERO,
            value: U256::from(1000u64),
            token: crate::constants::DEFAULT_TOKEN,
            validAfter: U256::from(0u64),
            validBefore: U256::from(u64::MAX),
            nonce: FixedBytes::ZERO,
        };

        let hash = signing_hash(&auth);
        let sig = signer.sign_hash_sync(&hash).unwrap();
        let sig_hex = encode_signature_hex(&sig);
        let sig_bytes = alloy::hex::decode(sig_hex.strip_prefix("0x").unwrap()).unwrap();

        let recovered = verify_signature(&auth, &sig_bytes).unwrap();
        assert_eq!(recovered, addr);
    }

    #[test]
    fn test_random_nonce_is_unique() {
        let n1 = random_nonce();
        let n2 = random_nonce();
        assert_ne!(n1, n2);
    }

    #[test]
    fn test_signature_encoding_roundtrip() {
        let r = U256::from(42u64);
        let s = U256::from(99u64);
        let sig = Signature::new(r, s, true);
        let bytes = sig.as_bytes();
        assert_eq!(bytes.len(), 65);
        assert_eq!(bytes[64], 28);

        let parsed = Signature::from_raw(&bytes).unwrap();
        assert_eq!(parsed.r(), r);
        assert_eq!(parsed.s(), s);
        assert!(parsed.v());
    }
}
