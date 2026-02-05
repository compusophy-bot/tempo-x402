use actix_web::{web, HttpResponse};

use crate::metrics::REGISTRY;

/// GET /health - Health check endpoint
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "x402-gateway",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// GET /metrics - Prometheus metrics endpoint
pub async fn metrics() -> HttpResponse {
    use prometheus::Encoder;

    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();

    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return HttpResponse::InternalServerError().body("Failed to encode metrics");
    }

    let output = String::from_utf8(buffer).unwrap_or_default();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(output)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health))
        .route("/metrics", web::get().to(metrics));
}
