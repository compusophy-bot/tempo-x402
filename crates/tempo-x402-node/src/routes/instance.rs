use actix_web::{web, HttpRequest, HttpResponse};
use alloy::primitives::{Address, U256};

use crate::db;
use crate::state::NodeState;

/// Validate that a string looks like a UUID (8-4-4-4-12 hex).
pub(crate) fn is_valid_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected_lens = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(expected_lens.iter())
        .all(|(part, &len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Validate that a string looks like an EVM address (0x + 40 hex chars).
fn is_valid_evm_address(s: &str) -> bool {
    if s.is_empty() {
        return true; // allow empty (not yet known)
    }
    s.len() == 42 && s.starts_with("0x") && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate that a URL uses HTTPS scheme.
fn is_valid_https_url(s: &str) -> bool {
    s.starts_with("https://") && s.len() > 8
}

/// DELETE /instance/peer/{instance_id} — remove a peer from the peers table (requires METRICS_TOKEN)
pub async fn delete_peer(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<NodeState>,
) -> HttpResponse {
    // Require Bearer token auth (METRICS_TOKEN)
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());
    let expected = state.gateway.config.metrics_token.as_deref();
    if let Err((status, msg)) =
        x402::security::check_metrics_auth(auth_header, expected.map(|s| s.as_bytes()), false)
    {
        return HttpResponse::build(
            actix_web::http::StatusCode::from_u16(status)
                .unwrap_or(actix_web::http::StatusCode::UNAUTHORIZED),
        )
        .json(serde_json::json!({ "error": msg }));
    }

    let instance_id = path.into_inner();

    if !is_valid_uuid(&instance_id) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "invalid instance_id format"
        }));
    }

    match db::delete_child(&state.gateway.db, &instance_id) {
        Ok(true) => {
            tracing::info!(instance_id = %instance_id, "Peer deleted");
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "instance_id": instance_id,
                "message": "peer removed"
            }))
        }
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "peer not found"
        })),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete peer");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "failed to delete peer"
            }))
        }
    }
}

/// GET /instance/info — returns identity, peers, version, uptime, clone availability
pub async fn info(state: web::Data<NodeState>) -> HttpResponse {
    let identity_info = state.identity.as_ref().map(|id| {
        serde_json::json!({
            "address": format!("{:#x}", id.address),
            "instance_id": id.instance_id,
            "parent_url": id.parent_url,
            "parent_address": id.parent_address.map(|a| format!("{:#x}", a)),
            "created_at": id.created_at.to_rfc3339(),
        })
    });

    // Query active (non-failed) peers via gateway DB (consistent with writes)
    let peers = db::list_children_active(&state.gateway.db).unwrap_or_default();

    let uptime_secs = (chrono::Utc::now() - state.started_at).num_seconds();

    let clone_available = state.agent.is_some()
        && state.clone_price.is_some()
        && (peers.len() as u32) < state.clone_max_children;

    // Fetch node wallet balance (best-effort, non-blocking for the response)
    let wallet_balance = if let Some(ref id) = state.identity {
        fetch_pathusd_balance(id.address).await
    } else {
        None
    };

    // Include registered endpoints so peers can discover available services
    let endpoints: Vec<serde_json::Value> = state
        .gateway
        .db
        .list_endpoints(500, 0)
        .unwrap_or_default()
        .into_iter()
        .map(|ep| {
            serde_json::json!({
                "slug": ep.slug,
                "price": ep.price_usd,
                "description": ep.description,
            })
        })
        .collect();

    // Include fitness score if soul DB is available
    let fitness = state
        .soul_db
        .as_ref()
        .and_then(|db| x402_soul::fitness::FitnessScore::load_current(db))
        .map(|f| {
            serde_json::json!({
                "total": f.total,
                "trend": f.trend,
                "economic": f.economic,
                "execution": f.execution,
                "evolution": f.evolution,
                "coordination": f.coordination,
                "introspection": f.introspection,
                "prediction": f.prediction,
                "measured_at": f.measured_at,
            })
        });

    // Node designation: "queen" if no parent, else DRONE_DESIGNATION env var
    let designation = std::env::var("DRONE_DESIGNATION")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "queen".to_string());

    HttpResponse::Ok().json(serde_json::json!({
        "identity": identity_info,
        "designation": designation,
        "agent_token_id": state.agent_token_id,
        "peers": peers,
        "peer_count": peers.len(),
        // Backwards compat — old frontends may read "children"
        "children": peers,
        "children_count": peers.len(),
        "clone_available": clone_available,
        "clone_price": state.clone_price,
        "clone_max_children": state.clone_max_children,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": uptime_secs,
        "wallet_balance": wallet_balance,
        "endpoints": endpoints,
        "fitness": fitness,
    }))
}

