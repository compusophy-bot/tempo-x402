use actix_web::{web, HttpRequest, HttpResponse};
use alloy::primitives::Address;
use x402::scheme::SchemeServer;

use crate::db::UpdateEndpoint;
use crate::error::GatewayError;
use crate::middleware::{payment_response_header, platform_requirements, require_payment};
use crate::state::AppState;

/// Public endpoint info (without internal fields)
#[derive(serde::Serialize)]
pub struct EndpointInfo {
    pub slug: String,
    pub gateway_url: String,
    pub price: String,
    pub description: Option<String>,
    pub created_at: i64,
}

/// Pagination query parameters
#[derive(Debug, serde::Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    100
}

/// GET /endpoints - List active endpoints (paginated)
pub async fn list_endpoints(
    query: web::Query<PaginationParams>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let endpoints = state.db.list_endpoints(query.limit, query.offset)?;

    let public_endpoints: Vec<EndpointInfo> = endpoints
        .into_iter()
        .map(|e| EndpointInfo {
            gateway_url: format!("/g/{}", e.slug),
            slug: e.slug,
            price: e.price_usd,
            description: e.description,
            created_at: e.created_at,
        })
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "endpoints": public_endpoints,
        "count": public_endpoints.len(),
        "limit": query.limit.clamp(1, 500),
        "offset": query.offset,
    })))
}

/// GET /endpoints/{slug} - Get endpoint details
pub async fn get_endpoint(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let slug = path.into_inner();

    let endpoint = state
        .db
        .get_endpoint(&slug)?
        .ok_or_else(|| GatewayError::EndpointNotFound(slug.clone()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "slug": endpoint.slug,
        "gateway_url": format!("/g/{}", endpoint.slug),
        "price": endpoint.price_usd,
        "description": endpoint.description,
        "created_at": endpoint.created_at,
    })))
}

/// PATCH /endpoints/{slug} - Update endpoint (owner only, requires payment)
pub async fn update_endpoint(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<UpdateEndpoint>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let slug = path.into_inner();

    // Check endpoint exists
    let endpoint = state
        .db
        .get_endpoint(&slug)?
        .ok_or_else(|| GatewayError::EndpointNotFound(slug.clone()))?;

    // Validate new target URL if provided
    if let Some(ref url) = body.target_url {
        crate::validation::validate_target_url(url)?;
    }
    if let Some(ref desc) = body.description {
        if desc.len() > 4096 {
            return Err(GatewayError::InvalidSlug(
                "description must be at most 4096 characters".to_string(),
            ));
        }
    }

    // Parse new price if provided
    let (price_usd, price_amount) = if let Some(ref price) = body.price {
        let scheme_server = x402::scheme_server::TempoSchemeServer::new();
        let (amount, _) = scheme_server
            .parse_price(price)
            .map_err(|e| GatewayError::InvalidPrice(e.to_string()))?;
        (Some(price.clone()), Some(amount))
    } else {
        (None, None)
    };

    // Extract payer address from the payment header BEFORE settling,
    // so we can verify ownership without risking irreversible payment loss.
    let payment_header = req
        .headers()
        .get("PAYMENT-SIGNATURE")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GatewayError::Internal("missing payment signature".to_string()))?;

    // Decode the payment to extract the payer address for ownership check
    let decoded: x402::payment::PaymentPayload = {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payment_header)
            .or_else(|_| {
                serde_json::from_str::<serde_json::Value>(payment_header)
                    .map(|_| payment_header.as_bytes().to_vec())
            })
            .map_err(|_| GatewayError::Internal("invalid payment header encoding".to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|_| GatewayError::Internal("invalid payment payload".to_string()))?
    };

    // Verify ownership BEFORE settling payment (prevents money loss on NotOwner)
    let claimed_payer = decoded.payload.from;
    let owner: Address = endpoint
        .owner_address
        .parse()
        .map_err(|_| GatewayError::Internal("invalid stored owner address".to_string()))?;

    if claimed_payer != owner {
        return Err(GatewayError::NotOwner);
    }

    // Build platform payment requirements
    let requirements = platform_requirements(
        state.config.platform_address,
        &state.config.platform_fee,
        &state.config.platform_fee_amount,
    );

    // Now settle payment (ownership pre-verified, facilitator will cryptographically
    // verify the signature matches the claimed payer)
    let settle = match require_payment(
        &req,
        requirements,
        &state.http_client,
        &state.config.facilitator_url,
        state.config.hmac_secret.as_deref(),
        state.facilitator.as_deref(),
    )
    .await
    {
        Ok(s) => s,
        Err(http_response) => return Ok(http_response),
    };

    // Post-settlement ownership verification: confirm the cryptographically
    // verified payer matches the endpoint owner (defense-in-depth against
    // spoofed `from` fields in unsigned payloads)
    if settle.payer != Some(owner) {
        return Err(GatewayError::NotOwner);
    }

    // Update the endpoint
    let updated = state.db.update_endpoint(
        &slug,
        body.target_url.as_deref(),
        price_usd.as_deref(),
        price_amount.as_deref(),
        body.description.as_deref(),
    )?;

    Ok(HttpResponse::Ok()
        .insert_header((
            "PAYMENT-RESPONSE",
            payment_response_header(&settle, state.config.hmac_secret.as_deref()),
        ))
        .json(serde_json::json!({
            "success": true,
            "endpoint": updated,
            "transaction": settle.transaction,
        })))
}

