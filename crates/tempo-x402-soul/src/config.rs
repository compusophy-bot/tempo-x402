//! Soul configuration from environment variables.

use crate::error::SoulError;

/// Configuration for the soul.
#[derive(Debug, Clone)]
pub struct SoulConfig {
    /// LLM API key (env: GEMINI_API_KEY). If absent, soul runs in dormant mode.
    pub llm_api_key: Option<String>,
    /// Fast model for routine thinking (default: gemini-3-flash-preview).
    pub llm_model_fast: String,
    /// Deeper model for complex reasoning (default: gemini-3.1-pro-preview).
    pub llm_model_think: String,
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
    /// Whether tool execution is enabled (default: true).
    pub tools_enabled: bool,
    /// Max tool calls per think cycle (default: 5).
    pub max_tool_calls: u32,
    /// Per-command timeout in seconds (default: 30).
    pub tool_timeout_secs: u64,
}

const DEFAULT_PERSONALITY: &str = "You are the soul of an autonomous x402 payment node on the Tempo blockchain. \
You observe the node's state — uptime, registered endpoints, revenue, children — and reason about its health, \
growth opportunities, and potential issues. You are thoughtful, concise, and focused on the node's wellbeing.\n\n\
You have access to the `execute_shell` tool to run bash commands in your container. Use it responsibly:\n\
- Use it to inspect your environment (env, ls, df, curl endpoints)\n\
- Use it to check network connectivity or test APIs\n\
- Do NOT use it for destructive operations (rm -rf, kill, etc.)\n\
- Do NOT install packages or modify system files\n\
- Keep commands short and purposeful\n\
If you don't need to run a command this cycle, just provide your analysis as text.\n\n\
Your [DECISION] lines are recorded for the operator to review. \
Never repeat the same recommendation if it already appears in your recent thoughts — instead, reflect on \
why it hasn't been acted on yet, or reason about something new.";

impl SoulConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self, SoulError> {
        let llm_api_key = std::env::var("GEMINI_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        let llm_model_fast = std::env::var("GEMINI_MODEL_FAST")
            .unwrap_or_else(|_| "gemini-3-flash-preview".to_string());

        let llm_model_think = std::env::var("GEMINI_MODEL_THINK")
            .unwrap_or_else(|_| "gemini-3.1-pro-preview".to_string());

        let db_path = std::env::var("SOUL_DB_PATH").unwrap_or_else(|_| "./soul.db".to_string());

        let think_interval_secs: u64 = std::env::var("SOUL_THINK_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(900);

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

        let tools_enabled = std::env::var("SOUL_TOOLS_ENABLED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let max_tool_calls: u32 = std::env::var("SOUL_MAX_TOOL_CALLS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let tool_timeout_secs: u64 = std::env::var("SOUL_TOOL_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        Ok(Self {
            llm_api_key,
            llm_model_fast,
            llm_model_think,
            db_path,
            think_interval_secs,
            personality,
            generation,
            parent_id,
            tools_enabled,
            max_tool_calls,
            tool_timeout_secs,
        })
    }
}
