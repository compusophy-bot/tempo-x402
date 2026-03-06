//! End-to-end payment test against live deployments.
//!
//! Run:  cargo test --test e2e_gateway -- --nocapture

use alloy::primitives::Address;
use alloy::providers::RootProvider;
use alloy::signers::local::PrivateKeySigner;
use base64::Engine;
use x402::client::TempoSchemeClient;
use x402::constants::{DEFAULT_TOKEN, SCHEME_NAME};
use x402::payment::{PaymentRequiredBody, PaymentRequirements};
use x402::response::SettleResponse;
use x402::scheme::SchemeClient;

fn gateway_url() -> String {
    std::env::var("GATEWAY_URL")
        .unwrap_or_else(|_| "https://x402-gateway-production-5018.up.railway.app".to_string())
}
const RPC_URL: &str = "https://rpc.moderato.tempo.xyz";
const FACILITATOR_ADDR: &str = "0x00B4a967685164aF45D4E6B58bF9F19e8119CA97";

fn client_signer() -> PrivateKeySigner {
    dotenvy::dotenv().ok();
    let key = std::env::var("EVM_PRIVATE_KEY").expect("EVM_PRIVATE_KEY required");
    key.parse().expect("invalid EVM_PRIVATE_KEY")
}

async fn check_balance(address: Address) -> u128 {
    let provider: RootProvider = RootProvider::new_http(RPC_URL.parse().unwrap());
    let balance = x402::tip20::balance_of(&provider, DEFAULT_TOKEN, address)
        .await
        .expect("balance_of failed");
    let b: u128 = balance.try_into().expect("balance too large");
    b
}

