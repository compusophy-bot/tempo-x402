use base64::Engine;
use x402::{
    PaymentPayload, PaymentRequiredBody, SchemeClient, SettleResponse, X402Error, SCHEME_NAME,
};

/// HTTP client that automatically handles 402 payment responses.
///
/// Wraps `reqwest::Client`. On a 402 response, it parses the payment
/// requirements, signs an EIP-712 authorization via the provided
/// [`SchemeClient`], and retries the request with a `PAYMENT-SIGNATURE` header.
pub struct X402Client<S: SchemeClient> {
    http: reqwest::Client,
    scheme: S,
}

impl<S: SchemeClient> X402Client<S> {
    pub fn new(scheme: S) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("failed to build HTTP client"),
            scheme,
        }
    }

    /// Create a client with a custom reqwest::Client.
    pub fn with_http_client(scheme: S, http: reqwest::Client) -> Self {
        Self { http, scheme }
    }

    /// Make a request, automatically handling 402 payment responses.
    /// Returns the final response and optional settlement info.
    pub async fn fetch(
        &self,
        url: &str,
        method: reqwest::Method,
    ) -> Result<(reqwest::Response, Option<SettleResponse>), X402Error> {
        self.fetch_with_body(url, method, None).await
    }

    /// Make a request with an optional body, automatically handling 402 payment responses.
    pub async fn fetch_with_body(
        &self,
        url: &str,
        method: reqwest::Method,
        body: Option<Vec<u8>>,
    ) -> Result<(reqwest::Response, Option<SettleResponse>), X402Error> {
        // First request
        let mut req = self.http.request(method.clone(), url);
        if let Some(ref b) = body {
            req = req.body(b.clone());
        }

        let resp = req
            .send()
            .await
            .map_err(|e| X402Error::HttpError(format!("request failed: {e}")))?;

        if resp.status().as_u16() != 402 {
            return Ok((resp, None));
        }

        // Parse 402 body
        let body_402: PaymentRequiredBody = resp
            .json()
            .await
            .map_err(|e| X402Error::HttpError(format!("failed to parse 402 body: {e}")))?;

        // Find a matching scheme
        let requirements = body_402
            .accepts
            .iter()
            .find(|r| r.scheme == SCHEME_NAME)
            .ok_or_else(|| {
                X402Error::UnsupportedScheme(format!(
                    "no supported scheme found in {:?}",
                    body_402
                        .accepts
                        .iter()
                        .map(|r| &r.scheme)
                        .collect::<Vec<_>>()
                ))
            })?;

        // Create signed payment payload
        let payload = self
            .scheme
            .create_payment_payload(body_402.x402_version, requirements)
            .await?;

        // Encode and retry
        let encoded = encode_payment(&payload)?;

        let mut req = self.http.request(method, url);
        req = req.header("PAYMENT-SIGNATURE", &encoded);
        if let Some(b) = body {
            req = req.body(b);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| X402Error::HttpError(format!("paid request failed: {e}")))?;

        // Extract settlement info from headers
        let settle = resp
            .headers()
            .get("payment-response")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                // Try to decode as base64 first
                base64::engine::general_purpose::STANDARD
                    .decode(s)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<SettleResponse>(&bytes).ok())
                    // Fall back to plain JSON
                    .or_else(|| serde_json::from_str::<SettleResponse>(s).ok())
            });

        Ok((resp, settle))
    }
}

/// Base64-encode a payment payload for the PAYMENT-SIGNATURE header.
pub fn encode_payment(payload: &PaymentPayload) -> Result<String, X402Error> {
    let json = serde_json::to_vec(payload)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&json))
}

/// Decode a payment payload from the PAYMENT-SIGNATURE header.
pub fn decode_payment(encoded: &str) -> Result<PaymentPayload, X402Error> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| X402Error::InvalidPayment(format!("invalid base64: {e}")))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| X402Error::InvalidPayment(format!("invalid JSON: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, FixedBytes};
    use x402::TempoPaymentData;

    fn sample_payload() -> PaymentPayload {
        PaymentPayload {
            x402_version: 1,
            payload: TempoPaymentData {
                from: Address::ZERO,
                to: Address::ZERO,
                value: "1000".to_string(),
                token: Address::ZERO,
                valid_after: 0,
                valid_before: u64::MAX,
                nonce: FixedBytes::ZERO,
                signature: "0xdead".to_string(),
            },
        }
    }

    #[test]
    fn test_encode_payment_roundtrip() {
        let payload = sample_payload();
        let encoded = encode_payment(&payload).unwrap();
        let decoded = decode_payment(&encoded).unwrap();

        assert_eq!(decoded.x402_version, payload.x402_version);
        assert_eq!(decoded.payload.from, payload.payload.from);
        assert_eq!(decoded.payload.value, payload.payload.value);
        assert_eq!(decoded.payload.signature, payload.payload.signature);
    }

    #[test]
    fn test_encode_produces_valid_base64() {
        let payload = sample_payload();
        let encoded = encode_payment(&payload).unwrap();

        // Should decode without error
        let result = base64::engine::general_purpose::STANDARD.decode(&encoded);
        assert!(result.is_ok());

        // Should be valid JSON
        let json: Result<serde_json::Value, _> = serde_json::from_slice(&result.unwrap());
        assert!(json.is_ok());
    }
}
