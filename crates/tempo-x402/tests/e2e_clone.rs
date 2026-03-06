//! End-to-end test for the clone endpoint.
//!
//! Run:  cargo test --test e2e_clone -- --nocapture --ignored

use alloy::signers::local::PrivateKeySigner;
use base64::Engine;
use x402::client::TempoSchemeClient;
use x402::constants::SCHEME_NAME;
use x402::payment::{PaymentRequiredBody, PaymentRequirements};
use x402::scheme::SchemeClient;

fn gateway_url() -> String {
    std::env::var("GATEWAY_URL")
        .unwrap_or_else(|_| "https://x402-gateway-production-5018.up.railway.app".to_string())
}

fn client_signer() -> PrivateKeySigner {
    dotenvy::dotenv().ok();
    let key = std::env::var("EVM_PRIVATE_KEY").expect("EVM_PRIVATE_KEY required");
    key.parse().expect("invalid EVM_PRIVATE_KEY")
}

async fn sign_payment(signer: &TempoSchemeClient, requirements: &PaymentRequirements) -> String {
    let payload = signer
        .create_payment_payload(1, requirements)
        .await
        .expect("failed to create payment payload");
    let json = serde_json::to_vec(&payload).expect("serialize payload");
    base64::engine::general_purpose::STANDARD.encode(&json)
}

#[tokio::test]
#[ignore]
async fn e2e_clone_flow() {
    let gateway_url = gateway_url();
    let signer = client_signer();
    let address = signer.address();
    let scheme = TempoSchemeClient::new(signer);
    let http = reqwest::Client::new();

    println!("\n=== x402 Clone E2E Test ===");
    println!("Gateway: {gateway_url}");
    println!("Wallet:  {address}\n");

    println!("Step 1: POST /clone without payment → expect 402");
    let resp = http
        .post(format!("{gateway_url}/clone"))
        .send()
        .await
        .expect("clone request failed");

    let status = resp.status().as_u16();
    assert_eq!(status, 402, "Expected 402 Payment Required, got {status}");

    let body_402: PaymentRequiredBody = resp.json().await.expect("parse 402 body");
    let requirements = body_402
        .accepts
        .iter()
        .find(|r| r.scheme == SCHEME_NAME)
        .expect("no tempo-tip20 scheme in 402 response");

    println!("\nStep 2: Sign payment and POST /clone with PAYMENT-SIGNATURE");
    let payment_header = sign_payment(&scheme, requirements).await;

    let resp = http
        .post(format!("{gateway_url}/clone"))
        .header("PAYMENT-SIGNATURE", &payment_header)
        .send()
        .await
        .expect("paid clone request failed");

    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    println!("        Status: {status}");

    let clone_result: serde_json::Value =
        serde_json::from_str(&body).expect("parse clone response");

    if status == 201 {
        let instance_id = clone_result["instance_id"]
            .as_str()
            .expect("missing instance_id");
        println!("        Clone created: {instance_id}");

        println!("\nStep 3: GET /clone/{instance_id}/status");
        let resp = http
            .get(format!("{gateway_url}/clone/{instance_id}/status"))
            .send()
            .await
            .expect("status request failed");
        assert_eq!(resp.status().as_u16(), 200);

        println!("\nStep 4: DELETE /clone/{instance_id}?force=true (cleanup)");
        let resp = http
            .delete(format!("{gateway_url}/clone/{instance_id}?force=true"))
            .send()
            .await
            .expect("delete clone request failed");
        let status = resp.status().as_u16();
        assert!(status == 200 || status == 404);
    } else if status == 409 {
        println!("        Clone limit reached (expected if already at max)");
    } else {
        panic!("Unexpected status {status}: {body}");
    }

    println!("\n=== Clone E2E Test PASSED ===\n");
}
