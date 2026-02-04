use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::RootProvider;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;

use x402::eip712;
use x402::PaymentAuthorization;
use x402::DEFAULT_TOKEN;

/// Helper: create a valid PaymentAuthorization and sign it.
fn make_signed_auth(
    signer: &PrivateKeySigner,
) -> (PaymentAuthorization, Vec<u8>) {
    let auth = PaymentAuthorization {
        from: signer.address(),
        to: Address::ZERO,
        value: U256::from(1000u64),
        token: DEFAULT_TOKEN,
        validAfter: U256::from(0u64),
        validBefore: U256::from(u64::MAX),
        nonce: FixedBytes::ZERO,
    };
    let hash = eip712::signing_hash(&auth);
    let sig = signer.sign_hash_sync(&hash).unwrap();
    let sig_bytes = sig.as_bytes().to_vec();
    (auth, sig_bytes)
}

// -- Signature failure tests --

#[test]
fn test_verify_wrong_signer() {
    let signer_a = PrivateKeySigner::random();
    let signer_b = PrivateKeySigner::random();

    let (mut auth, sig_bytes) = make_signed_auth(&signer_a);
    // Claim the auth is from signer_b
    auth.from = signer_b.address();

    let recovered = eip712::verify_signature(&auth, &sig_bytes).unwrap();
    assert_ne!(recovered, signer_b.address());
}

#[test]
fn test_verify_tampered_value() {
    let signer = PrivateKeySigner::random();
    let (mut auth, sig_bytes) = make_signed_auth(&signer);

    // Tamper with the value after signing
    auth.value = U256::from(9999u64);

    let recovered = eip712::verify_signature(&auth, &sig_bytes).unwrap();
    assert_ne!(recovered, signer.address());
}

#[test]
fn test_verify_tampered_nonce() {
    let signer = PrivateKeySigner::random();
    let (mut auth, sig_bytes) = make_signed_auth(&signer);

    // Tamper with the nonce after signing
    auth.nonce = FixedBytes::new([0xff; 32]);

    let recovered = eip712::verify_signature(&auth, &sig_bytes).unwrap();
    assert_ne!(recovered, signer.address());
}

#[test]
fn test_verify_invalid_signature_bytes() {
    let signer = PrivateKeySigner::random();
    let (auth, _) = make_signed_auth(&signer);

    // Pass garbage signature bytes (too short)
    let result = eip712::verify_signature(&auth, &[0xde, 0xad]);
    assert!(result.is_err());
}

// -- Nonce tracking tests --

#[test]
fn test_nonce_replay_detection() {
    let provider = RootProvider::<alloy::network::Ethereum>::new_http("http://localhost:1".parse().unwrap());
    let facilitator =
        x402::TempoSchemeFacilitator::new(provider, Address::ZERO);

    let nonce = FixedBytes::new([0x42; 32]);

    assert!(!facilitator.is_nonce_used(&nonce));
    facilitator.record_nonce(nonce);
    assert!(facilitator.is_nonce_used(&nonce));
}

#[test]
fn test_nonce_not_yet_used() {
    let provider = RootProvider::<alloy::network::Ethereum>::new_http("http://localhost:1".parse().unwrap());
    let facilitator =
        x402::TempoSchemeFacilitator::new(provider, Address::ZERO);

    let nonce = FixedBytes::new([0x01; 32]);
    assert!(!facilitator.is_nonce_used(&nonce));
}

#[test]
fn test_multiple_nonces_independent() {
    let provider = RootProvider::<alloy::network::Ethereum>::new_http("http://localhost:1".parse().unwrap());
    let facilitator =
        x402::TempoSchemeFacilitator::new(provider, Address::ZERO);

    let nonce_a = FixedBytes::new([0xaa; 32]);
    let nonce_b = FixedBytes::new([0xbb; 32]);

    facilitator.record_nonce(nonce_a);

    assert!(facilitator.is_nonce_used(&nonce_a));
    assert!(!facilitator.is_nonce_used(&nonce_b));
}

