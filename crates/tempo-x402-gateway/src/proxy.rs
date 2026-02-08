use actix_web::{HttpRequest, HttpResponse};
use bytes::Bytes;
use x402::SettleResponse;

use crate::error::GatewayError;
use crate::middleware::payment_response_header;
use crate::validation::validate_and_resolve_ip;

/// Headers to strip from client request before proxying
const HEADERS_TO_STRIP: &[&str] = &[
    "host",
    "connection",
    "keep-alive",
    "transfer-encoding",
    "payment-signature",
    "content-length", // Will be recalculated
    // Strip authentication headers to prevent credential leakage to upstream
    "authorization",
    "cookie",
    "proxy-authorization",
    "x-api-key",
    // Strip x402 verification headers to prevent client spoofing
    "x-x402-verified",
    "x-x402-payer",
    "x-x402-txhash",
    "x-x402-network",
];

/// Allowlist of response headers to forward from the upstream.
/// Prevents leaking internal upstream headers (e.g. Server, X-Powered-By).
const ALLOWED_RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-length",
    "content-encoding",
    "cache-control",
    "etag",
    "last-modified",
    "date",
    "vary",
    "x-request-id",
    "x-ratelimit-limit",
    "x-ratelimit-remaining",
    "x-ratelimit-reset",
    "access-control-allow-origin",
];

/// Maximum upstream response body size (10 MB).
const MAX_RESPONSE_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Proxy an HTTP request to the target URL
pub async fn proxy_request(
    client: &reqwest::Client,
    original_req: &HttpRequest,
    target_url: &str,
    body: Bytes,
    settle: &SettleResponse,
    include_payment_response: bool,
    hmac_secret: Option<&[u8]>,
) -> Result<HttpResponse, GatewayError> {
    // DNS rebinding fix: resolve DNS once and rewrite the URL to the validated IP.
    // This eliminates the TOCTOU gap where a second DNS lookup (by reqwest) could
    // resolve to a different (private) IP.
    let mut actual_url = target_url.to_string();
    let mut original_host: Option<String> = None;

    if let Ok(parsed) = url::Url::parse(target_url) {
        if let Some(host) = parsed.host_str() {
            // Skip IP-rewrite for hosts that are already IPs
            if host.parse::<std::net::Ipv4Addr>().is_err()
                && host.parse::<std::net::Ipv6Addr>().is_err()
            {
                let resolved_ip = validate_and_resolve_ip(host).await?;
                // Rewrite URL to use the resolved IP so reqwest connects to it directly
                let ip_str = match resolved_ip {
                    std::net::IpAddr::V6(ip) => format!("[{}]", ip),
                    std::net::IpAddr::V4(ip) => ip.to_string(),
                };
                actual_url = actual_url.replacen(host, &ip_str, 1);
                original_host = Some(host.to_string());
            } else {
                // Already an IP — just validate it
                validate_and_resolve_ip(host).await?;
            }
        }
    }

    // Build the proxied request
    let method = match original_req.method().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        other => {
            return Err(GatewayError::ProxyError(format!(
                "unsupported HTTP method: {}",
                other
            )));
        }
    };

    let mut request_builder = client.request(method, &actual_url);

    // Set the Host header to the original hostname (not the resolved IP)
    if let Some(ref host) = original_host {
        request_builder = request_builder.header("host", host.as_str());
    }

    // Copy headers from original request (except stripped ones)
    for (name, value) in original_req.headers() {
        let name_lower = name.as_str().to_lowercase();
        if !HEADERS_TO_STRIP.contains(&name_lower.as_str()) {
            if let Ok(value_str) = value.to_str() {
                request_builder = request_builder.header(name.as_str(), value_str);
            }
        }
    }

    // Add x402 verification headers
    request_builder = request_builder.header("X-X402-Verified", "true");

    if let Some(ref payer) = settle.payer {
        request_builder = request_builder.header("X-X402-Payer", format!("{:#x}", payer));
    }

    if let Some(ref tx) = settle.transaction {
        request_builder = request_builder.header("X-X402-TxHash", tx);
    }
    request_builder = request_builder.header("X-X402-Network", &settle.network);

    // Add body if present
    if !body.is_empty() {
        request_builder = request_builder.body(body.to_vec());
    }

    // Send the request
    let response = request_builder.send().await.map_err(|e| {
        tracing::error!(error = %e, "proxy request failed");
        GatewayError::ProxyError("upstream request failed".to_string())
    })?;

    // Build the response
    let status = response.status();
    let headers = response.headers().clone();

    // Check Content-Length before reading (fast path)
    if let Some(cl) = response.content_length() {
        if cl > MAX_RESPONSE_BODY_SIZE as u64 {
            return Err(GatewayError::ProxyError(format!(
                "upstream response too large: {} bytes (max {})",
                cl, MAX_RESPONSE_BODY_SIZE
            )));
        }
    }

    // Get response body with size limit
    let body = response.bytes().await.map_err(|e| {
        tracing::error!(error = %e, "failed to read proxy response body");
        GatewayError::ProxyError("failed to read upstream response".to_string())
    })?;

    if body.len() > MAX_RESPONSE_BODY_SIZE {
        return Err(GatewayError::ProxyError(format!(
            "upstream response too large: {} bytes (max {})",
            body.len(),
            MAX_RESPONSE_BODY_SIZE
        )));
    }

    // Build actix response
    let mut builder = HttpResponse::build(
        actix_web::http::StatusCode::from_u16(status.as_u16())
            .unwrap_or(actix_web::http::StatusCode::OK),
    );

    // Copy only allowlisted response headers from upstream
    for (name, value) in headers.iter() {
        let name_lower = name.as_str().to_lowercase();
        if ALLOWED_RESPONSE_HEADERS.contains(&name_lower.as_str()) {
            if let Ok(value_str) = value.to_str() {
                builder.insert_header((name.as_str(), value_str));
            }
        }
    }

    // Add payment response header if requested
    if include_payment_response {
        builder.insert_header((
            "PAYMENT-RESPONSE",
            payment_response_header(settle, hmac_secret),
        ));
    }

    Ok(builder.body(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers_to_strip() {
        assert!(HEADERS_TO_STRIP.contains(&"host"));
        assert!(HEADERS_TO_STRIP.contains(&"payment-signature"));
        assert!(!HEADERS_TO_STRIP.contains(&"content-type"));
    }

    #[test]
    fn test_allowed_response_headers() {
        assert!(ALLOWED_RESPONSE_HEADERS.contains(&"content-type"));
        assert!(ALLOWED_RESPONSE_HEADERS.contains(&"cache-control"));
        assert!(!ALLOWED_RESPONSE_HEADERS.contains(&"server"));
        assert!(!ALLOWED_RESPONSE_HEADERS.contains(&"x-powered-by"));
    }
}
