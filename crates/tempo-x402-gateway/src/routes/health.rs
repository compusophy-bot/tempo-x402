use actix_web::{web, HttpRequest, HttpResponse};

use crate::metrics::REGISTRY;
use crate::state::AppState;

/// Returns the git SHA for this build. Compile-time value from build.rs,
/// with runtime fallback to RAILWAY_GIT_COMMIT_SHA (Railway injects this
/// at runtime but not during Docker builds).
fn build_sha() -> &'static str {
    static SHA: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SHA.get_or_init(|| {
        let compile_time = env!("GIT_SHA");
        if compile_time != "dev" {
            return compile_time.to_string();
        }
        std::env::var("RAILWAY_GIT_COMMIT_SHA").unwrap_or_else(|_| "dev".to_string())
    })
}

/// GET /health - Health check endpoint
pub async fn health(state: web::Data<AppState>) -> HttpResponse {
    let mut response = serde_json::json!({
        "status": "ok",
        "service": "x402-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "build": build_sha(),
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

/// GET /metrics - Prometheus metrics endpoint (auth-gated by default)
pub async fn metrics(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    let token_bytes = state.config.metrics_token.as_ref().map(|s| s.as_bytes());
    let public_metrics = std::env::var("X402_PUBLIC_METRICS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if let Err((status, msg)) =
        x402::security::check_metrics_auth(auth_header, token_bytes, public_metrics)
    {
        return match status {
            401 => HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "unauthorized",
                "message": msg
            })),
            _ => HttpResponse::Forbidden().json(serde_json::json!({
                "error": "forbidden",
                "message": msg
            })),
        };
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
