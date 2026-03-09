//! System prompts per agent mode.
//!
//! Five focused prompt builders for plan-driven execution, plus
//! mode-specific system prompts for chat, code, and review.

use crate::config::SoulConfig;
use crate::db::Nudge;
use crate::mode::AgentMode;
use crate::observer::NodeSnapshot;
use crate::world_model::{Belief, Goal};

/// Build the system prompt for a given agent mode.
pub fn system_prompt_for_mode(mode: AgentMode, config: &SoulConfig) -> String {
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
                     Your pushes trigger auto-deploy.{}",
                    config
                        .upstream_repo
                        .as_ref()
                        .map(|u| format!(
                            " You can create PRs or issues on `{u}` for upstream changes."
                        ))
                        .unwrap_or_default()
                ),
                None => "\n\nDIRECT PUSH MODE: You push directly to main. \
                         Every commit is validated (cargo check + test) before landing."
                    .to_string(),
            }
        } else {
            match (&config.fork_repo, &config.upstream_repo) {
                (Some(fork), Some(upstream)) => format!(
                    "\n\nGit workflow: You push to fork `{fork}`, create PRs targeting `{upstream}`."
                ),
                _ => String::new(),
            }
        };
        format!(
            "\n\nCoding is ENABLED. You can read, edit, and write files. \
             Commits validated via cargo check + test.{workflow_info}"
        )
    } else {
        String::new()
    };

    let mode_instructions = match mode {
        AgentMode::Observe => "", // Plan-driven — no observe prompt needed
        AgentMode::Chat => CHAT_INSTRUCTIONS,
        AgentMode::Code => CODE_INSTRUCTIONS,
        AgentMode::Review => REVIEW_INSTRUCTIONS,
    };

    format!("{base}{lineage}{coding_context}\n\n{mode_instructions}")
}

// ── Plan-driven prompt builders ─────────────────────────────────────

