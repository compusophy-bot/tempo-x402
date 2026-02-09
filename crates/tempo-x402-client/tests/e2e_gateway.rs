//! End-to-end payment test against live deployments.
//!
//! Tests the full 402 payment flow:
//!   1. Check wallet balance & allowance
//!   2. GET gateway /register → 402 with payment requirements
//!   3. Sign EIP-712 payment authorization
//!   4. POST /register with PAYMENT-SIGNATURE header → 201 Created
//!   5. GET /g/{slug} → 402 (proxy payment gate)
//!   6. GET /g/{slug} with payment → 200 + proxied response
//!   7. Cleanup: DELETE /endpoints/{slug} with payment
//!
//! Run:  cargo test --test e2e_gateway -- --nocapture

use alloy::primitives::Address;
use alloy::providers::RootProvider;
use alloy::signers::local::PrivateKeySigner;
use base64::Engine;
use x402::{PaymentRequiredBody, SchemeClient, SettleResponse, DEFAULT_TOKEN, SCHEME_NAME};
use x402_client::TempoSchemeClient;

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

/// Sign a payment for the given requirements, return the base64-encoded PAYMENT-SIGNATURE header.
async fn sign_payment(
    signer: &TempoSchemeClient,
    requirements: &x402::PaymentRequirements,
) -> String {
    let payload = signer
        .create_payment_payload(1, requirements)
        .await
        .expect("failed to create payment payload");
    let json = serde_json::to_vec(&payload).expect("serialize payload");
    base64::engine::general_purpose::STANDARD.encode(&json)
}

#[tokio::test]
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

    // ── Step 1: Check balance & allowance ──────────────────────────────
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

    // ── Step 2: GET /register → expect 405 (method not allowed) or hit POST without payment → 402 ──
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
    println!("        x402 version: {}", body_402.x402_version);
    println!(
        "        Accepts: {:?}",
        body_402
            .accepts
            .iter()
            .map(|r| &r.scheme)
            .collect::<Vec<_>>()
    );

    let requirements = body_402
        .accepts
        .iter()
        .find(|r| r.scheme == SCHEME_NAME)
        .expect("no tempo-tip20 scheme in 402 response");

    println!("        Pay to: {}", requirements.pay_to);
    println!(
        "        Amount: {} (price: {})",
        requirements.amount, requirements.price
    );

    // ── Step 3: Sign payment and retry ─────────────────────────────────
    println!("\nStep 3: Sign EIP-712 payment and POST /register with PAYMENT-SIGNATURE");
    let payment_header = sign_payment(&scheme, requirements).await;
    println!("        Signed payment ({}B base64)", payment_header.len());

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
    println!("        Body: {resp_text}");
    assert!(
        status == 201 || status == 200,
        "Registration failed with status {status}: {resp_text}"
    );

    let register_result: serde_json::Value =
        serde_json::from_str(&resp_text).expect("parse register response");
    if let Some(tx) = register_result.get("transaction") {
        println!("        Tx: {tx}");
    }

    // ── Step 4: Verify endpoint exists ─────────────────────────────────
    println!("\nStep 4: GET /endpoints/{slug} → verify registration");
    let resp = http
        .get(format!("{gateway_url}/endpoints/{slug}"))
        .send()
        .await
        .expect("get endpoint failed");
    let status = resp.status().as_u16();
    println!("        Status: {status}");
    assert_eq!(status, 200, "Endpoint not found after registration");

    let endpoint_info: serde_json::Value = resp.json().await.expect("parse endpoint");
    println!(
        "        Endpoint: {}",
        serde_json::to_string_pretty(&endpoint_info).unwrap_or_default()
    );

    // ── Step 5: GET /g/{slug} without payment → 402 ───────────────────
    println!("\nStep 5: GET /g/{slug} without payment → expect 402");
    let resp = http
        .get(format!("{gateway_url}/g/{slug}"))
        .send()
        .await
        .expect("proxy request failed");
    let status = resp.status().as_u16();
    println!("        Status: {status}");
    assert_eq!(status, 402, "Expected 402 for unpaid proxy, got {status}");

    let proxy_402: PaymentRequiredBody = resp.json().await.expect("parse proxy 402 body");
    let proxy_requirements = proxy_402
        .accepts
        .iter()
        .find(|r| r.scheme == SCHEME_NAME)
        .expect("no tempo-tip20 in proxy 402");
    println!(
        "        Price: {} (amount: {})",
        proxy_requirements.price, proxy_requirements.amount
    );

    // ── Step 6: GET /g/{slug} with payment → proxied response ─────────
    println!("\nStep 6: GET /g/{slug} with payment → expect proxied 200");
    let proxy_payment = sign_payment(&scheme, proxy_requirements).await;

    let resp = http
        .get(format!("{gateway_url}/g/{slug}"))
        .header("PAYMENT-SIGNATURE", &proxy_payment)
        .send()
        .await
        .expect("paid proxy request failed");

    let status = resp.status().as_u16();
    // Check for PAYMENT-RESPONSE header
    let payment_response = resp
        .headers()
        .get("payment-response")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let body = resp.text().await.unwrap_or_default();
    println!("        Status: {status}");
    if let Some(ref pr) = payment_response {
        // Decode payment-response
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(pr) {
            if let Ok(settle) = serde_json::from_slice::<SettleResponse>(&bytes) {
                println!(
                    "        Settlement: success={}, tx={:?}",
                    settle.success, settle.transaction
                );
            }
        }
    }
    println!(
        "        Proxied body (first 200 chars): {}",
        &body[..body.len().min(200)]
    );
    assert_eq!(status, 200, "Paid proxy failed with {status}: {body}");

    // ── Step 7: Check post-test balance ────────────────────────────────
    let new_balance = check_balance(address).await;
    let spent = balance.saturating_sub(new_balance);
    println!("\nStep 7: Post-test balance");
    println!("        Before: {} pathUSD", balance as f64 / 1_000_000.0);
    println!(
        "        After:  {} pathUSD",
        new_balance as f64 / 1_000_000.0
    );
    println!(
        "        Spent:  {} pathUSD (raw: {})",
        spent as f64 / 1_000_000.0,
        spent
    );

    println!("\n=== E2E Test PASSED ===\n");
}

/// Register the permanent "demo" endpoint for the SPA frontend.
/// Idempotent — skips if already registered.
#[tokio::test]
async fn register_demo_endpoint() {
    let gateway_url = gateway_url();
    let http = reqwest::Client::new();

    // Check if demo already exists
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

    // POST /register without payment -> 402
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

    let body_402: x402::PaymentRequiredBody = resp.json().await.expect("parse 402");
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
