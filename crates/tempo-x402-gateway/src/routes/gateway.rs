use actix_web::{web, HttpRequest, HttpResponse};
use alloy::primitives::Address;

use crate::error::GatewayError;
use crate::middleware::{endpoint_requirements, require_payment};
use crate::proxy::proxy_request;
use crate::state::AppState;

/// Sanitize a query string to prevent CRLF injection and fragment smuggling.
fn sanitize_query(query: &str) -> Result<String, GatewayError> {
    // Reject CRLF injection
    if query.contains('\r') || query.contains('\n') {
        return Err(GatewayError::ProxyError(
            "query string must not contain newlines".to_string(),
        ));
    }

    // Strip fragment (everything after #) — fragments should not be sent to the server
    let sanitized = match query.find('#') {
        Some(idx) => &query[..idx],
        None => query,
    };

    // Reject null bytes
    if sanitized.contains('\0') {
        return Err(GatewayError::ProxyError(
            "query string must not contain null bytes".to_string(),
        ));
    }

    Ok(sanitized.to_string())
}

/// Sanitize a proxy path segment to prevent path traversal and URL authority injection.
fn sanitize_path(path: &str) -> Result<String, GatewayError> {
    // URL-decode the path first to catch encoded attacks (e.g. %2e%2e)
    let decoded = urlencoding::decode(path)
        .map_err(|_| GatewayError::ProxyError("invalid URL encoding in path".to_string()))?;

    // Reject path traversal
    if decoded.contains("..") {
        return Err(GatewayError::ProxyError(
            "path traversal not allowed".to_string(),
        ));
    }

    // Reject leading slashes (prevents //host authority injection)
    if decoded.starts_with('/') {
        return Err(GatewayError::ProxyError(
            "path must not start with /".to_string(),
        ));
    }

    // Reject @ (URL authority injection: user@host)
    if decoded.contains('@') {
        return Err(GatewayError::ProxyError(
            "path must not contain @".to_string(),
        ));
    }

    Ok(decoded.into_owned())
}

/// ANY /g/{slug}/{path:.*} - Proxy to target API with payment
pub async fn gateway_proxy(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let (slug, rest_path) = path.into_inner();

    // Sanitize the rest path
    let rest_path = sanitize_path(&rest_path)?;

    // Look up the endpoint
    let endpoint = state
        .db
        .get_endpoint(&slug)?
        .ok_or_else(|| GatewayError::EndpointNotFound(slug.clone()))?;

    // Parse owner address
    let owner: Address = endpoint
        .owner_address
        .parse()
        .map_err(|_| GatewayError::Internal("invalid stored owner address".to_string()))?;

    // Build payment requirements for this endpoint
    let requirements = endpoint_requirements(
        owner,
        &endpoint.price_usd,
        &endpoint.price_amount,
        endpoint.description.as_deref(),
    );

    // Require payment (returns 402 with requirements if no valid payment)
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

    // Build target URL
    let target_url = format!(
        "{}/{}",
        endpoint.target_url.trim_end_matches('/'),
        rest_path
    );

    // Add query string if present (sanitized)
    let target_url = if let Some(query) = req.uri().query() {
        let query = sanitize_query(query)?;
        if query.is_empty() {
            target_url
        } else {
            format!("{}?{}", target_url, query)
        }
    } else {
        target_url
    };

    // Proxy the request (includes PAYMENT-RESPONSE header)
    let response = proxy_request(
        &state.http_client,
        &req,
        &target_url,
        body,
        &settle,
        true,
        state.config.hmac_secret.as_deref(),
    )
    .await?;

    Ok(response)
}

/// Configure the gateway routes
/// Note: We need to handle the case where path is empty
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/g/{slug}").route(web::route().to(gateway_proxy_no_path)))
        .service(web::resource("/g/{slug}/{path:.*}").route(web::route().to(gateway_proxy)));
}

/// Handle requests without a trailing path
async fn gateway_proxy_no_path(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let slug = path.into_inner();

    // Look up the endpoint
    let endpoint = state
        .db
        .get_endpoint(&slug)?
        .ok_or_else(|| GatewayError::EndpointNotFound(slug.clone()))?;

    // Parse owner address
    let owner: Address = endpoint
        .owner_address
        .parse()
        .map_err(|_| GatewayError::Internal("invalid stored owner address".to_string()))?;

    // Build payment requirements for this endpoint
    let requirements = endpoint_requirements(
        owner,
        &endpoint.price_usd,
        &endpoint.price_amount,
        endpoint.description.as_deref(),
    );

    // Require payment (returns 402 with requirements if no valid payment)
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

    // Build target URL (just the base, with sanitized query string)
    let target_url = if let Some(query) = req.uri().query() {
        let query = sanitize_query(query)?;
        if query.is_empty() {
            endpoint.target_url.clone()
        } else {
            format!("{}?{}", endpoint.target_url, query)
        }
    } else {
        endpoint.target_url.clone()
    };

    // Proxy the request (includes PAYMENT-RESPONSE header)
    let response = proxy_request(
        &state.http_client,
        &req,
        &target_url,
        body,
        &settle,
        true,
        state.config.hmac_secret.as_deref(),
    )
    .await?;

    Ok(response)
}
