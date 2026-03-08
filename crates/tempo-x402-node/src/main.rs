//! x402-node: self-deploying x402 node with identity bootstrap + clone orchestration.
//!
//! Composes the x402-gateway (API proxy) with identity bootstrap and Railway
//! clone orchestration. Runs as a standalone binary that can spawn copies of itself.

use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{middleware::Logger, web, App, HttpServer};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "agent")]
use crate::clone::{CloneConfig, CloneOrchestrator};
#[cfg(feature = "agent")]
use crate::railway::RailwayClient;
use x402_gateway::{
    config::GatewayConfig, db::Database, metrics::register_metrics, state::AppState as GatewayState,
};

#[cfg(feature = "agent")]
#[allow(dead_code)]
mod clone;
mod db;
#[cfg(feature = "agent")]
#[allow(dead_code)]
mod railway;
mod routes;
#[cfg(feature = "soul")]
mod soul_observer;
mod state;

use state::NodeState;

/// Admin endpoint: POST /admin/endpoints — register a script endpoint without payment.
/// Intended for soul/local tools only. No authentication required (local access).
async fn admin_register_endpoint(
    body: web::Json<serde_json::Value>,
    state: web::Data<NodeState>,
) -> actix_web::HttpResponse {
    let slug = body["slug"].as_str().unwrap_or_default();
    let description = body["description"].as_str().unwrap_or("Script endpoint");

    if slug.is_empty() {
        return actix_web::HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "slug is required",
        }));
    }

    // Build target URL from self_url (instance's own URL)
    let self_url = std::env::var("RAILWAY_PUBLIC_DOMAIN")
        .map(|d| format!("https://{d}"))
        .unwrap_or_else(|_| {
            let port = std::env::var("PORT").unwrap_or_else(|_| "4023".to_string());
            format!("http://localhost:{port}")
        });
    // The script file on disk uses the slug without "script-" prefix
    let stem = slug.strip_prefix("script-").unwrap_or(slug);
    let target = format!("{self_url}/x/{stem}");
    let owner = std::env::var("EVM_ADDRESS").unwrap_or_default();

    match state.gateway.db.create_endpoint(
        slug,
        &owner,
        &target,
        "$0.001",
        "1000",
        Some(description),
    ) {
        Ok(_) => {
            tracing::info!(slug = %slug, "Admin registered endpoint");
            actix_web::HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "slug": slug,
                "target_url": target,
            }))
        }
        Err(e) => {
            // Already exists is not an error — update description silently
            tracing::debug!(slug = %slug, error = %e, "Endpoint registration skipped (likely exists)");
            actix_web::HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "slug": slug,
                "note": "already registered",
            }))
        }
    }
}

