//! Agent validation operations — register/remove validators, execute with validation.
//!
//! Deferred: validation hooks are the most complex part of ERC-8004 and least
//! immediately useful. These bindings are provided for completeness but real
//! integration should wait until validators are deployed on Tempo Moderato.

use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::Provider;
use std::time::Duration;
use x402::X402Error;

use crate::contracts::IAgentValidation;
use crate::types::AgentId;

/// Register a validator contract for an agent.
pub async fn register_validator<P: Provider>(
    provider: &P,
    registry: Address,
    agent_id: &AgentId,
    validator: Address,
) -> Result<(), X402Error> {
    let contract = IAgentValidation::new(registry, provider);
    let pending = tokio::time::timeout(
        Duration::from_secs(30),
        contract
            .registerValidator(agent_id.as_u256(), validator)
            .send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("registerValidator send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("registerValidator send failed: {e}")))?;

    let receipt = tokio::time::timeout(Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| {
            X402Error::ChainError("registerValidator receipt timed out after 60s".to_string())
        })?
        .map_err(|e| X402Error::ChainError(format!("registerValidator receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError(
            "registerValidator reverted".to_string(),
        ));
    }

    Ok(())
}

/// Remove a validator contract from an agent.
pub async fn remove_validator<P: Provider>(
    provider: &P,
    registry: Address,
    agent_id: &AgentId,
    validator: Address,
) -> Result<(), X402Error> {
    let contract = IAgentValidation::new(registry, provider);
    let pending = tokio::time::timeout(
        Duration::from_secs(30),
        contract
            .removeValidator(agent_id.as_u256(), validator)
            .send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("removeValidator send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("removeValidator send failed: {e}")))?;

    let receipt = tokio::time::timeout(Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| {
            X402Error::ChainError("removeValidator receipt timed out after 60s".to_string())
        })?
        .map_err(|e| X402Error::ChainError(format!("removeValidator receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError(
            "removeValidator reverted".to_string(),
        ));
    }

    Ok(())
}

/// Execute a call through the validation registry.
///
/// The validation contract will run all registered validators before
/// forwarding the call to the target.
pub async fn execute_with_validation<P: Provider>(
    provider: &P,
    registry: Address,
    agent_id: &AgentId,
    target: Address,
    data: Bytes,
    value: U256,
) -> Result<Bytes, X402Error> {
    let contract = IAgentValidation::new(registry, provider);
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        contract
            .executeWithValidation(agent_id.as_u256(), target, data, value)
            .call(),
    )
    .await
    .map_err(|_| X402Error::ChainError("executeWithValidation timed out after 60s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("executeWithValidation failed: {e}")))?;

    Ok(result)
}
