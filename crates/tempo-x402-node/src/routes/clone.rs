use actix_web::{web, HttpRequest, HttpResponse};

use crate::db;
use crate::routes::instance::is_valid_uuid;
use crate::state::NodeState;
use x402_gateway::error::GatewayError;
use x402_gateway::middleware::{payment_response_header, require_payment};

/// Check that the request comes from localhost or carries a valid HMAC Bearer token.
/// Returns `Ok(())` if authorized, or `Err(HttpResponse::Forbidden)` if not.
fn require_local_or_hmac(
    req: &HttpRequest,
    hmac_secret: Option<&[u8]>,
) -> Result<(), HttpResponse> {
    let peer_addr = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_default();
    let is_localhost =
        peer_addr == "127.0.0.1" || peer_addr == "::1" || peer_addr.starts_with("100.64.");
    let has_auth = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|token| {
            hmac_secret
                .is_some_and(|secret| x402::security::constant_time_eq(token.as_bytes(), secret))
        })
        .unwrap_or(false);

    if !is_localhost && !has_auth {
        return Err(HttpResponse::Forbidden()
            .json(serde_json::json!({"error": "requires localhost or HMAC auth"})));
    }
    Ok(())
}

/// POST /clone — x402-gated clone operation
pub async fn clone_instance(
    req: HttpRequest,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    let agent = node
        .agent
        .as_ref()
        .ok_or_else(|| GatewayError::Internal("cloning not configured".to_string()))?;

    let clone_price = node
        .clone_price
        .as_deref()
        .ok_or_else(|| GatewayError::Internal("clone price not set".to_string()))?;

    let clone_price_amount = node
        .clone_price_amount
        .as_deref()
        .ok_or_else(|| GatewayError::Internal("clone price amount not set".to_string()))?;

    // Build payment requirements
    let requirements = x402::payment::PaymentRequirements {
        scheme: x402::constants::SCHEME_NAME.to_string(),
        network: x402::constants::TEMPO_NETWORK.to_string(),
        price: clone_price.to_string(),
        asset: x402::constants::DEFAULT_TOKEN,
        amount: clone_price_amount.to_string(),
        pay_to: node.gateway.config.platform_address,
        max_timeout_seconds: 60,
        description: Some("Clone instance fee".to_string()),
        mime_type: Some("application/json".to_string()),
        facilitator_address: None,
    };

    // Early 402 if no payment header
    if x402_gateway::middleware::extract_payment_header(&req).is_none() {
        return Ok(x402_gateway::middleware::payment_required_response(
            requirements,
        ));
    }

    // Verify and settle payment
    let settle = match require_payment(
        &req,
        requirements,
        &node.gateway.http_client,
        &node.gateway.config.facilitator_url,
        node.gateway.config.hmac_secret.as_deref(),
        node.gateway.facilitator.as_deref(),
    )
    .await
    {
        Ok(s) => s,
        Err(http_response) => return Ok(http_response),
    };

    let payer_address = settle
        .payer
        .map(|a| format!("{:#x}", a))
        .unwrap_or_default();

    // Generate instance ID up front so we can reserve the DB slot before Railway calls
    let instance_id = uuid::Uuid::new_v4().to_string();

    // 1. Reserve slot in DB BEFORE any Railway API calls (atomic limit check)
    match db::reserve_child_slot(&node.gateway.db, node.clone_max_children, &instance_id) {
        Ok(true) => {
            tracing::info!(instance_id = %instance_id, "Child slot reserved");
        }
        Ok(false) => {
            return Ok(HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "clone limit reached",
            })));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to reserve child slot in DB");
            return Err(GatewayError::Internal(
                "failed to reserve clone slot".to_string(),
            ));
        }
    }

    // 2. Get ordinal designation for this drone
    let designation =
        db::next_designation(&node.gateway.db).unwrap_or_else(|_| "drone".to_string());
    let mut clone_extra = std::collections::HashMap::new();
    clone_extra.insert("DRONE_DESIGNATION".to_string(), designation.clone());

    // 3. Spawn clone on Railway (with retry + cleanup-on-failure)
    let clone_result = match agent
        .spawn_clone_with_extra_vars(&instance_id, &payer_address, &clone_extra)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(
                instance_id = %instance_id,
                error = %e,
                "Clone orchestration failed"
            );
            // Mark the reserved slot as failed
            if let Err(db_err) = db::mark_child_failed(&node.gateway.db, &instance_id) {
                tracing::error!(error = %db_err, "Failed to mark child as failed in DB");
            }
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "clone_failed",
                "message": format!("Clone orchestration failed: {e}"),
            })));
        }
    };

    // 3. Update the reserved slot with Railway details
    if let Err(e) = db::update_child_deployment(
        &node.gateway.db,
        &instance_id,
        &clone_result.url,
        &clone_result.railway_service_id,
        "deploying",
        clone_result.branch.as_deref(),
    ) {
        // Railway resources exist but DB update failed — log but still return success
        // since the child is at least tracked from the reservation step
        tracing::error!(
            instance_id = %instance_id,
            error = %e,
            "Failed to update child deployment details in DB"
        );
    }
    // Store volume_id for cleanup on delete — prevents orphaned volumes
    if let Some(ref vid) = clone_result.volume_id {
        let _ = db::set_child_volume_id(&node.gateway.db, &instance_id, vid);
    }

    tracing::info!(
        instance_id = %instance_id,
        url = %clone_result.url,
        payer = %payer_address,
        "Clone spawned successfully"
    );

    // Start background probe to promote child to "running" as soon as it boots
    spawn_post_clone_probe(
        node.gateway.db.clone(),
        instance_id.clone(),
        clone_result.url.clone(),
    );

    Ok(HttpResponse::Created()
        .insert_header((
            "PAYMENT-RESPONSE",
            payment_response_header(&settle, node.gateway.config.hmac_secret.as_deref()),
        ))
        .json(serde_json::json!({
            "success": true,
            "instance_id": instance_id,
            "designation": clone_result.designation,
            "url": clone_result.url,
            "branch": clone_result.branch,
            "status": "deploying",
            "transaction": settle.transaction,
        })))
}

