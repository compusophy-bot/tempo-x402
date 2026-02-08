use actix_web::{get, web, HttpRequest, HttpResponse};
use alloy::providers::{Provider, RootProvider};
use std::sync::Arc;

/// Constant-time byte comparison to prevent timing side-channel attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[get("/metrics")]
pub async fn metrics_endpoint(req: HttpRequest) -> HttpResponse {
    // Check bearer token if METRICS_TOKEN is set
    if let Ok(expected) = std::env::var("METRICS_TOKEN") {
        if !expected.is_empty() {
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
    }

    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(x402_server::metrics::metrics_output())
}

#[get("/health")]
pub async fn health(provider: web::Data<Arc<RootProvider>>) -> HttpResponse {
    match provider.get_block_number().await {
        Ok(block) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "chain": "tempo-moderato",
            "latestBlock": block.to_string(),
        })),
        Err(e) => {
            tracing::error!(error = %e, "health check: RPC unreachable");
            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "status": "degraded",
                "chain": "tempo-moderato",
                "error": "RPC unreachable",
            }))
        }
    }
}
