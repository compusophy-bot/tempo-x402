//! Agent identity operations — mint, query, metadata, recovery address.
//!
//! Follows the `tip20.rs` pattern: timeout-wrapped contract calls with revert checks.

use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use std::time::Duration;
use x402::X402Error;

use crate::contracts::IAgentIdentity;
use crate::types::AgentId;

/// Mint a new agent identity NFT.
///
/// Returns the token ID of the newly minted agent.
pub async fn mint<P: Provider>(
    provider: &P,
    registry: Address,
    owner: Address,
    metadata_uri: &str,
) -> Result<AgentId, X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let pending = tokio::time::timeout(
        Duration::from_secs(30),
        contract.mint(owner, metadata_uri.to_string()).send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("mint send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("mint send failed: {e}")))?;

    let receipt = tokio::time::timeout(Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| X402Error::ChainError("mint receipt timed out after 60s".to_string()))?
        .map_err(|e| X402Error::ChainError(format!("mint receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError("mint reverted".to_string()));
    }

    // Extract token ID from Transfer event logs (ERC-721 Transfer(from, to, tokenId))
    // Transfer event topic: keccak256("Transfer(address,address,uint256)")
    for log in receipt.inner.logs() {
        if log.topics().len() == 4 {
            // ERC-721 Transfer has tokenId as the 4th topic
            let token_id = U256::from_be_bytes(log.topics()[3].0);
            return Ok(AgentId::new(token_id));
        }
    }

    // Fallback: if no Transfer event found, return token ID 0 with a warning
    tracing::warn!("mint succeeded but no Transfer event found in receipt logs");
    Err(X402Error::ChainError(
        "mint succeeded but could not extract token ID from logs".to_string(),
    ))
}

/// Query the owner of an agent token.
pub async fn owner_of<P: Provider>(
    provider: &P,
    registry: Address,
    token_id: &AgentId,
) -> Result<Address, X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let owner = contract
        .ownerOf(token_id.as_u256())
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("ownerOf failed: {e}")))?;
    Ok(owner)
}

/// Set the recovery address for an agent token.
pub async fn set_recovery_address<P: Provider>(
    provider: &P,
    registry: Address,
    token_id: &AgentId,
    recovery: Address,
) -> Result<(), X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let pending = tokio::time::timeout(
        Duration::from_secs(30),
        contract
            .setRecoveryAddress(token_id.as_u256(), recovery)
            .send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("setRecoveryAddress send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("setRecoveryAddress send failed: {e}")))?;

    let receipt = tokio::time::timeout(Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| {
            X402Error::ChainError("setRecoveryAddress receipt timed out after 60s".to_string())
        })?
        .map_err(|e| X402Error::ChainError(format!("setRecoveryAddress receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError(
            "setRecoveryAddress reverted".to_string(),
        ));
    }

    Ok(())
}

/// Recover an agent to a new owner using the recovery address.
pub async fn recover_agent<P: Provider>(
    provider: &P,
    registry: Address,
    token_id: &AgentId,
    new_owner: Address,
) -> Result<(), X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let pending = tokio::time::timeout(
        Duration::from_secs(30),
        contract.recoverAgent(token_id.as_u256(), new_owner).send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("recoverAgent send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("recoverAgent send failed: {e}")))?;

    let receipt = tokio::time::timeout(Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| X402Error::ChainError("recoverAgent receipt timed out after 60s".to_string()))?
        .map_err(|e| X402Error::ChainError(format!("recoverAgent receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError("recoverAgent reverted".to_string()));
    }

    Ok(())
}

/// Update the metadata URI for an agent token.
pub async fn update_metadata<P: Provider>(
    provider: &P,
    registry: Address,
    token_id: &AgentId,
    uri: &str,
) -> Result<(), X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let pending = tokio::time::timeout(
        Duration::from_secs(30),
        contract
            .updateMetadata(token_id.as_u256(), uri.to_string())
            .send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("updateMetadata send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("updateMetadata send failed: {e}")))?;

    let receipt = tokio::time::timeout(Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| {
            X402Error::ChainError("updateMetadata receipt timed out after 60s".to_string())
        })?
        .map_err(|e| X402Error::ChainError(format!("updateMetadata receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError("updateMetadata reverted".to_string()));
    }

    Ok(())
}

/// Get the metadata URI for an agent token.
pub async fn get_metadata_uri<P: Provider>(
    provider: &P,
    registry: Address,
    token_id: &AgentId,
) -> Result<String, X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let uri = contract
        .getMetadataURI(token_id.as_u256())
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("getMetadataURI failed: {e}")))?;
    Ok(uri)
}

// ── ERC-721 Enumerable ──────────────────────────────────────────────

/// Get the total number of minted agent tokens.
pub async fn total_supply<P: Provider>(provider: &P, registry: Address) -> Result<U256, X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let supply = contract
        .totalSupply()
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("totalSupply failed: {e}")))?;
    Ok(supply)
}

/// Get the token ID at a given global index.
pub async fn token_by_index<P: Provider>(
    provider: &P,
    registry: Address,
    index: U256,
) -> Result<AgentId, X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let token_id = contract
        .tokenByIndex(index)
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("tokenByIndex failed: {e}")))?;
    Ok(AgentId::new(token_id))
}

/// Get the token ID owned by an address at a given index.
pub async fn token_of_owner_by_index<P: Provider>(
    provider: &P,
    registry: Address,
    owner: Address,
    index: U256,
) -> Result<AgentId, X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let token_id = contract
        .tokenOfOwnerByIndex(owner, index)
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("tokenOfOwnerByIndex failed: {e}")))?;
    Ok(AgentId::new(token_id))
}

/// Get the number of tokens owned by an address.
pub async fn balance_of<P: Provider>(
    provider: &P,
    registry: Address,
    owner: Address,
) -> Result<U256, X402Error> {
    let contract = IAgentIdentity::new(registry, provider);
    let balance = contract
        .balanceOf(owner)
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("balanceOf failed: {e}")))?;
    Ok(balance)
}