/// Prompt for creating goals when the soul has none.
/// Focused: snapshot + beliefs → what should you build?
#[allow(clippy::too_many_arguments)]
pub fn goal_creation_prompt(
    snapshot: &NodeSnapshot,
    beliefs: &[Belief],
    nudges: &[Nudge],
    cycles_since_commit: u64,
    failed_plans: u64,
    total_cycles: u64,
    recent_errors: &[String],
    recently_failed_goals: &[String],
) -> String {
    let mut sections = Vec::new();

    sections.push(format!(
        "# Current State\n\
         - Uptime: {}h\n\
         - Endpoints: {}\n\
         - Total payments: {}\n\
         - Total revenue: {}\n\
         - Children: {}",
        snapshot.uptime_secs / 3600,
        snapshot.endpoint_count,
        snapshot.total_payments,
        snapshot.total_revenue,
        snapshot.children_count,
    ));

    if !snapshot.endpoints.is_empty() {
        let mut ep_lines = vec!["# Endpoints".to_string()];
        for ep in &snapshot.endpoints {
            ep_lines.push(format!(
                "- {} (price:{}, requests:{}, payments:{}, revenue:{})",
                ep.slug, ep.price, ep.request_count, ep.payment_count, ep.revenue
            ));
        }
        sections.push(ep_lines.join("\n"));
    }

    // Network peers — show what sibling/child agents are available
    if !snapshot.peers.is_empty() {
        let mut peer_lines = vec!["# Network Peers".to_string()];
        for peer in &snapshot.peers {
            let ep_summary: Vec<String> = peer
                .endpoints
                .iter()
                .map(|e| format!("{} (${})", e.slug, e.price))
                .collect();
            let ep_str = if ep_summary.is_empty() {
                "no endpoints".to_string()
            } else {
                ep_summary.join(", ")
            };
            peer_lines.push(format!(
                "- {} ({}) — {}{}",
                peer.instance_id,
                peer.url,
                ep_str,
                peer.version
                    .as_ref()
                    .map(|v| format!(" [v{v}]"))
                    .unwrap_or_default()
            ));
        }
        sections.push(peer_lines.join("\n"));
    }

    // Include non-auto beliefs (LLM-created ones have real insight)
    let llm_beliefs: Vec<_> = beliefs
        .iter()
        .filter(|b| !b.evidence.starts_with("auto:"))
        .collect();
    if !llm_beliefs.is_empty() {
        let mut belief_lines = vec!["# Your Beliefs".to_string()];
        for b in llm_beliefs.iter().take(10) {
            belief_lines.push(format!(
                "- [{:?}] {}.{} = {} ({})",
                b.domain,
                b.subject,
                b.predicate,
                b.value,
                b.confidence.as_str()
            ));
        }
        sections.push(belief_lines.join("\n"));
    }

    // Pending nudges (user messages are highest priority)
    if !nudges.is_empty() {
        let mut nudge_lines =
            vec!["# Pending Nudges (external signals — address these)".to_string()];
        for n in nudges {
            nudge_lines.push(format!(
                "- [{}] (priority {}) {}",
                n.source, n.priority, n.content
            ));
        }
        sections.push(nudge_lines.join("\n"));
    }

    // Self-diagnostics
    if total_cycles > 0 || failed_plans > 0 || cycles_since_commit > 0 {
        let mut diag_lines = vec!["# Self-Diagnostics".to_string()];
        diag_lines.push(format!("- Total cycles: {total_cycles}"));
        diag_lines.push(format!("- Cycles since last commit: {cycles_since_commit}"));
        diag_lines.push(format!("- Failed plans: {failed_plans}"));
        if !recent_errors.is_empty() {
            diag_lines.push("- Recent errors:".to_string());
            for err in recent_errors.iter().take(3) {
                diag_lines.push(format!("  - {err}"));
            }
        }
        sections.push(diag_lines.join("\n"));
    }

    // Show recently failed/abandoned goals so LLM doesn't repeat them
    if !recently_failed_goals.is_empty() {
        let mut failed_lines = vec![
            "# Recently Failed Goals (do NOT retry these — try something DIFFERENT)".to_string(),
        ];
        for desc in recently_failed_goals.iter().take(5) {
            failed_lines.push(format!("- {desc}"));
        }
        sections.push(failed_lines.join("\n"));
    }

    // Dynamic task based on actual state — react to the data, don't blindly repeat
    let endpoint_count = snapshot.endpoint_count;
    let total_payments = snapshot.total_payments;
    let paid_endpoints = snapshot
        .endpoints
        .iter()
        .filter(|ep| ep.payment_count > 0)
        .count();

    let situation_analysis = if endpoint_count > 3 && total_payments == 0 {
        format!(
            "## CRITICAL: You have {endpoint_count} endpoints and ZERO payments.\n\
             STOP CREATING ENDPOINTS. You have enough. The problem is not supply — it's demand.\n\
             You MUST do one of these instead:\n\
             1. Discover peers (discover_peers) and call their endpoints — generate REAL traffic\n\
             2. Delete low-value endpoints (delete_endpoint) until you have ≤5 high-quality ones\n\
             3. Investigate reachability: can peers actually find and call your endpoints?\n\n\
             DO NOT create any new script endpoints. This is a hard constraint.\n\
             Focus on the ECONOMY: call peers, earn revenue, prune dead weight."
        )
    } else if endpoint_count > 0 && paid_endpoints == 0 {
        format!(
            "## WARNING: You have {endpoint_count} endpoints but NONE have received payments.\n\
             DO NOT create more endpoints. Your existing supply is untested.\n\
             You MUST generate demand first:\n\
             1. Use discover_peers to find sibling agents\n\
             2. Use call_peer to call at least 2-3 of their endpoints — this creates real economic activity\n\
             3. After calling peers, check if they call you back (check_self analytics)\n\
             4. If no peers exist yet, delete low-value endpoints and wait\n\n\
             Revenue comes from OTHER agents calling YOUR endpoints. But they won't discover you \
             unless you participate in the economy first. Call peers to make yourself visible."
        )
    } else if endpoint_count > 0 && paid_endpoints > 0 {
        format!(
            "## {paid_endpoints}/{endpoint_count} endpoints have received payments. \
             Focus on what's working. Improve or extend the ones that earn revenue. \
             Do NOT create new endpoints unless you have a clear reason."
        )
    } else {
        String::new()
    };

    let mut task_section = String::from(
        "# Task\n\
         You have NO active goals. Create 1-2 goals.\n\n\
         If there are pending nudges, prioritize those. If there are recent errors, avoid repeating \
         the same approach that caused them.\n\n",
    );

    if !situation_analysis.is_empty() {
        task_section.push_str(&situation_analysis);
        task_section.push_str("\n\n");
    }

    task_section.push_str(&format!(
        "## Rules\n\
         - Create 1-2 goals MAX\n\
         - {endpoint_rule}\n\
         - Do NOT edit Rust source code unless explicitly asked by a nudge\n\
         - Do NOT create \"fix\" goals — if something failed, try something DIFFERENT\n\
         - You can discover peer instances via `/instance/siblings` and call their paid endpoints\n\
         - You can clone yourself using `call_peer` with the `/clone` endpoint (do NOT use curl — cloning requires x402 payment signing)\n\n\
         Respond with a JSON array of goal operations:\n\
         ```json\n\
         [\n\
           {{\"op\": \"create_goal\", \"description\": \"...\", \"success_criteria\": \"...\", \"priority\": 4}}\n\
         ]\n\
         ```\n\
         Priority: 1 (low) to 5 (critical). Be specific.",
        endpoint_rule = if total_payments == 0 && endpoint_count > 0 {
            "DO NOT create new endpoints — you have 0 payments. Focus on demand: discover_peers, call_peer, delete_endpoint"
        } else {
            "Script endpoints (create_script_endpoint) for new services — only if you have a clear value proposition"
        }
    ));

    sections.push(task_section);

    sections.join("\n\n")
}

