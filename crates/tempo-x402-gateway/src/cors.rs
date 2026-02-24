//! CORS configuration for the gateway and node binaries.

use actix_cors::Cors;

/// Build the gateway/node CORS middleware from allowed origins.
///
/// Supports wildcard (`*`) origins for dev mode. In production, wildcard CORS
/// is rejected at config validation time.
pub fn build_cors(allowed_origins: &[String]) -> Cors {
    let allowed = allowed_origins.to_vec();
    Cors::default()
        .allowed_origin_fn(move |origin, _req_head| {
            let origin_str = origin.to_str().unwrap_or("");
            allowed.iter().any(|a| {
                if a == "*" {
                    // In dev mode (X402_INSECURE_NO_HMAC), wildcard is permitted
                    // In production, wildcard CORS is rejected at config validation
                    true
                } else {
                    a == origin_str
                }
            })
        })
        .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
        .allowed_headers(vec![
            actix_web::http::header::AUTHORIZATION,
            actix_web::http::header::ACCEPT,
            actix_web::http::header::CONTENT_TYPE,
            actix_web::http::header::HeaderName::from_static("x-payment"),
            actix_web::http::header::HeaderName::from_static("payment-signature"),
        ])
        .expose_headers(vec![
            actix_web::http::header::HeaderName::from_static("x-payment-response"),
            actix_web::http::header::HeaderName::from_static("payment-response"),
        ])
        .max_age(3600)
}
