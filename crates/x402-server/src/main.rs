use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpServer};
use alloy::providers::RootProvider;
use std::sync::Arc;

mod routes;

use x402_server::config::{PaymentConfig, PaymentGateConfig};

fn build_cors(origins: &[String]) -> Cors {
    if origins.is_empty() {
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

    let evm_address: alloy::primitives::Address = std::env::var("EVM_ADDRESS")
        .expect("EVM_ADDRESS environment variable is required")
        .parse()
        .expect("invalid EVM_ADDRESS");

    let facilitator_url =
        std::env::var("FACILITATOR_URL").unwrap_or_else(|_| "http://localhost:4022".to_string());

    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| x402_types::RPC_URL.to_string());

    let provider: RootProvider =
        RootProvider::new_http(rpc_url.parse().expect("invalid RPC_URL"));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4021);

    // Build payment config using the server scheme for price parsing
    let server_scheme = x402_tempo::TempoSchemeServer::new();
    let gate_config = PaymentGateConfig::from_env(&facilitator_url);

    if gate_config.hmac_secret.is_none() {
        tracing::warn!("FACILITATOR_SHARED_SECRET not set — HMAC auth disabled (dev mode)");
    }

    let payment_config = PaymentConfig::new(&server_scheme, evm_address, &gate_config);

    let payment_config = web::Data::new(payment_config);
    let provider = web::Data::new(Arc::new(provider));
    let http_client = web::Data::new(reqwest::Client::new());

    let cors_origins = gate_config.allowed_origins.clone();

    tracing::info!("x402 server listening at http://localhost:{port}");
    tracing::info!("Protected endpoint: GET /blockNumber ($0.001 per request)");
    tracing::info!("Payments go to: {evm_address}");
    tracing::info!("Using facilitator: {facilitator_url}");
    tracing::info!("Rate limit: {} req/min per IP", gate_config.rate_limit_rpm);

    let governor_conf = GovernorConfigBuilder::default()
        .requests_per_minute(gate_config.rate_limit_rpm)
        .finish()
        .expect("failed to build rate limiter config");

    HttpServer::new(move || {
        App::new()
            .wrap(build_cors(&cors_origins))
            .wrap(Governor::new(&governor_conf))
            .app_data(payment_config.clone())
            .app_data(provider.clone())
            .app_data(http_client.clone())
            .app_data(web::JsonConfig::default().limit(65_536))
            .service(routes::metrics_endpoint)
            .service(routes::health)
            .service(routes::block_number)
            .service(routes::api_demo)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
