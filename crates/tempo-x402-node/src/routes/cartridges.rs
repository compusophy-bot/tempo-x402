//! WASM cartridge endpoints — sandboxed app execution with payment rails.
//!
//! Mirrors the script endpoint pattern (`/x/{slug}`) but executes
//! precompiled WASM modules via wasmtime instead of bash scripts.

use actix_web::{web, HttpRequest, HttpResponse};
use alloy::primitives::Address;
use serde::Deserialize;
use x402_gateway::middleware::{endpoint_requirements, require_payment};

use crate::db;
use crate::state::NodeState;

/// `GET /c` — list all registered cartridges.
pub async fn list_cartridges(state: web::Data<NodeState>) -> HttpResponse {
    match db::list_cartridges(&state.gateway.db) {
        Ok(cartridges) => {
            let summary: Vec<serde_json::Value> = cartridges
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "slug": c.slug,
                        "name": c.name,
                        "description": c.description,
                        "version": c.version,
                        "price": c.price_usd,
                        "source_repo": c.source_repo,
                    })
                })
                .collect();
            HttpResponse::Ok().json(serde_json::json!({
                "cartridges": summary,
                "count": summary.len(),
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{e}"),
        })),
    }
}

/// `GET/POST /c/{slug}` or `/c/{slug}/{path:.*}` — execute a cartridge.
pub async fn handle_cartridge(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<NodeState>,
) -> HttpResponse {
    let slug = path.into_inner();

    // Validate slug
    if !slug
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "invalid cartridge slug"
        }));
    }

    // Look up cartridge in DB
    let cartridge = match db::get_cartridge(&state.gateway.db, &slug) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("cartridge '{slug}' not found")
            }));
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("{e}")
            }));
        }
    };

    // ── x402 payment gate ──
    let owner_address = &cartridge.owner_address;
    if !owner_address.is_empty() {
        if let Ok(owner) = owner_address.parse::<Address>() {
            let requirements = endpoint_requirements(
                owner,
                &cartridge.price_usd,
                &cartridge.price_amount,
                Some(&format!("WASM cartridge: /c/{slug}")),
                state
                    .gateway
                    .facilitator
                    .as_ref()
                    .map(|f| f.facilitator.facilitator_address()),
            );

            let settle = match require_payment(
                &req,
                requirements,
                &state.gateway.http_client,
                &state.gateway.config.facilitator_url,
                state.gateway.config.hmac_secret.as_deref(),
                state.gateway.facilitator.as_deref(),
            )
            .await
            {
                Ok(s) => Some(s),
                Err(http_response) => return http_response,
            };

            if settle.is_some() {
                if let Err(e) = state
                    .gateway
                    .db
                    .record_payment(&format!("cartridge-{slug}"), &cartridge.price_amount)
                {
                    tracing::warn!(slug = %slug, error = %e, "Failed to record cartridge payment");
                }

                #[cfg(feature = "erc8004")]
                if let Some(ref tx) = state.reputation_tx {
                    let _ = tx.try_send(crate::state::SettlementEvent {
                        endpoint_slug: format!("cartridge-{slug}"),
                        tx_hash: None,
                    });
                }
            }
        }
    }

    // ── Execute cartridge ──
    let engine = match &state.cartridge_engine {
        Some(e) => e,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "cartridge engine not initialized"
            }));
        }
    };

    // Build request
    let method = req.method().to_string();
    let req_path = req.match_info().get("path").unwrap_or("/").to_string();
    let body_str = String::from_utf8_lossy(&body).to_string();
    let mut headers = std::collections::HashMap::new();
    for (key, value) in req.headers() {
        if let Ok(v) = value.to_str() {
            headers.insert(key.to_string(), v.to_string());
        }
    }

    let cartridge_request = x402_cartridge::CartridgeRequest {
        method,
        path: req_path,
        body: body_str,
        headers,
        payment: None, // TODO: populate from settle result
    };

    // Load KV store for this cartridge
    let kv = db::cartridge_kv_load(&state.gateway.db, &slug).unwrap_or_default();

    // Execute in blocking context (wasmtime is synchronous)
    let slug_clone = slug.clone();
    let result =
        tokio::task::block_in_place(|| engine.execute(&slug_clone, &cartridge_request, kv, 30));

    match result {
        Ok(r) => {
            tracing::info!(
                slug = %slug,
                status = r.status,
                duration_ms = r.duration_ms,
                "Cartridge executed"
            );
            HttpResponse::build(
                actix_web::http::StatusCode::from_u16(r.status)
                    .unwrap_or(actix_web::http::StatusCode::OK),
            )
            .content_type(r.content_type)
            .body(r.body)
        }
        Err(e) => {
            tracing::warn!(slug = %slug, error = %e, "Cartridge execution failed");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("{e}"),
            }))
        }
    }
}

#[derive(Deserialize)]
pub struct UploadCartridge {
    pub slug: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub source_code: Option<String>,
}

