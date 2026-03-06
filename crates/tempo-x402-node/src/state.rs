//! Node-specific state extending the gateway's AppState.

#[cfg(feature = "agent")]
use crate::clone::CloneOrchestrator;
use std::sync::Arc;
use x402_gateway::state::AppState as GatewayState;
use x402_identity::InstanceIdentity;
#[cfg(feature = "soul")]
use x402_soul::{NodeObserver, SoulConfig, SoulDatabase};

/// A successful x402 payment settlement event for reputation tracking.
#[cfg(feature = "erc8004")]
#[derive(Clone, Debug)]
pub struct SettlementEvent {
    /// Slug of the endpoint that was paid for.
    pub endpoint_slug: String,
    /// Transaction hash of the settlement (if available).
    pub tx_hash: Option<String>,
}

/// Node state wrapping gateway state with identity + agent capabilities.
#[derive(Clone)]
pub struct NodeState {
    /// The underlying gateway state (config, db, http_client, facilitator)
    pub gateway: GatewayState,
    /// Instance identity (when AUTO_BOOTSTRAP is set)
    pub identity: Option<InstanceIdentity>,
    /// Clone orchestrator (when Railway credentials are configured)
    #[cfg(feature = "agent")]
    pub agent: Option<Arc<CloneOrchestrator>>,
    #[cfg(not(feature = "agent"))]
    pub agent: Option<()>,
    /// When the node started
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Read connection to the database for children queries
    pub db_path: String,
    /// Clone price (e.g., "$0.10")
    pub clone_price: Option<String>,
    /// Clone price in token units
    pub clone_price_amount: Option<String>,
    /// Maximum children
    pub clone_max_children: u32,
    /// ERC-8004 agent token ID (if minted)
    pub agent_token_id: Option<String>,
    /// Channel for sending settlement events to the reputation background task.
    /// Only populated when `erc8004` feature is enabled and reputation is configured.
    #[cfg(feature = "erc8004")]
    pub reputation_tx: Option<tokio::sync::mpsc::Sender<SettlementEvent>>,
    /// Soul database for querying thoughts/state (None if soul init failed)
    #[cfg(feature = "soul")]
    pub soul_db: Option<Arc<SoulDatabase>>,
    #[cfg(not(feature = "soul"))]
    pub soul_db: Option<()>,
    /// Whether the soul is dormant (no LLM API key)
    pub soul_dormant: bool,
    /// Soul config for chat handler (None if soul init failed)
    #[cfg(feature = "soul")]
    pub soul_config: Option<SoulConfig>,
    #[cfg(not(feature = "soul"))]
    pub soul_config: Option<()>,
    /// Soul observer for chat handler (None if soul init failed)
    #[cfg(feature = "soul")]
    pub soul_observer: Option<Arc<dyn NodeObserver>>,
    #[cfg(not(feature = "soul"))]
    pub soul_observer: Option<()>,
}
