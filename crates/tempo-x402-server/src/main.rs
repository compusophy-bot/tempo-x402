use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpServer};
use alloy::providers::RootProvider;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod routes;

use x402_server::config::PaymentGateConfig;

fn build_cors(origins: &[String]) -> Cors {
    if origins.is_empty() {
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
            .allowed_headers(vec![
                "content-type",
                "authorization",
                "payment-signature",
                "x-facilitator-auth",
            ])
            .max_age(3600)
    } else {
        let mut cors = Cors::default();
        for origin in origins {
            cors = cors.allowed_origin(origin);
        }
        cors.allow_any_method()
            .allowed_headers(vec![
                "content-type",
                "authorization",
                "payment-signature",
                "x-facilitator-auth",
            ])
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

    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| x402::constants::RPC_URL.to_string());
    let provider: RootProvider = RootProvider::new_http(rpc_url.parse().expect("invalid RPC_URL"));

    let facilitator_url =
        std::env::var("FACILITATOR_URL").unwrap_or_else(|_| "http://localhost:4022".to_string());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4021);

    let gate_config = PaymentGateConfig::from_env(&facilitator_url);
    let cors_origins = gate_config.allowed_origins.clone();

    let provider = web::Data::new(Arc::new(provider));

    tracing::info!("x402 server SDK listening at http://localhost:{port}");
    tracing::info!("This is a library/SDK server. For paid endpoints, use tempo-x402-gateway.");
    tracing::info!("Endpoints: GET /health, GET /metrics");
    tracing::info!("Rate limit: {} req/min per IP", gate_config.rate_limit_rpm);

    let governor_conf = GovernorConfigBuilder::default()
        .requests_per_minute(gate_config.rate_limit_rpm)
        .finish()
        .expect("failed to build rate limiter config");

    HttpServer::new(move || {
        App::new()
            .wrap(build_cors(&cors_origins))
            .wrap(Governor::new(&governor_conf))
            .app_data(web::JsonConfig::default().limit(65_536))
            .app_data(provider.clone())
            .service(routes::metrics_endpoint)
            .service(routes::health)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
