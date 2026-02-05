use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementWebhook {
    pub event: String,
    pub payer: String,
    pub amount: String,
    pub transaction: String,
    pub network: String,
    pub timestamp: u64,
}

/// Fire-and-forget POST to each webhook URL.
pub fn fire_webhooks(client: &reqwest::Client, urls: &[String], webhook: SettlementWebhook) {
    for url in urls {
        let client = client.clone();
        let url = url.clone();
        let body = webhook.clone();
        tokio::spawn(async move {
            let result = client
                .post(&url)
                .json(&body)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;
            match result {
                Ok(resp) => {
                    tracing::debug!(url = %url, status = %resp.status(), "webhook delivered")
                }
                Err(e) => tracing::warn!(url = %url, error = %e, "webhook delivery failed"),
            }
        });
    }
}