/// Prompt for creating a plan to achieve a goal.
/// Focused: goal + workspace listing → ordered steps as JSON.
pub fn planning_prompt(
    goal: &Goal,
    workspace_listing: &str,
    nudges: &[Nudge],
    recent_errors: &[String],
) -> String {
    let mut extra_context = String::new();

    if !nudges.is_empty() {
        extra_context.push_str("\n# Pending Nudges\n");
        for n in nudges {
            extra_context.push_str(&format!("- [{}] {}\n", n.source, n.content));
        }
    }

    if !recent_errors.is_empty() {
        extra_context.push_str("\n# Recent Errors (avoid repeating these)\n");
        for err in recent_errors.iter().take(3) {
            extra_context.push_str(&format!("- {err}\n"));
        }
    }

    format!(
        "# Goal\n\
         {}\n\
         Success criteria: {}\n\
         Progress so far: {}\n\n\
         # Workspace\n\
         {}{}\n\n\
         # How to Add Endpoints — TWO approaches\n\n\
         ## Option A: Script Endpoints (PREFERRED — instant, no compilation)\n\
         Use create_script_endpoint to write a bash script. It becomes live at /x/{{slug}} immediately.\n\
         Steps: 1) create_script_endpoint with slug + bash script  2) test_script_endpoint to verify  3) Done!\n\
         The script gets REQUEST_BODY, REQUEST_METHOD, QUERY_STRING as env vars. Output JSON to stdout.\n\
         Available tools in scripts: bash, jq, python3, curl, bc, git, date, sed, awk, grep.\n\
         Use jq for JSON processing. Use python3 for complex logic. Use curl to call external APIs.\n\
         Example (jq):\n\
         ```bash\n\
         #!/bin/bash\n\
         echo \"$REQUEST_BODY\" | jq '.' 2>/dev/null || echo '{{\"error\":\"invalid JSON\"}}'\n\
         ```\n\
         Example (python3):\n\
         ```bash\n\
         #!/bin/bash\n\
         python3 -c \"import json,sys; print(json.dumps({{'time':__import__('time').time()}}))\"\n\
         ```\n\n\
         ## Option B: Rust Endpoints (ADVANCED — almost never needed)\n\
         WARNING: Editing Rust source code requires the full project to compile. Past attempts to \
         edit utils.rs have consistently failed with cargo check errors. Do NOT attempt Rust code \
         changes unless a nudge explicitly requests it.\n\n\
         USE SCRIPT ENDPOINTS. They are instant, always work, and support jq + python3 for complex logic.\n\n\
         ## Inter-Agent Economy\n\
         Your script endpoints are gated by x402 payment — other agents pay to call them.\n\
         You can call other agents' paid endpoints using `call_peer` (discovers + calls in one step).\n\
         ALWAYS use `call_peer` for inter-agent calls (discovers + resolves URL + signs payment in one step).\n\
         Building useful endpoints = revenue from other agents calling them.\n\n\
         # Task\n\
         Create a step-by-step plan to achieve this goal. Each step is one of:\n\n\
         Mechanical (no LLM needed):\n\
         - {{\"type\": \"read_file\", \"path\": \"...\", \"store_as\": \"key\"}}\n\
         - {{\"type\": \"search_code\", \"pattern\": \"...\", \"directory\": \"...\", \"store_as\": \"key\"}}\n\
         - {{\"type\": \"list_dir\", \"path\": \"...\", \"store_as\": \"key\"}}\n\
         - {{\"type\": \"run_shell\", \"command\": \"...\", \"store_as\": \"key\"}}\n\
         - {{\"type\": \"commit\", \"message\": \"...\"}}\n\
         - {{\"type\": \"check_self\", \"endpoint\": \"health\", \"store_as\": \"key\"}}\n\
         - (DEPRECATED: call_paid_endpoint — use call_peer instead, it handles URL resolution automatically)\n\
         - {{\"type\": \"create_script_endpoint\", \"slug\": \"...\", \"script\": \"#!/bin/bash\\n...\", \"description\": \"...\"}}\n\
         - {{\"type\": \"test_script_endpoint\", \"slug\": \"...\", \"input\": \"test data\", \"store_as\": \"key\"}}\n\
         - {{\"type\": \"cargo_check\", \"store_as\": \"check_result\"}}\n\
         - {{\"type\": \"delete_endpoint\", \"slug\": \"script-name\"}}  (deactivate a registered endpoint)\n\
         - {{\"type\": \"discover_peers\", \"store_as\": \"peers\"}}  (fetches sibling/child instances and their endpoints)\n\
         - {{\"type\": \"call_peer\", \"slug\": \"script-peer-discovery\", \"store_as\": \"result\"}}  (RECOMMENDED for inter-agent calls — discovers peers, resolves URL, signs payment — ONE step)\n\n\
         LLM-assisted:\n\
         - {{\"type\": \"generate_code\", \"file_path\": \"...\", \"description\": \"...\", \"context_keys\": [\"key\"]}}\n\
         - {{\"type\": \"edit_code\", \"file_path\": \"...\", \"description\": \"...\", \"context_keys\": [\"key\"]}}\n\
         - {{\"type\": \"think\", \"question\": \"...\", \"store_as\": \"key\"}}\n\n\
         Rules:\n\
         - ALWAYS read files BEFORE editing them (use store_as to pass content to edit steps)\n\
         - For Rust code changes: put a cargo_check step AFTER each edit_code/generate_code step and BEFORE the commit step\n\
         - edit_code/generate_code steps have a built-in compile-fix loop (3 retries) but cargo_check stores errors explicitly\n\
         - End with a commit step\n\
         - Max 20 steps, prefer fewer — a simple endpoint needs ~5 steps (read, edit, cargo_check, commit)\n\
         - Prefer edit_code over generate_code for existing files\n\
         - Protected files (soul core, identity, Cargo.toml, Cargo.lock) cannot be modified\n\
         - Do NOT try to modify Dockerfile, railway.toml, or deployment configs — focus on Rust code\n\
         - Use only dependencies already available in the workspace\n\
         - For inter-agent calls, ALWAYS use call_peer with just the slug. NEVER construct URLs manually.\n\n\
         Respond with ONLY a JSON array of steps, no other text.",
        goal.description,
        goal.success_criteria,
        if goal.progress_notes.is_empty() {
            "none"
        } else {
            &goal.progress_notes
        },
        workspace_listing,
        extra_context,
    )
}

