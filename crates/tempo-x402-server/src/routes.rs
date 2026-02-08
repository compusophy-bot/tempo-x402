use actix_web::{get, web, HttpRequest, HttpResponse};
use alloy::providers::{Provider, RootProvider};
use std::sync::Arc;

/// Constant-time byte comparison — delegates to the shared implementation
/// in x402::security which uses the `subtle` crate.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    x402::security::constant_time_eq(a, b)
}

/// Cached metrics token, read once at first access.
static METRICS_TOKEN: std::sync::LazyLock<Option<String>> = std::sync::LazyLock::new(|| {
    std::env::var("METRICS_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
});

/// Cached public metrics opt-in flag, read once at first access.
static PUBLIC_METRICS: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::env::var("X402_PUBLIC_METRICS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
});

#[get("/metrics")]
pub async fn metrics_endpoint(req: HttpRequest) -> HttpResponse {
    match &*METRICS_TOKEN {
        Some(expected) => {
            let authorized = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|token| constant_time_eq(token.as_bytes(), expected.as_bytes()))
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
            if !*PUBLIC_METRICS {
                return HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "forbidden",
                    "message": "Set METRICS_TOKEN or X402_PUBLIC_METRICS=true to access /metrics"
                }));
            }
        }
    }

    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(x402_server::metrics::metrics_output())
}

#[get("/health")]
pub async fn health(provider: web::Data<Arc<RootProvider>>) -> HttpResponse {
    match provider.get_block_number().await {
        Ok(_block) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "service": "x402-server",
        })),
        Err(e) => {
            tracing::error!(error = %e, "health check: RPC unreachable");
            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "status": "degraded",
                "service": "x402-server",
            }))
        }
    }
}
