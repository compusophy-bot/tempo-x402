//! NodeObserver implementation for the x402 node.
//!
//! Reads analytics from the gateway database and identity info
//! to build a NodeSnapshot for the soul's thinking loop.

use std::sync::Arc;
use x402_gateway::state::AppState as GatewayState;
use x402_identity::InstanceIdentity;
use x402_soul::error::SoulError;
use x402_soul::observer::{NodeObserver, NodeSnapshot};

/// Observer that reads node state from the gateway database and identity.
pub struct NodeObserverImpl {
    gateway: GatewayState,
    identity: Option<InstanceIdentity>,
    generation: u32,
    started_at: chrono::DateTime<chrono::Utc>,
    db_path: String,
}

impl NodeObserverImpl {
    pub fn new(
        gateway: GatewayState,
        identity: Option<InstanceIdentity>,
        generation: u32,
        started_at: chrono::DateTime<chrono::Utc>,
        db_path: String,
    ) -> Arc<Self> {
        Arc::new(Self {
            gateway,
            identity,
            generation,
            started_at,
            db_path,
        })
    }
}

impl NodeObserver for NodeObserverImpl {
    fn observe(&self) -> Result<NodeSnapshot, SoulError> {
        let uptime_secs = (chrono::Utc::now() - self.started_at).num_seconds().max(0) as u64;

        // Read endpoint count + revenue from gateway DB
        let (endpoint_count, total_revenue, total_payments) = {
            let endpoints = self
                .gateway
                .db
                .list_endpoints(500, 0)
                .map_err(|e| SoulError::Observer(format!("failed to list endpoints: {e}")))?;
            let endpoint_count = endpoints.len() as u32;

            let stats = self
                .gateway
                .db
                .list_endpoint_stats(500, 0)
                .map_err(|e| SoulError::Observer(format!("failed to list stats: {e}")))?;

            let mut total_revenue: u128 = 0;
            let mut total_payments: u64 = 0;
            for s in &stats {
                total_revenue += s.revenue_total.parse::<u128>().unwrap_or(0);
                total_payments += s.payment_count as u64;
            }

            (endpoint_count, total_revenue.to_string(), total_payments)
        };

        // Read children count from node DB
        let children_count = self
            .gateway
            .db
            .with_connection(|conn| {
                crate::db::query_children_active(conn)
                    .map(|c| c.len() as u32)
                    .map_err(|e| x402_gateway::error::GatewayError::Internal(e.to_string()))
            })
            .unwrap_or(0);

        Ok(NodeSnapshot {
            uptime_secs,
            endpoint_count,
            total_revenue,
            total_payments,
            children_count,
            wallet_address: self
                .identity
                .as_ref()
                .map(|id| format!("{:#x}", id.address))
                .or_else(|| Some(format!("{:#x}", self.gateway.config.platform_address))),
            instance_id: self.identity.as_ref().map(|id| id.instance_id.clone()),
            generation: self.generation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x402_gateway::config::GatewayConfig;
    use x402_gateway::db::Database;
    use alloy::primitives::address;

    #[test]
    fn test_observe_fallback_wallet() {
        let db_path = "test_observer.db";
        let _ = std::fs::remove_file(db_path);
        let db = Database::new(db_path).unwrap();
        
        let platform_addr = address!("cda66883e29901f9ed811b393f068ac7df369e25");
        let config = GatewayConfig {
            platform_address: platform_addr,
            facilitator_url: "http://localhost".into(),
            hmac_secret: Some(vec![0; 32]),
            db_path: db_path.into(),
            port: 4023,
            platform_fee: "$0.01".into(),
            platform_fee_amount: "10000".into(),
            allowed_origins: vec![],
            rate_limit_rpm: 60,
            facilitator_private_key: None,
            nonce_db_path: "test_nonces.db".into(),
            webhook_urls: vec![],
            rpc_url: "http://localhost".into(),
            spa_dir: None,
            metrics_token: None,
        };

        let gateway = GatewayState::new(config, db, None);
        let observer = NodeObserverImpl::new(
            gateway,
            None, // no identity
            0,
            chrono::Utc::now(),
            db_path.into(),
        );

        let snapshot = observer.observe().unwrap();
        assert_eq!(snapshot.wallet_address, Some("0xcda66883e29901f9ed811b393f068ac7df369e25".to_string()));
        assert_eq!(snapshot.instance_id, None);
        
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file("test_nonces.db");
    }
}
