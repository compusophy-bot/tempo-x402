use crate::{ChainConfig, SchemeServer, X402Error};
use alloy::primitives::Address;

/// Server-side scheme: parses prices and builds payment requirements.
pub struct TempoSchemeServer {
    config: ChainConfig,
}

impl TempoSchemeServer {
    pub fn new() -> Self {
        Self {
            config: ChainConfig::default(),
        }
    }

    pub fn with_chain_config(config: ChainConfig) -> Self {
        Self { config }
    }
}

impl Default for TempoSchemeServer {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemeServer for TempoSchemeServer {
    fn parse_price(&self, price: &str) -> Result<(String, Address), X402Error> {
        // Strip non-numeric characters (except '.') -- handles "$0.001", "0.01", "$1", etc.
        let cleaned: String = price
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect();

        if cleaned.is_empty() {
            return Err(X402Error::InvalidPayment(format!(
                "invalid price '{price}': no numeric content"
            )));
        }

        // Integer-only parsing: split on decimal point, compute from parts.
        // No f64 anywhere in the pipeline.
        let amount = match cleaned.split_once('.') {
            Some((integer_part, fractional_part)) => {
                let integer: u64 = if integer_part.is_empty() {
                    0
                } else {
                    integer_part.parse::<u64>().map_err(|e| {
                        X402Error::InvalidPayment(format!(
                            "invalid price '{price}': integer part: {e}"
                        ))
                    })?
                };

                // Pad or truncate fractional part to TOKEN_DECIMALS digits
                let decimals = self.config.token_decimals as usize;
                let frac_str = if fractional_part.len() >= decimals {
                    &fractional_part[..decimals]
                } else {
                    // Pad with trailing zeros -- we'll handle this inline
                    fractional_part
                };

                let fractional: u64 = if frac_str.is_empty() {
                    0
                } else {
                    frac_str.parse::<u64>().map_err(|e| {
                        X402Error::InvalidPayment(format!(
                            "invalid price '{price}': fractional part: {e}"
                        ))
                    })?
                };

                // Scale fractional part if it had fewer digits than TOKEN_DECIMALS
                let actual_digits = frac_str.len();
                let scale = if actual_digits < decimals {
                    10u64.pow((decimals - actual_digits) as u32)
                } else {
                    1
                };

                let multiplier = 10u64.pow(self.config.token_decimals);
                let integer_part = integer.checked_mul(multiplier).ok_or_else(|| {
                    X402Error::InvalidPayment(format!("invalid price '{price}': overflow"))
                })?;
                let fractional_part = fractional.checked_mul(scale).ok_or_else(|| {
                    X402Error::InvalidPayment(format!("invalid price '{price}': overflow"))
                })?;
                integer_part.checked_add(fractional_part).ok_or_else(|| {
                    X402Error::InvalidPayment(format!("invalid price '{price}': overflow"))
                })?
            }
            None => {
                // No decimal point -- treat as whole number
                let integer: u64 = cleaned.parse::<u64>().map_err(|e| {
                    X402Error::InvalidPayment(format!("invalid price '{price}': {e}"))
                })?;
                let multiplier = 10u64.pow(self.config.token_decimals);
                integer.checked_mul(multiplier).ok_or_else(|| {
                    X402Error::InvalidPayment(format!("invalid price '{price}': overflow"))
                })?
            }
        };

        Ok((amount.to_string(), self.config.default_token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_TOKEN;

    #[test]
    fn test_parse_dollar_price() {
        let server = TempoSchemeServer::new();
        let (amount, asset) = server.parse_price("$0.001").unwrap();
        assert_eq!(amount, "1000");
        assert_eq!(asset, DEFAULT_TOKEN);
    }

    #[test]
    fn test_parse_numeric_price() {
        let server = TempoSchemeServer::new();
        let (amount, _) = server.parse_price("0.01").unwrap();
        assert_eq!(amount, "10000");
    }

    #[test]
    fn test_parse_whole_dollar() {
        let server = TempoSchemeServer::new();
        let (amount, _) = server.parse_price("$1").unwrap();
        assert_eq!(amount, "1000000");
    }

    #[test]
    fn test_parse_large_amount() {
        let server = TempoSchemeServer::new();
        let (amount, _) = server.parse_price("$100.50").unwrap();
        assert_eq!(amount, "100500000");
    }

    #[test]
    fn test_parse_six_decimals() {
        let server = TempoSchemeServer::new();
        let (amount, _) = server.parse_price("0.000001").unwrap();
        assert_eq!(amount, "1");
    }

    #[test]
    fn test_parse_truncates_beyond_decimals() {
        let server = TempoSchemeServer::new();
        // 7 decimal digits -- should truncate to 6
        let (amount, _) = server.parse_price("0.0000019").unwrap();
        assert_eq!(amount, "1");
    }

    #[test]
    fn test_parse_empty_fails() {
        let server = TempoSchemeServer::new();
        assert!(server.parse_price("$").is_err());
    }

    #[test]
    fn test_parse_overflow_fails() {
        let server = TempoSchemeServer::new();
        // This would overflow u64 when multiplied by 10^6
        assert!(server.parse_price("$99999999999999999999").is_err());
    }
}
