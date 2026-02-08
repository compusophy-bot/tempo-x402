use serde::Serialize;
use std::net::{Ipv4Addr, Ipv6Addr};

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

/// Check if an IPv4 address is private/loopback/non-routable.
fn is_private_ipv4(ip: &Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xC0) == 64)
}

/// Check if an IPv6 address is private/loopback/non-routable.
fn is_private_ipv6(ip: &Ipv6Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified() || {
        let segments = ip.segments();
        (segments[0] & 0xFE00) == 0xFC00
            || (segments[0] & 0xFFC0) == 0xFE80
            || match ip.to_ipv4_mapped() {
                Some(v4) => is_private_ipv4(&v4),
                None => false,
            }
    }
}

/// Validate that all webhook URLs use HTTPS and do not target private IPs.
/// Should be called at startup. Returns an error for any invalid URL.
pub fn validate_webhook_urls(urls: &[String]) -> Result<(), String> {
    for url_str in urls {
        if !url_str.starts_with("https://") {
            return Err(format!(
                "webhook URL must use HTTPS: {url_str} — cleartext webhook delivery is not allowed"
            ));
        }

        // Check for private/loopback IPs in webhook URLs
        if let Ok(parsed) = url::Url::parse(url_str) {
            match parsed.host() {
                Some(url::Host::Ipv4(ip)) => {
                    if is_private_ipv4(&ip) {
                        return Err(format!(
                            "webhook URL targets a private/loopback IP: {url_str}"
                        ));
                    }
                }
                Some(url::Host::Domain(domain)) => {
                    let d = domain.to_lowercase();
                    if d == "localhost" || d.ends_with(".local") || d.ends_with(".internal") {
                        return Err(format!(
                            "webhook URL targets localhost/local domain: {url_str}"
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Resolve a webhook URL's hostname and validate the IP is not private.
/// Returns Ok(()) if safe to connect, Err with reason if not.
async fn validate_webhook_ip(url_str: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url_str).map_err(|e| format!("invalid URL: {e}"))?;
    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return Err("URL has no host".to_string()),
    };

    // Direct IP check
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        if is_private_ipv4(&ip) {
            return Err("resolves to private IP".to_string());
        }
        return Ok(());
    }
    if let Ok(ip) = host.parse::<Ipv6Addr>() {
        if is_private_ipv6(&ip) {
            return Err("resolves to private IP".to_string());
        }
        return Ok(());
    }

    // DNS resolution
    let lookup = format!("{}:{}", host, parsed.port().unwrap_or(443));
    let addrs: Vec<_> = tokio::net::lookup_host(&lookup)
        .await
        .map_err(|e| format!("DNS resolution failed: {e}"))?
        .collect();

    for addr in &addrs {
        match addr.ip() {
            std::net::IpAddr::V4(ip) => {
                if is_private_ipv4(&ip) {
                    return Err(format!("resolves to private IP: {ip}"));
                }
            }
            std::net::IpAddr::V6(ip) => {
                if is_private_ipv6(&ip) {
                    return Err(format!("resolves to private IP: {ip}"));
                }
            }
        }
    }

    if addrs.is_empty() {
        return Err("DNS returned no addresses".to_string());
    }
    Ok(())
}

/// Resolve webhook URL DNS, validate IP, and rewrite URL to the resolved IP.
/// This pins the IP to prevent DNS rebinding between validation and connection.
async fn pin_webhook_url(url_str: &str) -> Result<String, String> {
    validate_webhook_ip(url_str).await?;

    let mut parsed = url::Url::parse(url_str).map_err(|e| format!("invalid URL: {e}"))?;
    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return Err("URL has no host".to_string()),
    };

    // If already an IP, no rewrite needed
    if host.parse::<Ipv4Addr>().is_ok() || host.parse::<Ipv6Addr>().is_ok() {
        return Ok(url_str.to_string());
    }

    // Resolve and rewrite
    let lookup = format!("{}:{}", host, parsed.port().unwrap_or(443));
    let addr = tokio::net::lookup_host(&lookup)
        .await
        .map_err(|e| format!("DNS resolution failed: {e}"))?
        .next()
        .ok_or_else(|| "DNS returned no addresses".to_string())?;

    let ip_host = match addr.ip() {
        std::net::IpAddr::V6(ip) => format!("[{}]", ip),
        std::net::IpAddr::V4(ip) => ip.to_string(),
    };

    parsed
        .set_host(Some(&ip_host))
        .map_err(|_| "failed to set resolved IP in webhook URL".to_string())?;

    Ok(parsed.to_string())
}

/// Create a webhook-specific HTTP client with redirects disabled.
pub fn webhook_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to create webhook HTTP client")
}

/// Global semaphore to limit concurrent webhook deliveries.
/// Prevents resource exhaustion from too many simultaneous webhook spawns.
static WEBHOOK_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(50);

/// Total timeout for a single webhook delivery task (including all retries).
const WEBHOOK_TASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Fire-and-forget POST to each webhook URL.
/// If `hmac_secret` is provided, includes an `X-Webhook-Signature` HMAC header.
/// Validates resolved IPs at delivery time to prevent DNS rebinding SSRF.
/// Uses a no-redirect client to prevent redirect-based SSRF.
/// Concurrency is limited to 50 simultaneous webhook deliveries.
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
            // Acquire semaphore permit to limit concurrent webhook deliveries
            let _permit = match WEBHOOK_SEMAPHORE.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    tracing::error!(url = %url, "webhook semaphore closed");
                    return;
                }
            };

            // Wrap entire delivery in a timeout to prevent unbounded retries
            let delivery = async {
            // Validate resolved IP at delivery time and pin it to prevent DNS rebinding.
            let pinned_url = match pin_webhook_url(&url).await {
                Ok(u) => u,
                Err(reason) => {
                    tracing::warn!(
                        url = %url,
                        reason = %reason,
                        "webhook delivery blocked — target resolves to unsafe IP"
                    );
                    return;
                }
            };

            let original_host = url::Url::parse(&url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()));

            // Retry with exponential backoff: 1s, 5s, 15s
            let delays = [1, 5, 15];
            for (attempt, delay_secs) in std::iter::once(&0u64).chain(delays.iter()).enumerate() {
                if *delay_secs > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(*delay_secs)).await;
                }

                let mut req = client
                    .post(&pinned_url)
                    .header("content-type", "application/json")
                    .timeout(std::time::Duration::from_secs(5));

                if let Some(ref host) = original_host {
                    req = req.header("host", host.as_str());
                }
                if let Some(ref sig) = hmac_sig {
                    req = req.header("X-Webhook-Signature", sig.as_str());
                }

                match req.body(body.clone()).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        tracing::debug!(url = %url, status = %resp.status(), "webhook delivered");
                        return;
                    }
                    Ok(resp) if resp.status().is_redirection() => {
                        tracing::warn!(
                            url = %url, status = %resp.status(),
                            "webhook endpoint returned redirect — delivery not confirmed (redirects disabled)"
                        );
                        return; // Do not retry: the data was sent but may not have been processed
                    }
                    Ok(resp) => {
                        tracing::warn!(
                            url = %url, status = %resp.status(), attempt = attempt + 1,
                            "webhook delivery failed (non-success status)"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            url = %url, error = %e, attempt = attempt + 1,
                            "webhook delivery failed"
                        );
                    }
                }
            }
            tracing::error!(url = %url, "webhook delivery failed after all retries");
            }; // end async block

            if tokio::time::timeout(WEBHOOK_TASK_TIMEOUT, delivery).await.is_err() {
                tracing::error!(url = %url, "webhook delivery timed out after {:?}", WEBHOOK_TASK_TIMEOUT);
            }
        });
    }
}