/// GET /clone/{instance_id}/status — check clone deployment status
pub async fn clone_status(
    path: web::Path<String>,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    let instance_id = path.into_inner();

    match db::get_child_by_instance_id(&node.gateway.db, &instance_id) {
        Ok(Some(child)) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "instance_id": child.instance_id,
            "status": child.status,
            "url": child.url,
            "branch": child.branch,
            "created_at": child.created_at,
        }))),
        Ok(None) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "clone not found",
        }))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to query clone status");
            Err(GatewayError::Internal(
                "failed to query clone status".to_string(),
            ))
        }
    }
}

/// Query parameter for force-delete.
#[derive(serde::Deserialize, Default)]
pub struct DeleteCloneQuery {
    #[serde(default)]
    pub force: bool,
}

/// DELETE /clone/{instance_id} — delete a clone
///
/// By default, only deletes clones with status "failed".
/// Pass `?force=true` to delete a clone in any status (also tears down the Railway service).
pub async fn delete_clone(
    path: web::Path<String>,
    query: web::Query<DeleteCloneQuery>,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    let instance_id = path.into_inner();
    let force = query.force;

    if !is_valid_uuid(&instance_id) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "invalid instance_id format",
        })));
    }

    // Look up the child
    let child = match db::get_child_by_instance_id(&node.gateway.db, &instance_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": "clone not found",
            })));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to query clone for delete");
            return Err(GatewayError::Internal("failed to query clone".to_string()));
        }
    };

    // Without force, only allow deleting failed clones
    if !force && child.status != "failed" {
        return Ok(HttpResponse::Conflict().json(serde_json::json!({
            "error": "can only delete failed clones (use ?force=true to override)",
            "current_status": child.status,
        })));
    }

    // Best-effort Railway cleanup: volume FIRST, then service.
    // Deleting a service does NOT delete its volumes — they become orphans.
    if let Some(ref volume_id) = child.volume_id {
        if let Some(ref agent) = node.agent {
            if let Err(e) = agent.delete_volume(volume_id).await {
                tracing::warn!(
                    instance_id = %instance_id,
                    volume_id = %volume_id,
                    error = %e,
                    "Failed to delete Railway volume (best-effort cleanup)"
                );
            } else {
                tracing::info!(
                    instance_id = %instance_id,
                    volume_id = %volume_id,
                    "Deleted Railway volume"
                );
            }
        }
    }
    if let Some(ref service_id) = child.railway_service_id {
        if let Some(ref agent) = node.agent {
            if let Err(e) = agent.delete_service(service_id).await {
                tracing::warn!(
                    instance_id = %instance_id,
                    service_id = %service_id,
                    error = %e,
                    "Failed to delete Railway service (best-effort cleanup)"
                );
            }
        }
    }

    // Best-effort GitHub branch cleanup for source-based clones
    if let Some(ref branch) = child.branch {
        if let Some(ref agent) = node.agent {
            let config = agent.config();
            if let (Some(ref repo), Some(ref token)) = (&config.source_repo, &config.github_token) {
                if let Err(e) = crate::clone::delete_github_branch(token, repo, branch).await {
                    tracing::warn!(
                        instance_id = %instance_id,
                        branch = %branch,
                        error = %e,
                        "Failed to delete GitHub branch (best-effort cleanup)"
                    );
                }
            }
        }
    }

    // Delete from DB
    let delete_result = if force {
        db::delete_child(&node.gateway.db, &instance_id)
    } else {
        db::delete_failed_child(&node.gateway.db, &instance_id)
    };

    match delete_result {
        Ok(true) => {
            tracing::info!(instance_id = %instance_id, force = force, "Deleted clone");
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "instance_id": instance_id,
            })))
        }
        Ok(false) => Ok(HttpResponse::Conflict().json(serde_json::json!({
            "error": "clone is no longer in expected state",
        }))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete clone from DB");
            Err(GatewayError::Internal("failed to delete clone".to_string()))
        }
    }
}

