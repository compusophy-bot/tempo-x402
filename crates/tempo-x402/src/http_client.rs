use crate::{
    PaymentPayload, PaymentRequiredBody, SchemeClient, SettleResponse, X402Error, SCHEME_NAME,
};
use base64::Engine;

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
            http: reqwest::Client::new(),
            scheme,
        }
    }

    /// Make a request, automatically handling 402 payment responses.
    /// Returns the final response and optional settlement info.
    pub async fn fetch(
        &self,
        url: &str,
        method: reqwest::Method,
    ) -> Result<(reqwest::Response, Option<SettleResponse>), X402Error> {
        // First request
        let resp = self
            .http
            .request(method.clone(), url)
            .send()
            .await
            .map_err(|e| X402Error::HttpError(format!("request failed: {e}")))?;

        if resp.status().as_u16() != 402 {
            return Ok((resp, None));
        }

        // Parse 402 body
        let body: PaymentRequiredBody = resp
            .json()
            .await
            .map_err(|e| X402Error::HttpError(format!("failed to parse 402 body: {e}")))?;

        // Find a matching scheme
        let requirements = body
            .accepts
            .iter()
            .find(|r| r.scheme == SCHEME_NAME)
            .ok_or_else(|| {
                X402Error::UnsupportedScheme(format!(
                    "no supported scheme found in {:?}",
                    body.accepts.iter().map(|r| &r.scheme).collect::<Vec<_>>()
                ))
            })?;

        // Create signed payment payload
        let payload = self
            .scheme
            .create_payment_payload(body.x402_version, requirements)
            .await?;

        // Encode and retry
        let encoded = encode_payment(&payload)?;

        let resp = self
            .http
            .request(method, url)
            .header("PAYMENT-SIGNATURE", &encoded)
            .send()
            .await
            .map_err(|e| X402Error::HttpError(format!("paid request failed: {e}")))?;

        // Extract settlement info from headers
        let settle = resp
            .headers()
            .get("payment-response")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| serde_json::from_str::<SettleResponse>(s).ok());

        Ok((resp, settle))
    }
}

/// Base64-encode a payment payload for the PAYMENT-SIGNATURE header.
pub fn encode_payment(payload: &PaymentPayload) -> Result<String, X402Error> {
    let json = serde_json::to_vec(payload)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TempoPaymentData;
    use alloy::primitives::{Address, FixedBytes};

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

        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        let decoded: PaymentPayload = serde_json::from_slice(&decoded_bytes).unwrap();

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
