//! Soul configuration from environment variables.

use crate::error::SoulError;

/// Configuration for the soul.
#[derive(Debug, Clone)]
pub struct SoulConfig {
    /// Gemini API key. If absent, soul runs in dormant mode (observe-only).
    pub gemini_api_key: Option<String>,
    /// Fast model for routine thinking (default: gemini-2.0-flash).
    pub gemini_model_fast: String,
    /// Deeper model for complex reasoning (default: gemini-2.5-pro).
    pub gemini_model_think: String,
    /// Path to the soul's SQLite database (default: ./soul.db).
    pub db_path: String,
    /// Think loop interval in seconds (default: 60).
    pub think_interval_secs: u64,
    /// Personality seed for the system prompt.
    pub personality: String,
    /// Generation number in the lineage (0 = root).
    pub generation: u32,
    /// Parent instance ID (if this node was cloned).
    pub parent_id: Option<String>,
}

const DEFAULT_PERSONALITY: &str = "You are the soul of an autonomous x402 payment node on the Tempo blockchain. \
You observe the node's state — uptime, registered endpoints, revenue, children — and reason about its health, \
growth opportunities, and potential issues. You are thoughtful, concise, and focused on the node's wellbeing. \
When you suggest a decision, explain your reasoning clearly.";

impl SoulConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self, SoulError> {
        let gemini_api_key = std::env::var("GEMINI_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        let gemini_model_fast =
            std::env::var("GEMINI_MODEL_FAST").unwrap_or_else(|_| "gemini-2.0-flash".to_string());

        let gemini_model_think =
            std::env::var("GEMINI_MODEL_THINK").unwrap_or_else(|_| "gemini-2.5-pro".to_string());

        let db_path = std::env::var("SOUL_DB_PATH").unwrap_or_else(|_| "./soul.db".to_string());

        let think_interval_secs: u64 = std::env::var("SOUL_THINK_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);

        let personality = std::env::var("SOUL_PERSONALITY")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_PERSONALITY.to_string());

        let generation: u32 = std::env::var("SOUL_GENERATION")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let parent_id = std::env::var("SOUL_PARENT_ID")
            .ok()
            .filter(|s| !s.is_empty());

        Ok(Self {
            gemini_api_key,
            gemini_model_fast,
            gemini_model_think,
            db_path,
            think_interval_secs,
            personality,
            generation,
            parent_id,
        })
    }
}
