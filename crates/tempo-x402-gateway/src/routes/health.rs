use actix_web::{web, HttpRequest, HttpResponse};

use crate::metrics::REGISTRY;
use crate::state::AppState;

/// GET /health - Health check endpoint
pub async fn health(state: web::Data<AppState>) -> HttpResponse {
    let mut response = serde_json::json!({
        "status": "ok",
        "service": "x402-gateway",
        "version": env!("CARGO_PKG_VERSION"),
    });

    // If facilitator is embedded, include its health
    if let Some(ref fac) = state.facilitator {
        match fac.facilitator.health_check().await {
            Ok(block) => {
                response["facilitator_status"] = serde_json::json!("ok");
                response["latestBlock"] = serde_json::json!(block.to_string());
            }
            Err(_) => {
                response["status"] = serde_json::json!("degraded");
                response["facilitator_status"] = serde_json::json!("degraded");
                response["facilitator_error"] = serde_json::json!("RPC unreachable");
            }
        }
    }

    if response["status"] == "degraded" {
        HttpResponse::ServiceUnavailable().json(response)
    } else {
        HttpResponse::Ok().json(response)
    }
}

/// Constant-time byte comparison that does not leak input lengths.
/// Both inputs are hashed to fixed-length digests before comparison.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use sha2::{Digest, Sha256};
    let ha = Sha256::digest(a);
    let hb = Sha256::digest(b);
    let mut result = 0u8;
    for (x, y) in ha.iter().zip(hb.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// GET /metrics - Prometheus metrics endpoint (optionally auth-gated)
pub async fn metrics(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    // Check bearer token if METRICS_TOKEN is configured
    if let Some(ref expected_token) = state.config.metrics_token {
        let authorized = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|token| constant_time_eq(token.as_bytes(), expected_token.as_bytes()))
            .unwrap_or(false);

        if !authorized {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "unauthorized",
                "message": "Valid Bearer token required for /metrics"
            }));
        }
    }

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