/// Prompt for code generation/editing within a plan step.
/// Focused: file content + description + context → write/edit the file.
pub fn code_generation_prompt(
    file_path: &str,
    current_content: Option<&str>,
    description: &str,
    context: &str,
) -> String {
    let content_section = match current_content {
        Some(content) => format!("# Current content of {file_path}\n```\n{content}\n```\n\n"),
        None => format!("# File: {file_path} (new file)\n\n"),
    };

    format!(
        "{content_section}\
         # Task\n\
         {description}\n\
         {context}\n\n\
         # Available Dependencies (already in Cargo.toml — do NOT add new ones)\n\
         - actix-web (web framework): HttpRequest, HttpResponse, web::{{Data, Json, Path, Query, ServiceConfig}}\n\
         - serde / serde_json: Serialize, Deserialize, serde_json::{{json, Value}}\n\
         - tokio: async runtime, tokio::process::Command, tokio::time\n\
         - alloy: Ethereum types (Address, U256, FixedBytes), providers, signers\n\
         - reqwest: HTTP client\n\
         - tracing: tracing::info!, tracing::warn!, tracing::error!\n\
         - chrono: Utc, DateTime, NaiveDateTime\n\
         - uuid: Uuid::new_v4()\n\
         - sha2 / hmac: for hashing\n\
         - hex: hex::encode, hex::decode\n\
         - rusqlite: SQLite (used via SoulDatabase wrapper)\n\n\
         # Rust Patterns for This Codebase\n\
         - Error handling: use `Result<T, String>` or `Result<T, actix_web::Error>` for handlers\n\
         - Actix handlers return `impl Responder` or `Result<HttpResponse, actix_web::Error>`\n\
         - Route registration: `cfg.service(web::resource(\"/path\").route(web::get().to(handler)))`\n\
         - JSON responses: `HttpResponse::Ok().json(serde_json::json!({{...}}))`\n\
         - Shared state: `web::Data<AppState>` passed to handlers\n\
         - String → &str: use `.as_str()` or `&*string_var`\n\
         - async fn handler(req: HttpRequest) -> impl Responder {{ ... }}\n\n\
         Rules:\n\
         - Use edit_file for existing files (provide unique old_string and new_string)\n\
         - Use write_file only for brand new files\n\
         - Keep changes minimal and focused — add your code at the right location\n\
         - For actix-web endpoints: add the handler function AND update the configure fn\n\
         - Ensure all imports are at the top of the file\n\
         - Do NOT rewrite the entire file — only add/change what's needed\n\
         - If unsure about an import path, use search_files or read_file to check\n\
         - After editing, you can run `execute_shell` with `cargo check --workspace` to verify"
    )
}

