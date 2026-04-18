//! # tempo-x402-identity
//!
//! Identity management for x402 node instances.
//!
//! Handles the full identity lifecycle: wallet key generation, filesystem persistence
//! (with restricted file permissions), faucet funding, and parent node registration.
//!
//! With the `erc8004` feature (default), adds on-chain agent identity via ERC-8004 NFTs:
//! contract deployment, identity minting, reputation feedback, peer discovery, and recovery proofs.
//!
//! ## Bootstrap
//!
//! Call [`bootstrap()`] at startup to generate or load an identity. It injects
//! `EVM_ADDRESS`, `FACILITATOR_PRIVATE_KEY`, and `FACILITATOR_SHARED_SECRET` as
//! environment variables (only if not already set).
//!
//! Part of the [`tempo-x402`](https://docs.rs/tempo-x402) workspace.

use alloy::primitives::Address;

// ── ERC-8004 on-chain identity modules (feature-gated) ──────────────────

/// Solidity ABI bindings for ERC-8004 contracts.
#[cfg(feature = "erc8004")]
pub mod contracts;
/// Self-deployment of ERC-8004 contracts from embedded bytecode.
#[cfg(feature = "erc8004")]
pub mod deploy;
/// Decentralized peer discovery via on-chain agent registry.
#[cfg(feature = "erc8004")]
pub mod discovery;
/// On-chain agent NFT operations (mint, metadata, recovery).
#[cfg(feature = "erc8004")]
pub mod onchain;
/// Recovery proof construction and verification.
#[cfg(feature = "erc8004")]
pub mod recovery;
/// Reputation feedback submission and queries.
#[cfg(feature = "erc8004")]
pub mod reputation;
/// Domain types: AgentId, ReputationScore, AgentMetadata.
#[cfg(feature = "erc8004")]
pub mod types;
/// Validator hooks (deferred — contracts are complex).
#[cfg(feature = "erc8004")]
pub mod validation;