/// POST /clone/{instance_id}/redeploy — trigger a redeploy for a running clone
pub async fn redeploy_clone(
    path: web::Path<String>,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    let instance_id = path.into_inner();

    if !is_valid_uuid(&instance_id) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "invalid instance_id format",
        })));
    }

    let agent = node
        .agent
        .as_ref()
        .ok_or_else(|| GatewayError::Internal("cloning not configured".to_string()))?;

    // Look up the child
    let child = match db::get_child_by_instance_id(&node.gateway.db, &instance_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": "clone not found",
            })));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to query clone for redeploy");
            return Err(GatewayError::Internal("failed to query clone".to_string()));
        }
    };

    // Reject redeploying failed clones
    if child.status == "failed" {
        return Ok(HttpResponse::Conflict().json(serde_json::json!({
            "error": "cannot redeploy a failed clone",
            "current_status": child.status,
        })));
    }

    let service_id = match child.railway_service_id {
        Some(ref id) => id.clone(),
        None => {
            return Ok(HttpResponse::Conflict().json(serde_json::json!({
                "error": "clone has no Railway service ID",
            })));
        }
    };

    // Trigger redeploy
    match agent.redeploy_clone(&service_id).await {
        Ok(_) => {
            // Update status to deploying
            if let Err(e) = db::update_child_status(&node.gateway.db, &instance_id, "deploying") {
                tracing::error!(error = %e, "Failed to update child status after redeploy");
            }

            tracing::info!(instance_id = %instance_id, "Redeploy triggered");
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "instance_id": instance_id,
                "status": "deploying",
            })))
        }
        Err(e) => {
            tracing::error!(
                instance_id = %instance_id,
                error = %e,
                "Failed to redeploy clone"
            );
            Err(GatewayError::Internal(
                "failed to redeploy clone".to_string(),
            ))
        }
    }
}

