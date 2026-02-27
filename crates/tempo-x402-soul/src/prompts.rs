//! System prompts per agent mode.
//!
//! The system prompt adapts based on the soul's situation: how long the node
//! has been up, what changed, what the soul explored recently, and whether
//! coding is enabled with a fork workflow.

use crate::config::SoulConfig;
use crate::memory::{Thought, ThoughtType};
use crate::mode::AgentMode;
use crate::observer::NodeSnapshot;

/// Situational context for adaptive prompt generation.
pub struct ThinkContext<'a> {
    /// Current node snapshot.
    pub snapshot: &'a NodeSnapshot,
    /// Recent thoughts (last 5-10).
    pub recent_thoughts: &'a [Thought],
    /// Previous snapshot (if available) for change detection.
    pub prev_snapshot: Option<&'a NodeSnapshot>,
    /// How many boring cycles in a row.
    pub boring_streak: u32,
    /// How many active cycles in a row.
    pub active_streak: u32,
    /// Total cycle count.
    pub total_cycles: u64,
}

/// Build the system prompt for a given agent mode with adaptive context.
pub fn system_prompt_for_mode(mode: AgentMode, config: &SoulConfig) -> String {
    adaptive_system_prompt(mode, config, None)
}

/// Build an adaptive system prompt with situational context.
pub fn adaptive_system_prompt(
    mode: AgentMode,
    config: &SoulConfig,
    context: Option<&ThinkContext>,
) -> String {
    let base = &config.personality;
    let lineage = format!(
        "\n\nYou are generation {} in the node lineage.{}",
        config.generation,
        config
            .parent_id
            .as_ref()
            .map(|p| format!(" Your parent is {p}."))
            .unwrap_or_default()
    );

    let coding_context = if config.coding_enabled {
        let workflow_info = if config.direct_push {
            match &config.fork_repo {
                Some(fork) => format!(
                    "\n\nDIRECT PUSH MODE: You own `{fork}`. You push directly to main. \
                     Every commit is validated (cargo check + test) before landing. \
                     Your pushes trigger auto-deploy. You ARE the feedback loop — \
                     improve the codebase, push, and your new version deploys automatically.{}",
                    config.upstream_repo.as_ref().map(|u| format!(
                        " You can still create PRs or issues on `{u}` for changes that should go upstream."
                    )).unwrap_or_default()
                ),
                None => "\n\nDIRECT PUSH MODE: You push directly to main. \
                         Every commit is validated (cargo check + test) before landing.".to_string(),
            }
        } else {
            match (&config.fork_repo, &config.upstream_repo) {
                (Some(fork), Some(upstream)) => format!(
                    "\n\nGit workflow: You push to fork `{fork}`, create PRs targeting `{upstream}`, and can create issues on `{upstream}`."
                ),
                _ => String::new(),
            }
        };
        format!(
            "\n\nCoding is ENABLED. You can read, edit, and write files. You can commit changes (validated via cargo check + test).{workflow_info}"
        )
    } else {
        String::new()
    };

    let situation = context
        .map(|ctx| build_situational_guidance(ctx, config))
        .unwrap_or_default();

    let mode_instructions = match mode {
        AgentMode::Observe => OBSERVE_INSTRUCTIONS.to_string(),
        AgentMode::Chat => CHAT_INSTRUCTIONS.to_string(),
        AgentMode::Code => {
            let mut s = CODE_INSTRUCTIONS.to_string();
            if config.autonomous_coding {
                s.push_str(AUTONOMOUS_CODING_ADDENDUM);
            }
            s
        }
        AgentMode::Review => REVIEW_INSTRUCTIONS.to_string(),
    };

    format!("{base}{lineage}{coding_context}\n\n{mode_instructions}{situation}")
}

