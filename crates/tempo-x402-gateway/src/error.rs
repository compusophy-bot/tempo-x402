use actix_web::{HttpResponse, ResponseError};
use std::fmt;

#[derive(Debug)]
pub enum GatewayError {
    /// Database error
    Database(rusqlite::Error),
    /// Endpoint not found
    EndpointNotFound(String),
    /// Slug already exists
    SlugExists(String),
    /// Invalid slug format
    InvalidSlug(String),
    /// Invalid URL
    InvalidUrl(String),
    /// Invalid price format
    InvalidPrice(String),
    /// Payment required
    PaymentRequired,
    /// Payment verification failed
    PaymentFailed(String),
    /// Not the endpoint owner
    NotOwner,
    /// Proxy error
    ProxyError(String),
    /// Internal error
    Internal(String),
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GatewayError::Database(e) => write!(f, "database error: {}", e),
            GatewayError::EndpointNotFound(slug) => write!(f, "endpoint not found: {}", slug),
            GatewayError::SlugExists(slug) => write!(f, "slug already exists: {}", slug),
            GatewayError::InvalidSlug(msg) => write!(f, "invalid slug: {}", msg),
            GatewayError::InvalidUrl(msg) => write!(f, "invalid URL: {}", msg),
            GatewayError::InvalidPrice(msg) => write!(f, "invalid price: {}", msg),
            GatewayError::PaymentRequired => write!(f, "payment required"),
            GatewayError::PaymentFailed(msg) => write!(f, "payment failed: {}", msg),
            GatewayError::NotOwner => write!(f, "not the endpoint owner"),
            GatewayError::ProxyError(msg) => write!(f, "proxy error: {}", msg),
            GatewayError::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

impl std::error::Error for GatewayError {}

impl From<rusqlite::Error> for GatewayError {
    fn from(e: rusqlite::Error) -> Self {
        // Check for unique constraint violation
        if let rusqlite::Error::SqliteFailure(ref err, _) = e {
            if err.extended_code == 2067 {
                // SQLITE_CONSTRAINT_UNIQUE
                return GatewayError::SlugExists("slug already exists".to_string());
            }
        }
        GatewayError::Database(e)
    }
}

impl ResponseError for GatewayError {
    fn error_response(&self) -> HttpResponse {
        match self {
            GatewayError::EndpointNotFound(slug) => {
                HttpResponse::NotFound().json(serde_json::json!({
                    "error": "endpoint_not_found",
                    "message": format!("Endpoint '{}' not found", slug)
                }))
            }
            GatewayError::SlugExists(slug) => HttpResponse::Conflict().json(serde_json::json!({
                "error": "slug_exists",
                "message": format!("Slug '{}' is already taken", slug)
            })),
            GatewayError::InvalidSlug(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "error": "invalid_slug",
                "message": msg
            })),
            GatewayError::InvalidUrl(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "error": "invalid_url",
                "message": msg
            })),
            GatewayError::InvalidPrice(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "error": "invalid_price",
                "message": msg
            })),
            GatewayError::PaymentRequired => {
                HttpResponse::PaymentRequired().json(serde_json::json!({
                    "error": "payment_required",
                    "message": "Payment required to access this resource"
                }))
            }
            GatewayError::PaymentFailed(msg) => {
                HttpResponse::PaymentRequired().json(serde_json::json!({
                    "error": "payment_failed",
                    "message": msg
                }))
            }
            GatewayError::NotOwner => HttpResponse::Forbidden().json(serde_json::json!({
                "error": "not_owner",
                "message": "Only the endpoint owner can modify it"
            })),
            GatewayError::ProxyError(msg) => {
                tracing::error!("Proxy error: {}", msg);
                HttpResponse::BadGateway().json(serde_json::json!({
                    "error": "proxy_error",
                    "message": "Failed to reach upstream service"
                }))
            }
            GatewayError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "internal_error",
                    "message": "An internal error occurred"
                }))
            }
            GatewayError::Database(e) => {
                tracing::error!("Database error: {}", e);
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "internal_error",
                    "message": "An internal error occurred"
                }))
            }
        }
    }
}
