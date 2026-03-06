//! Recovery proof construction and verification.
//!
//! Uses EIP-191 `sign_message` from `x402-wallet` for generating proofs
//! that a wallet controls a specific address — needed for recovery address setup.

use alloy::primitives::Address;
use x402::{recover_message_signer, WalletSigner};

/// A signed proof that the signer controls a specific address.
#[derive(Clone, Debug)]
pub struct RecoveryProof {
    /// The message that was signed.
    pub message: Vec<u8>,
    /// The 65-byte EIP-191 signature (r + s + v).
    pub signature: Vec<u8>,
    /// The address recovered from the signature.
    pub signer: Address,
}

/// Build a recovery proof message for a given agent token ID and recovery address.
///
/// Message format: "ERC-8004 Recovery Proof\nAgent: {token_id}\nRecovery: {recovery_address}"
pub fn recovery_proof_message(token_id: &str, recovery_address: Address) -> Vec<u8> {
    format!(
        "ERC-8004 Recovery Proof\nAgent: {}\nRecovery: {}",
        token_id, recovery_address
    )
    .into_bytes()
}

/// Generate a recovery proof signed by the given wallet.
///
/// This proves that the wallet holder authorizes `recovery_address` as the
/// recovery address for the specified agent token.
pub fn generate_recovery_proof(
    signer: &WalletSigner,
    token_id: &str,
    recovery_address: Address,
) -> Result<RecoveryProof, String> {
    let message = recovery_proof_message(token_id, recovery_address);
    let sig_hex = signer.sign_message(&message)?;

    // Decode hex signature to bytes
    let sig_bytes = alloy::hex::decode(&sig_hex[2..])
        .map_err(|e| format!("failed to decode signature hex: {e}"))?;

    Ok(RecoveryProof {
        message,
        signature: sig_bytes,
        signer: signer.address(),
    })
}

/// Verify a recovery proof — checks that the signature recovers to the expected address.
pub fn verify_recovery_proof(proof: &RecoveryProof) -> Result<bool, String> {
    let recovered = recover_message_signer(&proof.message, &proof.signature)?;
    Ok(recovered == proof.signer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_proof_roundtrip() {
        let signer = WalletSigner::random();
        let recovery_addr = WalletSigner::random().address();
        let token_id = "42";

        let proof = generate_recovery_proof(&signer, token_id, recovery_addr).unwrap();
        assert_eq!(proof.signer, signer.address());
        assert_eq!(proof.signature.len(), 65);

        let valid = verify_recovery_proof(&proof).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_recovery_proof_wrong_signer() {
        let signer = WalletSigner::random();
        let other = WalletSigner::random();
        let recovery_addr = WalletSigner::random().address();

        let mut proof = generate_recovery_proof(&signer, "1", recovery_addr).unwrap();
        // Claim it's from a different signer
        proof.signer = other.address();

        let valid = verify_recovery_proof(&proof).unwrap();
        assert!(!valid);
    }

    #[test]
    fn test_recovery_proof_message_format() {
        let addr = Address::ZERO;
        let msg = recovery_proof_message("123", addr);
        let msg_str = String::from_utf8(msg).unwrap();
        assert!(msg_str.starts_with("ERC-8004 Recovery Proof\n"));
        assert!(msg_str.contains("Agent: 123"));
        assert!(msg_str.contains("Recovery: "));
    }
}