/// POST /instance/register — child callback, updates children table
pub async fn register(
    body: web::Json<serde_json::Value>,
    state: web::Data<NodeState>,
) -> HttpResponse {
    let instance_id = match body.get("instance_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "missing instance_id"
            }));
        }
    };

    // Validate instance_id is a UUID to prevent injection
    if !is_valid_uuid(instance_id) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "invalid instance_id format"
        }));
    }

    let address = body.get("address").and_then(|v| v.as_str()).unwrap_or("");

    // Validate address format if provided
    if !is_valid_evm_address(address) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "invalid address format"
        }));
    }

    let url = body.get("url").and_then(|v| v.as_str());

    // Validate URL is HTTPS if provided
    if let Some(u) = url {
        if !is_valid_https_url(u) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "url must use https"
            }));
        }
    }

    match db::update_child(
        &state.gateway.db,
        instance_id,
        Some(address),
        url,
        Some("running"),
    ) {
        Ok(()) => {
            tracing::info!(
                instance_id = %instance_id,
                "Child instance registered"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "registered",
            }))
        }
        Err(e) => {
            tracing::warn!(
                instance_id = %instance_id,
                error = %e,
                "Failed to update child record"
            );
            // Don't leak internal error details
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "acknowledged",
            }))
        }
    }
}

/// GET /instance/siblings — returns list of active sibling instances with their URLs and endpoints
pub async fn siblings(state: web::Data<NodeState>) -> HttpResponse {
    let children = db::list_children_active(&state.gateway.db).unwrap_or_default();

    let mut siblings = Vec::new();
    for child in &children {
        // Include children that are running or deploying (if they have a URL, they're likely reachable)
        if child.status != "running" && child.status != "deploying" {
            continue;
        }
        let Some(url) = child.url.as_ref() else {
            continue;
        };

        // Include known endpoint slugs for this child (from gateway DB)
        let endpoints: Vec<String> = state
            .gateway
            .db
            .list_endpoints(500, 0)
            .unwrap_or_default()
            .into_iter()
            .filter(|ep| ep.target_url.starts_with(url.as_str()))
            .map(|ep| ep.slug)
            .collect();

        siblings.push(serde_json::json!({
            "instance_id": child.instance_id,
            "url": url,
            "address": child.address,
            "status": child.status,
            "endpoints": endpoints,
        }));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "siblings": siblings,
        "count": siblings.len(),
    }))
}

/// POST /instance/link — manually link an independent peer by URL.
/// Fetches the peer's /instance/info to get its instance_id and address,
/// then inserts it into the children table so it appears in /instance/siblings.
pub async fn link(body: web::Json<serde_json::Value>, state: web::Data<NodeState>) -> HttpResponse {
    let peer_url = match body.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.trim_end_matches('/'),
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "missing 'url' field"
            }));
        }
    };

    if !is_valid_https_url(peer_url) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "url must use https"
        }));
    }

    // Fetch the peer's /instance/info to get identity
    let info_url = format!("{peer_url}/instance/info");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_default();

    let resp = match client.get(&info_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": format!("failed to reach peer: {e}")
            }));
        }
    };

    if !resp.status().is_success() {
        return HttpResponse::BadGateway().json(serde_json::json!({
            "error": format!("peer returned status {}", resp.status())
        }));
    }

    let info: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::BadGateway().json(serde_json::json!({
                "error": format!("invalid peer response: {e}")
            }));
        }
    };

    // Extract instance_id — try identity.instance_id first, fall back to generating one
    let instance_id = info
        .get("identity")
        .and_then(|id| id.get("instance_id"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let instance_id = match instance_id {
        Some(id) if is_valid_uuid(&id) => id,
        _ => {
            // No identity — use a deterministic ID from the URL
            let hash = format!("{:x}", md5_hash(peer_url.as_bytes()));
            format!(
                "{}-{}-{}-{}-{}",
                &hash[..8],
                &hash[8..12],
                &hash[12..16],
                &hash[16..20],
                &hash[20..32]
            )
        }
    };

    let address = info
        .get("identity")
        .and_then(|id| id.get("address"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Prevent self-link
    if let Some(ref identity) = state.identity {
        if instance_id == identity.instance_id {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "cannot link self as peer"
            }));
        }
    }

    // Prevent linking our own parent as a child
    if let Ok(parent_url_env) = std::env::var("PARENT_URL") {
        let parent_norm = parent_url_env.trim_end_matches('/');
        if peer_url == parent_norm {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "cannot link parent as child"
            }));
        }
    }

    // Insert/update the peer in the children table
    match db::link_peer(&state.gateway.db, &instance_id, address, peer_url) {
        Ok(()) => {
            tracing::info!(
                instance_id = %instance_id,
                url = %peer_url,
                "Peer linked successfully"
            );
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "instance_id": instance_id,
                "url": peer_url,
                "message": "peer linked — will appear in /instance/siblings"
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to link peer");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "failed to store peer"
            }))
        }
    }
}