// -- Time window tests (via verify) --

#[tokio::test]
async fn test_expired_authorization() {
    use x402::{PaymentPayload, PaymentRequirements, SchemeFacilitator, TempoPaymentData};

    let signer = PrivateKeySigner::random();

    let auth = PaymentAuthorization {
        from: signer.address(),
        to: Address::ZERO,
        value: U256::from(1000u64),
        token: DEFAULT_TOKEN,
        validAfter: U256::from(0u64),
        validBefore: U256::from(0u64), // expired
        nonce: eip712::random_nonce(),
    };

    let hash = eip712::signing_hash(&auth);
    let sig = signer.sign_hash_sync(&hash).unwrap();
    let sig_hex = eip712::encode_signature_hex(&sig);

    let payload = PaymentPayload {
        x402_version: 1,
        payload: TempoPaymentData {
            from: signer.address(),
            to: Address::ZERO,
            value: "1000".to_string(),
            token: DEFAULT_TOKEN,
            valid_after: 0,
            valid_before: 0, // expired
            nonce: auth.nonce,
            signature: sig_hex,
        },
    };

    let requirements = PaymentRequirements {
        scheme: "tempo-tip20".to_string(),
        network: "eip155:42431".to_string(),
        price: "$0.001".to_string(),
        asset: DEFAULT_TOKEN,
        amount: "1000".to_string(),
        pay_to: Address::ZERO,
        max_timeout_seconds: 30,
        description: None,
        mime_type: None,
    };

    let provider = RootProvider::<alloy::network::Ethereum>::new_http("http://localhost:1".parse().unwrap());
    let facilitator =
        x402::TempoSchemeFacilitator::new(provider, Address::ZERO);

    let result = facilitator.verify(&payload, &requirements).await.unwrap();
    assert!(!result.is_valid);
    assert_eq!(
        result.invalid_reason.as_deref(),
        Some("Authorization expired")
    );
}

#[tokio::test]
async fn test_not_yet_valid_authorization() {
    use x402::{PaymentPayload, PaymentRequirements, SchemeFacilitator, TempoPaymentData};

    let signer = PrivateKeySigner::random();

    let auth = PaymentAuthorization {
        from: signer.address(),
        to: Address::ZERO,
        value: U256::from(1000u64),
        token: DEFAULT_TOKEN,
        validAfter: U256::from(u64::MAX), // far future
        validBefore: U256::from(u64::MAX),
        nonce: eip712::random_nonce(),
    };

    let hash = eip712::signing_hash(&auth);
    let sig = signer.sign_hash_sync(&hash).unwrap();
    let sig_hex = eip712::encode_signature_hex(&sig);

    let payload = PaymentPayload {
        x402_version: 1,
        payload: TempoPaymentData {
            from: signer.address(),
            to: Address::ZERO,
            value: "1000".to_string(),
            token: DEFAULT_TOKEN,
            valid_after: u64::MAX,
            valid_before: u64::MAX,
            nonce: auth.nonce,
            signature: sig_hex,
        },
    };

    let requirements = PaymentRequirements {
        scheme: "tempo-tip20".to_string(),
        network: "eip155:42431".to_string(),
        price: "$0.001".to_string(),
        asset: DEFAULT_TOKEN,
        amount: "1000".to_string(),
        pay_to: Address::ZERO,
        max_timeout_seconds: 30,
        description: None,
        mime_type: None,
    };

    let provider = RootProvider::<alloy::network::Ethereum>::new_http("http://localhost:1".parse().unwrap());
    let facilitator =
        x402::TempoSchemeFacilitator::new(provider, Address::ZERO);

    let result = facilitator.verify(&payload, &requirements).await.unwrap();
    assert!(!result.is_valid);
    assert_eq!(
        result.invalid_reason.as_deref(),
        Some("Authorization not yet valid")
    );
}
