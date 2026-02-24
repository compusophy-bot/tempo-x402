use actix_web::{get, web, HttpRequest, HttpResponse};
use alloy::providers::{Provider, RootProvider};
use std::sync::Arc;

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
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    let token_bytes = METRICS_TOKEN.as_ref().map(|s| s.as_bytes());

    if let Err((status, msg)) =
        x402::security::check_metrics_auth(auth_header, token_bytes, *PUBLIC_METRICS)
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
