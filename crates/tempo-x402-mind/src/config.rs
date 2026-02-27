//! Mind configuration from environment variables.

use x402_soul::config::SoulConfig;
use x402_soul::SoulError;

use crate::hemisphere::{HemisphereConfig, HemisphereRole};

/// Configuration for the mind (dual-soul architecture).
#[derive(Debug, Clone)]
pub struct MindConfig {
    /// Master switch (env: MIND_ENABLED, default: false).
    pub enabled: bool,
    /// Left hemisphere configuration.
    pub left: HemisphereConfig,
    /// Right hemisphere configuration.
    pub right: HemisphereConfig,
    /// How often the callosum syncs hemispheres in seconds (default: 300).
    pub integration_interval_secs: u64,
    /// Confidence threshold below which System 2 (right) activates (default: 0.3).
    pub escalation_threshold: f32,
    /// Whether both hemispheres share a single DB (default: true).
    pub shared_db: bool,
}

impl MindConfig {
    /// Check if mind mode is enabled via environment.
    pub fn is_enabled() -> bool {
        std::env::var("MIND_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
    }

    /// Load mind configuration from environment variables.
    /// Builds two SoulConfigs (one per hemisphere) with overrides.
    pub fn from_env() -> Result<Self, SoulError> {
        let enabled = Self::is_enabled();

        let integration_interval_secs: u64 = std::env::var("MIND_INTEGRATION_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);

        let escalation_threshold: f32 = std::env::var("MIND_ESCALATION_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.3);

        let shared_db = std::env::var("MIND_SHARED_DB")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        // Build base soul config from env
        let base_config = SoulConfig::from_env()?;

        // Left hemisphere overrides
        let left_interval: u64 = std::env::var("MIND_LEFT_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(900);

        let left_model = std::env::var("MIND_LEFT_MODEL")
            .ok()
            .filter(|s| !s.is_empty());

        let mut left_soul_config = base_config.clone();
        left_soul_config.think_interval_secs = left_interval;
        if let Some(model) = left_model {
            left_soul_config.llm_model_fast = model;
        }
        // Left hemisphere is the coder — enable coding tools
        left_soul_config.max_tool_calls = 50;

        let left = HemisphereConfig {
            soul_config: left_soul_config,
            role: HemisphereRole::Left,
            think_interval_secs: left_interval,
        };

        // Right hemisphere overrides
        let right_interval: u64 = std::env::var("MIND_RIGHT_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1800);

        let right_model = std::env::var("MIND_RIGHT_MODEL")
            .ok()
            .filter(|s| !s.is_empty());

        let mut right_soul_config = base_config;
        right_soul_config.think_interval_secs = right_interval;
        if let Some(model) = right_model {
            right_soul_config.llm_model_fast = model;
        }
        // Right hemisphere is read-only — disable coding, fewer tool calls
        right_soul_config.coding_enabled = false;
        right_soul_config.autonomous_coding = false;
        right_soul_config.max_tool_calls = 10;

        // Right hemisphere uses separate DB path if not shared
        if !shared_db {
            let base_path = right_soul_config.db_path.clone();
            let right_path = if base_path.ends_with(".db") {
                base_path.replace(".db", "-right.db")
            } else {
                format!("{base_path}-right")
            };
            right_soul_config.db_path = right_path;
        }

        let right = HemisphereConfig {
            soul_config: right_soul_config,
            role: HemisphereRole::Right,
            think_interval_secs: right_interval,
        };

        Ok(Self {
            enabled,
            left,
            right,
            integration_interval_secs,
            escalation_threshold,
            shared_db,
        })
    }
}