/// `POST /admin/cartridges` — register and optionally compile a new cartridge.
pub async fn upload_cartridge(
    body: web::Json<UploadCartridge>,
    state: web::Data<NodeState>,
) -> HttpResponse {
    let slug = &body.slug;
    let name = body.name.as_deref().unwrap_or(slug);
    let cartridge_dir = format!("/data/cartridges/{slug}");
    let src_dir = format!("{cartridge_dir}/src");
    let bin_dir = format!("{cartridge_dir}/bin");

    // Create directories
    if let Err(e) = std::fs::create_dir_all(&src_dir) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to create source dir: {e}")
        }));
    }
    if let Err(e) = std::fs::create_dir_all(format!("{src_dir}/src")) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to create src/src dir: {e}")
        }));
    }
    if let Err(e) = std::fs::create_dir_all(&bin_dir) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to create bin dir: {e}")
        }));
    }

    // Write source files
    let cargo_toml = x402_cartridge::compiler::default_cargo_toml(slug);
    let lib_rs = body
        .source_code
        .as_deref()
        .map(String::from)
        .unwrap_or_else(|| x402_cartridge::compiler::default_lib_rs(slug));

    if let Err(e) = std::fs::write(format!("{src_dir}/Cargo.toml"), &cargo_toml) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to write Cargo.toml: {e}")
        }));
    }
    if let Err(e) = std::fs::write(format!("{src_dir}/src/lib.rs"), &lib_rs) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to write lib.rs: {e}")
        }));
    }

    // Register in DB (wasm_path empty until compiled)
    let now = chrono::Utc::now().timestamp();
    let owner = std::env::var("EVM_ADDRESS").unwrap_or_default();
    let record = db::CartridgeRecord {
        slug: slug.to_string(),
        name: name.to_string(),
        description: body.description.clone(),
        version: "0.1.0".to_string(),
        price_usd: "$0.001".to_string(),
        price_amount: "1000".to_string(),
        owner_address: owner,
        source_repo: None,
        wasm_path: String::new(),
        wasm_hash: String::new(),
        active: true,
        created_at: now,
        updated_at: now,
    };
    if let Err(e) = db::upsert_cartridge(&state.gateway.db, &record) {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to register: {e}")
        }));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "status": "created",
        "slug": slug,
        "source_dir": src_dir,
        "message": "Source written. POST /admin/cartridges/{slug}/compile to build.",
    }))
}

/// `POST /admin/cartridges/{slug}/compile` — compile a cartridge from source.
pub async fn compile_cartridge(
    path: web::Path<String>,
    state: web::Data<NodeState>,
) -> HttpResponse {
    let slug = path.into_inner();
    let src_dir = format!("/data/cartridges/{slug}/src");
    let bin_dir = format!("/data/cartridges/{slug}/bin");

    if !std::path::Path::new(&src_dir).join("Cargo.toml").exists() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("no source found for cartridge '{slug}'")
        }));
    }

    match x402_cartridge::compiler::compile_cartridge(
        std::path::Path::new(&src_dir),
        std::path::Path::new(&bin_dir),
    )
    .await
    {
        Ok(wasm_path) => {
            // Compute hash
            let hash = x402_cartridge::CartridgeEngine::hash_wasm(&wasm_path)
                .unwrap_or_else(|_| "unknown".to_string());

            // Update DB with wasm path + hash
            if let Ok(Some(mut record)) = db::get_cartridge(&state.gateway.db, &slug) {
                record.wasm_path = wasm_path.to_string_lossy().to_string();
                record.wasm_hash = hash.clone();
                record.updated_at = chrono::Utc::now().timestamp();
                let _ = db::upsert_cartridge(&state.gateway.db, &record);
            }

            // Load into engine
            if let Some(ref engine) = state.cartridge_engine {
                if let Err(e) = engine.load_module(&slug, &wasm_path) {
                    tracing::warn!(slug = %slug, error = %e, "Failed to load compiled cartridge");
                }
            }

            HttpResponse::Ok().json(serde_json::json!({
                "status": "compiled",
                "slug": slug,
                "wasm_path": wasm_path.to_string_lossy(),
                "wasm_hash": hash,
            }))
        }
        Err(e) => HttpResponse::UnprocessableEntity().json(serde_json::json!({
            "status": "compilation_failed",
            "error": format!("{e}"),
        })),
    }
}

/// Configure cartridge routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/c", web::get().to(list_cartridges))
        .route("/c/{slug}", web::get().to(handle_cartridge))
        .route("/c/{slug}", web::post().to(handle_cartridge))
        .route("/c/{slug}/{path:.*}", web::get().to(handle_cartridge))
        .route("/c/{slug}/{path:.*}", web::post().to(handle_cartridge))
        .route("/admin/cartridges", web::post().to(upload_cartridge))
        .route(
            "/admin/cartridges/{slug}/compile",
            web::post().to(compile_cartridge),
        );
}