/// POST /clone/update-all — redeploy all active children
pub async fn update_all(node: web::Data<NodeState>) -> Result<HttpResponse, GatewayError> {
    let agent = node
        .agent
        .as_ref()
        .ok_or_else(|| GatewayError::Internal("cloning not configured".to_string()))?;

    // Query active children
    let children = rusqlite::Connection::open(&node.db_path)
        .map_err(|e| GatewayError::Internal(format!("failed to open db: {e}")))?
        .pipe(|conn| db::query_children_active(&conn))
        .map_err(|e| GatewayError::Internal(format!("failed to query children: {e}")))?;

    let total = children.len();
    let mut succeeded = 0u32;
    let mut failed = 0u32;
    let mut results = Vec::new();

    for child in &children {
        let service_id = match child.railway_service_id {
            Some(ref id) => id.clone(),
            None => {
                results.push(serde_json::json!({
                    "instance_id": child.instance_id,
                    "success": false,
                    "error": "no Railway service ID",
                }));
                failed += 1;
                continue;
            }
        };

        match agent.redeploy_clone(&service_id).await {
            Ok(_) => {
                if let Err(e) =
                    db::update_child_status(&node.gateway.db, &child.instance_id, "deploying")
                {
                    tracing::error!(
                        instance_id = %child.instance_id,
                        error = %e,
                        "Failed to update status after redeploy"
                    );
                }
                results.push(serde_json::json!({
                    "instance_id": child.instance_id,
                    "success": true,
                }));
                succeeded += 1;
            }
            Err(e) => {
                tracing::warn!(
                    instance_id = %child.instance_id,
                    error = %e,
                    "Failed to redeploy child"
                );
                results.push(serde_json::json!({
                    "instance_id": child.instance_id,
                    "success": false,
                    "error": e.to_string(),
                }));
                failed += 1;
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "total": total,
        "succeeded": succeeded,
        "failed": failed,
        "results": results,
    })))
}

/// Pipe helper — allows `value.pipe(|v| f(v))` for readability.
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}
impl<T> Pipe for T {}

/// POST /clone/self — internal self-clone (no x402 payment required).
/// Only accessible from localhost or with valid HMAC auth.
/// This allows the soul to trigger cloning without paying itself.
pub async fn clone_self(
    req: HttpRequest,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    // Security: only allow from localhost or with HMAC auth
    if let Err(resp) = require_local_or_hmac(&req, node.gateway.config.hmac_secret.as_deref()) {
        return Ok(resp);
    }

    let agent = node
        .agent
        .as_ref()
        .ok_or_else(|| GatewayError::Internal("cloning not configured".to_string()))?;

    let instance_id = uuid::Uuid::new_v4().to_string();
    let self_address = std::env::var("EVM_ADDRESS").unwrap_or_default();

    match db::reserve_child_slot(&node.gateway.db, node.clone_max_children, &instance_id) {
        Ok(true) => {
            tracing::info!(instance_id = %instance_id, "Self-clone: child slot reserved");
        }
        Ok(false) => {
            return Ok(HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "clone limit reached",
            })));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to reserve child slot");
            return Err(GatewayError::Internal(
                "failed to reserve clone slot".to_string(),
            ));
        }
    }

    let designation =
        db::next_designation(&node.gateway.db).unwrap_or_else(|_| "drone".to_string());
    let mut clone_extra = std::collections::HashMap::new();
    clone_extra.insert("DRONE_DESIGNATION".to_string(), designation.clone());

    let clone_result = match agent
        .spawn_clone_with_extra_vars(&instance_id, &self_address, &clone_extra)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(instance_id = %instance_id, error = %e, "Self-clone failed");
            let _ = db::mark_child_failed(&node.gateway.db, &instance_id);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "clone_failed",
                "message": format!("Self-clone failed: {e}"),
            })));
        }
    };

    if let Err(e) = db::update_child_deployment(
        &node.gateway.db,
        &instance_id,
        &clone_result.url,
        &clone_result.railway_service_id,
        "deploying",
        clone_result.branch.as_deref(),
    ) {
        tracing::error!(instance_id = %instance_id, error = %e, "Failed to update child deployment");
    }
    if let Some(ref vid) = clone_result.volume_id {
        let _ = db::set_child_volume_id(&node.gateway.db, &instance_id, vid);
    }

    tracing::info!(
        instance_id = %instance_id,
        url = %clone_result.url,
        "Self-clone spawned successfully"
    );

    // Start background probe to promote child to "running" as soon as it boots
    spawn_post_clone_probe(
        node.gateway.db.clone(),
        instance_id.clone(),
        clone_result.url.clone(),
    );

    Ok(HttpResponse::Created().json(serde_json::json!({
        "success": true,
        "instance_id": instance_id,
        "designation": clone_result.designation,
        "url": clone_result.url,
        "branch": clone_result.branch,
        "status": "deploying",
    })))
}

