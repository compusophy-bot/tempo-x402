use actix_web::{web, HttpRequest, HttpResponse};
use alloy::primitives::Address;

use crate::error::GatewayError;
use crate::metrics::{ENDPOINT_PAYMENTS, ENDPOINT_REVENUE};
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

    // Reject path traversal in query parameters (both decoded and percent-encoded)
    let decoded = urlencoding::decode(sanitized).unwrap_or(std::borrow::Cow::Borrowed(sanitized));
    if decoded.contains("..") {
        return Err(GatewayError::ProxyError(
            "query string must not contain path traversal sequences".to_string(),
        ));
    }

    Ok(sanitized.to_string())
}

/// Sanitize a proxy path segment to prevent path traversal and URL authority injection.
/// Validates against the decoded form but returns the original (still-encoded) path
/// to prevent query/fragment injection from decoded URL-special characters.
fn sanitize_path(path: &str) -> Result<String, GatewayError> {
    // URL-decode the path to catch encoded attacks (e.g. %2e%2e)
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

    // Reject CRLF injection
    if decoded.contains('\r') || decoded.contains('\n') {
        return Err(GatewayError::ProxyError(
            "path must not contain newlines".to_string(),
        ));
    }

    // Reject null bytes
    if decoded.contains('\0') {
        return Err(GatewayError::ProxyError(
            "path must not contain null bytes".to_string(),
        ));
    }

    // Return the original (still-encoded) path to prevent query/fragment injection.
    // E.g. %3F stays as %3F, not decoded to ? which would alter the target URL.
    Ok(path.to_string())
}

/// Shared implementation for gateway proxy with and without a trailing path.
async fn do_gateway_proxy(
    req: &HttpRequest,
    state: &web::Data<AppState>,
    slug: &str,
    rest_path: Option<&str>,
    body: web::Bytes,
) -> Result<HttpResponse, GatewayError> {
    // Look up the endpoint
    let endpoint = state
        .db
        .get_endpoint(slug)?
        .ok_or_else(|| GatewayError::EndpointNotFound(slug.to_string()))?;

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
        req,
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
    let base_url = match rest_path {
        Some(path) => format!("{}/{}", endpoint.target_url.trim_end_matches('/'), path),
        None => endpoint.target_url.clone(),
    };

    // Add query string if present (sanitized)
    let target_url = if let Some(query) = req.uri().query() {
        let query = sanitize_query(query)?;
        if query.is_empty() {
            base_url
        } else {
            format!("{}?{}", base_url, query)
        }
    } else {
        base_url
    };

    // Proxy the request (includes PAYMENT-RESPONSE header)
    let response = proxy_request(
        &state.http_client,
        req,
        &target_url,
        body,
        &settle,
        true,
        state.config.hmac_secret.as_deref(),
    )
    .await?;

    // Record payment stats
    record_endpoint_stats(state, slug, &endpoint.price_amount);

    Ok(response)
}

/// ANY /g/{slug}/{path:.*} - Proxy to target API with payment
pub async fn gateway_proxy(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    let (slug, rest_path) = path.into_inner();
    let rest_path = sanitize_path(&rest_path)?;
    do_gateway_proxy(&req, &state, &slug, Some(&rest_path), body).await
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
    do_gateway_proxy(&req, &state, &slug, None, body).await
}

/// Record payment stats in DB and Prometheus metrics.
fn record_endpoint_stats(state: &AppState, slug: &str, price_amount: &str) {
    if let Err(e) = state.db.record_payment(slug, price_amount) {
        tracing::warn!(slug = %slug, error = %e, "failed to record payment stats");
    }
    ENDPOINT_PAYMENTS.with_label_values(&[slug]).inc();
    let amount: u64 = price_amount
        .parse::<u128>()
        .unwrap_or(0)
        .try_into()
        .unwrap_or(u64::MAX);
    ENDPOINT_REVENUE.with_label_values(&[slug]).inc_by(amount);
}
