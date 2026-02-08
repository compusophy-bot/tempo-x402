use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementWebhook {
    pub event: String,
    pub payer: String,
    pub amount: String,
    pub transaction: Option<String>,
    pub network: String,
    pub timestamp: u64,
}

/// Validate that all webhook URLs use HTTPS. Should be called at startup.
pub fn validate_webhook_urls(urls: &[String]) {
    for url in urls {
        if !url.starts_with("https://") {
            tracing::warn!(
                url = %url,
                "webhook URL does not use HTTPS — payloads will be sent in cleartext"
            );
        }
    }
}

/// Fire-and-forget POST to each webhook URL.
/// If `hmac_secret` is provided, includes an `X-Webhook-Signature` HMAC header.
pub fn fire_webhooks(
    client: &reqwest::Client,
    urls: &[String],
    webhook: SettlementWebhook,
    hmac_secret: Option<&[u8]>,
) {
    let body_bytes = match serde_json::to_vec(&webhook) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "failed to serialize webhook payload");
            return;
        }
    };

    for url in urls {
        let client = client.clone();
        let url = url.clone();
        let body = body_bytes.clone();
        let hmac_sig = hmac_secret.map(|secret| x402::hmac::compute_hmac(secret, &body));

        tokio::spawn(async move {
            let mut req = client
                .post(&url)
                .header("content-type", "application/json")
                .timeout(std::time::Duration::from_secs(5));

            if let Some(ref sig) = hmac_sig {
                req = req.header("X-Webhook-Signature", sig.as_str());
            }

            let result = req.body(body).send().await;
            match result {
                Ok(resp) => {
                    tracing::debug!(url = %url, status = %resp.status(), "webhook delivered")
                }
                Err(e) => tracing::warn!(url = %url, error = %e, "webhook delivery failed"),
            }
        });
    }
}
