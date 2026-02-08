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

/// Check if an IPv4 address is private, loopback, or otherwise non-routable.
fn is_private_ipv4(ip: &std::net::Ipv4Addr) -> bool {
    ip.is_loopback()          // 127.0.0.0/8
        || ip.is_private()    // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
        || ip.is_link_local() // 169.254.0.0/16
        || ip.is_broadcast()  // 255.255.255.255
        || ip.is_unspecified() // 0.0.0.0
        || ip.octets()[0] == 100 && (ip.octets()[1] & 0xC0) == 64 // 100.64.0.0/10 (CGNAT)
}

/// Check if an IPv6 address is private, loopback, or otherwise non-routable.
fn is_private_ipv6(ip: &std::net::Ipv6Addr) -> bool {
    ip.is_loopback()       // ::1
        || ip.is_unspecified() // ::
        || {
            let segments = ip.segments();
            // fc00::/7 (unique local)
            (segments[0] & 0xFE00) == 0xFC00
            // fe80::/10 (link-local)
            || (segments[0] & 0xFFC0) == 0xFE80
            // IPv4-mapped IPv6: check the mapped IPv4 address
            || match ip.to_ipv4_mapped() {
                Some(v4) => is_private_ipv4(&v4),
                None => false,
            }
        }
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

    // Prevent SSRF: validate the host is not a private/loopback address
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => {
            if is_private_ipv4(&ip) {
                return Err(GatewayError::InvalidUrl(
                    "target cannot be a private or loopback IP address".to_string(),
                ));
            }
        }
        Some(url::Host::Ipv6(ip)) => {
            if is_private_ipv6(&ip) {
                return Err(GatewayError::InvalidUrl(
                    "target cannot be a private or loopback IP address".to_string(),
                ));
            }
        }
        Some(url::Host::Domain(domain)) => {
            let domain_lower = domain.to_lowercase();
            if domain_lower == "localhost"
                || domain_lower.ends_with(".localhost")
                || domain_lower.ends_with(".local")
                || domain_lower.ends_with(".internal")
            {
                return Err(GatewayError::InvalidUrl(
                    "target cannot be localhost or local domain".to_string(),
                ));
            }
        }
        None => {
            return Err(GatewayError::InvalidUrl(
                "target URL must have a host".to_string(),
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

    // Require payment (returns 402 with requirements if no valid payment)
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
        Err(http_response) => return Ok(http_response), // Already a proper 402 response
    };

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
        .insert_header(("PAYMENT-RESPONSE", payment_response_header(&settle)))
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
