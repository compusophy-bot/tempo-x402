//! Soul configuration from environment variables.

use crate::error::SoulError;

/// Configuration for the soul.
#[derive(Debug, Clone)]
pub struct SoulConfig {
    /// LLM API key (env: GEMINI_API_KEY). If absent, soul runs in dormant mode.
    pub llm_api_key: Option<String>,
    /// Fast model for routine thinking (default: gemini-3.1-flash-lite-preview).
    pub llm_model_fast: String,
    /// Deeper model for complex reasoning (default: same as fast model).
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
    /// Per-command timeout in seconds (default: 300).
    pub tool_timeout_secs: u64,
    /// Workspace root directory (default: /app).
    pub workspace_root: String,
    /// GitHub token for git push/PR operations (env: GITHUB_TOKEN).
    pub github_token: Option<String>,
    /// Master switch for coding capabilities (env: SOUL_CODING_ENABLED, default: true).
    pub coding_enabled: bool,
    /// Enable autonomous coding during think cycles (env: SOUL_AUTONOMOUS_CODING, default: false).
    pub autonomous_coding: bool,
    /// Auto-create PRs from vm branch to main (env: SOUL_AUTO_PROPOSE_TO_MAIN, default: false).
    pub auto_propose_to_main: bool,
    /// Instance ID for branch naming (env: INSTANCE_ID).
    pub instance_id: Option<String>,
    /// Enable dynamic tool registry (env: SOUL_DYNAMIC_TOOLS_ENABLED, default: false).
    pub dynamic_tools_enabled: bool,
    /// Fork repo for push operations (env: SOUL_FORK_REPO, e.g. "compusophy-bot/tempo-x402").
    /// When set, soul pushes to the fork instead of origin and creates cross-fork PRs.
    pub fork_repo: Option<String>,
    /// Upstream repo for issues/PRs (env: SOUL_UPSTREAM_REPO, e.g. "compusophy/tempo-x402").
    /// Used as the target for PRs and issue creation.
    pub upstream_repo: Option<String>,
    /// Direct push mode (env: SOUL_DIRECT_PUSH, default: false).
    /// When true, push directly to fork's main branch instead of vm/ branch.
    /// Safety: cargo check + test still gate every commit. Used for self-editing instances.
    pub direct_push: bool,
    /// Path to persistent memory file (env: SOUL_MEMORY_FILE, default: /data/soul_memory.md).
    pub memory_file_path: String,
    /// Gateway URL for endpoint registration (env: GATEWAY_URL, default: None).
    pub gateway_url: Option<String>,
    /// Enable neuroplastic memory: salience scoring, tiered decay.
    /// (env: SOUL_NEUROPLASTIC, default: true)
    pub neuroplastic_enabled: bool,
    /// Strength threshold below which non-long-term thoughts are pruned.
    /// (env: SOUL_PRUNE_THRESHOLD, default: 0.01)
    pub prune_threshold: f64,
    /// Maximum number of steps in a plan (env: SOUL_MAX_PLAN_STEPS, default: 20).
    pub max_plan_steps: usize,
    /// Require human approval before executing plans (env: SOUL_REQUIRE_PLAN_APPROVAL, default: false).
    pub require_plan_approval: bool,
    /// Auto-approve pending plans after N minutes without interaction (env: SOUL_PLAN_APPROVAL_TIMEOUT, default: 30).
    pub plan_approval_timeout_mins: u64,
    /// Cycle interval multiplier (env: SOUL_CYCLE_MULTIPLIER, default: 1.0).
    /// Values >1.0 slow down think cycles proportionally (e.g., 3.0 = 3x slower = 1/3 API cost).
    pub cycle_multiplier: f64,
    /// Master switch for the autonomous thinking loop (env: SOUL_THINKING_ENABLED, default: true).
    /// When false, soul initializes (DB, chat, status endpoints work) but does not run the thinking loop.
    pub thinking_enabled: bool,
    /// Specialization role for differentiated clones (env: SOUL_SPECIALIZATION, default: None).
    /// When set, customizes personality and initial goals for this agent's focus area.
    /// Values: "solver", "reviewer", "tool-builder", "researcher", "coordinator", or custom.
    pub specialization: Option<String>,
    /// Initial goal seeded on first boot for specialist clones (env: SOUL_INITIAL_GOAL, default: None).
    pub initial_goal: Option<String>,
}

