use base64::Engine;
use x402_types::{
    PaymentPayload, PaymentRequiredBody, SchemeClient, SettleResponse, X402Error, SCHEME_NAME,
};

/// HTTP client wrapper that automatically handles 402 responses.
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
            .header("X-PAYMENT", &encoded)
            .send()
            .await
            .map_err(|e| X402Error::HttpError(format!("paid request failed: {e}")))?;

        // Extract settlement info from headers
        let settle = resp
            .headers()
            .get("x-payment-response")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| serde_json::from_str::<SettleResponse>(s).ok());

        Ok((resp, settle))
    }
}

/// Base64-encode a payment payload for the X-PAYMENT header.
pub fn encode_payment(payload: &PaymentPayload) -> Result<String, X402Error> {
    let json = serde_json::to_vec(payload)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&json))
}
