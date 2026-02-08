use actix_web::{HttpRequest, HttpResponse};
use bytes::Bytes;
use x402::SettleResponse;

use crate::error::GatewayError;
use crate::middleware::payment_response_header;

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

/// Proxy an HTTP request to the target URL
pub async fn proxy_request(
    client: &reqwest::Client,
    original_req: &HttpRequest,
    target_url: &str,
    body: Bytes,
    settle: &SettleResponse,
    include_payment_response: bool,
) -> Result<HttpResponse, GatewayError> {
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

    let mut request_builder = client.request(method, target_url);

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

    // Get response body
    let body = response.bytes().await.map_err(|e| {
        tracing::error!(error = %e, "failed to read proxy response body");
        GatewayError::ProxyError("failed to read upstream response".to_string())
    })?;

    // Build actix response
    let mut builder = HttpResponse::build(
        actix_web::http::StatusCode::from_u16(status.as_u16())
            .unwrap_or(actix_web::http::StatusCode::OK),
    );

    // Copy response headers
    for (name, value) in headers.iter() {
        // Skip hop-by-hop headers
        let name_lower = name.as_str().to_lowercase();
        if name_lower == "transfer-encoding" || name_lower == "connection" {
            continue;
        }
        if let Ok(value_str) = value.to_str() {
            builder.insert_header((name.as_str(), value_str));
        }
    }

    // Add payment response header if requested
    if include_payment_response {
        builder.insert_header(("PAYMENT-RESPONSE", payment_response_header(settle)));
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
}