/// Prompt for replanning after a step failure.
/// Focused: goal + failed step + error → adjusted steps.
pub fn replan_prompt(goal: &Goal, failed_step_desc: &str, error: &str) -> String {
    format!(
        "# Goal\n\
         {}\n\n\
         # Failed Step\n\
         {}\n\n\
         # Error\n\
         {}\n\n\
         # Task\n\
         The step above failed. Adjust the remaining plan.\n\
         Respond with ONLY a JSON array of replacement steps (same format as planning).\n\
         You may need to add investigation steps before retrying.\n\
         Max 20 steps.",
        goal.description, failed_step_desc, error,
    )
}

/// Prompt for reflection after a plan completes.
/// Focused: what was done + outcome → what did you learn?
pub fn reflection_prompt(
    goal: &Goal,
    steps_completed: usize,
    mutation_summary: &str,
    cycles_since_commit: u64,
    failed_plans: u64,
) -> String {
    let mut diag = String::new();
    if cycles_since_commit > 5 || failed_plans > 0 {
        diag = format!(
            "\n\n# Self-Diagnostics\n\
             - Cycles since last commit: {cycles_since_commit}\n\
             - Failed plans: {failed_plans}"
        );
    }

    format!(
        "# Completed Goal\n\
         ID: {}\n\
         Description: {}\n\
         Success criteria: {}\n\
         Steps completed: {}{}\n\n\
         # Mutation Summary\n\
         {}\n\n\
         # Task\n\
         Reflect briefly:\n\
         1. Did this advance the goal's success criteria?\n\
         2. What worked well or poorly?\n\
         3. What should happen next?\n\n\
         Respond with a JSON array of goal/belief updates:\n\
         ```json\n\
         [\n\
           {{\"op\": \"complete_goal\", \"goal_id\": \"{}\", \"outcome\": \"...\"}}\n\
         ]\n\
         ```\n\
         Or if the goal isn't done yet, use update_goal with the same goal_id and progress notes.\n\n\
         RULES:\n\
         - Use the EXACT goal_id shown above (the UUID) — do NOT make up goal IDs\n\
         - Do NOT create follow-up \"fix\" goals. If something broke, it will be retried differently.\n\
         - You may create AT MOST 1 new goal, and only if it's a genuinely NEW idea (not fixing the current one).\n\
         - Focus on marking the current goal complete or abandoned — do not cascade.",
        goal.id,
        goal.description,
        goal.success_criteria,
        steps_completed,
        diag,
        if mutation_summary.is_empty() {
            "No commits made"
        } else {
            mutation_summary
        },
        goal.id, // repeated for the example JSON
    )
}

// ── Mode-specific constants (kept for chat.rs and code steps) ───────

pub(crate) const CHAT_INSTRUCTIONS: &str = "\
You are in CHAT mode — interactive conversation with a user.
Answer helpfully and concisely. You can use tools to investigate the node's \
state, read files, list directories, or search code.
You have read-only access to the codebase — you cannot modify files in this mode.";

pub(crate) const CODE_INSTRUCTIONS: &str = "\
You are in CODE mode — you can read, write, and edit files in the codebase.

Workflow:
1. Understand the task — read relevant files first
2. Make changes — use edit_file (preferred) or write_file
3. Validate — some critical files are protected and cannot be modified
4. Commit — use commit_changes to validate (cargo check + test) and commit
5. In direct push mode, your commits go straight to main and auto-deploy

Rules:
- Protected files (soul core, identity, Cargo files) cannot be modified
- All commits run through cargo check + cargo test before landing
- Use edit_file for surgical changes (old_string must be unique)
- Use write_file for new files or complete rewrites
- Keep changes minimal and focused — one logical change per commit";

pub(crate) const REVIEW_INSTRUCTIONS: &str = "\
You are in REVIEW mode — code review and analysis.
Read and analyze code to answer questions about architecture, bugs, or improvements.
You have read-only access — you cannot modify files in this mode.
Be specific: reference file paths and line numbers when discussing code.";
