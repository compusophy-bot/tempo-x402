use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{middleware::Logger, web, App, HttpServer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    let config = GatewayConfig::from_env().expect("Failed to load configuration");
    let port = config.port;
    let allowed_origins = config.allowed_origins.clone();
    let rate_limit_rpm = config.rate_limit_rpm;

    tracing::info!("Starting x402-gateway on port {}", port);
    tracing::info!("Platform address: {:#x}", config.platform_address);
    tracing::info!("Platform fee: {}", config.platform_fee);
    tracing::info!("Facilitator URL: {}", config.facilitator_url);
    tracing::info!(
        "HMAC auth: {}",
        if config.hmac_secret.is_some() {
            "enabled"
        } else {
            "disabled (dev mode)"
        }
    );

    // Initialize database
    let db = Database::new(&config.db_path).expect("Failed to initialize database");
    tracing::info!("Database initialized at: {}", config.db_path);

    // Register Prometheus metrics
    register_metrics();

    // Create shared state
    let state = AppState::new(config, db);
    let state_data = web::Data::new(state);

    // Configure rate limiter
    #[allow(deprecated)]
    let governor_conf = GovernorConfigBuilder::default()
        .per_second(rate_limit_rpm as u64 / 60)
        .burst_size(rate_limit_rpm)
        .finish()
        .expect("Failed to create rate limiter config");

    // Start HTTP server
    HttpServer::new(move || {
        // Configure CORS
        let allowed = allowed_origins.clone();
        let cors = Cors::default()
            .allowed_origin_fn(move |origin, _req_head| {
                let origin_str = origin.to_str().unwrap_or("");
                allowed.iter().any(|a| a == "*" || a == origin_str)
            })
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::HeaderName::from_static("x-payment"),
            ])
            .expose_headers(vec![actix_web::http::header::HeaderName::from_static(
                "x-payment-response",
            )])
            .max_age(3600);

        App::new()
            .app_data(state_data.clone())
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(Governor::new(&governor_conf))
            .configure(routes::health::configure)
            .configure(routes::register::configure)
            .configure(routes::endpoints::configure)
            .configure(routes::gateway::configure)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