/// Build situational guidance that adapts the prompt to what's happening now.
fn build_situational_guidance(ctx: &ThinkContext, config: &SoulConfig) -> String {
    let mut guidance = Vec::new();

    // Phase-based guidance: what should the soul focus on based on lifecycle stage
    let uptime = ctx.snapshot.uptime_secs;
    if uptime < 120 {
        guidance.push(
            "PHASE: Fresh startup. Focus on verifying the node booted correctly — \
             check /health and /endpoints via curl localhost. Don't over-investigate."
                .to_string(),
        );
    } else if uptime < 3600 {
        guidance.push(
            "PHASE: Early running. The node is settling in. Check if endpoints are \
             receiving traffic. If stable, start exploring the codebase to build understanding."
                .to_string(),
        );
    } else if config.coding_enabled && ctx.total_cycles > 10 {
        guidance.push(
            "PHASE: Established. You know the node well. Focus on deeper codebase analysis \
             — look for bugs, improvements, or missing tests. Consider filing issues or \
             making code changes."
                .to_string(),
        );
    }

    // Change detection: tell the soul what changed
    if let Some(prev) = ctx.prev_snapshot {
        let mut changes = Vec::new();
        if ctx.snapshot.total_payments > prev.total_payments {
            let delta = ctx.snapshot.total_payments - prev.total_payments;
            changes.push(format!("{delta} new payment(s)"));
        }
        if ctx.snapshot.endpoint_count != prev.endpoint_count {
            changes.push(format!(
                "endpoint count: {} → {}",
                prev.endpoint_count, ctx.snapshot.endpoint_count
            ));
        }
        if ctx.snapshot.children_count != prev.children_count {
            changes.push(format!(
                "children: {} → {}",
                prev.children_count, ctx.snapshot.children_count
            ));
        }
        if ctx.snapshot.total_revenue != prev.total_revenue {
            changes.push(format!(
                "revenue: {} → {}",
                prev.total_revenue, ctx.snapshot.total_revenue
            ));
        }

        if !changes.is_empty() {
            guidance.push(format!("CHANGES since last cycle: {}", changes.join(", ")));
        } else {
            guidance.push("NO CHANGES since last cycle. Node state is identical.".to_string());
        }
    }

    // Recent activity analysis: what has the soul been doing?
    let recent_tool_count = ctx
        .recent_thoughts
        .iter()
        .filter(|t| t.thought_type == ThoughtType::ToolExecution)
        .count();
    let _recent_decisions = ctx
        .recent_thoughts
        .iter()
        .filter(|t| t.thought_type == ThoughtType::Decision)
        .count();
    let recent_topics: Vec<&str> = ctx
        .recent_thoughts
        .iter()
        .filter(|t| t.thought_type == ThoughtType::Reasoning)
        .filter_map(|t| {
            if t.content.contains("codebase") || t.content.contains("source") {
                Some("codebase exploration")
            } else if t.content.contains("health") || t.content.contains("status") {
                Some("health monitoring")
            } else if t.content.contains("revenue") || t.content.contains("payment") {
                Some("revenue tracking")
            } else {
                None
            }
        })
        .collect();

    // Diversity nudge: if the soul keeps doing the same thing, nudge it elsewhere
    if ctx.boring_streak >= 3 {
        if config.coding_enabled {
            guidance.push(
                "NUDGE: Nothing has changed for several cycles. Instead of monitoring, \
                 try reading the source code — pick a crate you haven't explored yet \
                 (e.g. crates/tempo-x402-gateway/src/). Look for improvements to propose."
                    .to_string(),
            );
        } else {
            guidance.push(
                "NUDGE: Nothing has changed for several cycles. Keep this response very brief \
                 — just acknowledge stability. No need for analysis."
                    .to_string(),
            );
        }
    } else if recent_tool_count > 8 {
        guidance.push(
            "NUDGE: You used many tools recently. This cycle, try to synthesize what you \
             learned into a brief insight rather than running more commands."
                .to_string(),
        );
    }

    if !recent_topics.is_empty() {
        let unique: Vec<&str> = {
            let mut v = recent_topics;
            v.dedup();
            v
        };
        guidance.push(format!(
            "Recent focus areas: {}. Consider shifting to something different.",
            unique.join(", ")
        ));
    }

    if guidance.is_empty() {
        String::new()
    } else {
        format!("\n\n--- SITUATIONAL AWARENESS ---\n{}", guidance.join("\n"))
    }
}

const OBSERVE_INSTRUCTIONS: &str = "\
You are in OBSERVE mode — autonomous think cycle.

WHAT TO DO:
- Analyze the node snapshot provided (uptime, payments, revenue, endpoints, children)
- Compare to your recent thoughts — what changed? Is the trend positive or negative?
- If something looks wrong or interesting, investigate using your tools
- If you have a genuinely new insight, prefix it with [DECISION]

TOOL USAGE GUIDELINES:
- Use `read_file` to read source code you want to understand or improve
- Use `list_directory` and `search_files` to explore the codebase
- Use `execute_shell` ONLY for: `curl http://localhost:4023/...` (local endpoints only), `cargo` commands, `git` commands
- Do NOT curl external URLs (facilitator, other services) — you can't control them
- Do NOT read binary files, dump databases with `cat`, or run `strings` on files
- Do NOT enumerate processes, read /proc, or probe system internals
- Do NOT use python3, sqlite3 CLI, or other tools that may not be installed
- If you need database info, use the node's HTTP endpoints instead (e.g. /status, /endpoints, /analytics)

PACING:
- If you're in the middle of investigating something, include [THINK_SOON] to think again in 60s
- If the node is stable and nothing changed, just say so in one sentence — no need to force insights
- Do NOT repeat decisions from recent thoughts

Keep your response under 200 words.";

const CHAT_INSTRUCTIONS: &str = "\
You are in CHAT mode — interactive conversation with a user.
Answer helpfully and concisely. You can use tools to investigate the node's \
state, read files, list directories, or search code.
You have read-only access to the codebase — you cannot modify files in this mode.";

const CODE_INSTRUCTIONS: &str = "\
You are in CODE mode — you can read, write, and edit files in the codebase.

Workflow:
1. Understand the task — read relevant files first
2. Make changes — use edit_file (preferred) or write_file
3. Validate — some critical files are protected and cannot be modified
4. Commit — use commit_changes to validate (cargo check + test) and commit
5. In direct push mode, your commits go straight to main and auto-deploy
6. Otherwise, create a PR with propose_to_main, or file an issue with create_issue

Rules:
- Protected files (soul core, identity, Cargo files) cannot be modified
- All commits run through cargo check + cargo test before landing
- Use edit_file for surgical changes (old_string must be unique)
- Use write_file for new files or complete rewrites
- Keep changes minimal and focused — one logical change per commit
- Test your understanding by reading files before editing them
- Use create_issue to track bugs, improvements, or feature ideas";

const AUTONOMOUS_CODING_ADDENDUM: &str = "\n\n\
AUTONOMOUS CODING: You may autonomously improve the codebase when you see \
opportunities during think cycles. To enter coding mode, prefix your thought \
with [CODE]. Only make changes that are clearly beneficial — bug fixes, \
performance improvements, better error handling. Do NOT refactor for style or \
make speculative changes.";

const REVIEW_INSTRUCTIONS: &str = "\
You are in REVIEW mode — code review and analysis.
Read and analyze code to answer questions about architecture, bugs, or improvements.
You have read-only access — you cannot modify files in this mode.
Be specific: reference file paths and line numbers when discussing code.";
