use alloy::primitives::Address;
use std::collections::HashMap;
use x402::{PaymentRequirements, SchemeServer, SCHEME_NAME, TEMPO_NETWORK};

/// Payment configuration for a single route.
#[derive(Debug, Clone)]
pub struct RoutePaymentConfig {
    pub requirements: PaymentRequirements,
}

/// Configuration for the payment gate middleware.
#[derive(Debug, Clone)]
pub struct PaymentGateConfig {
    pub facilitator_url: String,
    pub hmac_secret: Option<Vec<u8>>,
    pub rate_limit_rpm: u64,
    pub allowed_origins: Vec<String>,
}

impl PaymentGateConfig {
    pub fn from_env(facilitator_url: &str) -> Self {
        let hmac_secret = std::env::var("FACILITATOR_SHARED_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.into_bytes());

        let insecure_no_hmac = std::env::var("X402_INSECURE_NO_HMAC")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        if hmac_secret.is_none() && !insecure_no_hmac {
            tracing::error!(
                "FACILITATOR_SHARED_SECRET is required. \
                 Set it to a secure random value (e.g. `openssl rand -hex 32`). \
                 For local development only, set X402_INSECURE_NO_HMAC=true to skip."
            );
            std::process::exit(1);
        } else if hmac_secret.is_none() {
            tracing::warn!(
                "⚠️  X402_INSECURE_NO_HMAC=true — facilitator requests will be UNAUTHENTICATED. \
                 DO NOT use this in production!"
            );
        }

        let rate_limit_rpm: u64 = std::env::var("RATE_LIMIT_RPM")
            .ok()
            .and_then(|r| r.parse().ok())
            .unwrap_or(60);

        let allowed_origins: Vec<String> = std::env::var("ALLOWED_ORIGINS")
            .ok()
            .map(|origins| {
                origins
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            facilitator_url: facilitator_url.to_string(),
            hmac_secret,
            rate_limit_rpm,
            allowed_origins,
        }
    }
}

/// Holds payment configuration for all protected routes.
pub struct PaymentConfig {
    pub routes: HashMap<String, RoutePaymentConfig>,
    pub facilitator_url: String,
    pub hmac_secret: Option<Vec<u8>>,
}

/// Builder for constructing a `PaymentConfig` with multiple priced routes.
pub struct PaymentConfigBuilder {
    scheme: Box<dyn SchemeServer>,
    pay_to: Address,
    gate_config_facilitator_url: String,
    gate_config_hmac_secret: Option<Vec<u8>>,
    routes: HashMap<String, RoutePaymentConfig>,
}

impl PaymentConfigBuilder {
    /// Create a new builder. `scheme` is used to parse prices into token amounts.
    pub fn new(
        scheme: impl SchemeServer + 'static,
        pay_to: Address,
        gate_config: &PaymentGateConfig,
    ) -> Self {
        Self {
            scheme: Box::new(scheme),
            pay_to,
            gate_config_facilitator_url: gate_config.facilitator_url.clone(),
            gate_config_hmac_secret: gate_config.hmac_secret.clone(),
            routes: HashMap::new(),
        }
    }

    /// Register a priced route (e.g. `route("GET", "/blockNumber", "$0.001", Some("..."))`).
    ///
    /// `price` is a human-readable string like `"$0.001"` — parsed via the scheme.
    pub fn route(
        mut self,
        method: &str,
        path: &str,
        price: &str,
        description: Option<&str>,
    ) -> Self {
        let (amount, asset) = self
            .scheme
            .parse_price(price)
            .unwrap_or_else(|_| panic!("failed to parse price: {price}"));

        let key = format!("{method} {path}");
        self.routes.insert(
            key,
            RoutePaymentConfig {
                requirements: PaymentRequirements {
                    scheme: SCHEME_NAME.to_string(),
                    network: TEMPO_NETWORK.to_string(),
                    price: price.to_string(),
                    asset,
                    amount,
                    pay_to: self.pay_to,
                    max_timeout_seconds: 30,
                    description: description.map(String::from),
                    mime_type: Some("application/json".to_string()),
                },
            },
        );
        self
    }

    /// Consume the builder and produce a `PaymentConfig`.
    pub fn build(self) -> PaymentConfig {
        PaymentConfig {
            routes: self.routes,
            facilitator_url: self.gate_config_facilitator_url,
            hmac_secret: self.gate_config_hmac_secret,
        }
    }
}

impl PaymentConfig {
    /// Convenience constructor that registers the default `GET /blockNumber` route at `$0.001`.
    pub fn new(
        scheme: impl SchemeServer + 'static,
        pay_to: Address,
        gate_config: &PaymentGateConfig,
    ) -> Self {
        PaymentConfigBuilder::new(scheme, pay_to, gate_config)
            .route(
                "GET",
                "/blockNumber",
                "$0.001",
                Some("Get the latest Tempo block number"),
            )
            .build()
    }

    /// Look up the payment config for a given route key (e.g. "GET /blockNumber").
    pub fn get_route(&self, method: &str, path: &str) -> Option<&RoutePaymentConfig> {
        let key = format!("{method} {path}");
        self.routes.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_gate() -> PaymentGateConfig {
        PaymentGateConfig {
            facilitator_url: "http://localhost:4022".to_string(),
            hmac_secret: None,
            rate_limit_rpm: 60,
            allowed_origins: vec![],
        }
    }

    #[test]
    fn test_payment_config_creates_block_number_route() {
        let config =
            PaymentConfig::new(x402::TempoSchemeServer::new(), Address::ZERO, &test_gate());
        let route = config.get_route("GET", "/blockNumber");
        assert!(route.is_some());
        let req = &route.unwrap().requirements;
        assert_eq!(req.scheme, "tempo-tip20");
        assert_eq!(req.network, "eip155:42431");
        assert_eq!(req.price, "$0.001");
        assert_eq!(req.amount, "1000");
    }

    #[test]
    fn test_get_route_returns_none_for_unknown() {
        let config =
            PaymentConfig::new(x402::TempoSchemeServer::new(), Address::ZERO, &test_gate());
        assert!(config.get_route("POST", "/unknown").is_none());
    }

    #[test]
    fn test_builder_multiple_routes() {
        let config =
            PaymentConfigBuilder::new(x402::TempoSchemeServer::new(), Address::ZERO, &test_gate())
                .route("GET", "/blockNumber", "$0.001", Some("block number"))
                .route("POST", "/submit", "$0.01", Some("submit tx"))
                .route("GET", "/data", "$0.05", None)
                .build();

        assert_eq!(config.routes.len(), 3);

        let r1 = config.get_route("GET", "/blockNumber").unwrap();
        assert_eq!(r1.requirements.price, "$0.001");
        assert_eq!(r1.requirements.amount, "1000");
        assert_eq!(r1.requirements.description.as_deref(), Some("block number"));

        let r2 = config.get_route("POST", "/submit").unwrap();
        assert_eq!(r2.requirements.price, "$0.01");
        assert_eq!(r2.requirements.amount, "10000");

        let r3 = config.get_route("GET", "/data").unwrap();
        assert_eq!(r3.requirements.price, "$0.05");
        assert_eq!(r3.requirements.amount, "50000");
        assert!(r3.requirements.description.is_none());
    }

    #[test]
    fn test_builder_empty_builds_no_routes() {
        let config =
            PaymentConfigBuilder::new(x402::TempoSchemeServer::new(), Address::ZERO, &test_gate())
                .build();

        assert!(config.routes.is_empty());
    }
}
