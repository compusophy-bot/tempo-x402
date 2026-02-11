use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{middleware::Logger, web, App, HttpServer};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use x402_facilitator::state::AppState as FacilitatorState;
use x402_gateway::{
    config::GatewayConfig, db::Database, metrics::register_metrics, routes, state::AppState,
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let mut config = GatewayConfig::from_env().expect("Failed to load configuration");
    let port = config.port;
    let allowed_origins = config.allowed_origins.clone();
    let rate_limit_rpm = config.rate_limit_rpm;
    let spa_dir = config.spa_dir.clone();

    // Extract the private key early to minimize copies of key material in memory.
    let facilitator_private_key = config.facilitator_private_key.take();

    tracing::info!("Starting x402-gateway on port {}", port);
    tracing::info!("Platform address: {:#x}", config.platform_address);
    tracing::info!("Platform fee: {}", config.platform_fee);
    tracing::info!(
        "HMAC auth: {}",
        if config.hmac_secret.is_some() {
            "enabled"
        } else {
            "disabled (dev mode)"
        }
    );

    // Bootstrap embedded facilitator if FACILITATOR_PRIVATE_KEY is set
    let facilitator_state = if let Some(ref key) = facilitator_private_key {
        // Require HMAC when running embedded facilitator to prevent unauthenticated
        // access to the /facilitator/verify-and-settle endpoint
        if config.hmac_secret.is_none() {
            tracing::error!(
                "FACILITATOR_SHARED_SECRET is required when FACILITATOR_PRIVATE_KEY is set. \
                 Without HMAC, the embedded facilitator settlement endpoint is unauthenticated."
            );
            std::process::exit(1);
        }
        tracing::info!("Embedded facilitator: bootstrapping in-process");

        let signer: PrivateKeySigner = key.parse().expect("invalid FACILITATOR_PRIVATE_KEY");
        let facilitator_address = signer.address();

        let provider = ProviderBuilder::new()
            .wallet(alloy::network::EthereumWallet::from(signer))
            .connect_http(config.rpc_url.parse().expect("invalid RPC_URL"));

        // Set up nonce storage — SQLite is mandatory for replay protection
        let nonce_store: Arc<dyn x402::nonce_store::NonceStore> =
            match x402::nonce_store::SqliteNonceStore::open(&config.nonce_db_path) {
                Ok(store) => {
                    tracing::info!("Nonce store: SQLite at {}", config.nonce_db_path);
                    Arc::new(store)
                }
                Err(e) => {
                    // CRITICAL: Do not fall back to in-memory. In-memory nonces are lost
                    // on restart, enabling replay of any recently-settled payment.
                    tracing::error!(
                        "Failed to open SQLite nonce store at {}: {}",
                        config.nonce_db_path,
                        e
                    );
                    tracing::error!(
                        "Refusing to start — in-memory fallback would enable replay attacks on restart"
                    );
                    std::process::exit(1);
                }
            };

        let facilitator = x402::TempoSchemeFacilitator::new(provider, facilitator_address)
            .with_nonce_store(nonce_store);

        facilitator.start_nonce_cleanup();

        tracing::info!("Embedded facilitator address: {facilitator_address}");

        if !config.webhook_urls.is_empty() {
            tracing::info!("Webhook URLs configured: {}", config.webhook_urls.len());
            if let Err(e) = x402_facilitator::webhook::validate_webhook_urls(&config.webhook_urls) {
                tracing::error!("Invalid webhook configuration: {e}");
                std::process::exit(1);
            }
        }

        // Derive domain-separated webhook HMAC key
        let webhook_hmac_key = config
            .hmac_secret
            .as_ref()
            .map(|secret| x402::hmac::compute_hmac(secret, b"x402-webhook-hmac").into_bytes());

        Some(Arc::new(FacilitatorState {
            facilitator,
            // HMAC is guaranteed to be set when FACILITATOR_PRIVATE_KEY is configured
            // (enforced at lines 54-60 above).
            hmac_secret: config
                .hmac_secret
                .clone()
                .expect("HMAC secret must be set when embedded facilitator is enabled"),
            chain_config: x402::ChainConfig::default(),
            webhook_urls: config.webhook_urls.clone(),
            http_client: x402_facilitator::webhook::webhook_client(),
            metrics_token: config.metrics_token.as_ref().map(|t| t.as_bytes().to_vec()),
            webhook_hmac_key,
        }))
    } else {
        tracing::info!("Facilitator URL: {}", config.facilitator_url);
        None
    };

    // Initialize database
    let db = Database::new(&config.db_path).expect("Failed to initialize database");
    tracing::info!("Database initialized at: {}", config.db_path);

    // Purge stale slug reservations from previous crashes (older than 5 minutes)
    match db.purge_stale_reservations(300) {
        Ok(0) => {}
        Ok(n) => tracing::info!("Purged {n} stale slug reservations from previous runs"),
        Err(e) => tracing::warn!("Failed to purge stale reservations: {e}"),
    }

    // Clean up leftover e2e test endpoints
    match db.purge_endpoints_by_prefix("e2e-test-") {
        Ok(0) => {}
        Ok(n) => tracing::info!("Purged {n} stale e2e-test endpoints"),
        Err(e) => tracing::warn!("Failed to purge e2e-test endpoints: {e}"),
    }

    // Register Prometheus metrics
    register_metrics();

    // Create shared state
    let state = AppState::new(config, db, facilitator_state.clone());
    let state_data = web::Data::new(state);

    // Wrap facilitator state for facilitator routes (if embedded)
    let facilitator_data = facilitator_state.map(web::Data::from);

    // Configure rate limiter
    let governor_conf = GovernorConfigBuilder::default()
        .requests_per_minute(rate_limit_rpm as u64)
        .finish()
        .expect("Failed to create rate limiter config");

    if let Some(ref dir) = spa_dir {
        tracing::info!("Serving SPA from: {}", dir);
    }

    // Start HTTP server
    HttpServer::new(move || {
        // Configure CORS
        let allowed = allowed_origins.clone();
        let cors = Cors::default()
            .allowed_origin_fn(move |origin, _req_head| {
                let origin_str = origin.to_str().unwrap_or("");
                allowed.iter().any(|a| {
                    if a == "*" {
                        // In dev mode (X402_INSECURE_NO_HMAC), wildcard is permitted
                        // In production, wildcard CORS is rejected at config validation
                        true
                    } else {
                        a == origin_str
                    }
                })
            })
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::HeaderName::from_static("x-payment"),
                actix_web::http::header::HeaderName::from_static("payment-signature"),
            ])
            .expose_headers(vec![
                actix_web::http::header::HeaderName::from_static("x-payment-response"),
                actix_web::http::header::HeaderName::from_static("payment-response"),
            ])
            .max_age(3600);

        let mut app = App::new()
            .app_data(state_data.clone())
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024)) // 10MB body limit
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(Governor::new(&governor_conf))
            .configure(routes::health::configure)
            .configure(routes::register::configure)
            .configure(routes::endpoints::configure)
            .configure(routes::analytics::configure)
            .configure(routes::gateway::configure);

        // Mount facilitator HTTP routes if embedded (for external callers)
        if let Some(ref fac_data) = facilitator_data {
            app = app.service(
                web::scope("/facilitator")
                    .app_data(fac_data.clone())
                    .service(x402_facilitator::routes::supported)
                    .service(x402_facilitator::routes::verify_and_settle),
            );
        }

        // Serve SPA static files last (catch-all) if configured
        if let Some(ref dir) = spa_dir {
            let index_path = format!("{}/index.html", dir);
            app = app.service(
                actix_files::Files::new("/", dir)
                    .index_file("index.html")
                    .default_handler(web::to(move || {
                        let path = index_path.clone();
                        async move { actix_files::NamedFile::open_async(path).await }
                    })),
            );
        }

        app
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