/// DELETE /endpoints/{slug} - Deactivate endpoint (owner only, requires payment)
pub async fn delete_endpoint(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let slug = path.into_inner();

    // Check endpoint exists
    let endpoint = state
        .db
        .get_endpoint(&slug)?
        .ok_or_else(|| GatewayError::EndpointNotFound(slug.clone()))?;

    // Extract payer address from the payment header BEFORE settling,
    // so we can verify ownership without risking irreversible payment loss.
    let payment_header = req
        .headers()
        .get("PAYMENT-SIGNATURE")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GatewayError::Internal("missing payment signature".to_string()))?;

    // Decode the payment to extract the payer address for ownership check
    let decoded: x402::payment::PaymentPayload = {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payment_header)
            .or_else(|_| {
                serde_json::from_str::<serde_json::Value>(payment_header)
                    .map(|_| payment_header.as_bytes().to_vec())
            })
            .map_err(|_| GatewayError::Internal("invalid payment header encoding".to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|_| GatewayError::Internal("invalid payment payload".to_string()))?
    };

    // Verify ownership BEFORE settling payment (prevents money loss on NotOwner)
    let claimed_payer = decoded.payload.from;
    let owner: Address = endpoint
        .owner_address
        .parse()
        .map_err(|_| GatewayError::Internal("invalid stored owner address".to_string()))?;

    if claimed_payer != owner {
        return Err(GatewayError::NotOwner);
    }

    // Build platform payment requirements
    let requirements = platform_requirements(
        state.config.platform_address,
        &state.config.platform_fee,
        &state.config.platform_fee_amount,
    );

    // Now settle payment (ownership pre-verified, facilitator will cryptographically
    // verify the signature matches the claimed payer)
    let settle = match require_payment(
        &req,
        requirements,
        &state.http_client,
        &state.config.facilitator_url,
        state.config.hmac_secret.as_deref(),
        state.facilitator.as_deref(),
    )
    .await
    {
        Ok(s) => s,
        Err(http_response) => return Ok(http_response),
    };

    // Post-settlement ownership verification: confirm the cryptographically
    // verified payer matches the endpoint owner
    if settle.payer != Some(owner) {
        return Err(GatewayError::NotOwner);
    }

    // Delete (deactivate) the endpoint
    state.db.delete_endpoint(&slug)?;

    Ok(HttpResponse::Ok()
        .insert_header((
            "PAYMENT-RESPONSE",
            payment_response_header(&settle, state.config.hmac_secret.as_deref()),
        ))
        .json(serde_json::json!({
            "success": true,
            "message": format!("Endpoint '{}' has been deactivated", slug),
            "transaction": settle.transaction,
        })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/endpoints").route(web::get().to(list_endpoints)))
        .service(
            web::resource("/endpoints/{slug}")
                .route(web::get().to(get_endpoint))
                .route(web::patch().to(update_endpoint))
                .route(web::delete().to(delete_endpoint)),
        );
}