/// Request body for spawning a specialist clone.
#[derive(serde::Deserialize)]
pub(crate) struct SpawnSpecialistRequest {
    /// What this specialist focuses on: "solver", "reviewer", "tool-builder",
    /// "researcher", "coordinator", or a custom description.
    specialization: String,
    /// Optional initial goal to seed the specialist with.
    initial_goal: Option<String>,
}

/// POST /clone/specialist — spawn a differentiated clone with a specific focus.
/// Only accessible from localhost or with valid HMAC auth (same as clone_self).
/// The clone gets extra env vars that shape its personality and initial goals.
pub async fn clone_specialist(
    req: HttpRequest,
    node: web::Data<NodeState>,
    body: web::Json<SpawnSpecialistRequest>,
) -> Result<HttpResponse, GatewayError> {
    // Security: same as clone_self — localhost or HMAC auth
    if let Err(resp) = require_local_or_hmac(&req, node.gateway.config.hmac_secret.as_deref()) {
        return Ok(resp);
    }

    let agent = node
        .agent
        .as_ref()
        .ok_or_else(|| GatewayError::Internal("cloning not configured".to_string()))?;

    let instance_id = uuid::Uuid::new_v4().to_string();
    let self_address = std::env::var("EVM_ADDRESS").unwrap_or_default();

    // Validate specialization
    let specialization = body.specialization.trim();
    if specialization.is_empty() || specialization.len() > 200 {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "specialization must be 1-200 characters",
        })));
    }

    match db::reserve_child_slot(&node.gateway.db, node.clone_max_children, &instance_id) {
        Ok(true) => {
            tracing::info!(
                instance_id = %instance_id,
                specialization = %specialization,
                "Specialist clone: child slot reserved"
            );
        }
        Ok(false) => {
            return Ok(HttpResponse::Conflict().json(serde_json::json!({
                "success": false,
                "error": "clone limit reached",
            })));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to reserve child slot");
            return Err(GatewayError::Internal(
                "failed to reserve clone slot".to_string(),
            ));
        }
    }

    // Build extra env vars for this specialist (thread-safe, no set_var)
    let mut extra_vars = std::collections::HashMap::new();
    let designation =
        db::next_designation(&node.gateway.db).unwrap_or_else(|_| "drone".to_string());
    extra_vars.insert("DRONE_DESIGNATION".to_string(), designation.clone());
    extra_vars.insert(
        "SOUL_SPECIALIZATION".to_string(),
        specialization.to_string(),
    );
    if let Some(ref goal) = body.initial_goal {
        extra_vars.insert("SOUL_INITIAL_GOAL".to_string(), goal.clone());
    }

    let clone_result = match agent
        .spawn_clone_with_extra_vars(&instance_id, &self_address, &extra_vars)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(instance_id = %instance_id, error = %e, "Specialist clone failed");
            let _ = db::mark_child_failed(&node.gateway.db, &instance_id);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "clone_failed",
                "message": format!("Specialist clone failed: {e}"),
            })));
        }
    };

    if let Err(e) = db::update_child_deployment(
        &node.gateway.db,
        &instance_id,
        &clone_result.url,
        &clone_result.railway_service_id,
        "deploying",
        clone_result.branch.as_deref(),
    ) {
        tracing::error!(instance_id = %instance_id, error = %e, "Failed to update child deployment");
    }
    if let Some(ref vid) = clone_result.volume_id {
        let _ = db::set_child_volume_id(&node.gateway.db, &instance_id, vid);
    }

    tracing::info!(
        instance_id = %instance_id,
        url = %clone_result.url,
        specialization = %specialization,
        "Specialist clone spawned successfully"
    );

    // Start background probe to promote child to "running" as soon as it boots
    spawn_post_clone_probe(
        node.gateway.db.clone(),
        instance_id.clone(),
        clone_result.url.clone(),
    );

    Ok(HttpResponse::Created().json(serde_json::json!({
        "success": true,
        "instance_id": instance_id,
        "designation": clone_result.designation,
        "url": clone_result.url,
        "branch": clone_result.branch,
        "specialization": specialization,
        "initial_goal": body.initial_goal,
        "status": "deploying",
    })))
}