/// Simple non-crypto hash for generating deterministic IDs from URLs.
fn md5_hash(data: &[u8]) -> u128 {
    // FNV-1a 128-bit — good enough for deterministic ID generation
    let mut hash: u128 = 0x6c62272e07bb0142_62b821756295c58d;
    for &byte in data {
        hash ^= byte as u128;
        hash = hash.wrapping_mul(0x0000000001000000_000000000000013B);
    }
    hash
}

/// GET /instance/peers — decentralized peer discovery via on-chain ERC-8004 registry
#[cfg(feature = "erc8004")]
pub async fn peers(state: web::Data<NodeState>) -> HttpResponse {
    let registry = x402_identity::identity_registry();
    if registry == alloy::primitives::Address::ZERO {
        return HttpResponse::Ok().json(serde_json::json!({
            "source": "none",
            "error": "no identity registry configured",
            "peers": [],
            "count": 0,
        }));
    }

    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://rpc.moderato.tempo.xyz".to_string());
    let self_address = state.identity.as_ref().map(|id| id.address);

    let Ok(rpc_parsed) = rpc_url.parse() else {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "invalid RPC URL"
        }));
    };

    let provider = alloy::providers::RootProvider::<alloy::network::Ethereum>::new_http(rpc_parsed);

    match x402_identity::discovery::discover_peers(&provider, registry, self_address, 100).await {
        Ok(peers) => HttpResponse::Ok().json(serde_json::json!({
            "source": "on-chain",
            "registry": format!("{:#x}", registry),
            "peers": peers,
            "count": peers.len(),
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "source": "on-chain",
            "error": format!("{e}"),
            "peers": [],
            "count": 0,
        })),
    }
}

/// Fetch pathUSD balance for an address (best-effort, returns None on any error).
async fn fetch_pathusd_balance(address: Address) -> Option<serde_json::Value> {
    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://rpc.moderato.tempo.xyz".to_string());
    let provider = alloy::providers::ProviderBuilder::new()
        .connect_http(rpc_url.parse::<reqwest::Url>().ok()?);
    let token = x402::constants::DEFAULT_TOKEN;
    let balance = x402::tip20::balance_of(&provider, token, address)
        .await
        .ok()?;
    // pathUSD has 6 decimals
    let whole = balance / U256::from(1_000_000u64);
    let frac = balance % U256::from(1_000_000u64);
    Some(serde_json::json!({
        "token": "pathUSD",
        "raw": balance.to_string(),
        "formatted": format!("{}.{:06}", whole, frac),
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/instance")
        .route("/info", web::get().to(info))
        .route("/register", web::post().to(register))
        .route("/siblings", web::get().to(siblings))
        .route("/link", web::post().to(link))
        .route("/peer/{instance_id}", web::delete().to(delete_peer));

    #[cfg(feature = "erc8004")]
    let scope = scope.route("/peers", web::get().to(peers));

    cfg.service(scope);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_uuid() {
        assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_valid_uuid("a1b2c3d4-e5f6-7890-abcd-ef1234567890"));
        assert!(!is_valid_uuid("not-a-uuid"));
        assert!(!is_valid_uuid("550e8400e29b41d4a716446655440000"));
        assert!(!is_valid_uuid(""));
        assert!(!is_valid_uuid("550e8400-e29b-41d4-a716-44665544000g"));
    }

    #[test]
    fn test_valid_evm_address() {
        assert!(is_valid_evm_address(
            "0x1234567890abcdef1234567890abcdef12345678"
        ));
        assert!(is_valid_evm_address("")); // empty allowed
        assert!(!is_valid_evm_address("0x123")); // too short
        assert!(!is_valid_evm_address(
            "1234567890abcdef1234567890abcdef12345678"
        )); // no 0x
        assert!(!is_valid_evm_address(
            "0xGGGG567890abcdef1234567890abcdef12345678"
        )); // invalid hex
    }

    #[test]
    fn test_valid_https_url() {
        assert!(is_valid_https_url("https://example.com"));
        assert!(is_valid_https_url("https://x402-abc.up.railway.app"));
        assert!(!is_valid_https_url("http://example.com"));
        assert!(!is_valid_https_url("https://"));
        assert!(!is_valid_https_url("ftp://example.com"));
    }
}
