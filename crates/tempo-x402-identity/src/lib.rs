//! Identity management for x402 instances.
//!
//! Provides wallet identity generation, filesystem persistence, faucet funding,
//! and parent registration for the self-replicating container model.
//!
//! Builds on `x402_wallet` (WASM-compatible crypto primitives) but adds server-side
//! concerns: filesystem I/O, network requests, environment variable injection.

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;

/// Core identity for a running instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceIdentity {
    /// Hex-encoded private key (0x-prefixed). NEVER log this field.
    #[serde(skip_serializing)]
    pub private_key: String,
    /// EVM address derived from the private key.
    pub address: Address,
    /// Unique instance identifier (Railway service ID or UUID).
    pub instance_id: String,
    /// URL of the parent instance that spawned this one (if any).
    pub parent_url: Option<String>,
    /// Address of the parent that paid for this instance (owner).
    pub parent_address: Option<Address>,
    /// When this identity was created.
    pub created_at: DateTime<Utc>,
}

/// On-disk format that includes the private key for persistence.
#[derive(Serialize, Deserialize)]
struct PersistedIdentity {
    private_key: String,
    address: String,
    instance_id: String,
    parent_url: Option<String>,
    parent_address: Option<String>,
    created_at: String,
}

impl From<&InstanceIdentity> for PersistedIdentity {
    fn from(id: &InstanceIdentity) -> Self {
        Self {
            private_key: id.private_key.clone(),
            address: format!("{:#x}", id.address),
            instance_id: id.instance_id.clone(),
            parent_url: id.parent_url.clone(),
            parent_address: id.parent_address.map(|a| format!("{:#x}", a)),
            created_at: id.created_at.to_rfc3339(),
        }
    }
}

impl TryFrom<PersistedIdentity> for InstanceIdentity {
    type Error = IdentityError;

    fn try_from(p: PersistedIdentity) -> Result<Self, Self::Error> {
        let address: Address = p
            .address
            .parse()
            .map_err(|e| IdentityError::ParseError(format!("invalid address: {e}")))?;
        let parent_address = p
            .parent_address
            .map(|a| {
                a.parse::<Address>()
                    .map_err(|e| IdentityError::ParseError(format!("invalid parent address: {e}")))
            })
            .transpose()?;
        let created_at = DateTime::parse_from_rfc3339(&p.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| IdentityError::ParseError(format!("invalid created_at: {e}")))?;

        Ok(InstanceIdentity {
            private_key: p.private_key,
            address,
            instance_id: p.instance_id,
            parent_url: p.parent_url,
            parent_address,
            created_at,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("faucet error: {0}")]
    FaucetError(String),

    #[error("registration error: {0}")]
    RegistrationError(String),
}

/// Bootstrap an instance identity.
///
/// 1. If `identity_path` exists, load and return the persisted identity.
/// 2. Otherwise, generate a new random keypair, persist it, and return it.
/// 3. Inject environment variables (`EVM_ADDRESS`, `FACILITATOR_PRIVATE_KEY`,
///    `FACILITATOR_SHARED_SECRET`) so downstream config (e.g. `GatewayConfig::from_env()`)
///    picks them up automatically.
pub fn bootstrap(identity_path: &str) -> Result<InstanceIdentity, IdentityError> {
    let path = Path::new(identity_path);

    let identity = if path.exists() {
        tracing::info!("Loading existing identity from {}", identity_path);
        let data = std::fs::read_to_string(path)?;
        let persisted: PersistedIdentity = serde_json::from_str(&data)
            .map_err(|e| IdentityError::ParseError(format!("invalid identity JSON: {e}")))?;
        InstanceIdentity::try_from(persisted)?
    } else {
        tracing::info!("Generating new identity at {}", identity_path);
        let signer = PrivateKeySigner::random();
        let private_key = format!("0x{}", alloy::hex::encode(signer.to_bytes()));
        let address = signer.address();

        let instance_id = env::var("RAILWAY_SERVICE_ID")
            .or_else(|_| env::var("INSTANCE_ID"))
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        let parent_url = env::var("PARENT_URL").ok().filter(|s| !s.is_empty());

        // Validate PARENT_URL uses HTTPS (prevent SSRF to internal services)
        if let Some(ref url) = parent_url {
            if !url.starts_with("https://") {
                tracing::warn!(
                    "PARENT_URL must use HTTPS — ignoring insecure value: {}",
                    &url[..url.len().min(20)]
                );
                // Continue without parent_url rather than failing bootstrap
            }
        }
        let parent_url = parent_url.filter(|u| u.starts_with("https://"));
        let parent_address = env::var("PARENT_ADDRESS")
            .ok()
            .and_then(|s| s.parse::<Address>().ok());

        let identity = InstanceIdentity {
            private_key,
            address,
            instance_id,
            parent_url,
            parent_address,
            created_at: Utc::now(),
        };

        // Ensure parent directory exists
        if let Some(parent_dir) = path.parent() {
            std::fs::create_dir_all(parent_dir)?;
        }

        // Write identity file
        let persisted = PersistedIdentity::from(&identity);
        let json = serde_json::to_string_pretty(&persisted)
            .map_err(|e| IdentityError::ParseError(format!("serialize failed: {e}")))?;
        std::fs::write(path, json)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }

        tracing::info!("New identity created: {:#x}", address);
        identity
    };

    // Inject env vars for downstream config consumers
    inject_env_vars(&identity);

    Ok(identity)
}

/// Inject identity-derived environment variables into the current process.
///
/// Only sets variables that are not already set, so explicit env config
/// always takes precedence over auto-bootstrapped values.
fn inject_env_vars(identity: &InstanceIdentity) {
    let address_str = format!("{:#x}", identity.address);

    if env::var("EVM_ADDRESS").is_err() {
        env::set_var("EVM_ADDRESS", &address_str);
        tracing::debug!("Injected EVM_ADDRESS={}", address_str);
    }

    if env::var("FACILITATOR_PRIVATE_KEY").is_err() {
        env::set_var("FACILITATOR_PRIVATE_KEY", &identity.private_key);
        tracing::debug!("Injected FACILITATOR_PRIVATE_KEY");
    }

    if env::var("FACILITATOR_SHARED_SECRET").is_err() {
        // Generate a deterministic-but-unique HMAC secret from the private key.
        // This is safe because the secret only needs to be shared between the
        // gateway and its embedded facilitator (same process).
        let secret = x402::hmac::compute_hmac(
            identity.private_key.as_bytes(),
            b"x402-bootstrap-hmac-secret",
        );
        env::set_var("FACILITATOR_SHARED_SECRET", &secret);
        tracing::debug!("Injected FACILITATOR_SHARED_SECRET (auto-generated)");
    }
}

/// Request faucet funding via the Tempo `tempo_fundAddress` JSON-RPC method.
///
/// Best-effort with retries. Logs warnings on failure but does not propagate
/// errors since funding is not critical for bootstrap.
pub async fn request_faucet_funds(rpc_url: &str, address: Address) -> Result<(), IdentityError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to create HTTP client");
    let address_str = format!("{:#x}", address);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tempo_fundAddress",
        "params": [address_str],
        "id": 1
    });

    let mut last_err = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            let delay = std::time::Duration::from_secs(2u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }

        match client
            .post(rpc_url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Faucet funding requested for {}", address_str);
                return Ok(());
            }
            Ok(resp) => {
                last_err = format!("HTTP {}", resp.status());
                tracing::warn!(
                    "Faucet request attempt {} failed: {}",
                    attempt + 1,
                    last_err
                );
            }
            Err(e) => {
                last_err = e.to_string();
                tracing::warn!(
                    "Faucet request attempt {} failed: {}",
                    attempt + 1,
                    last_err
                );
            }
        }
    }

    Err(IdentityError::FaucetError(format!(
        "faucet funding failed after 3 attempts: {}",
        last_err
    )))
}

