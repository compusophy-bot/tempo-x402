//! Agent reputation operations — submit feedback, query reputation.

use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use std::time::Duration;
use x402::X402Error;

use crate::contracts::IAgentReputation;
use crate::types::{AgentId, ReputationScore};

/// Submit reputation feedback for an agent.
///
/// `metadata_uri` can contain a reference to the transaction (e.g., tx hash) that
/// prompted this feedback.
pub async fn submit_feedback<P: Provider>(
    provider: &P,
    registry: Address,
    agent_id: &AgentId,
    is_positive: bool,
    metadata_uri: &str,
) -> Result<(), X402Error> {
    let contract = IAgentReputation::new(registry, provider);
    let pending = tokio::time::timeout(
        Duration::from_secs(30),
        contract
            .submitFeedback(agent_id.as_u256(), is_positive, metadata_uri.to_string())
            .send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("submitFeedback send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("submitFeedback send failed: {e}")))?;

    let receipt = tokio::time::timeout(Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| {
            X402Error::ChainError("submitFeedback receipt timed out after 60s".to_string())
        })?
        .map_err(|e| X402Error::ChainError(format!("submitFeedback receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError("submitFeedback reverted".to_string()));
    }

    Ok(())
}

/// Query the reputation of an agent.
pub async fn get_reputation<P: Provider>(
    provider: &P,
    registry: Address,
    agent_id: &AgentId,
) -> Result<ReputationScore, X402Error> {
    let contract = IAgentReputation::new(registry, provider);
    let result = contract
        .getReputation(agent_id.as_u256())
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("getReputation failed: {e}")))?;

    Ok(ReputationScore {
        positive: result.positive,
        negative: result.negative,
        neutral: result.neutral,
    })
}

/// Helper: check if an agent has net positive reputation above a threshold.
pub async fn has_minimum_reputation<P: Provider>(
    provider: &P,
    registry: Address,
    agent_id: &AgentId,
    min_net: U256,
) -> Result<bool, X402Error> {
    let score = get_reputation(provider, registry, agent_id).await?;
    Ok(score.net() >= min_net)
}
