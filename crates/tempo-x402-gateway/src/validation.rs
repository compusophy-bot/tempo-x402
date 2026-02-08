use std::net::{Ipv4Addr, Ipv6Addr};

use url::Url;

use crate::error::GatewayError;

/// Check if an IPv4 address is private, loopback, or otherwise non-routable.
pub fn is_private_ipv4(ip: &Ipv4Addr) -> bool {
    ip.is_loopback()          // 127.0.0.0/8
        || ip.is_private()    // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
        || ip.is_link_local() // 169.254.0.0/16
        || ip.is_broadcast()  // 255.255.255.255
        || ip.is_unspecified() // 0.0.0.0
        || ip.octets()[0] == 100 && (ip.octets()[1] & 0xC0) == 64 // 100.64.0.0/10 (CGNAT)
}

/// Check if an IPv6 address is private, loopback, or otherwise non-routable.
pub fn is_private_ipv6(ip: &Ipv6Addr) -> bool {
    ip.is_loopback()       // ::1
        || ip.is_unspecified() // ::
        || {
            let segments = ip.segments();
            // fc00::/7 (unique local)
            (segments[0] & 0xFE00) == 0xFC00
            // fe80::/10 (link-local)
            || (segments[0] & 0xFFC0) == 0xFE80
            // IPv4-mapped IPv6: check the mapped IPv4 address
            || match ip.to_ipv4_mapped() {
                Some(v4) => is_private_ipv4(&v4),
                None => false,
            }
        }
}

/// Validate target URL (HTTPS, no private/loopback IPs, no localhost domains).
pub fn validate_target_url(url: &str) -> Result<(), GatewayError> {
    let parsed =
        Url::parse(url).map_err(|_| GatewayError::InvalidUrl("invalid URL format".to_string()))?;

    if parsed.scheme() != "https" {
        return Err(GatewayError::InvalidUrl(
            "target must use HTTPS".to_string(),
        ));
    }

    // Prevent SSRF: validate the host is not a private/loopback address
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => {
            if is_private_ipv4(&ip) {
                return Err(GatewayError::InvalidUrl(
                    "target cannot be a private or loopback IP address".to_string(),
                ));
            }
        }
        Some(url::Host::Ipv6(ip)) => {
            if is_private_ipv6(&ip) {
                return Err(GatewayError::InvalidUrl(
                    "target cannot be a private or loopback IP address".to_string(),
                ));
            }
        }
        Some(url::Host::Domain(domain)) => {
            let domain_lower = domain.to_lowercase();
            if domain_lower == "localhost"
                || domain_lower.ends_with(".localhost")
                || domain_lower.ends_with(".local")
                || domain_lower.ends_with(".internal")
            {
                return Err(GatewayError::InvalidUrl(
                    "target cannot be localhost or local domain".to_string(),
                ));
            }
        }
        None => {
            return Err(GatewayError::InvalidUrl(
                "target URL must have a host".to_string(),
            ));
        }
    }

    Ok(())
}

/// Resolve a hostname and verify the resolved IPs are not private/loopback.
/// Prevents DNS rebinding attacks where a domain initially resolves to a public IP
/// but later resolves to a private IP.
pub async fn validate_resolved_ip(host: &str) -> Result<(), GatewayError> {
    // If the host is already an IP, parse and check directly
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        if is_private_ipv4(&ip) {
            return Err(GatewayError::ProxyError(
                "target resolves to a private IP address".to_string(),
            ));
        }
        return Ok(());
    }
    if let Ok(ip) = host.parse::<Ipv6Addr>() {
        if is_private_ipv6(&ip) {
            return Err(GatewayError::ProxyError(
                "target resolves to a private IP address".to_string(),
            ));
        }
        return Ok(());
    }

    // DNS resolution — add port 443 as required by lookup_host
    let lookup = format!("{}:443", host);
    let addrs = tokio::net::lookup_host(&lookup).await.map_err(|e| {
        GatewayError::ProxyError(format!("DNS resolution failed for {}: {}", host, e))
    })?;

    for addr in addrs {
        match addr.ip() {
            std::net::IpAddr::V4(ip) => {
                if is_private_ipv4(&ip) {
                    return Err(GatewayError::ProxyError(
                        "target resolves to a private IP address".to_string(),
                    ));
                }
            }
            std::net::IpAddr::V6(ip) => {
                if is_private_ipv6(&ip) {
                    return Err(GatewayError::ProxyError(
                        "target resolves to a private IP address".to_string(),
                    ));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_ipv4() {
        assert!(is_private_ipv4(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_ipv4(&"10.0.0.1".parse().unwrap()));
        assert!(is_private_ipv4(&"192.168.1.1".parse().unwrap()));
        assert!(is_private_ipv4(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_ipv4(&"0.0.0.0".parse().unwrap()));
        assert!(is_private_ipv4(&"100.64.0.1".parse().unwrap()));
        assert!(!is_private_ipv4(&"8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn test_private_ipv6() {
        assert!(is_private_ipv6(&"::1".parse().unwrap()));
        assert!(is_private_ipv6(&"::".parse().unwrap()));
        assert!(is_private_ipv6(&"fc00::1".parse().unwrap()));
        assert!(is_private_ipv6(&"fe80::1".parse().unwrap()));
        assert!(!is_private_ipv6(&"2001:db8::1".parse().unwrap()));
    }

    #[test]
    fn test_validate_target_url() {
        assert!(validate_target_url("https://api.example.com").is_ok());
        assert!(validate_target_url("http://api.example.com").is_err());
        assert!(validate_target_url("https://localhost").is_err());
        assert!(validate_target_url("https://127.0.0.1").is_err());
        assert!(validate_target_url("https://192.168.1.1").is_err());
    }
}
