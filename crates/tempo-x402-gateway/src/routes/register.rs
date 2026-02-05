use actix_web::{web, HttpRequest, HttpResponse};
use url::Url;
use x402::SchemeServer;

use crate::db::CreateEndpoint;
use crate::error::GatewayError;
use crate::middleware::{payment_response_header, platform_requirements, require_payment};
use crate::state::AppState;

/// Validate slug format
pub fn validate_slug(slug: &str) -> Result<(), GatewayError> {
    if slug.len() < 3 {
        return Err(GatewayError::InvalidSlug(
            "slug must be at least 3 characters".to_string(),
        ));
    }
    if slug.len() > 64 {
        return Err(GatewayError::InvalidSlug(
            "slug must be at most 64 characters".to_string(),
        ));
    }
    if !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(GatewayError::InvalidSlug(
            "slug must contain only alphanumeric characters and hyphens".to_string(),
        ));
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(GatewayError::InvalidSlug(
            "slug cannot start or end with a hyphen".to_string(),
        ));
    }
    Ok(())
}

/// Validate target URL
pub fn validate_target_url(url: &str) -> Result<(), GatewayError> {
    let parsed =
        Url::parse(url).map_err(|_| GatewayError::InvalidUrl("invalid URL format".to_string()))?;

    if parsed.scheme() != "https" {
        return Err(GatewayError::InvalidUrl(
            "target must use HTTPS".to_string(),
        ));
    }

    // Prevent SSRF to localhost/private IPs
    if let Some(host) = parsed.host_str() {
        let host_lower = host.to_lowercase();
        if host_lower == "localhost"
            || host_lower.starts_with("127.")
            || host_lower.starts_with("10.")
            || host_lower.starts_with("192.168.")
            || host_lower.starts_with("172.16.")
            || host_lower.starts_with("172.17.")
            || host_lower.starts_with("172.18.")
            || host_lower.starts_with("172.19.")
            || host_lower.starts_with("172.2")
            || host_lower.starts_with("172.30.")
            || host_lower.starts_with("172.31.")
            || host_lower.starts_with("169.254.") // link-local
            || host_lower == "0.0.0.0"
            || host_lower == "[::]"
            || host_lower == "[::1]"
            || host_lower.starts_with("[fc") // IPv6 private fc00::/7
            || host_lower.starts_with("[fd") // IPv6 private
            || host_lower.starts_with("[fe80")
        // IPv6 link-local
        {
            return Err(GatewayError::InvalidUrl(
                "target cannot be localhost or private IP".to_string(),
            ));
        }
    }

    Ok(())
}

/// POST /register - Register a new endpoint
pub async fn register(
    req: HttpRequest,
    body: web::Json<CreateEndpoint>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    // Validate inputs first (before requiring payment)
    validate_slug(&body.slug)?;
    validate_target_url(&body.target_url)?;

    // Check if slug already exists
    if state.db.slug_exists(&body.slug)? {
        return Err(GatewayError::SlugExists(body.slug.clone()));
    }

    // Parse price
    let scheme_server = x402::TempoSchemeServer::new();
    let (price_amount, _) = scheme_server
        .parse_price(&body.price)
        .map_err(|e| GatewayError::InvalidPrice(e.to_string()))?;

    // Build platform payment requirements
    let requirements = platform_requirements(
        state.config.platform_address,
        &state.config.platform_fee,
        &state.config.platform_fee_amount,
    );

    // Require payment
    let settle = require_payment(
        &req,
        requirements,
        &state.http_client,
        &state.config.facilitator_url,
        state.config.hmac_secret.as_deref(),
    )
    .await
    .map_err(|_| GatewayError::PaymentRequired)?;

    // Extract payer address from settlement response
    let owner_address = settle
        .payer
        .ok_or_else(|| GatewayError::Internal("settlement missing payer address".to_string()))?;

    // Create the endpoint
    let endpoint = state.db.create_endpoint(
        &body.slug,
        &format!("{:#x}", owner_address),
        &body.target_url,
        &body.price,
        &price_amount,
        body.description.as_deref(),
    )?;

    // Return success with payment response header
    Ok(HttpResponse::Created()
        .insert_header(("X-Payment-Response", payment_response_header(&settle)))
        .json(serde_json::json!({
            "success": true,
            "endpoint": endpoint,
            "transaction": settle.transaction,
        })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/register", web::post().to(register));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_slug_valid() {
        assert!(validate_slug("my-api").is_ok());
        assert!(validate_slug("api123").is_ok());
        assert!(validate_slug("test-api-v2").is_ok());
        assert!(validate_slug("abc").is_ok());
    }

    #[test]
    fn test_validate_slug_invalid() {
        assert!(validate_slug("ab").is_err()); // too short
        assert!(validate_slug("-api").is_err()); // starts with hyphen
        assert!(validate_slug("api-").is_err()); // ends with hyphen
        assert!(validate_slug("my_api").is_err()); // underscore not allowed
        assert!(validate_slug("my api").is_err()); // space not allowed
    }

    #[test]
    fn test_validate_url_valid() {
        assert!(validate_target_url("https://api.example.com").is_ok());
        assert!(validate_target_url("https://api.example.com/v1/endpoint").is_ok());
    }

    #[test]
    fn test_validate_url_invalid() {
        assert!(validate_target_url("http://api.example.com").is_err()); // not HTTPS
        assert!(validate_target_url("https://localhost").is_err()); // localhost
        assert!(validate_target_url("https://127.0.0.1").is_err()); // loopback
        assert!(validate_target_url("https://192.168.1.1").is_err()); // private
        assert!(validate_target_url("not-a-url").is_err()); // invalid
    }
}
