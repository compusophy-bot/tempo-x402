use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpServer};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
            .allowed_headers(vec!["content-type", "authorization", "x-facilitator-auth"])
            .max_age(3600)
    } else {
        let mut cors = Cors::default();
        for origin in origins {
            cors = cors.allowed_origin(origin);
        }
        cors.allow_any_method()
            .allowed_headers(vec!["content-type", "authorization", "x-facilitator-auth"])
            .max_age(3600)
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let key = std::env::var("FACILITATOR_PRIVATE_KEY")
        .expect("FACILITATOR_PRIVATE_KEY environment variable is required");

    let signer: PrivateKeySigner = key.parse().expect("invalid FACILITATOR_PRIVATE_KEY");
    let facilitator_address = signer.address();

    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| x402::constants::RPC_URL.to_string());

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
                // CRITICAL: Do not fall back to in-memory. In-memory nonces are lost
                // on restart, enabling replay of any recently-settled payment.
                tracing::error!("Failed to open SQLite nonce store at {nonce_db_path}: {e}");
                tracing::error!(
                    "Refusing to start — in-memory fallback would enable replay attacks on restart"
                );
                std::process::exit(1);
            }
        };

    let facilitator =
        x402::scheme_facilitator::TempoSchemeFacilitator::new(provider, facilitator_address)
            .with_nonce_store(nonce_store);

    // Start background nonce cleanup
    facilitator.start_nonce_cleanup();

    let hmac_secret: Vec<u8> = match std::env::var("FACILITATOR_SHARED_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
    {
        Some(s) => {
            let bytes = s.into_bytes();
            if bytes.len() < 32 {
                tracing::warn!(
                    "FACILITATOR_SHARED_SECRET is only {} bytes (minimum 32 recommended) — \
                     use `openssl rand -hex 32` to generate a secure secret",
                    bytes.len()
                );
            }
            bytes
        }
        None => {
            tracing::error!(
                "FACILITATOR_SHARED_SECRET is required. \
                 Set it to a secure random value (e.g. `openssl rand -hex 32`). \
                 For local development, any non-empty value will work."
            );
            std::process::exit(1);
        }
    };

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
        if let Err(e) = x402_facilitator::webhook::validate_webhook_urls(&webhook_urls) {
            tracing::error!("Invalid webhook configuration: {e}");
            std::process::exit(1);
        }
    }

    // Separate metrics token (falls back to HMAC secret for backward compat)
    let metrics_token = std::env::var("METRICS_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.into_bytes());

    if metrics_token.is_none() {
        tracing::warn!("METRICS_TOKEN not set — /metrics endpoint is publicly accessible");
    }

    // Derive a domain-separated webhook HMAC key from the shared secret
    let webhook_hmac_key =
        Some(x402::hmac::compute_hmac(&hmac_secret, b"x402-webhook-hmac").into_bytes());

    let state = web::Data::new(AppState {
        facilitator,
        hmac_secret,
        chain_config: x402::constants::ChainConfig::default(),
        webhook_urls,
        http_client: x402_facilitator::webhook::webhook_client(),
        metrics_token,
        webhook_hmac_key,
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
            .service(routes::verify_and_settle)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