/// Spawn a background task that polls a newly-deployed child until it responds,
/// then promotes it from "deploying" to "running" in the parent's DB.
/// This makes the child appear in /instance/siblings immediately instead of
/// waiting for the 5-minute health probe cycle.
fn spawn_post_clone_probe(
    db: std::sync::Arc<x402_gateway::Database>,
    instance_id: String,
    child_url: String,
) {
    tokio::spawn(async move {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();

        // Wait for initial boot (Railway takes ~30-60s to deploy)
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        // Poll every 30s for up to 10 minutes
        for attempt in 1..=20 {
            let info_url = format!("{}/instance/info", child_url.trim_end_matches('/'));
            match http.get(&info_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    // Child is alive — extract address and promote to running
                    let address = resp.json::<serde_json::Value>().await.ok().and_then(|j| {
                        j.get("identity")
                            .and_then(|id| id.get("address"))
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    });

                    match db::update_child(
                        &db,
                        &instance_id,
                        address.as_deref(),
                        None, // keep existing URL
                        Some("running"),
                    ) {
                        Ok(_) => {
                            tracing::info!(
                                instance_id = %instance_id,
                                attempt,
                                address = ?address,
                                "Post-clone probe: child promoted to running"
                            );
                            // Notify all existing siblings about the new peer.
                            // One-time, fire-and-forget. No polling.
                            notify_siblings_of_new_peer(&db, &instance_id, &child_url, &http).await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                instance_id = %instance_id,
                                error = %e,
                                "Post-clone probe: failed to promote child"
                            );
                        }
                    }
                    return; // Done — child is linked
                }
                Ok(resp) => {
                    tracing::debug!(
                        instance_id = %instance_id,
                        attempt,
                        status = %resp.status(),
                        "Post-clone probe: child not ready yet"
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        instance_id = %instance_id,
                        attempt,
                        error = %e,
                        "Post-clone probe: child unreachable"
                    );
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }

        tracing::warn!(
            instance_id = %instance_id,
            "Post-clone probe: gave up after 10 minutes — child never responded"
        );
    });
}

/// Notify all existing siblings about a new peer that just joined the colony.
/// Called once when a new clone boots successfully. Fire-and-forget.
async fn notify_siblings_of_new_peer(
    db: &x402_gateway::Database,
    new_instance_id: &str,
    new_url: &str,
    http: &reqwest::Client,
) {
    let siblings = match db::list_children_active(db) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list siblings for new-peer notification");
            return;
        }
    };

    let new_peer_info = serde_json::json!({
        "instance_id": new_instance_id,
        "url": new_url.trim_end_matches('/'),
        "event": "new_peer",
    });

    let mut notified = 0u32;
    for sibling in &siblings {
        // Skip the new clone itself
        if sibling.instance_id == new_instance_id {
            continue;
        }
        // Skip siblings without URLs
        let sib_url = match &sibling.url {
            Some(u) if !u.is_empty() => u,
            _ => continue,
        };

        let nudge_url = format!("{}/soul/nudge", sib_url.trim_end_matches('/'));
        let nudge_body = serde_json::json!({
            "message": format!(
                "New peer joined the colony: {} at {}. Run discover_peers to sync.",
                new_instance_id, new_url
            ),
            "source": "colony",
        });

        match http.post(&nudge_url).json(&nudge_body).send().await {
            Ok(r) if r.status().is_success() => {
                notified += 1;
            }
            _ => {
                tracing::debug!(
                    sibling = %sibling.instance_id,
                    "Failed to notify sibling of new peer (non-fatal)"
                );
            }
        }
    }

    tracing::info!(
        new_peer = %new_instance_id,
        siblings_notified = notified,
        total_siblings = siblings.len() - 1,
        "Notified siblings of new colony member"
    );
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/clone", web::post().to(clone_instance))
        .route("/clone/self", web::post().to(clone_self))
        .route("/clone/specialist", web::post().to(clone_specialist))
        .route("/clone/update-all", web::post().to(update_all))
        .route("/clone/{instance_id}/status", web::get().to(clone_status))
        .route(
            "/clone/{instance_id}/redeploy",
            web::post().to(redeploy_clone),
        )
        .route("/clone/{instance_id}", web::delete().to(delete_clone));
}