async fn check_allowance(address: Address) -> u128 {
    let provider: RootProvider = RootProvider::new_http(RPC_URL.parse().unwrap());
    let facilitator: Address = FACILITATOR_ADDR.parse().unwrap();
    let allowance = x402::tip20::allowance(&provider, DEFAULT_TOKEN, address, facilitator)
        .await
        .expect("allowance failed");
    let a: u128 = allowance.try_into().unwrap_or(u128::MAX);
    a
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
async fn e2e_full_payment_flow() {
    let gateway_url = gateway_url();
    let signer = client_signer();
    let address = signer.address();
    let scheme = TempoSchemeClient::new(signer);
    let http = reqwest::Client::new();

    println!("\n=== x402 End-to-End Payment Test ===");
    println!("Gateway:  {gateway_url}");
    println!("Wallet:   {address}");
    println!();

    let balance = check_balance(address).await;
    let allowance = check_allowance(address).await;
    println!(
        "Step 1: Balance = {} pathUSD (raw: {})",
        balance as f64 / 1_000_000.0,
        balance
    );
    println!(
        "        Allowance = {}",
        if allowance == u128::MAX {
            "MAX".to_string()
        } else {
            allowance.to_string()
        }
    );
    assert!(
        balance > 10_000,
        "Insufficient pathUSD balance for test (need > $0.01)"
    );
    assert!(allowance > 10_000, "Insufficient allowance for test");

    println!("\nStep 2: POST /register without payment → expect 402");
    let slug = format!(
        "e2e-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let resp = http
        .post(format!("{gateway_url}/register"))
        .json(&serde_json::json!({
            "slug": slug,
            "target_url": "https://httpbin.org/get",
            "price": "$0.001",
            "description": "E2E test endpoint"
        }))
        .send()
        .await
        .expect("register request failed");

    let status = resp.status().as_u16();
    println!("        Status: {status}");
    assert_eq!(status, 402, "Expected 402 Payment Required, got {status}");

    let body_402: PaymentRequiredBody = resp.json().await.expect("parse 402 body");
    let requirements = body_402
        .accepts
        .iter()
        .find(|r| r.scheme == SCHEME_NAME)
        .expect("no tempo-tip20 scheme in 402 response");

    println!("\nStep 3: Sign EIP-712 payment and POST /register with PAYMENT-SIGNATURE");
    let payment_header = sign_payment(&scheme, requirements).await;

    let resp = http
        .post(format!("{gateway_url}/register"))
        .header("PAYMENT-SIGNATURE", &payment_header)
        .json(&serde_json::json!({
            "slug": slug,
            "target_url": "https://httpbin.org/get",
            "price": "$0.001",
            "description": "E2E test endpoint"
        }))
        .send()
        .await
        .expect("paid register request failed");

    let status = resp.status().as_u16();
    let resp_text = resp.text().await.unwrap_or_default();
    println!("        Status: {status}");
    assert!(
        status == 201 || status == 200,
        "Registration failed with status {status}: {resp_text}"
    );

    println!("\nStep 4: GET /endpoints/{slug} → verify registration");
    let resp = http
        .get(format!("{gateway_url}/endpoints/{slug}"))
        .send()
        .await
        .expect("get endpoint failed");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "Endpoint not found after registration"
    );

    println!("\nStep 5: GET /g/{slug} without payment → expect 402");
    let resp = http
        .get(format!("{gateway_url}/g/{slug}"))
        .send()
        .await
        .expect("proxy request failed");
    assert_eq!(resp.status().as_u16(), 402);

    let proxy_402: PaymentRequiredBody = resp.json().await.expect("parse proxy 402 body");
    let proxy_requirements = proxy_402
        .accepts
        .iter()
        .find(|r| r.scheme == SCHEME_NAME)
        .expect("no tempo-tip20 in proxy 402");

    println!("\nStep 6: GET /g/{slug} with payment → expect proxied 200");
    let proxy_payment = sign_payment(&scheme, proxy_requirements).await;

    let resp = http
        .get(format!("{gateway_url}/g/{slug}"))
        .header("PAYMENT-SIGNATURE", &proxy_payment)
        .send()
        .await
        .expect("paid proxy request failed");

    let status = resp.status().as_u16();
    let payment_response = resp
        .headers()
        .get("payment-response")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let body = resp.text().await.unwrap_or_default();
    println!("        Status: {status}");
    if let Some(ref pr) = payment_response {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(pr) {
            if let Ok(settle) = serde_json::from_slice::<SettleResponse>(&bytes) {
                println!(
                    "        Settlement: success={}, tx={:?}",
                    settle.success, settle.transaction
                );
            }
        }
    }
    assert_eq!(status, 200, "Paid proxy failed with {status}: {body}");

    let new_balance = check_balance(address).await;
    let spent = balance.saturating_sub(new_balance);
    println!("\nStep 7: Spent {} pathUSD", spent as f64 / 1_000_000.0);
    println!("\n=== E2E Test PASSED ===\n");
}

#[tokio::test]
#[ignore]
async fn register_demo_endpoint() {
    let gateway_url = gateway_url();
    let http = reqwest::Client::new();

    let resp = http
        .get(format!("{gateway_url}/endpoints/demo"))
        .send()
        .await
        .expect("request failed");
    if resp.status().as_u16() == 200 {
        println!("demo endpoint already registered, skipping");
        return;
    }

    let signer = client_signer();
    let scheme = TempoSchemeClient::new(signer);

    let resp = http
        .post(format!("{gateway_url}/register"))
        .json(&serde_json::json!({
            "slug": "demo",
            "target_url": "https://httpbin.org/get",
            "price": "$0.001",
            "description": "Demo endpoint for x402 payment flow"
        }))
        .send()
        .await
        .expect("register request failed");

    assert_eq!(resp.status().as_u16(), 402);

    let body_402: PaymentRequiredBody = resp.json().await.expect("parse 402");
    let requirements = body_402
        .accepts
        .iter()
        .find(|r| r.scheme == SCHEME_NAME)
        .expect("no tempo-tip20 scheme");

    let payment_header = sign_payment(&scheme, requirements).await;

    let resp = http
        .post(format!("{gateway_url}/register"))
        .header("PAYMENT-SIGNATURE", &payment_header)
        .json(&serde_json::json!({
            "slug": "demo",
            "target_url": "https://httpbin.org/get",
            "price": "$0.001",
            "description": "Demo endpoint for x402 payment flow"
        }))
        .send()
        .await
        .expect("paid register failed");

    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    println!("Registered demo endpoint: status={status} body={body}");
    assert!(status == 201 || status == 200, "Failed: {status} {body}");
}
