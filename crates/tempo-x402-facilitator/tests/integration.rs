use actix_web::{test, web, App};
use alloy::network::EthereumWallet;
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

use x402_facilitator::routes;
use x402_facilitator::state::AppState;

/// Build an AppState with a dummy wallet provider and HMAC secret.
/// HMAC is always required (no Option).
fn make_state(hmac_secret: Vec<u8>) -> web::Data<AppState> {
    let signer = PrivateKeySigner::random();
    let facilitator_address = signer.address();

    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(signer))
        .connect_http("http://localhost:1".parse().unwrap());

    let facilitator = x402::TempoSchemeFacilitator::new(provider, facilitator_address);

    web::Data::new(AppState {
        facilitator,
        hmac_secret,
        chain_config: x402::ChainConfig::default(),
        webhook_urls: vec![],
        http_client: reqwest::Client::new(),
        metrics_token: None,
        webhook_hmac_key: None,
    })
}

#[actix_rt::test]
async fn test_supported_returns_scheme_and_network() {
    let state = make_state(b"test-secret".to_vec());
    let app = test::init_service(App::new().app_data(state).service(routes::supported)).await;

    let req = test::TestRequest::get().uri("/supported").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["schemes"][0], "tempo-tip20");
    assert_eq!(body["networks"][0], "eip155:42431");
}

#[actix_rt::test]
async fn test_verify_and_settle_requires_hmac_when_configured() {
    let state = make_state(b"test-secret".to_vec());
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(web::JsonConfig::default().limit(65_536))
            .service(routes::verify_and_settle),
    )
    .await;

    // Send without X-Facilitator-Auth header
    let req = test::TestRequest::post()
        .uri("/verify-and-settle")
        .set_payload("{}")
        .insert_header(("Content-Type", "application/json"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"], "authentication required");
}

#[actix_rt::test]
async fn test_verify_and_settle_rejects_bad_hmac() {
    let state = make_state(b"test-secret".to_vec());
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(web::JsonConfig::default().limit(65_536))
            .service(routes::verify_and_settle),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/verify-and-settle")
        .set_payload("{}")
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("X-Facilitator-Auth", "deadbeef"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"], "authentication failed");
}

#[actix_rt::test]
async fn test_verify_and_settle_accepts_valid_hmac() {
    let state = make_state(b"test-secret".to_vec());
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(web::JsonConfig::default().limit(65_536))
            .service(routes::verify_and_settle),
    )
    .await;

    // Compute valid HMAC over the body
    let body_bytes = b"{}";
    let sig = x402::hmac::compute_hmac(b"test-secret", body_bytes);

    let req = test::TestRequest::post()
        .uri("/verify-and-settle")
        .set_payload(&body_bytes[..])
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("X-Facilitator-Auth", sig))
        .to_request();
    let resp = test::call_service(&app, req).await;

    // Should pass HMAC but fail on body parse -> 400, not 401
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn test_verify_and_settle_always_requires_hmac() {
    // HMAC is always mandatory — even with a secret set, missing header = 401
    let state = make_state(b"some-secret".to_vec());
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(web::JsonConfig::default().limit(65_536))
            .service(routes::verify_and_settle),
    )
    .await;

    // No HMAC header -> should be rejected with 401
    let req = test::TestRequest::post()
        .uri("/verify-and-settle")
        .set_payload("{}")
        .insert_header(("Content-Type", "application/json"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"], "authentication required");
}

#[actix_rt::test]
async fn test_verify_and_settle_rejects_malformed_body() {
    let secret = b"test-secret";
    let state = make_state(secret.to_vec());
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(web::JsonConfig::default().limit(65_536))
            .service(routes::verify_and_settle),
    )
    .await;

    let body_bytes = b"not valid json at all";
    let sig = x402::hmac::compute_hmac(secret, body_bytes);

    let req = test::TestRequest::post()
        .uri("/verify-and-settle")
        .set_payload(&body_bytes[..])
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("X-Facilitator-Auth", sig))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["success"], false);
    assert!(body["errorReason"].as_str().unwrap().contains("invalid"));
}

/// Build an AppState with separate metrics token.
fn make_state_with_metrics_token(
    hmac_secret: Vec<u8>,
    metrics_token: Option<Vec<u8>>,
) -> web::Data<AppState> {
    let signer = PrivateKeySigner::random();
    let facilitator_address = signer.address();

    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(signer))
        .connect_http("http://localhost:1".parse().unwrap());

    let facilitator = x402::TempoSchemeFacilitator::new(provider, facilitator_address);

    web::Data::new(AppState {
        facilitator,
        hmac_secret,
        chain_config: x402::ChainConfig::default(),
        webhook_urls: vec![],
        http_client: reqwest::Client::new(),
        metrics_token,
        webhook_hmac_key: None,
    })
}

#[actix_rt::test]
async fn test_metrics_requires_separate_token() {
    // Configure HMAC secret AND a separate metrics token
    let state =
        make_state_with_metrics_token(b"hmac-secret".to_vec(), Some(b"metrics-token-123".to_vec()));

    let app =
        test::init_service(App::new().app_data(state).service(routes::metrics_endpoint)).await;

    // No bearer token -> 401
    let req = test::TestRequest::get().uri("/metrics").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    // Wrong bearer token (the HMAC secret, not the metrics token) -> 401
    let req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Authorization", "Bearer hmac-secret"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    // Correct metrics token -> 200
    let req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Authorization", "Bearer metrics-token-123"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_rt::test]
async fn test_metrics_forbidden_when_no_token() {
    // No metrics token configured -> 403 by default (requires X402_PUBLIC_METRICS=true)
    let state = make_state(b"hmac-secret".to_vec());
    let app =
        test::init_service(App::new().app_data(state).service(routes::metrics_endpoint)).await;

    let req = test::TestRequest::get().uri("/metrics").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
}
