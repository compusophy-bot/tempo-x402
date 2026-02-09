//! Clone orchestration logic.
//!
//! Coordinates the full lifecycle of spawning a child instance on Railway:
//! service creation, environment configuration, Docker image deployment,
//! volume attachment, domain assignment, and deployment trigger.

use crate::railway::{RailwayClient, RailwayError};
use serde::{Deserialize, Serialize};

/// Configuration for clone operations.
#[derive(Clone, Debug)]
pub struct CloneConfig {
    /// Docker image to deploy (e.g., `ghcr.io/compusophy/tempo-x402:latest`)
    pub docker_image: String,
    /// RPC URL for the Tempo chain
    pub rpc_url: String,
    /// URL of this (parent) instance, so children can register back
    pub self_url: String,
    /// Maximum number of children this instance can spawn
    pub max_children: u32,
}

/// Result of a successful clone operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloneResult {
    /// Unique ID for the child instance
    pub instance_id: String,
    /// Public URL of the child (Railway domain)
    pub url: String,
    /// Railway service ID
    pub railway_service_id: String,
    /// Railway deployment ID
    pub deployment_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CloneError {
    #[error("Railway API error: {0}")]
    Railway(#[from] RailwayError),

    #[error("clone limit reached: {current}/{max} children")]
    LimitReached { current: u32, max: u32 },

    #[error("clone error: {0}")]
    Other(String),
}

/// Orchestrates the clone flow: create service, configure, deploy.
pub struct CloneOrchestrator {
    railway: RailwayClient,
    config: CloneConfig,
}

impl CloneOrchestrator {
    pub fn new(railway: RailwayClient, config: CloneConfig) -> Self {
        Self { railway, config }
    }

    /// Spawn a new child clone on Railway.
    ///
    /// Steps:
    /// 1. Generate child instance ID
    /// 2. Create Railway service
    /// 3. Get default environment
    /// 4. Set environment variables (AUTO_BOOTSTRAP, PARENT_URL, etc.)
    /// 5. Set Docker image source
    /// 6. Add persistent volume at /data
    /// 7. Create public domain
    /// 8. Trigger deployment
    pub async fn spawn_clone(&self, parent_address: &str) -> Result<CloneResult, CloneError> {
        let instance_id = uuid::Uuid::new_v4().to_string();
        let service_name = format!("x402-{}", &instance_id[..8]);

        tracing::info!(
            instance_id = %instance_id,
            service_name = %service_name,
            "Spawning clone"
        );

        // 1. Create service
        let service_id = self.railway.create_service(&service_name).await?;
        tracing::info!(service_id = %service_id, "Railway service created");

        // 2. Get default environment
        let env_id = self.railway.get_default_environment().await?;

        // 3. Set environment variables
        let env_vars = serde_json::json!({
            "AUTO_BOOTSTRAP": "true",
            "INSTANCE_ID": instance_id,
            "PARENT_URL": self.config.self_url,
            "PARENT_ADDRESS": parent_address,
            "IDENTITY_PATH": "/data/identity.json",
            "DB_PATH": "/data/gateway.db",
            "NONCE_DB_PATH": "/data/x402-nonces.db",
            "RPC_URL": self.config.rpc_url,
            "SPA_DIR": "/app/spa",
            "PORT": "4023",
        });
        self.railway
            .set_variables(&service_id, &env_id, env_vars)
            .await?;
        tracing::info!("Environment variables configured");

        // 4. Set Docker image
        self.railway
            .set_docker_image(&service_id, &self.config.docker_image)
            .await?;
        tracing::info!(image = %self.config.docker_image, "Docker image set");

        // 5. Add volume
        self.railway
            .add_volume(&service_id, &env_id, "/data")
            .await?;
        tracing::info!("Volume attached at /data");

        // 6. Create domain
        let url = self.railway.create_domain(&service_id, &env_id).await?;
        tracing::info!(url = %url, "Domain created");

        // 7. Deploy
        let deployment_id = self.railway.deploy_service(&service_id, &env_id).await?;
        tracing::info!(deployment_id = %deployment_id, "Deployment triggered");

        Ok(CloneResult {
            instance_id,
            url,
            railway_service_id: service_id,
            deployment_id,
        })
    }

    pub fn config(&self) -> &CloneConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_config() {
        let config = CloneConfig {
            docker_image: "ghcr.io/compusophy/tempo-x402:latest".to_string(),
            rpc_url: "https://rpc.moderato.tempo.xyz".to_string(),
            self_url: "https://my-instance.up.railway.app".to_string(),
            max_children: 10,
        };
        assert_eq!(config.max_children, 10);
    }

    #[test]
    fn test_clone_error_display() {
        let err = CloneError::LimitReached {
            current: 10,
            max: 10,
        };
        assert!(err.to_string().contains("10/10"));
    }
}