/// Admin endpoint: DELETE /admin/endpoints/{slug} — deactivate an endpoint without payment.
/// Intended for soul/local tools only. No authentication required (local access).
async fn admin_delete_endpoint(
    path: web::Path<String>,
    state: web::Data<NodeState>,
) -> actix_web::HttpResponse {
    let slug = path.into_inner();
    match state.gateway.db.delete_endpoint(&slug) {
        Ok(()) => {
            tracing::info!(slug = %slug, "Admin deleted endpoint");
            actix_web::HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": format!("Endpoint '{}' deactivated", slug),
            }))
        }
        Err(e) => actix_web::HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": format!("{e}"),
        })),
    }
}

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

        Some(
            x402_gateway::facilitator::bootstrap::bootstrap_embedded_facilitator(
                x402_gateway::facilitator::bootstrap::BootstrapConfig {
                    private_key: key,
                    rpc_url: &config.rpc_url,
                    nonce_db_path: &config.nonce_db_path,
                    hmac_secret: config
                        .hmac_secret
                        .clone()
                        .expect("HMAC secret must be set when embedded facilitator is enabled"),
                    webhook_urls: config.webhook_urls.clone(),
                    metrics_token: config.metrics_token.as_ref().map(|t| t.as_bytes().to_vec()),
                },
            ),
        )
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

    // Clean up leftover e2e test endpoints
    match gateway_db.purge_endpoints_by_prefix("e2e-test-") {
        Ok(0) => {}
        Ok(n) => tracing::info!("Purged {n} stale e2e-test endpoints"),
        Err(e) => tracing::warn!("Failed to purge e2e-test endpoints: {e}"),
    }

    // Initialize children table (node extension on top of gateway DB)
    db::init_children_schema(&gateway_db).expect("Failed to initialize children schema");
    tracing::info!("Children schema initialized");

    // Register Prometheus metrics
    register_metrics();

    // ── Clone orchestrator config ───────────────────────────────────────
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

    if clone_price.is_some() {
        tracing::info!(
            "Clone price: {} (max children: {})",
            clone_price.as_deref().unwrap_or("?"),
            clone_max_children
        );
    }

    // ── Auto-register node endpoints ────────────────────────────────────
    let owner = std::env::var("EVM_ADDRESS").unwrap_or_default();
    if !owner.is_empty() {
        let default_clone_price = "$1.00".to_string();
        let default_clone_amount = "1000000".to_string();
        let endpoints: Vec<(&str, String, &str, &str, &str)> = vec![
            (
                "chat",
                format!("{}/soul/chat", self_url),
                "$0.01",
                "10000",
                "Interactive chat with the node's soul",
            ),
            (
                "soul",
                format!("{}/soul/status", self_url),
                "$0.0001",
                "100",
                "Soul status and recent thoughts",
            ),
            (
                "chat-sessions",
                format!("{}/soul/chat/sessions", self_url),
                "$0.0001",
                "100",
                "List soul chat sessions",
            ),
            (
                "session-messages",
                format!("{}/soul/chat/sessions/{{id}}", self_url),
                "$0.0001",
                "100",
                "View session messages",
            ),
            (
                "pending-plan",
                format!("{}/soul/plan/pending", self_url),
                "$0.0001",
                "100",
                "Get pending plan needing approval",
            ),
            (
                "nudges",
                format!("{}/soul/nudges", self_url),
                "$0.0001",
                "100",
                "List unprocessed nudges",
            ),
            (
                "info",
                format!("{}/instance/info", self_url),
                "$0.0001",
                "100",
                "Node identity, version, uptime",
            ),
            (
                "clone",
                format!("{}/clone", self_url),
                clone_price.as_deref().unwrap_or(&default_clone_price),
                clone_price_amount
                    .as_deref()
                    .unwrap_or(&default_clone_amount),
                "Spawn a new x402-node instance",
            ),
        ];
        for (slug, target, price, amount, desc) in &endpoints {
            match gateway_db.create_endpoint(slug, &owner, target, price, amount, Some(desc)) {
                Ok(_) => tracing::info!(slug, "Auto-registered endpoint"),
                Err(_) => tracing::debug!(slug, "Endpoint already exists, skipping"),
            }
        }

        // Auto-register existing script endpoints with default pricing
        let scripts_dir = std::path::Path::new("/data/endpoints");
        if scripts_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(scripts_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "sh") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            // Strip any existing "script-" prefix to avoid double-prefixing
                            let base = stem.strip_prefix("script-").unwrap_or(stem);
                            let script_slug = format!("script-{base}");
                            let target = format!("{}/x/{stem}", self_url);
                            // Read first line as description
                            let desc = std::fs::read_to_string(&path)
                                .ok()
                                .and_then(|content| {
                                    content
                                        .lines()
                                        .next()
                                        .and_then(|line| line.strip_prefix("# ").map(String::from))
                                })
                                .unwrap_or_else(|| format!("Script endpoint: {stem}"));
                            let (price_usd, price_amount) = routes::scripts::get_script_pricing(base);
                            match gateway_db.create_endpoint(
                                &script_slug,
                                &owner,
                                &target,
                                price_usd,
                                price_amount,
                                Some(&desc),
                            ) {
                                Ok(_) => {
                                    tracing::info!(slug = %script_slug, "Auto-registered script endpoint")
                                }
                                Err(_) => {
                                    tracing::debug!(slug = %script_slug, "Script endpoint already registered")
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Gateway state ───────────────────────────────────────────────────
    let gateway_state = GatewayState::new(config, gateway_db, facilitator_state.clone());

    // ── Clone orchestrator ──────────────────────────────────────────────
    #[cfg(feature = "agent")]
    let agent: Option<Arc<CloneOrchestrator>> = {
        let railway_token = std::env::var("RAILWAY_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());
        let railway_project_id = std::env::var("RAILWAY_PROJECT_ID")
            .ok()
            .filter(|s| !s.is_empty());
        let docker_image = std::env::var("DOCKER_IMAGE").ok().filter(|s| !s.is_empty());
        let source_repo = std::env::var("CLONE_SOURCE_REPO")
            .ok()
            .filter(|s| !s.is_empty());
        let github_token = std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty());

        // Clone orchestrator requires Railway credentials + at least one deployment source
        let has_deploy_source = docker_image.is_some() || source_repo.is_some();

        match (railway_token, railway_project_id, has_deploy_source) {
            (Some(token), Some(project_id), true) => {
                if let Some(ref repo) = source_repo {
                    tracing::info!("Clone orchestrator: enabled (source: {})", repo);
                } else if let Some(ref image) = docker_image {
                    tracing::info!("Clone orchestrator: enabled (image: {})", image);
                }
                let railway = RailwayClient::new(token, project_id);
                let clone_cpu: u32 = std::env::var("CLONE_CPU_MILLICORES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(2000);
                let clone_mem: u32 = std::env::var("CLONE_MEMORY_MB")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(2048);

                // Build child env vars from parent's soul-related env vars
                let mut child_env_vars = std::collections::HashMap::new();
                let child_multiplier =
                    std::env::var("CLONE_CYCLE_MULTIPLIER").unwrap_or_else(|_| "3.0".to_string());
                child_env_vars.insert("SOUL_CYCLE_MULTIPLIER".into(), child_multiplier);
                if let Ok(key) = std::env::var("GEMINI_API_KEY") {
                    child_env_vars.insert("GEMINI_API_KEY".into(), key);
                }
                child_env_vars.insert("SOUL_CODING_ENABLED".into(), "true".into());
                child_env_vars.insert("SOUL_DB_PATH".into(), "/data/soul.db".into());

                // Source-based clones get coding-specific env vars
                if source_repo.is_some() {
                    if let Some(ref repo) = source_repo {
                        // Clone's SOUL_FORK_REPO = same fork (it pushes to its own branch)
                        child_env_vars.insert("SOUL_FORK_REPO".into(), repo.clone());
                        // Clone PRs target fork's main
                        child_env_vars.insert("SOUL_UPSTREAM_REPO".into(), repo.clone());
                    }
                    child_env_vars.insert("SOUL_DIRECT_PUSH".into(), "true".into());
                    if let Some(ref gh_token) = github_token {
                        child_env_vars.insert("GITHUB_TOKEN".into(), gh_token.clone());
                    }
                } else {
                    // Docker-based clones: inherit parent's fork/upstream settings
                    if let Ok(fork) = std::env::var("SOUL_FORK_REPO") {
                        child_env_vars.insert("SOUL_FORK_REPO".into(), fork);
                    }
                    if let Ok(upstream) = std::env::var("SOUL_UPSTREAM_REPO") {
                        child_env_vars.insert("SOUL_UPSTREAM_REPO".into(), upstream);
                    }
                }

                let clone_config = CloneConfig {
                    docker_image,
                    source_repo,
                    github_token,
                    rpc_url: rpc_url.clone(),
                    self_url: self_url.clone(),
                    max_children: clone_max_children,
                    clone_cpu_millicores: clone_cpu,
                    clone_memory_mb: clone_mem,
                    child_env_vars,
                };
                Some(Arc::new(CloneOrchestrator::new(railway, clone_config)))
            }
            _ => {
                tracing::info!("Clone orchestrator: disabled (missing RAILWAY_TOKEN, RAILWAY_PROJECT_ID, or deployment source)");
                None
            }
        }
    };
    #[cfg(not(feature = "agent"))]
    let agent: Option<()> = None;

    // ── Soul init (before NodeState so we can store the DB ref) ────────
    #[cfg(feature = "soul")]
    let (
        soul_db,
        soul_dormant,
        soul,
        soul_generation,
        soul_config_for_state,
        soul_thinking_enabled,
    ) = match x402_soul::SoulConfig::from_env() {
        Ok(soul_config) => {
            let dormant = soul_config.llm_api_key.is_none();
            let generation = soul_config.generation;
            let thinking = soul_config.thinking_enabled;
            let config_clone = soul_config.clone();
            match x402_soul::Soul::new(soul_config) {
                Ok(soul) => {
                    let db = soul.database().clone();
                    (
                        Some(db),
                        dormant,
                        Some(soul),
                        generation,
                        Some(config_clone),
                        thinking,
                    )
                }
                Err(e) => {
                    tracing::warn!("Soul init failed (non-fatal): {e}");
                    (None, true, None, generation, None, false)
                }
            }
        }
        Err(e) => {
            tracing::warn!("Soul config failed (non-fatal): {e}");
            (None, true, None, 0, None, false)
        }
    };
    #[cfg(not(feature = "soul"))]
    let (soul_db, soul_dormant, soul_generation): (Option<()>, bool, u32) = (None, true, 0);

    // ── Node state ──────────────────────────────────────────────────────
    let started_at = chrono::Utc::now();

    // Build observer early so we can share it between NodeState and soul spawn
    #[cfg(feature = "soul")]
    let soul_observer_impl: Option<std::sync::Arc<soul_observer::NodeObserverImpl>> =
        if soul.is_some() || soul_config_for_state.is_some() {
            Some(soul_observer::NodeObserverImpl::new(
                gateway_state.clone(),
                identity.clone(),
                soul_generation,
                started_at,
                db_path.clone(),
            ))
        } else {
            None
        };
    let soul_observer: Option<std::sync::Arc<dyn x402_soul::NodeObserver>> = soul_observer_impl
        .clone()
        .map(|o| o as std::sync::Arc<dyn x402_soul::NodeObserver>);
    #[cfg(not(feature = "soul"))]
    let soul_observer: Option<()> = None;

    // ── ERC-8004 reputation channel ─────────────────────────────────────
    #[cfg(feature = "erc8004")]
    let reputation_tx = if x402_identity::reputation_enabled() {
        let rep_registry = x402_identity::reputation_registry();
        if rep_registry != alloy::primitives::Address::ZERO {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<state::SettlementEvent>(256);
            let rep_rpc = rpc_url.clone();
            let agent_token = identity.as_ref().and_then(|id| id.agent_token_id.clone());
            let rep_private_key = identity.as_ref().map(|id| id.private_key.clone());
            tokio::spawn(async move {
                use x402_identity::types::AgentId;

                let Some(pk) = rep_private_key else {
                    tracing::info!("ERC-8004 reputation: no identity, skipping");
                    while rx.recv().await.is_some() {}
                    return;
                };
                let signer: alloy::signers::local::PrivateKeySigner = pk
                    .strip_prefix("0x")
                    .unwrap_or(&pk)
                    .parse()
                    .expect("invalid private key");
                let wallet = alloy::network::EthereumWallet::from(signer);
                let provider = alloy::providers::ProviderBuilder::new()
                    .wallet(wallet)
                    .connect_http(rep_rpc.parse().expect("invalid RPC URL"));

                let Some(token_id_str) = agent_token else {
                    tracing::info!("ERC-8004 reputation: no agent token ID, skipping");
                    // Drain channel without submitting
                    while rx.recv().await.is_some() {}
                    return;
                };

                let agent_id = AgentId::new(
                    token_id_str
                        .parse::<alloy::primitives::U256>()
                        .unwrap_or_default(),
                );

                tracing::info!(agent_id = %agent_id, "ERC-8004 reputation submitter started");

                while let Some(event) = rx.recv().await {
                    let metadata = event.tx_hash.as_deref().unwrap_or(&event.endpoint_slug);
                    if let Err(e) = x402_identity::reputation::submit_feedback(
                        &provider,
                        rep_registry,
                        &agent_id,
                        true, // positive feedback for successful settlement
                        metadata,
                    )
                    .await
                    {
                        tracing::debug!(
                            endpoint = %event.endpoint_slug,
                            error = %e,
                            "Reputation submission failed (non-fatal)"
                        );
                    }
                }
            });
            Some(tx)
        } else {
            None
        }
    } else {
        None
    };

    // Soul liveness flag — shared between NodeState (for health checks) and the soul task.
    // Created upfront so the Arc is shared before NodeState is cloned into web::Data.
    let soul_alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let node_state = NodeState {
        gateway: gateway_state,
        identity: identity.clone(),
        agent,
        started_at,
        db_path: db_path.clone(),
        clone_price,
        clone_price_amount,
        clone_max_children,
        agent_token_id: identity.as_ref().and_then(|id| id.agent_token_id.clone()),
        #[cfg(feature = "erc8004")]
        reputation_tx,
        soul_db,
        soul_dormant,
        #[cfg(feature = "soul")]
        soul_config: soul_config_for_state,
        #[cfg(not(feature = "soul"))]
        soul_config: None,
        #[cfg(feature = "soul")]
        soul_observer: soul_observer.clone(),
        #[cfg(not(feature = "soul"))]
        soul_observer: None,
        soul_alive: Some(soul_alive.clone()),
    };

    let node_data = web::Data::new(node_state.clone());
    let gateway_data = web::Data::new(node_state.gateway.clone());
    let facilitator_data = facilitator_state.map(web::Data::from);

    // ── Soul spawn ────────────────────────────────────────────────────
    #[cfg(feature = "soul")]
    if let Some(soul) = soul {
        if let Some(observer) = soul_observer {
            if soul_thinking_enabled {
                // Spawn background peer discovery refresh (every 5 minutes)
                if let Some(ref obs_impl) = soul_observer_impl {
                    let obs_for_peers = obs_impl.clone();
                    tokio::spawn(async move {
                        // Initial delay — let the node finish starting up
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        loop {
                            obs_for_peers.refresh_peers().await;
                            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                        }
                    });
                }

                let _soul_handle = soul.spawn(observer, soul_alive.clone());
                tracing::info!(
                    dormant = node_state.soul_dormant,
                    generation = soul_generation,
                    "Soul spawned"
                );
            } else {
                tracing::info!("Soul thinking disabled (SOUL_THINKING_ENABLED=false)");
            }
        }
    }

    // ── Background tasks ────────────────────────────────────────────────
    if let Some(ref id) = identity {
        let rpc = rpc_url.clone();
        let addr = id.address;
        // Faucet funding (only if balance is low)
        tokio::spawn(async move {
            // Check balance first — skip faucet if already funded
            let token = x402::constants::DEFAULT_TOKEN;
            if let Ok(rpc_parsed) = rpc.parse::<reqwest::Url>() {
                let provider = alloy::providers::ProviderBuilder::new().connect_http(rpc_parsed);
                if let Ok(balance) = x402::tip20::balance_of(&provider, token, addr).await {
                    // 10 pathUSD (10 * 10^6) threshold — skip if already funded
                    if balance >= alloy::primitives::U256::from(10_000_000u64) {
                        tracing::info!(
                            address = %addr,
                            balance = %balance,
                            "Wallet already funded, skipping faucet"
                        );
                        return;
                    }
                }
            }
            if let Err(e) = x402_identity::request_faucet_funds(&rpc, addr).await {
                tracing::warn!("Faucet funding failed (non-fatal): {e}");
            }
        });

        // Auto-approve own facilitator for pathUSD (needed for x402 payments).
        // The node's wallet key and facilitator key are the same, so facilitator = self.
        // Wait for faucet to fund first (need gas for approve tx).
        {
            let rpc = rpc_url.clone();
            let pk = id.private_key.clone();
            tokio::spawn(async move {
                // Wait for faucet to settle
                tokio::time::sleep(std::time::Duration::from_secs(20)).await;

                let Ok(rpc_parsed) = rpc.parse::<reqwest::Url>() else {
                    return;
                };
                let Ok(signer) = pk
                    .strip_prefix("0x")
                    .unwrap_or(&pk)
                    .parse::<alloy::signers::local::PrivateKeySigner>()
                else {
                    return;
                };
                let wallet_addr = signer.address();
                let wallet = alloy::network::EthereumWallet::from(signer);
                let provider = alloy::providers::ProviderBuilder::new()
                    .wallet(wallet)
                    .connect_http(rpc_parsed);
                let token = x402::constants::DEFAULT_TOKEN;

                // Check current allowance (self-approval: wallet approves itself as facilitator)
                let current_allowance =
                    x402::tip20::allowance(&provider, token, wallet_addr, wallet_addr)
                        .await
                        .unwrap_or(alloy::primitives::U256::ZERO);

                if current_allowance < alloy::primitives::U256::from(1_000_000_000_000_000u64) {
                    match x402::tip20::approve(
                        &provider,
                        token,
                        wallet_addr,
                        alloy::primitives::U256::MAX,
                    )
                    .await
                    {
                        Ok(tx) => {
                            tracing::info!(
                                address = %wallet_addr,
                                tx = %tx,
                                "Auto-approved own facilitator for pathUSD"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "Auto-approval failed (non-fatal — can retry via /wallet/setup)"
                            );
                        }
                    }
                } else {
                    tracing::debug!(
                        address = %wallet_addr,
                        "Facilitator already approved, skipping"
                    );
                }
            });
        }

        // ERC-8004 auto-deploy + auto-mint (if enabled and no token ID yet)
        #[cfg(feature = "erc8004")]
        if x402_identity::auto_mint_enabled() && id.agent_token_id.is_none() {
            // Try loading previously deployed registries from disk
            let registries_path = std::env::var("ERC8004_REGISTRIES_PATH")
                .unwrap_or_else(|_| "/data/erc8004_registries.json".to_string());
            x402_identity::load_persisted_registries(&registries_path);

            let rpc_clone = rpc_url.clone();
            let owner = id.address;
            let metadata_uri = format!("{}/instance/info", self_url);
            let identity_path = std::env::var("IDENTITY_PATH")
                .unwrap_or_else(|_| "/data/identity.json".to_string());
            let mut id_clone = id.clone();
            let private_key = id.private_key.clone();
            let reg_path = registries_path.clone();
            tokio::spawn(async move {
                // Wait for faucet to fund the wallet first
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;

                let signer: alloy::signers::local::PrivateKeySigner = private_key
                    .strip_prefix("0x")
                    .unwrap_or(&private_key)
                    .parse()
                    .expect("invalid private key");
                let wallet = alloy::network::EthereumWallet::from(signer);
                let provider = alloy::providers::ProviderBuilder::new()
                    .wallet(wallet)
                    .connect_http(rpc_clone.parse().expect("invalid RPC URL"));

                // If no identity registry is configured, self-deploy contracts
                let mut identity_registry = x402_identity::identity_registry();
                if identity_registry == alloy::primitives::Address::ZERO {
                    tracing::info!(
                        "ERC-8004: no registry addresses configured — self-deploying contracts"
                    );
                    match x402_identity::deploy::deploy_all(&provider).await {
                        Ok(registries) => {
                            tracing::info!(
                                identity = %registries.identity,
                                reputation = %registries.reputation,
                                validation = %registries.validation,
                                "ERC-8004: contracts deployed"
                            );
                            // Set env vars so the rest of startup picks them up
                            std::env::set_var(
                                "ERC8004_IDENTITY_REGISTRY",
                                format!("{:#x}", registries.identity),
                            );
                            std::env::set_var(
                                "ERC8004_REPUTATION_REGISTRY",
                                format!("{:#x}", registries.reputation),
                            );
                            std::env::set_var(
                                "ERC8004_VALIDATION_REGISTRY",
                                format!("{:#x}", registries.validation),
                            );
                            identity_registry = registries.identity;
                            // Persist to disk for next restart
                            if let Err(e) =
                                x402_identity::save_deployed_registries(&reg_path, &registries)
                            {
                                tracing::warn!("Failed to persist registry addresses: {e}");
                            }
                        }
                        Err(e) => {
                            tracing::warn!("ERC-8004 contract deployment failed (non-fatal): {e}");
                            return;
                        }
                    }
                }

                tracing::info!("ERC-8004: attempting to mint agent identity NFT");
                match x402_identity::identity::mint(
                    &provider,
                    identity_registry,
                    owner,
                    &metadata_uri,
                )
                .await
                {
                    Ok(agent_id) => {
                        tracing::info!(
                            token_id = %agent_id,
                            "ERC-8004: agent identity minted"
                        );
                        if let Err(e) = x402_identity::save_agent_token_id(
                            &identity_path,
                            &mut id_clone,
                            &agent_id.to_string(),
                        ) {
                            tracing::warn!("Failed to persist agent token ID: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("ERC-8004 mint failed (non-fatal): {e}");
                    }
                }
            });
        }

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

    // ── Background: health probe + version check + auto-redeploy ───────
    #[cfg(feature = "agent")]
    if node_state.agent.is_some() {
        let version_check_state = node_state.clone();
        tokio::spawn(async move {
            // Wait for children to finish booting
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;

            let probe_interval_secs: u64 = std::env::var("HEALTH_PROBE_INTERVAL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300);
            let probe_interval = std::time::Duration::from_secs(probe_interval_secs);

            let parent_version = env!("CARGO_PKG_VERSION");
            let parent_build = {
                let compile_time = env!("GIT_SHA");
                if compile_time != "dev" {
                    compile_time.to_string()
                } else {
                    std::env::var("RAILWAY_GIT_COMMIT_SHA").unwrap_or_else(|_| "dev".to_string())
                }
            };

            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default();

            let agent = match version_check_state.agent.as_ref() {
                Some(a) => a,
                None => {
                    tracing::warn!("Health probe: no agent available, exiting");
                    return;
                }
            };

            tracing::info!(
                interval_secs = probe_interval_secs,
                "Health probe loop started (parent v{parent_version} build={parent_build})"
            );

            loop {
                let children = match rusqlite::Connection::open(&version_check_state.db_path) {
                    Ok(conn) => db::query_children_active(&conn).unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!("Version check: failed to open db: {e}");
                        tokio::time::sleep(probe_interval).await;
                        continue;
                    }
                };

                // Children with a URL that we can probe (running OR stuck deploying)
                let probeworthy: Vec<_> = children
                    .into_iter()
                    .filter(|c| {
                        c.url.is_some() && (c.status == "running" || c.status == "deploying")
                    })
                    .collect();

                if probeworthy.is_empty() {
                    tracing::debug!("Version check: no children to check");
                    tokio::time::sleep(probe_interval).await;
                    continue;
                }

                for child in &probeworthy {
                    let url = match child.url.as_ref() {
                        Some(u) => u,
                        None => continue,
                    };

                    // Probe /health to see if the child is actually alive
                    let health_url = format!("{url}/health");
                    let health_json = match http.get(&health_url).send().await {
                        Ok(resp) => match resp.json::<serde_json::Value>().await {
                            Ok(json) => json,
                            Err(e) => {
                                tracing::warn!(
                                    instance_id = %child.instance_id,
                                    error = %e,
                                    "Health probe: failed to parse response"
                                );

                                // Mark children returning bad health responses as failed after timeout
                                let age_secs = chrono::Utc::now().timestamp() - child.created_at;
                                let stale = match child.status.as_str() {
                                    "deploying" => age_secs > 600, // 10 min
                                    "running" => age_secs > 300,   // 5 min
                                    _ => false,
                                };
                                if stale {
                                    tracing::info!(
                                        instance_id = %child.instance_id,
                                        status = %child.status,
                                        age_secs = age_secs,
                                        "Marking child with bad health response as failed"
                                    );
                                    let _ = db::mark_child_failed(
                                        &version_check_state.gateway.db,
                                        &child.instance_id,
                                    );
                                }

                                continue;
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                instance_id = %child.instance_id,
                                error = %e,
                                "Health probe: failed to reach child"
                            );

                            // Mark unreachable children as failed if they've been around long enough
                            let age_secs = chrono::Utc::now().timestamp() - child.created_at;
                            let stale = match child.status.as_str() {
                                "deploying" => age_secs > 600, // 10 min for deploying
                                "running" => age_secs > 300, // 5 min for running (was reachable before)
                                _ => false,
                            };

                            if stale {
                                tracing::info!(
                                    instance_id = %child.instance_id,
                                    status = %child.status,
                                    age_secs = age_secs,
                                    "Marking unreachable child as failed"
                                );
                                if let Err(mark_err) = db::mark_child_failed(
                                    &version_check_state.gateway.db,
                                    &child.instance_id,
                                ) {
                                    tracing::warn!(
                                        instance_id = %child.instance_id,
                                        error = %mark_err,
                                        "Failed to mark unreachable child as failed"
                                    );
                                }
                            }

                            continue;
                        }
                    };

                    let child_version = health_json
                        .get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let child_build = health_json
                        .get("build")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // ── Fix stuck "deploying" children ──────────────────────
                    // Child is alive but parent DB still says "deploying".
                    // Fetch its identity and promote to "running".
                    if child.status == "deploying" {
                        tracing::info!(
                            instance_id = %child.instance_id,
                            "Stuck deploying child is alive — recovering status"
                        );

                        // Try to get the child's address from its /instance/info
                        let mut child_address: Option<String> = None;
                        let info_url = format!("{url}/instance/info");
                        if let Ok(resp) = http.get(&info_url).send().await {
                            if let Ok(info) = resp.json::<serde_json::Value>().await {
                                child_address = info
                                    .get("identity")
                                    .and_then(|id| id.get("address"))
                                    .and_then(|v| v.as_str())
                                    .map(String::from);
                            }
                        }

                        if let Err(e) = db::update_child(
                            &version_check_state.gateway.db,
                            &child.instance_id,
                            child_address.as_deref(),
                            None, // keep existing URL
                            Some("running"),
                        ) {
                            tracing::warn!(
                                instance_id = %child.instance_id,
                                error = %e,
                                "Failed to recover stuck child status"
                            );
                        } else {
                            tracing::info!(
                                instance_id = %child.instance_id,
                                address = ?child_address,
                                "Child status recovered to running"
                            );
                        }
                    }

                    // ── Build hash check & auto-redeploy ────────────────────
                    // Compare build hashes (git SHA) for exact match. Fall back
                    // to semver if the child doesn't report a build hash yet
                    // (old image without the `build` field).
                    let up_to_date =
                        if !child_build.is_empty() && child_build != "dev" && parent_build != "dev"
                        {
                            child_build == parent_build
                        } else {
                            child_version == parent_version
                        };

                    if up_to_date {
                        tracing::debug!(
                            instance_id = %child.instance_id,
                            version = %child_version,
                            build = %child_build,
                            "Child is up to date"
                        );
                        continue;
                    }

                    tracing::info!(
                        instance_id = %child.instance_id,
                        child_version = %child_version,
                        child_build = %child_build,
                        parent_version = %parent_version,
                        parent_build = %parent_build,
                        "Child build mismatch — triggering redeploy"
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
                            let err_str = format!("{e}");
                            // If Railway says the service doesn't exist, mark child as failed
                            if err_str.contains("not found") || err_str.contains("Not Found") {
                                tracing::warn!(
                                    instance_id = %child.instance_id,
                                    error = %e,
                                    "Service not found on Railway — marking child as failed"
                                );
                                let _ = db::mark_child_failed(
                                    &version_check_state.gateway.db,
                                    &child.instance_id,
                                );
                            } else {
                                tracing::warn!(
                                    instance_id = %child.instance_id,
                                    error = %e,
                                    "Auto-redeploy failed (non-fatal)"
                                );
                            }
                        }
                    }
                }

                tracing::info!("Health probe cycle complete");
                tokio::time::sleep(probe_interval).await;
            } // end loop
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
        let cors = x402_gateway::cors::build_cors(&allowed_origins);

        let mut app = App::new()
            .app_data(gateway_data.clone())
            .app_data(node_data.clone())
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024))
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(Governor::new(&governor_conf))
            // Node health (extends gateway health with soul liveness)
            .configure(crate::routes::health::configure)
            .configure(x402_gateway::routes::register::configure)
            .configure(x402_gateway::routes::endpoints::configure)
            .configure(x402_gateway::routes::analytics::configure)
            .configure(x402_gateway::routes::gateway::configure)
            // Node routes (identity, clone, soul)
            .configure(crate::routes::instance::configure)
            .configure(crate::routes::wallet::configure)
            // Script endpoints — soul-created dynamic handlers (no compilation needed)
            .configure(crate::routes::scripts::configure)
            // Admin endpoint — local-only endpoint management (no payment required)
            .route("/admin/endpoints", web::post().to(admin_register_endpoint))
            .route(
                "/admin/endpoints/{slug}",
                web::delete().to(admin_delete_endpoint),
            );

        #[cfg(feature = "agent")]
        {
            app = app.configure(crate::routes::clone::configure);
        }

        #[cfg(feature = "soul")]
        {
            app = app.configure(crate::routes::soul::configure);
        }

        // Mount facilitator HTTP routes if embedded
        if let Some(ref fac_data) = facilitator_data {
            app = app.service(
                web::scope("/facilitator")
                    .app_data(fac_data.clone())
                    .service(x402_gateway::facilitator::routes::supported)
                    .service(x402_gateway::facilitator::routes::verify_and_settle),
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
