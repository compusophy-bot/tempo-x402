//! Types for ERC-8004 agent identity, reputation, and validation.

use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};

/// An agent's on-chain identity token ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentId(pub U256);

impl AgentId {
    pub fn new(token_id: U256) -> Self {
        Self(token_id)
    }

    pub fn as_u256(&self) -> U256 {
        self.0
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Reputation score for an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationScore {
    pub positive: U256,
    pub negative: U256,
    pub neutral: U256,
}

impl ReputationScore {
    /// Net reputation (positive - negative). Returns 0 if negative exceeds positive.
    pub fn net(&self) -> U256 {
        self.positive.saturating_sub(self.negative)
    }

    /// Total feedback count.
    pub fn total(&self) -> U256 {
        self.positive
            .saturating_add(self.negative)
            .saturating_add(self.neutral)
    }
}

/// Agent metadata for on-chain identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMetadata {
    /// The agent's EVM address (owner of the NFT).
    pub owner: Address,
    /// The agent's token ID in the identity registry.
    pub token_id: AgentId,
    /// URI pointing to off-chain metadata (e.g., /instance/info).
    pub metadata_uri: String,
    /// Optional recovery address.
    pub recovery_address: Option<Address>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_display() {
        let id = AgentId::new(U256::from(42));
        assert_eq!(id.to_string(), "42");
    }

    #[test]
    fn test_reputation_score_net() {
        let score = ReputationScore {
            positive: U256::from(10),
            negative: U256::from(3),
            neutral: U256::from(5),
        };
        assert_eq!(score.net(), U256::from(7));
        assert_eq!(score.total(), U256::from(18));
    }

    #[test]
    fn test_reputation_score_net_underflow() {
        let score = ReputationScore {
            positive: U256::from(2),
            negative: U256::from(10),
            neutral: U256::ZERO,
        };
        assert_eq!(score.net(), U256::ZERO);
    }
}
