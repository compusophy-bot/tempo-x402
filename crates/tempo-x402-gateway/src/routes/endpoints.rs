use actix_web::{web, HttpRequest, HttpResponse};
use alloy::primitives::Address;
use x402::SchemeServer;

use crate::db::UpdateEndpoint;
use crate::error::GatewayError;
use crate::middleware::{
    extract_payer_from_header, payment_required_response, payment_response_header,
    platform_requirements, require_payment,
};
use crate::state::AppState;

/// Public endpoint info (without internal fields)
#[derive(serde::Serialize)]
pub struct EndpointInfo {
    pub slug: String,
    pub target_url: String,
    pub price: String,
    pub description: Option<String>,
    pub created_at: i64,
}

/// GET /endpoints - List all active endpoints
pub async fn list_endpoints(state: web::Data<AppState>) -> Result<HttpResponse, GatewayError> {
    let endpoints = state.db.list_endpoints()?;

    let public_endpoints: Vec<EndpointInfo> = endpoints
        .into_iter()
        .map(|e| EndpointInfo {
            slug: e.slug,
            target_url: e.target_url,
            price: e.price_usd,
            description: e.description,
            created_at: e.created_at,
        })
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "endpoints": public_endpoints,
        "count": public_endpoints.len(),
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
        "target_url": endpoint.target_url,
        "price": endpoint.price_usd,
        "description": endpoint.description,
        "created_at": endpoint.created_at,
        "gateway_url": format!("/g/{}", endpoint.slug),
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
        super::register::validate_target_url(url)?;
    }

    // Parse new price if provided
    let (price_usd, price_amount) = if let Some(ref price) = body.price {
        let scheme_server = x402::TempoSchemeServer::new();
        let (amount, _) = scheme_server
            .parse_price(price)
            .map_err(|e| GatewayError::InvalidPrice(e.to_string()))?;
        (Some(price.clone()), Some(amount))
    } else {
        (None, None)
    };

    // Build platform payment requirements
    let requirements = platform_requirements(
        state.config.platform_address,
        &state.config.platform_fee,
        &state.config.platform_fee_amount,
    );

    // Verify ownership BEFORE settling payment (so non-owners don't lose money)
    let payer = match extract_payer_from_header(&req) {
        Some(p) => p,
        None => return Ok(payment_required_response(requirements)),
    };

    let owner: Address = endpoint
        .owner_address
        .parse()
        .map_err(|_| GatewayError::Internal("invalid stored owner address".to_string()))?;

    if payer != owner {
        return Err(GatewayError::NotOwner);
    }

    // Now settle payment (ownership verified, safe to charge)
    let settle = match require_payment(
        &req,
        requirements,
        &state.http_client,
        &state.config.facilitator_url,
        state.config.hmac_secret.as_deref(),
    )
    .await
    {
        Ok(s) => s,
        Err(http_response) => return Ok(http_response),
    };

    // Update the endpoint
    let updated = state.db.update_endpoint(
        &slug,
        body.target_url.as_deref(),
        price_usd.as_deref(),
        price_amount.as_deref(),
        body.description.as_deref(),
    )?;

    Ok(HttpResponse::Ok()
        .insert_header(("PAYMENT-RESPONSE", payment_response_header(&settle)))
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

    // Build platform payment requirements
    let requirements = platform_requirements(
        state.config.platform_address,
        &state.config.platform_fee,
        &state.config.platform_fee_amount,
    );

    // Verify ownership BEFORE settling payment (so non-owners don't lose money)
    let payer = match extract_payer_from_header(&req) {
        Some(p) => p,
        None => return Ok(payment_required_response(requirements)),
    };

    let owner: Address = endpoint
        .owner_address
        .parse()
        .map_err(|_| GatewayError::Internal("invalid stored owner address".to_string()))?;

    if payer != owner {
        return Err(GatewayError::NotOwner);
    }

    // Now settle payment (ownership verified, safe to charge)
    let settle = match require_payment(
        &req,
        requirements,
        &state.http_client,
        &state.config.facilitator_url,
        state.config.hmac_secret.as_deref(),
    )
    .await
    {
        Ok(s) => s,
        Err(http_response) => return Ok(http_response),
    };

    // Delete (deactivate) the endpoint
    state.db.delete_endpoint(&slug)?;

    Ok(HttpResponse::Ok()
        .insert_header(("PAYMENT-RESPONSE", payment_response_header(&settle)))
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
