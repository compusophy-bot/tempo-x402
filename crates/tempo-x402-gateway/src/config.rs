use alloy::primitives::Address;
use std::env;
use url::Url;

const DEFAULT_FACILITATOR_URL: &str = "https://x402-facilitator-production-ec87.up.railway.app";
const DEFAULT_PORT: u16 = 4023;
const DEFAULT_PLATFORM_FEE: &str = "$0.01";
const DEFAULT_DB_PATH: &str = "./gateway.db";
const DEFAULT_RATE_LIMIT_RPM: u32 = 60;

#[derive(Clone)]
pub struct GatewayConfig {
    /// Platform fee recipient address
    pub platform_address: Address,
    /// Facilitator URL for payment verification
    pub facilitator_url: String,
    /// HMAC shared secret for facilitator auth (None = dev mode)
    pub hmac_secret: Option<Vec<u8>>,
    /// SQLite database path
    pub db_path: String,
    /// Server port
    pub port: u16,
    /// Platform registration fee (e.g., "$0.01")
    pub platform_fee: String,
    /// Platform fee amount in token units (computed from platform_fee)
    pub platform_fee_amount: String,
    /// CORS allowed origins
    pub allowed_origins: Vec<String>,
    /// Rate limit requests per minute
    pub rate_limit_rpm: u32,
    /// Facilitator private key — if set, run facilitator in-process
    pub facilitator_private_key: Option<String>,
    /// Nonce DB path for embedded facilitator
    pub nonce_db_path: String,
    /// Webhook URLs for settlement notifications
    pub webhook_urls: Vec<String>,
    /// RPC URL for chain access
    pub rpc_url: String,
    /// Directory to serve SPA static files from (None = don't serve SPA)
    pub spa_dir: Option<String>,
    /// Bearer token required for /metrics endpoint (None = public)
    pub metrics_token: Option<String>,
}

