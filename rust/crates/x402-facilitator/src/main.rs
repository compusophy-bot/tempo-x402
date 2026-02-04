use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpServer};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

mod routes;
mod state;

use state::AppState;

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
                    .map(|o| o.starts_with("http://localhost"))
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

    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| x402_types::RPC_URL.to_string());

    let provider = ProviderBuilder::new()
        .wallet(alloy::network::EthereumWallet::from(signer))
        .connect_http(rpc_url.parse().expect("invalid RPC_URL"));

    let facilitator =
        x402_tempo::TempoSchemeFacilitator::new(provider, facilitator_address);

    // Start background nonce cleanup
    facilitator.start_nonce_cleanup();

    let hmac_secret = std::env::var("FACILITATOR_SHARED_SECRET")
        .ok()
        .map(|s| s.into_bytes());

    if hmac_secret.is_none() {
        tracing::warn!("FACILITATOR_SHARED_SECRET not set — HMAC auth disabled (dev mode)");
    }

    let state = web::Data::new(AppState {
        facilitator,
        hmac_secret,
    });

    let port: u16 = std::env::var("FACILITATOR_PORT")
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
            .service(routes::supported)
            .service(routes::verify)
            .service(routes::verify_and_settle)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