// Re-exports
use alloy::signers::local::PrivateKeySigner;
use chrono::{DateTime, Utc};
#[cfg(feature = "erc8004")]
pub use discovery::PeerInfo;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
#[cfg(feature = "erc8004")]
pub use types::{AgentId, AgentMetadata, ReputationScore};

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
    /// ERC-8004 agent token ID (if minted on-chain).
    #[serde(default)]
    pub agent_token_id: Option<String>,
    /// Separate facilitator private key (0x-prefixed). Each node gets its own
    /// so that on-chain tx nonces don't collide across nodes.
    /// Generated on first bootstrap, persisted in identity.json.
    #[serde(skip_serializing, default)]
    pub facilitator_private_key: Option<String>,
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
    /// ERC-8004 agent token ID (if minted).
    #[serde(default)]
    agent_token_id: Option<String>,
    /// Per-node facilitator private key (generated on first bootstrap).
    #[serde(default)]
    facilitator_private_key: Option<String>,
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
            agent_token_id: id.agent_token_id.clone(),
            facilitator_private_key: id.facilitator_private_key.clone(),
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

        // Allow PARENT_URL env var to override persisted value.
        // This lets operators fix parent_url without recreating the identity file.
        let parent_url = env::var("PARENT_URL")
            .ok()
            .filter(|s| !s.is_empty() && s.starts_with("https://"))
            .or(p.parent_url);

        Ok(InstanceIdentity {
            private_key: p.private_key,
            address,
            instance_id: p.instance_id,
            parent_url,
            parent_address,
            created_at,
            agent_token_id: p.agent_token_id,
            facilitator_private_key: p.facilitator_private_key,
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
///    `EVM_PRIVATE_KEY`, `FACILITATOR_SHARED_SECRET`) so downstream config (e.g. `GatewayConfig::from_env()`)
///    picks them up automatically.
pub fn bootstrap(identity_path: &str) -> Result<InstanceIdentity, IdentityError> {
    let path = Path::new(identity_path);

    let mut identity = if path.exists() {
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

        let instance_id = env::var("INSTANCE_ID")
            .or_else(|_| env::var("RAILWAY_SERVICE_ID"))
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

        // Generate a separate facilitator key so each node has its own on-chain
        // nonce space — prevents tx nonce collisions when multiple nodes settle
        // payments concurrently.
        let fac_signer = PrivateKeySigner::random();
        let facilitator_private_key = format!("0x{}", alloy::hex::encode(fac_signer.to_bytes()));

        let identity = InstanceIdentity {
            private_key,
            address,
            instance_id,
            parent_url,
            parent_address,
            created_at: Utc::now(),
            agent_token_id: None,
            facilitator_private_key: Some(facilitator_private_key),
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

    // Migrate: generate facilitator key if existing identity lacks one
    if identity.facilitator_private_key.is_none() {
        tracing::info!("Generating separate facilitator key for existing identity");
        let fac_signer = PrivateKeySigner::random();
        identity.facilitator_private_key =
            Some(format!("0x{}", alloy::hex::encode(fac_signer.to_bytes())));

        // Re-persist with the new facilitator key
        let persisted = PersistedIdentity::from(&identity);
        let json = serde_json::to_string_pretty(&persisted)
            .map_err(|e| IdentityError::ParseError(format!("serialize failed: {e}")))?;
        std::fs::write(path, json)?;
        tracing::info!("Identity updated with separate facilitator key");
    }

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

    // Always inject the per-node facilitator key (overrides any shared env var).
    // Each node MUST have its own facilitator key to avoid on-chain tx nonce collisions.
    let fac_key = identity
        .facilitator_private_key
        .as_deref()
        .unwrap_or(&identity.private_key);
    env::set_var("FACILITATOR_PRIVATE_KEY", fac_key);
    tracing::debug!("Injected FACILITATOR_PRIVATE_KEY (per-node)");

    // The node's wallet key is also used as the client signing key for x402 payments.
    // Without this, `call_paid_endpoint` and `register_endpoint` tools fail.
    if env::var("EVM_PRIVATE_KEY").is_err() {
        env::set_var("EVM_PRIVATE_KEY", &identity.private_key);
        tracing::debug!("Injected EVM_PRIVATE_KEY");
    }

    if env::var("FACILITATOR_SHARED_SECRET").is_err() {
        // Generate a deterministic-but-unique HMAC secret from the facilitator key.
        // This is safe because the secret only needs to be shared between the
        // gateway and its embedded facilitator (same process).
        let secret = x402::hmac::compute_hmac(fac_key.as_bytes(), b"x402-bootstrap-hmac-secret");
        env::set_var("FACILITATOR_SHARED_SECRET", &secret);
        tracing::debug!("Injected FACILITATOR_SHARED_SECRET (auto-generated)");
    }
}

/// Update the persisted identity file with a new agent token ID.
///
/// Called after successful ERC-8004 minting to persist the token ID.
pub fn save_agent_token_id(
    identity_path: &str,
    identity: &mut InstanceIdentity,
    token_id: &str,
) -> Result<(), IdentityError> {
    identity.agent_token_id = Some(token_id.to_string());
    let persisted = PersistedIdentity::from(&*identity);
    let json = serde_json::to_string_pretty(&persisted)
        .map_err(|e| IdentityError::ParseError(format!("serialize failed: {e}")))?;
    std::fs::write(identity_path, json)?;
    Ok(())
}

/// Request faucet funding via the Tempo `tempo_fundAddress` JSON-RPC method.
///
/// Best-effort with retries. Logs warnings on failure but does not propagate
/// errors since funding is not critical for bootstrap.
pub async fn request_faucet_funds(rpc_url: &str, address: Address) -> Result<(), IdentityError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
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
        .redirect(reqwest::redirect::Policy::limited(5))
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

// ── ERC-8004 registry configuration ──────────────────────────────────────

/// Get the identity registry contract address.
///
/// Reads from `ERC8004_IDENTITY_REGISTRY` env var, defaults to `Address::ZERO`.
#[cfg(feature = "erc8004")]
pub fn identity_registry() -> Address {
    std::env::var("ERC8004_IDENTITY_REGISTRY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO)
}

/// Get the reputation registry contract address.
#[cfg(feature = "erc8004")]
pub fn reputation_registry() -> Address {
    std::env::var("ERC8004_REPUTATION_REGISTRY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO)
}

/// Get the validation registry contract address.
#[cfg(feature = "erc8004")]
pub fn validation_registry() -> Address {
    std::env::var("ERC8004_VALIDATION_REGISTRY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO)
}

/// Check whether ERC-8004 identity minting is enabled.
/// Defaults to true — blockchain peer discovery is the primary mechanism.
#[cfg(feature = "erc8004")]
pub fn auto_mint_enabled() -> bool {
    std::env::var("ERC8004_AUTO_MINT")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true)
}

/// Check whether reputation submission is enabled.
#[cfg(feature = "erc8004")]
pub fn reputation_enabled() -> bool {
    std::env::var("ERC8004_REPUTATION_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// Get the configured recovery address (if any).
#[cfg(feature = "erc8004")]
pub fn recovery_address() -> Option<Address> {
    std::env::var("ERC8004_RECOVERY_ADDRESS")
        .ok()
        .and_then(|s| s.parse().ok())
}

/// Load previously deployed registry addresses from a JSON file and inject as env vars.
#[cfg(feature = "erc8004")]
pub fn load_persisted_registries(path: &str) -> bool {
    let Ok(data) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
        return false;
    };

    let mut loaded = false;
    for (key, env_var) in [
        ("identity", "ERC8004_IDENTITY_REGISTRY"),
        ("reputation", "ERC8004_REPUTATION_REGISTRY"),
        ("validation", "ERC8004_VALIDATION_REGISTRY"),
    ] {
        if let Some(addr) = json.get(key).and_then(|v| v.as_str()) {
            if std::env::var(env_var).is_err() || std::env::var(env_var).ok().as_deref() == Some("")
            {
                std::env::set_var(env_var, addr);
                loaded = true;
            }
        }
    }
    loaded
}

/// Persist deployed registry addresses to a JSON file.
#[cfg(feature = "erc8004")]
pub fn save_deployed_registries(
    path: &str,
    registries: &deploy::DeployedRegistries,
) -> Result<(), std::io::Error> {
    let json = serde_json::json!({
        "identity": format!("{:#x}", registries.identity),
        "reputation": format!("{:#x}", registries.reputation),
        "validation": format!("{:#x}", registries.validation),
    });
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&json).unwrap())?;
    Ok(())
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
            agent_token_id: None,
            facilitator_private_key: None,
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
