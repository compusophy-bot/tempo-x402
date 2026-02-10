//! x402-node: self-deploying x402 node with identity bootstrap + clone orchestration.
//!
//! Composes the x402-gateway (API proxy) with identity bootstrap and Railway
//! clone orchestration. Runs as a standalone binary that can spawn copies of itself.

use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{middleware::Logger, web, App, HttpServer};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use x402_agent::{CloneConfig, CloneOrchestrator, RailwayClient};
use x402_facilitator::state::AppState as FacilitatorState;
use x402_gateway::{
    config::GatewayConfig, db::Database, metrics::register_metrics, state::AppState as GatewayState,
};

mod db;
mod routes;
mod state;

use state::NodeState;

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

    // ── Identity bootstrap ──────────────────────────────────────────────
    // Must run BEFORE GatewayConfig::from_env() so that injected env vars
    // (EVM_ADDRESS, FACILITATOR_PRIVATE_KEY, FACILITATOR_SHARED_SECRET)
    // are visible to the gateway config loader.
    let auto_bootstrap = std::env::var("AUTO_BOOTSTRAP")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let identity = if auto_bootstrap {
        let identity_path =
            std::env::var("IDENTITY_PATH").unwrap_or_else(|_| "/data/identity.json".to_string());
        let id = x402_identity::bootstrap(&identity_path).expect("Failed to bootstrap identity");
        tracing::info!("Instance identity: {:#x} ({})", id.address, id.instance_id);
        Some(id)
    } else {
        tracing::info!("AUTO_BOOTSTRAP not set — running without identity");
        None
    };

    // ── Gateway config (same as gateway main.rs) ────────────────────────
    let mut config = GatewayConfig::from_env().expect("Failed to load configuration");
    let port = config.port;
    let allowed_origins = config.allowed_origins.clone();
    let rate_limit_rpm = config.rate_limit_rpm;
    let spa_dir = config.spa_dir.clone();
    let rpc_url = config.rpc_url.clone();
    let db_path = config.db_path.clone();

    // Extract the private key early to minimize copies of key material in memory.
    let facilitator_private_key = config.facilitator_private_key.take();

    tracing::info!("Starting x402-node on port {}", port);
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

    // ── Embedded facilitator bootstrap (same as gateway) ────────────────
    let facilitator_state = if let Some(ref key) = facilitator_private_key {
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

        let nonce_store: Arc<dyn x402::nonce_store::NonceStore> =
            match x402::nonce_store::SqliteNonceStore::open(&config.nonce_db_path) {
                Ok(store) => {
                    tracing::info!("Nonce store: SQLite at {}", config.nonce_db_path);
                    Arc::new(store)
                }
                Err(e) => {
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

        let webhook_hmac_key = config
            .hmac_secret
            .as_ref()
            .map(|secret| x402::hmac::compute_hmac(secret, b"x402-webhook-hmac").into_bytes());

        Some(Arc::new(FacilitatorState {
            facilitator,
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

    // ── Database ────────────────────────────────────────────────────────
    let gateway_db = Database::new(&config.db_path).expect("Failed to initialize database");
    tracing::info!("Database initialized at: {}", config.db_path);

    match gateway_db.purge_stale_reservations(300) {
        Ok(0) => {}
        Ok(n) => tracing::info!("Purged {n} stale slug reservations from previous runs"),
        Err(e) => tracing::warn!("Failed to purge stale reservations: {e}"),
    }

    // Initialize children table (node extension on top of gateway DB)
    db::init_children_schema(&gateway_db).expect("Failed to initialize children schema");
    tracing::info!("Children schema initialized");

    // Register Prometheus metrics
    register_metrics();

    // ── Gateway state ───────────────────────────────────────────────────
    let gateway_state = GatewayState::new(config, gateway_db, facilitator_state.clone());

    // ── Clone orchestrator ──────────────────────────────────────────────
    let railway_token = std::env::var("RAILWAY_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    let railway_project_id = std::env::var("RAILWAY_PROJECT_ID")
        .ok()
        .filter(|s| !s.is_empty());
    let docker_image = std::env::var("DOCKER_IMAGE").ok().filter(|s| !s.is_empty());

    let clone_price = std::env::var("CLONE_PRICE").ok().filter(|s| !s.is_empty());
    let clone_price_amount = clone_price.as_ref().map(|p| {
        x402_gateway::config::parse_price_to_amount(p).expect("Failed to parse CLONE_PRICE")
    });
    let clone_max_children: u32 = std::env::var("CLONE_MAX_CHILDREN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let self_url = std::env::var("RAILWAY_PUBLIC_DOMAIN")
        .ok()
        .map(|d| format!("https://{d}"))
        .or_else(|| std::env::var("SELF_URL").ok())
        .unwrap_or_else(|| format!("http://localhost:{port}"));

    let agent: Option<Arc<CloneOrchestrator>> = match (
        railway_token,
        railway_project_id,
        docker_image,
    ) {
        (Some(token), Some(project_id), Some(image)) => {
            tracing::info!("Clone orchestrator: enabled (image: {})", image);
            let railway = RailwayClient::new(token, project_id);
            let clone_config = CloneConfig {
                docker_image: image,
                rpc_url: rpc_url.clone(),
                self_url: self_url.clone(),
                max_children: clone_max_children,
            };
            Some(Arc::new(CloneOrchestrator::new(railway, clone_config)))
        }
        _ => {
            tracing::info!("Clone orchestrator: disabled (missing RAILWAY_TOKEN, RAILWAY_PROJECT_ID, or DOCKER_IMAGE)");
            None
        }
    };

    if clone_price.is_some() {
        tracing::info!(
            "Clone price: {} (max children: {})",
            clone_price.as_deref().unwrap_or("?"),
            clone_max_children
        );
    }

    // ── Node state ──────────────────────────────────────────────────────
    let node_state = NodeState {
        gateway: gateway_state,
        identity: identity.clone(),
        agent,
        started_at: chrono::Utc::now(),
        db_path: db_path.clone(),
        clone_price,
        clone_price_amount,
        clone_max_children,
    };

    let node_data = web::Data::new(node_state.clone());
    let gateway_data = web::Data::new(node_state.gateway.clone());
    let facilitator_data = facilitator_state.map(web::Data::from);

    // ── Background tasks ────────────────────────────────────────────────
    if let Some(ref id) = identity {
        let rpc = rpc_url.clone();
        let addr = id.address;
        // Faucet funding (best-effort)
        tokio::spawn(async move {
            if let Err(e) = x402_identity::request_faucet_funds(&rpc, addr).await {
                tracing::warn!("Faucet funding failed (non-fatal): {e}");
            }
        });

        // Parent registration (if PARENT_URL set)
        if let Some(ref parent_url) = id.parent_url {
            let parent = parent_url.clone();
            let id_clone = id.clone();
            let url = self_url.clone();
            tokio::spawn(async move {
                if let Err(e) = x402_identity::register_with_parent(&parent, &id_clone, &url).await
                {
                    tracing::warn!("Parent registration failed (non-fatal): {e}");
                }
            });
        }
    }

    // ── Background: version check + auto-redeploy outdated children ────
    if node_state.agent.is_some() {
        let version_check_state = node_state.clone();
        tokio::spawn(async move {
            // Wait for children to finish booting
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let parent_version = env!("CARGO_PKG_VERSION");
            tracing::info!("Checking children versions against parent v{parent_version}");

            let children = match rusqlite::Connection::open(&version_check_state.db_path) {
                Ok(conn) => db::query_children_active(&conn).unwrap_or_default(),
                Err(e) => {
                    tracing::warn!("Version check: failed to open db: {e}");
                    return;
                }
            };

            let running: Vec<_> = children
                .into_iter()
                .filter(|c| c.status == "running" && c.url.is_some())
                .collect();

            if running.is_empty() {
                tracing::info!("Version check: no running children to check");
                return;
            }

            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default();

            let agent = match version_check_state.agent.as_ref() {
                Some(a) => a,
                None => return,
            };

            for child in &running {
                let url = match child.url.as_ref() {
                    Some(u) => u,
                    None => continue,
                };

                let health_url = format!("{url}/health");
                let child_version = match http.get(&health_url).send().await {
                    Ok(resp) => match resp.json::<serde_json::Value>().await {
                        Ok(json) => json
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        Err(e) => {
                            tracing::warn!(
                                instance_id = %child.instance_id,
                                error = %e,
                                "Version check: failed to parse health response"
                            );
                            continue;
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            instance_id = %child.instance_id,
                            error = %e,
                            "Version check: failed to reach child"
                        );
                        continue;
                    }
                };

                if child_version == parent_version {
                    tracing::debug!(
                        instance_id = %child.instance_id,
                        version = %child_version,
                        "Child is up to date"
                    );
                    continue;
                }

                tracing::info!(
                    instance_id = %child.instance_id,
                    child_version = %child_version,
                    parent_version = %parent_version,
                    "Child version mismatch — triggering redeploy"
                );

                let service_id = match child.railway_service_id.as_ref() {
                    Some(id) => id,
                    None => {
                        tracing::warn!(
                            instance_id = %child.instance_id,
                            "Cannot redeploy: no Railway service ID"
                        );
                        continue;
                    }
                };

                match agent.redeploy_clone(service_id).await {
                    Ok(_) => {
                        if let Err(e) = db::update_child_status(
                            &version_check_state.gateway.db,
                            &child.instance_id,
                            "deploying",
                        ) {
                            tracing::warn!(
                                instance_id = %child.instance_id,
                                error = %e,
                                "Failed to update status after auto-redeploy"
                            );
                        }
                        tracing::info!(
                            instance_id = %child.instance_id,
                            "Auto-redeploy triggered"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            instance_id = %child.instance_id,
                            error = %e,
                            "Auto-redeploy failed (non-fatal)"
                        );
                    }
                }
            }

            tracing::info!("Version check complete");
        });
    }

    // ── Rate limiter ────────────────────────────────────────────────────
    let governor_conf = GovernorConfigBuilder::default()
        .requests_per_minute(rate_limit_rpm as u64)
        .finish()
        .expect("Failed to create rate limiter config");

    if let Some(ref dir) = spa_dir {
        tracing::info!("Serving SPA from: {}", dir);
    }

    // ── HTTP server ─────────────────────────────────────────────────────
    HttpServer::new(move || {
        let allowed = allowed_origins.clone();
        let cors = Cors::default()
            .allowed_origin_fn(move |origin, _req_head| {
                let origin_str = origin.to_str().unwrap_or("");
                allowed
                    .iter()
                    .any(|a| if a == "*" { true } else { a == origin_str })
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
            .app_data(gateway_data.clone())
            .app_data(node_data.clone())
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(Governor::new(&governor_conf))
            // Gateway routes
            .configure(x402_gateway::routes::health::configure)
            .configure(x402_gateway::routes::register::configure)
            .configure(x402_gateway::routes::endpoints::configure)
            .configure(x402_gateway::routes::gateway::configure)
            // Node routes (identity, clone)
            .configure(crate::routes::instance::configure)
            .configure(crate::routes::clone::configure);

        // Mount facilitator HTTP routes if embedded
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
