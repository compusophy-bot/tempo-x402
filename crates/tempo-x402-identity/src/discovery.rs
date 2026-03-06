//! Decentralized peer discovery via on-chain agent registry.
//!
//! Enumerates all minted agent NFTs, fetches their metadata URIs,
//! and resolves them to live peer info by hitting each agent's
//! `/instance/info` endpoint.

use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use serde::{Deserialize, Serialize};
use x402::X402Error;

use crate::onchain;
use crate::types::AgentId;

/// Discovered peer from the on-chain registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// On-chain agent token ID.
    pub agent_id: String,
    /// Owner address of the agent NFT.
    pub owner: String,
    /// The metadata URI stored on-chain (typically `https://{url}/instance/info`).
    pub metadata_uri: String,
    /// Resolved base URL of the peer (derived from metadata_uri).
    pub url: Option<String>,
    /// EVM address of the peer (from /instance/info).
    pub address: Option<String>,
    /// Instance ID of the peer (from /instance/info).
    pub instance_id: Option<String>,
    /// Whether the peer responded to a health check.
    pub reachable: bool,
}

/// Enumerate all agents on the identity registry.
///
/// Returns `(AgentId, Address)` pairs — token ID and owner — for every
/// minted agent. Read-only, no wallet needed.
pub async fn enumerate_agents<P: Provider>(
    provider: &P,
    registry: Address,
) -> Result<Vec<(AgentId, Address)>, X402Error> {
    let supply = onchain::total_supply(provider, registry).await?;
    let count: u64 = supply.try_into().unwrap_or(0);

    let mut agents = Vec::with_capacity(count as usize);
    for i in 0..count {
        let idx = U256::from(i);
        let agent_id = onchain::token_by_index(provider, registry, idx).await?;
        let owner = onchain::owner_of(provider, registry, &agent_id).await?;
        agents.push((agent_id, owner));
    }

    Ok(agents)
}

/// Discover live peers from the on-chain registry.
///
/// 1. Enumerates all agents via `totalSupply` + `tokenByIndex`
/// 2. Fetches each agent's `metadataURI` (stored on-chain)
/// 3. Resolves the URI to get live peer info via HTTP
/// 4. Skips self (by `self_address`) and unreachable peers
///
/// `max_peers` caps how many peers to return (0 = unlimited).
pub async fn discover_peers<P: Provider>(
    provider: &P,
    registry: Address,
    self_address: Option<Address>,
    max_peers: usize,
) -> Result<Vec<PeerInfo>, X402Error> {
    let agents = enumerate_agents(provider, registry).await?;

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| X402Error::ChainError(format!("failed to create HTTP client: {e}")))?;

    let mut peers = Vec::new();

    for (agent_id, owner) in &agents {
        // Skip self
        if self_address.is_some_and(|addr| addr == *owner) {
            continue;
        }

        // Fetch metadata URI from chain
        let metadata_uri = match onchain::get_metadata_uri(provider, registry, agent_id).await {
            Ok(uri) => uri,
            Err(e) => {
                tracing::debug!(agent_id = %agent_id, error = %e, "Failed to fetch metadata URI");
                continue;
            }
        };

        let mut peer = PeerInfo {
            agent_id: agent_id.to_string(),
            owner: format!("{:#x}", owner),
            metadata_uri: metadata_uri.clone(),
            url: None,
            address: None,
            instance_id: None,
            reachable: false,
        };

        // Derive base URL from metadata URI
        // Metadata URI is typically `https://host/instance/info`
        let base_url = if let Some(base) = metadata_uri.strip_suffix("/instance/info") {
            base.to_string()
        } else {
            // Try the URI directly as the info endpoint
            metadata_uri.clone()
        };

        // Resolve: fetch /instance/info to get live peer details
        let info_url = format!("{}/instance/info", base_url.trim_end_matches('/'));
        match http.get(&info_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    peer.url = Some(base_url);
                    peer.reachable = true;
                    peer.address = json
                        .get("identity")
                        .and_then(|id| id.get("address"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    peer.instance_id = json
                        .get("identity")
                        .and_then(|id| id.get("instance_id"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }
            }
            Ok(resp) => {
                tracing::debug!(
                    agent_id = %agent_id,
                    status = %resp.status(),
                    "Peer returned non-success status"
                );
            }
            Err(e) => {
                tracing::debug!(agent_id = %agent_id, error = %e, "Peer unreachable");
            }
        }

        peers.push(peer);

        if max_peers > 0 && peers.len() >= max_peers {
            break;
        }
    }

    Ok(peers)
}

/// Convenience: discover only reachable peers.
pub async fn discover_live_peers<P: Provider>(
    provider: &P,
    registry: Address,
    self_address: Option<Address>,
    max_peers: usize,
) -> Result<Vec<PeerInfo>, X402Error> {
    let all = discover_peers(provider, registry, self_address, max_peers).await?;
    Ok(all.into_iter().filter(|p| p.reachable).collect())
}