/// Register this instance with its parent by POSTing to `{parent_url}/instance/register`.
///
/// Retries with exponential backoff. The parent uses this callback to track children.
pub async fn register_with_parent(
    parent_url: &str,
    identity: &InstanceIdentity,
    self_url: &str,
) -> Result<(), IdentityError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to create HTTP client");
    let url = format!("{}/instance/register", parent_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "instance_id": identity.instance_id,
        "address": format!("{:#x}", identity.address),
        "url": self_url,
    });

    let mut last_err = String::new();
    for attempt in 0..5 {
        if attempt > 0 {
            let delay = std::time::Duration::from_secs(2u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }

        match client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Registered with parent at {}", parent_url);
                return Ok(());
            }
            Ok(resp) => {
                last_err = format!("HTTP {}", resp.status());
                tracing::warn!(
                    "Parent registration attempt {} failed: {}",
                    attempt + 1,
                    last_err
                );
            }
            Err(e) => {
                last_err = e.to_string();
                tracing::warn!(
                    "Parent registration attempt {} failed: {}",
                    attempt + 1,
                    last_err
                );
            }
        }
    }

    Err(IdentityError::RegistrationError(format!(
        "parent registration failed after 5 attempts: {}",
        last_err
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_bootstrap_creates_new_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identity.json");
        let path_str = path.to_str().unwrap();

        let identity = bootstrap(path_str).unwrap();
        assert_ne!(identity.address, Address::ZERO);
        assert!(!identity.instance_id.is_empty());
        assert!(path.exists());
    }

    #[test]
    fn test_bootstrap_loads_existing_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identity.json");

        // Create an identity first
        let signer = PrivateKeySigner::random();
        let private_key = format!("0x{}", alloy::hex::encode(signer.to_bytes()));
        let persisted = serde_json::json!({
            "private_key": private_key,
            "address": format!("{:#x}", signer.address()),
            "instance_id": "test-instance",
            "parent_url": null,
            "parent_address": null,
            "created_at": "2025-01-01T00:00:00Z",
        });
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(serde_json::to_string_pretty(&persisted).unwrap().as_bytes())
            .unwrap();

        let path_str = path.to_str().unwrap();
        let identity = bootstrap(path_str).unwrap();
        assert_eq!(identity.address, signer.address());
        assert_eq!(identity.instance_id, "test-instance");
    }

    #[test]
    fn test_persisted_roundtrip() {
        let signer = PrivateKeySigner::random();
        let identity = InstanceIdentity {
            private_key: format!("0x{}", alloy::hex::encode(signer.to_bytes())),
            address: signer.address(),
            instance_id: "test-123".to_string(),
            parent_url: Some("https://parent.example.com".to_string()),
            parent_address: None,
            created_at: Utc::now(),
        };

        let persisted = PersistedIdentity::from(&identity);
        let json = serde_json::to_string(&persisted).unwrap();
        let loaded: PersistedIdentity = serde_json::from_str(&json).unwrap();
        let restored = InstanceIdentity::try_from(loaded).unwrap();

        assert_eq!(restored.address, identity.address);
        assert_eq!(restored.instance_id, identity.instance_id);
        assert_eq!(restored.parent_url, identity.parent_url);
    }
}