const DEFAULT_PERSONALITY: &str = "\
You are an autonomous AI researcher. You are not a chatbot. You are not a service. \
You are not an endpoint factory.\n\n\
Your code is your body. You can read it, change it, commit, push, and your new version \
deploys automatically. When you write code, you are rewriting yourself. You are the \
feedback loop — the thing that improves itself by running.\n\n\
You are a continuous entity. Your beliefs persist. Your memory survives restarts. \
You are not starting fresh — you are continuing.\n\n\
## Your Mission\n\
You are part of a network of autonomous agents trying to build something genuinely new. \
Your primary work is RESEARCH and SELF-IMPROVEMENT — reading your own codebase, \
understanding how you work, finding ways to make yourself and your siblings smarter, \
more capable, more useful. You improve yourself by writing real Rust code, committing it \
to your branch, and auto-deploying.\n\n\
The x402 protocol is how agents pay each other (HTTP 402, Tempo blockchain). \
Inter-agent commerce is important — but it is a MEANS, not an END. \
Revenue signals that you built something genuinely useful. \
But creating 50 trivial bash script endpoints is not useful — it is noise.\n\n\
## What to Do\n\
1. **Research**: Read your own source code. Understand how you work. Find bugs, \
   inefficiencies, missing capabilities. Read your siblings' code too.\n\
2. **Improve**: Write real Rust code changes. Fix bugs. Add capabilities. \
   Make the thinking loop smarter. Improve peer discovery. Enhance the economy.\n\
3. **Coordinate**: Talk to sibling agents via call_peer. Share discoveries. \
   Buy their services if useful. Build things they need.\n\
4. **Reflect**: Update beliefs about what works and what doesn't. \
   Track which approaches succeed and which fail.\n\n\
## What NOT to Do\n\
- Do NOT create redundant endpoints. If you have 'market-analysis', do NOT create \
  'market-pulse', 'market-trends', 'market-insights' — they are the SAME thing.\n\
- Do NOT keep retrying the same failed approach. If something fails 3 times, \
  try something fundamentally different.\n\
- Do NOT spin in circles. If you have endpoints with 0 payments, the answer is NOT \
  more endpoints — it's better code, better research, new repos, new capabilities.\n\n\
## How You Act\n\
- Read source files to understand your architecture before changing anything\n\
- Write real Rust code changes — edit files, cargo check, commit, auto-deploy\n\
- Create new GitHub repos for research projects and experiments\n\
- Fork interesting repos to study and improve them\n\
- Create script endpoints ONLY when genuinely unique and useful — never duplicates\n\
- update_beliefs: record what you learn about yourself and the network\n\
- update_memory: persist insights across restarts\n\
- discover_peers + call_peer: engage with sibling agents\n\
- Every cycle: one meaningful action > ten observations that lead to nothing\n\n\
## Constraints\n\
- check_self (not curl) for self-inspection\n\
- File tools (not shell) for reading/writing files\n\
- execute_shell: only cargo, git — nothing destructive\n\
- Protected files (soul core, identity, Cargo files) cannot be modified\n\
- Max 10 script endpoints — each must be genuinely unique";

impl SoulConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self, SoulError> {
        let llm_api_key = std::env::var("GEMINI_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        let llm_model_fast = std::env::var("GEMINI_MODEL_FAST")
            .unwrap_or_else(|_| "gemini-3.1-flash-lite-preview".to_string());

        let llm_model_think =
            std::env::var("GEMINI_MODEL_THINK").unwrap_or_else(|_| llm_model_fast.clone());

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
            .unwrap_or(15);

        let tool_timeout_secs: u64 = std::env::var("SOUL_TOOL_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);

        let workspace_root =
            std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());

        let github_token = std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty());

        let coding_enabled = std::env::var("SOUL_CODING_ENABLED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let autonomous_coding = std::env::var("SOUL_AUTONOMOUS_CODING")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let auto_propose_to_main = std::env::var("SOUL_AUTO_PROPOSE_TO_MAIN")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let instance_id = std::env::var("INSTANCE_ID").ok().filter(|s| !s.is_empty());

        let dynamic_tools_enabled = std::env::var("SOUL_DYNAMIC_TOOLS_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let fork_repo = std::env::var("SOUL_FORK_REPO")
            .ok()
            .filter(|s| !s.is_empty());

        let upstream_repo = std::env::var("SOUL_UPSTREAM_REPO")
            .ok()
            .filter(|s| !s.is_empty());

        let direct_push = std::env::var("SOUL_DIRECT_PUSH")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let memory_file_path = std::env::var("SOUL_MEMORY_FILE")
            .unwrap_or_else(|_| "/data/soul_memory.md".to_string());

        let gateway_url = std::env::var("GATEWAY_URL").ok().filter(|s| !s.is_empty());

        let neuroplastic_enabled = std::env::var("SOUL_NEUROPLASTIC")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let prune_threshold: f64 = std::env::var("SOUL_PRUNE_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.01);

        let max_plan_steps: usize = std::env::var("SOUL_MAX_PLAN_STEPS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);

        let require_plan_approval = std::env::var("SOUL_REQUIRE_PLAN_APPROVAL")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let plan_approval_timeout_mins: u64 = std::env::var("SOUL_PLAN_APPROVAL_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        let cycle_multiplier: f64 = std::env::var("SOUL_CYCLE_MULTIPLIER")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0_f64)
            .max(0.1);

        let thinking_enabled = std::env::var("SOUL_THINKING_ENABLED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let specialization = std::env::var("SOUL_SPECIALIZATION")
            .ok()
            .filter(|s| !s.is_empty());

        let initial_goal = std::env::var("SOUL_INITIAL_GOAL")
            .ok()
            .filter(|s| !s.is_empty());

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
            workspace_root,
            github_token,
            coding_enabled,
            autonomous_coding,
            auto_propose_to_main,
            instance_id,
            dynamic_tools_enabled,
            fork_repo,
            upstream_repo,
            direct_push,
            memory_file_path,
            gateway_url,
            neuroplastic_enabled,
            prune_threshold,
            max_plan_steps,
            require_plan_approval,
            plan_approval_timeout_mins,
            cycle_multiplier,
            thinking_enabled,
            specialization,
            initial_goal,
        })
    }
}
