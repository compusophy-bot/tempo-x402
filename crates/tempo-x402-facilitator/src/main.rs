use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpServer};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

use std::sync::Arc;

use x402_facilitator::routes;
use x402_facilitator::state::AppState;

fn parse_cors_origins() -> Vec<String> {
    match std::env::var("ALLOWED_ORIGINS") {
        Ok(origins) => origins
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Err(_) => vec![],
    }
}

fn build_cors(origins: &[String]) -> Cors {
    if origins.is_empty() {
        // Default: allow localhost on any port
        Cors::default()
            .allowed_origin_fn(|origin, _| {
                origin
                    .to_str()
                    .map(|o| {
                        // Match http://localhost or http://localhost:PORT exactly
                        o == "http://localhost" || o.starts_with("http://localhost:")
                    })
                    .unwrap_or(false)
            })
            .allow_any_method()
            .allow_any_header()
            .max_age(3600)
    } else {
        let mut cors = Cors::default();
        for origin in origins {
            cors = cors.allowed_origin(origin);
        }
        cors.allow_any_method().allow_any_header().max_age(3600)
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let key = std::env::var("FACILITATOR_PRIVATE_KEY")
        .expect("FACILITATOR_PRIVATE_KEY environment variable is required");

    let signer: PrivateKeySigner = key.parse().expect("invalid FACILITATOR_PRIVATE_KEY");
    let facilitator_address = signer.address();

    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| x402::RPC_URL.to_string());

    let provider = ProviderBuilder::new()
        .wallet(alloy::network::EthereumWallet::from(signer))
        .connect_http(rpc_url.parse().expect("invalid RPC_URL"));

    // Set up nonce storage (SQLite for persistence, falls back to in-memory)
    let nonce_db_path =
        std::env::var("NONCE_DB_PATH").unwrap_or_else(|_| "./x402-nonces.db".to_string());

    let nonce_store: Arc<dyn x402::nonce_store::NonceStore> =
        match x402::nonce_store::SqliteNonceStore::open(&nonce_db_path) {
            Ok(store) => {
                tracing::info!("Nonce store: SQLite at {nonce_db_path}");
                Arc::new(store)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to open SQLite nonce store at {nonce_db_path}: {e} — using in-memory"
                );
                Arc::new(x402::nonce_store::InMemoryNonceStore::new())
            }
        };

    let facilitator = x402::TempoSchemeFacilitator::new(provider, facilitator_address)
        .with_nonce_store(nonce_store);

    // Start background nonce cleanup
    facilitator.start_nonce_cleanup();

    let hmac_secret = std::env::var("FACILITATOR_SHARED_SECRET")
        .ok()
        .map(|s| s.into_bytes());

    let allow_insecure = std::env::var("ALLOW_UNAUTHENTICATED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if hmac_secret.is_none() {
        if allow_insecure {
            tracing::warn!("FACILITATOR_SHARED_SECRET not set — HMAC auth disabled (ALLOW_UNAUTHENTICATED=true)");
        } else {
            tracing::error!("FACILITATOR_SHARED_SECRET not set. Set it for production, or set ALLOW_UNAUTHENTICATED=true for dev mode.");
            std::process::exit(1);
        }
    }

    let webhook_urls: Vec<String> = std::env::var("WEBHOOK_URLS")
        .ok()
        .map(|urls| {
            urls.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    if !webhook_urls.is_empty() {
        tracing::info!("Webhook URLs configured: {}", webhook_urls.len());
        x402_facilitator::webhook::validate_webhook_urls(&webhook_urls);
    }

    let state = web::Data::new(AppState {
        facilitator,
        hmac_secret,
        chain_config: x402::ChainConfig::default(),
        webhook_urls,
        http_client: reqwest::Client::new(),
    });

    let port: u16 = std::env::var("FACILITATOR_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4022);

    let rate_limit_rpm: u64 = std::env::var("RATE_LIMIT_RPM")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(120);

    let cors_origins = parse_cors_origins();

    tracing::info!("Tempo x402 Facilitator listening on port {port}");
    tracing::info!("Facilitator address: {facilitator_address}");
    tracing::info!("Rate limit: {rate_limit_rpm} req/min per IP");
    tracing::info!("  GET  http://localhost:{port}/supported");
    tracing::info!("  POST http://localhost:{port}/verify");
    tracing::info!("  POST http://localhost:{port}/verify-and-settle");

    let governor_conf = GovernorConfigBuilder::default()
        .requests_per_minute(rate_limit_rpm)
        .finish()
        .expect("failed to build rate limiter config");

    HttpServer::new(move || {
        App::new()
            .wrap(build_cors(&cors_origins))
            .wrap(Governor::new(&governor_conf))
            .app_data(state.clone())
            .app_data(web::JsonConfig::default().limit(65_536))
            .service(routes::health)
            .service(routes::metrics_endpoint)
            .service(routes::supported)
            .service(routes::verify)
            .service(routes::verify_and_settle)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
