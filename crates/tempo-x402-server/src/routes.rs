use actix_web::{get, web, HttpResponse};
use alloy::providers::{Provider, RootProvider};
use std::sync::Arc;

#[get("/metrics")]
pub async fn metrics_endpoint() -> HttpResponse {
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
