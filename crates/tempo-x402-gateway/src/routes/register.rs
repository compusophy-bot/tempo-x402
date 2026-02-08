use actix_web::{web, HttpRequest, HttpResponse};
use x402::SchemeServer;

use crate::db::CreateEndpoint;
use crate::error::GatewayError;
use crate::middleware::{payment_response_header, platform_requirements, require_payment};
use crate::state::AppState;
use crate::validation::validate_target_url;

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

/// POST /register - Register a new endpoint
pub async fn register(
    req: HttpRequest,
    body: web::Json<CreateEndpoint>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GatewayError> {
    // Validate inputs first (before requiring payment)
    validate_slug(&body.slug)?;
    validate_target_url(&body.target_url)?;
    if let Some(ref desc) = body.description {
        if desc.len() > 4096 {
            return Err(GatewayError::InvalidSlug(
                "description must be at most 4096 characters".to_string(),
            ));
        }
    }

    // Parse price early so we fail fast on bad input
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

    // Check for PAYMENT-SIGNATURE header first WITHOUT touching the database.
    // This prevents DoS via rapid POST /register with no payment header, which
    // would otherwise cause INSERT/DELETE write amplification on SQLite.
    if crate::middleware::extract_payment_header(&req).is_none() {
        return Ok(crate::middleware::payment_required_response(requirements));
    }

    // Reserve the slug atomically BEFORE settlement.
    // This prevents a race where two concurrent registrations both pass the
    // slug_exists check, both pay, but only one can actually create the endpoint.
    state.db.reserve_slug(&body.slug)?;

    // Require payment (will re-extract header and verify+settle)
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
        Err(http_response) => {
            // Payment verification/settlement failed — release the slug reservation
            let _ = state.db.delete_reserved_slug(&body.slug);
            return Ok(http_response);
        }
    };

    // Extract payer address from settlement response
    let owner_address = match settle.payer {
        Some(addr) => addr,
        None => {
            let _ = state.db.delete_reserved_slug(&body.slug);
            return Err(GatewayError::Internal(
                "settlement missing payer address".to_string(),
            ));
        }
    };

    // Activate the reserved slug with full endpoint data
    let endpoint = match state.db.activate_endpoint(
        &body.slug,
        &format!("{:#x}", owner_address),
        &body.target_url,
        &body.price,
        &price_amount,
        body.description.as_deref(),
    ) {
        Ok(ep) => ep,
        Err(e) => {
            let _ = state.db.delete_reserved_slug(&body.slug);
            return Err(e);
        }
    };

    // Return success with payment response header
    Ok(HttpResponse::Created()
        .insert_header((
            "PAYMENT-RESPONSE",
            payment_response_header(&settle, state.config.hmac_secret.as_deref()),
        ))
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
