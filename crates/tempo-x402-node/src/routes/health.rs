//! Node-level health endpoint that extends the gateway health check with soul liveness.

use actix_web::{web, HttpResponse};
use std::sync::atomic::Ordering;

use crate::state::NodeState;

/// GET /health — includes soul liveness alongside facilitator status.
pub async fn health(
    node: web::Data<NodeState>,
    gateway: web::Data<x402_gateway::state::AppState>,
) -> HttpResponse {
    let mut response = serde_json::json!({
        "status": "ok",
        "service": "x402-node",
        "version": env!("CARGO_PKG_VERSION"),
        "build": x402_gateway::routes::health::build_sha(),
    });

    // Check embedded facilitator
    if let Some(ref fac) = gateway.facilitator {
        match fac.facilitator.health_check().await {
            Ok(_) => {
                response["facilitator_status"] = serde_json::json!("ok");
            }
            Err(e) => {
                tracing::error!(error = %e, "facilitator health check failed");
                response["status"] = serde_json::json!("degraded");
                response["facilitator_status"] = serde_json::json!("degraded");
            }
        }
    }

    // Check soul liveness
    if let Some(ref alive) = node.soul_alive {
        let soul_running = alive.load(Ordering::Relaxed);
        response["soul_status"] = serde_json::json!(if soul_running { "ok" } else { "restarting" });
        if !soul_running && !node.soul_dormant {
            response["status"] = serde_json::json!("degraded");
        }
    } else if !node.soul_dormant {
        response["soul_status"] = serde_json::json!("not_spawned");
    }

    // Check build environment (can agents compile code?)
    let has_cargo = std::process::Command::new("which")
        .arg("cargo")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_ssl = std::path::Path::new("/usr/include/openssl/ssl.h").exists()
        || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libssl.so").exists();
    let has_gcc = std::process::Command::new("which")
        .arg("gcc")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let build_env_ok = has_cargo && has_ssl && has_gcc;
    response["build_env"] = serde_json::json!({
        "ok": build_env_ok,
        "cargo": has_cargo,
        "libssl": has_ssl,
        "gcc": has_gcc,
    });
    if !build_env_ok {
        response["status"] = serde_json::json!("degraded");
    }

    if response["status"] == "degraded" {
        HttpResponse::ServiceUnavailable().json(response)
    } else {
        HttpResponse::Ok().json(response)
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health)).route(
        "/metrics",
        web::get().to(x402_gateway::routes::health::metrics),
    );
}
