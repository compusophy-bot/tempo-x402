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
            .map(|s| s.into_bytes());

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

impl PaymentConfig {
    pub fn new(
        scheme: &dyn SchemeServer,
        pay_to: Address,
        gate_config: &PaymentGateConfig,
    ) -> Self {
        let mut routes = HashMap::new();

        // Gate GET /blockNumber at $0.001
        let (amount, asset) = scheme.parse_price("$0.001").expect("failed to parse price");

        routes.insert(
            "GET /blockNumber".to_string(),
            RoutePaymentConfig {
                requirements: PaymentRequirements {
                    scheme: SCHEME_NAME.to_string(),
                    network: TEMPO_NETWORK.to_string(),
                    price: "$0.001".to_string(),
                    asset,
                    amount,
                    pay_to,
                    max_timeout_seconds: 30,
                    description: Some("Get the latest Tempo block number".to_string()),
                    mime_type: Some("application/json".to_string()),
                },
            },
        );

        Self {
            routes,
            facilitator_url: gate_config.facilitator_url.clone(),
            hmac_secret: gate_config.hmac_secret.clone(),
        }
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

    #[test]
    fn test_payment_config_creates_block_number_route() {
        let scheme = x402::TempoSchemeServer::new();
        let gate = PaymentGateConfig {
            facilitator_url: "http://localhost:4022".to_string(),
            hmac_secret: None,
            rate_limit_rpm: 60,
            allowed_origins: vec![],
        };
        let config = PaymentConfig::new(&scheme, Address::ZERO, &gate);
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
        let scheme = x402::TempoSchemeServer::new();
        let gate = PaymentGateConfig {
            facilitator_url: "http://test".to_string(),
            hmac_secret: None,
            rate_limit_rpm: 60,
            allowed_origins: vec![],
        };
        let config = PaymentConfig::new(&scheme, Address::ZERO, &gate);
        assert!(config.get_route("POST", "/unknown").is_none());
    }
}
