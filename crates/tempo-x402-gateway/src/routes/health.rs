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

    // If facilitator is embedded, include its health status
    // Block numbers and error details are not exposed to unauthenticated callers
    if let Some(ref fac) = state.facilitator {
        match fac.facilitator.health_check().await {
            Ok(_block) => {
                response["facilitator_status"] = serde_json::json!("ok");
            }
            Err(e) => {
                tracing::error!(error = %e, "facilitator health check failed");
                response["status"] = serde_json::json!("degraded");
                response["facilitator_status"] = serde_json::json!("degraded");
            }
        }
    }

    if response["status"] == "degraded" {
        HttpResponse::ServiceUnavailable().json(response)
    } else {
        HttpResponse::Ok().json(response)
    }
}

/// Constant-time byte comparison — delegates to the shared implementation
/// in x402::security which uses the `subtle` crate.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    x402::security::constant_time_eq(a, b)
}

/// GET /metrics - Prometheus metrics endpoint (auth-gated by default)
pub async fn metrics(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    match &state.config.metrics_token {
        Some(expected_token) => {
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
        None => {
            // No token configured — metrics are protected by default.
            let public_metrics = std::env::var("X402_PUBLIC_METRICS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false);
            if !public_metrics {
                return HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "forbidden",
                    "message": "Set METRICS_TOKEN or X402_PUBLIC_METRICS=true to access /metrics"
                }));
            }
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