impl std::fmt::Debug for GatewayConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayConfig")
            .field("platform_address", &self.platform_address)
            .field("facilitator_url", &self.facilitator_url)
            .field(
                "hmac_secret",
                &self.hmac_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("db_path", &self.db_path)
            .field("port", &self.port)
            .field("platform_fee", &self.platform_fee)
            .field("platform_fee_amount", &self.platform_fee_amount)
            .field("allowed_origins", &self.allowed_origins)
            .field("rate_limit_rpm", &self.rate_limit_rpm)
            .field(
                "facilitator_private_key",
                &self.facilitator_private_key.as_ref().map(|_| "[REDACTED]"),
            )
            .field("nonce_db_path", &self.nonce_db_path)
            .field("webhook_urls", &self.webhook_urls)
            .field("rpc_url", &self.rpc_url)
            .field("spa_dir", &self.spa_dir)
            .field(
                "metrics_token",
                &self.metrics_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl GatewayConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        // Required: platform address
        let platform_address_str =
            env::var("EVM_ADDRESS").map_err(|_| ConfigError::MissingRequired("EVM_ADDRESS"))?;
        let platform_address: Address = platform_address_str
            .parse()
            .map_err(|_| ConfigError::InvalidAddress(platform_address_str))?;

        // Optional: facilitator URL
        let facilitator_url =
            env::var("FACILITATOR_URL").unwrap_or_else(|_| DEFAULT_FACILITATOR_URL.to_string());
        // Validate URL
        Url::parse(&facilitator_url)
            .map_err(|_| ConfigError::InvalidUrl(facilitator_url.clone()))?;

        // Optional: HMAC secret
        let hmac_secret = env::var("FACILITATOR_SHARED_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.into_bytes());

        // Optional: database path
        let db_path = env::var("DB_PATH").unwrap_or_else(|_| DEFAULT_DB_PATH.to_string());

        // Optional: port
        let port = env::var("PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT);

        // Optional: platform fee
        let platform_fee =
            env::var("PLATFORM_FEE").unwrap_or_else(|_| DEFAULT_PLATFORM_FEE.to_string());

        // Parse platform fee to amount using tempo-x402
        let platform_fee_amount = parse_price_to_amount(&platform_fee)?;

        // Optional: allowed origins
        let mut allowed_origins: Vec<String> = env::var("ALLOWED_ORIGINS")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| {
                vec![
                    "http://localhost:3000".to_string(),
                    "http://localhost:5173".to_string(),
                ]
            });
        // Each instance serves its own WASM frontend, so no external demo origin needed.

        // Optional: rate limit
        let rate_limit_rpm = env::var("RATE_LIMIT_RPM")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_RPM);

        // Optional: embedded facilitator private key
        let facilitator_private_key = env::var("FACILITATOR_PRIVATE_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        // Optional: nonce DB path for embedded facilitator
        let nonce_db_path =
            env::var("NONCE_DB_PATH").unwrap_or_else(|_| "./x402-nonces.db".to_string());

        // Optional: webhook URLs
        let webhook_urls: Vec<String> = env::var("WEBHOOK_URLS")
            .ok()
            .map(|urls| {
                urls.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Optional: RPC URL
        let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| x402::constants::RPC_URL.to_string());

        // Optional: SPA directory
        let spa_dir = env::var("SPA_DIR").ok().filter(|s| !s.is_empty());

        // Optional: metrics token
        let metrics_token = env::var("METRICS_TOKEN").ok().filter(|s| !s.is_empty());

        let insecure_no_hmac = env::var("X402_INSECURE_NO_HMAC")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        if let Some(ref secret) = hmac_secret {
            if secret.len() < 32 {
                tracing::warn!(
                    "FACILITATOR_SHARED_SECRET is too short ({} bytes, minimum 32) — \
                     use `openssl rand -hex 32` to generate a secure secret",
                    secret.len()
                );
            }
        } else if insecure_no_hmac {
            tracing::warn!(
                "⚠️  X402_INSECURE_NO_HMAC=true — facilitator requests will be UNAUTHENTICATED. \
                 DO NOT use this in production!"
            );
        } else {
            tracing::error!(
                "FACILITATOR_SHARED_SECRET is required. \
                 Set it to a secure random value (e.g. `openssl rand -hex 32`). \
                 For local development only, set X402_INSECURE_NO_HMAC=true to skip."
            );
            return Err(ConfigError::MissingRequired("FACILITATOR_SHARED_SECRET"));
        }

        // Reject wildcard CORS in production mode
        if allowed_origins.contains(&"*".to_string()) && !insecure_no_hmac {
            tracing::error!(
                "Wildcard CORS origin '*' is not allowed for payment endpoints in production. \
                 Specify explicit origins in ALLOWED_ORIGINS, or set X402_INSECURE_NO_HMAC=true for dev."
            );
            return Err(ConfigError::InvalidUrl(
                "wildcard CORS origin '*' is not allowed in production".to_string(),
            ));
        }

        if metrics_token.is_none() {
            tracing::warn!("METRICS_TOKEN not set — /metrics endpoint is publicly accessible");
        }

        Ok(Self {
            platform_address,
            facilitator_url,
            hmac_secret,
            db_path,
            port,
            platform_fee,
            platform_fee_amount,
            allowed_origins,
            rate_limit_rpm,
            facilitator_private_key,
            nonce_db_path,
            webhook_urls,
            rpc_url,
            spa_dir,
            metrics_token,
        })
    }
}

/// Parse a price string like "$0.01" to token amount string
pub fn parse_price_to_amount(price: &str) -> Result<String, ConfigError> {
    use x402::scheme::SchemeServer;
    use x402::scheme_server::TempoSchemeServer;

    let scheme = TempoSchemeServer::new();
    let (amount, _) = scheme
        .parse_price(price)
        .map_err(|e| ConfigError::InvalidPrice(format!("{}: {}", price, e)))?;
    Ok(amount)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingRequired(&'static str),

    #[error("invalid address: {0}")]
    InvalidAddress(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("invalid price: {0}")]
    InvalidPrice(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_price_to_amount() {
        // $0.01 = 10000 (6 decimals)
        let amount = parse_price_to_amount("$0.01").unwrap();
        assert_eq!(amount, "10000");

        // $1.00 = 1000000
        let amount = parse_price_to_amount("$1.00").unwrap();
        assert_eq!(amount, "1000000");

        // $0.001 = 1000
        let amount = parse_price_to_amount("$0.001").unwrap();
        assert_eq!(amount, "1000");
    }
}
