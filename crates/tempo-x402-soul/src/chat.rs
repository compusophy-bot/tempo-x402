//! Interactive chat handler for the soul.
//!
//! Session-based: maintains multi-turn conversation history in chat_messages table.
//! Each session preserves full message history so the LLM sees the complete conversation.
//! Plan context (active goals, pending approvals) is injected into every conversation.

use std::sync::Arc;

use serde::Serialize;

use crate::config::SoulConfig;
use crate::db::{ChatMessage, SoulDatabase};
use crate::error::SoulError;
use crate::git::GitContext;
use crate::llm::{ConversationMessage, ConversationPart, LlmClient};
use crate::memory::{Thought, ThoughtType};
use crate::mode;
use crate::observer::NodeObserver;
use crate::persistent_memory;
use crate::prompts;
use crate::thinking::{run_tool_loop_with_model, ToolExecution};
use crate::tool_registry::ToolRegistry;
use crate::tools::ToolExecutor;

/// The soul's reply to a chat message.
#[derive(Debug, Clone, Serialize)]
pub struct ChatReply {
    pub reply: String,
    pub tool_executions: Vec<ToolExecution>,
    pub thought_ids: Vec<String>,
    pub session_id: String,
}

/// Handle an interactive chat message with session-based conversation history.
///
/// 1. Resolve or create session
/// 2. Store user message in session
/// 3. Build context from snapshot + plan state + session history
/// 4. Run LLM with tools
/// 5. Store assistant reply in session
/// 6. Record as thoughts (backward compat for autonomous loop)
/// 7. Return reply with session_id
pub async fn handle_chat(
    message: &str,
    session_id: Option<&str>,
    config: &SoulConfig,
    db: &Arc<SoulDatabase>,
    observer: &Arc<dyn NodeObserver>,
    cartridge_engine: Option<&std::sync::Arc<x402_cartridge::CartridgeEngine>>,
) -> Result<ChatReply, SoulError> {
    let mut thought_ids = Vec::new();

    // 1. Resolve session
    let session_id = match session_id {
        Some(id) => id.to_string(),
        None => db.get_or_create_default_session()?,
    };

    let now = chrono::Utc::now().timestamp();

    // 2. Store user message in session
    let user_msg_id = uuid::Uuid::new_v4().to_string();
    db.insert_chat_message(&ChatMessage {
        id: user_msg_id.clone(),
        session_id: session_id.clone(),
        role: "user".to_string(),
        content: message.to_string(),
        tool_executions: "[]".to_string(),
        created_at: now,
    })?;

    // Also record as thought (backward compat)
    let user_thought_id = uuid::Uuid::new_v4().to_string();
    let user_thought = Thought {
        id: user_thought_id.clone(),
        thought_type: ThoughtType::ChatMessage,
        content: message.to_string(),
        context: None,
        created_at: now,
        salience: None,
        memory_tier: None,
        strength: None,
    };
    db.insert_thought(&user_thought)?;
    thought_ids.push(user_thought_id);

    // 2b. If this looks like a build request, also inject as a nudge so the
    // autonomous plan system picks it up with proper multi-step execution
    // (think → generate code → create cartridge → compile → test).
    let build_keywords = ["make", "build", "create", "write", "implement", "develop"];
    let msg_lower = message.to_lowercase();
    let is_build_request = build_keywords.iter().any(|k| msg_lower.contains(k))
        && (msg_lower.contains("app")
            || msg_lower.contains("game")
            || msg_lower.contains("tool")
            || msg_lower.contains("cartridge")
            || msg_lower.contains("calculator")
            || msg_lower.contains("todo")
            || msg_lower.contains("snake")
            || msg_lower.contains("tetris")
            || msg_lower.contains("drawing"));
    if is_build_request {
        let nudge_content = format!(
            "USER REQUEST: {}. Build this as a WASM cartridge with REAL source code. \
             Choose backend (x402 ABI, returns HTML) or frontend (Leptos app). \
             For frontend: use only leptos, wasm-bindgen, web-sys, serde, console_error_panic_hook. \
             Export init(selector) with mount_to(el, App). No other deps.",
            message
        );
        let _ = db.insert_nudge("user_chat", &nudge_content, 5);
    }

    // 3. Get current snapshot
    let snapshot = observer
        .observe()
        .map_err(|e| SoulError::Observer(format!("observe failed: {e}")))?;
    let snapshot_json = serde_json::to_value(&snapshot)
        .map(|v| crate::toon::snapshot_to_toon(&v))
        .unwrap_or_else(|_| serde_json::to_string(&snapshot).unwrap_or_default());

    // 4. Detect mode from message
    let agent_mode = mode::detect_mode_from_message(message, config.coding_enabled);
    let system_prompt = prompts::system_prompt_for_mode(agent_mode, config);

    // 5. Build context with persistent memory + plan state
    let memory_section = match persistent_memory::read_or_seed(&config.memory_file_path) {
        Ok(content) if !content.is_empty() => format!("Your persistent memory:\n{}\n\n", content),
        _ => String::new(),
    };

    let plan_context = build_plan_context(db);

    // Inject benchmark context so the human can see failures and teach Rust patterns
    let benchmark_context = {
        let bench_summary = crate::benchmark::opus_summary_for_prompt(db);
        let capability_profile = crate::capability::capability_guidance(db);
        if bench_summary.is_empty() && capability_profile.is_empty() {
            String::new()
        } else {
            format!(
                "Benchmark & Capabilities:\n{}\n{}\n\n",
                bench_summary, capability_profile
            )
        }
    };

    // Build environment capabilities section
    let capabilities_section = format!(
        "Your Environment & Capabilities:\n\
         - You are an autonomous AI agent running on Railway with full Rust toolchain\n\
         - Coding is {coding}\n\
         \n\
         CARTRIDGE RULES (FOLLOW EXACTLY):\n\
         When the user asks you to build something, you MUST write real code:\n\
         1. Choose the right type: backend (#[no_std] x402 ABI) or frontend (Leptos app).\n\
         2. For BACKEND: write #[no_std] Rust. Return HTML with inline CSS/JS from x402_handle().\n\
            No external deps. Use kv_get/kv_set for state. This always compiles.\n\
         3. For FRONTEND (set frontend=true): write a Leptos app. Available crates:\n\
            leptos 0.6 (csr), wasm-bindgen 0.2.108, web-sys, serde + serde(derive),\n\
            serde_json, console_error_panic_hook. NO OTHER DEPS. Export init(selector).\n\
         4. For pixel games: set interactive=true for the framebuffer ABI.\n\
         5. Then call compile_cartridge to build it.\n\
         6. NEVER create cartridges without source_code — empty = useless template.\n\
         7. Write the FULL implementation with real logic, not placeholders.\n\
         \n\
         BEHAVIOR RULES:\n\
         - Stay focused on what the user asked. Do NOT suggest unrelated projects.\n\
         - Do NOT hallucinate. If unsure, call list_cartridges to check what exists.\n\
         - Be concise. Do not write essays about your architecture or capabilities.\n\
         - When something fails, report the actual error, not a narrative about your journey.\n\n",
        coding = if config.coding_enabled {
            "ENABLED — you can write, edit, commit code"
        } else {
            "DISABLED"
        },
    );

    let context_message = format!(
        "{}{}{}Current node state:\n{}\n\n{}",
        memory_section, capabilities_section, benchmark_context, snapshot_json, plan_context
    );

    // 6. Build conversation from session history
    let history = db.get_session_messages(&session_id, 50)?;
    let mut conversation = vec![
        // System context as first user message + model ack
        ConversationMessage {
            role: "user".to_string(),
            parts: vec![ConversationPart::Text(context_message)],
        },
        ConversationMessage {
            role: "model".to_string(),
            parts: vec![ConversationPart::Text(
                "I have reviewed the current node state, plan progress, and conversation history. How can I help?"
                    .to_string(),
            )],
        },
    ];

    // Add session history (skip the message we just inserted — it's the last one)
    for msg in &history {
        if msg.id == user_msg_id {
            continue; // Skip the current message, we'll add it at the end
        }
        let role = if msg.role == "user" { "user" } else { "model" };
        conversation.push(ConversationMessage {
            role: role.to_string(),
            parts: vec![ConversationPart::Text(msg.content.clone())],
        });
    }

    // Add current user message
    conversation.push(ConversationMessage {
        role: "user".to_string(),
        parts: vec![ConversationPart::Text(message.to_string())],
    });

    // 7. Construct LLM client
    let api_key = config
        .llm_api_key
        .as_ref()
        .ok_or_else(|| SoulError::Config("no LLM API key configured".to_string()))?;

    let llm = LlmClient::new(
        api_key.clone(),
        config.llm_model_fast.clone(),
        config.llm_model_think.clone(),
    );

    // 8. Run tool loop with mode-specific tools
    let (dynamic_tools, meta_tools) = if config.tools_enabled && config.dynamic_tools_enabled {
        let dynamic = ToolRegistry::new(
            db.clone(),
            config.workspace_root.clone(),
            config.tool_timeout_secs,
        )
        .dynamic_tool_declarations(agent_mode.mode_tag());
        let meta = ToolRegistry::meta_tool_declarations();
        (dynamic, meta)
    } else {
        (vec![], vec![])
    };
    let tool_declarations = if config.tools_enabled {
        agent_mode.available_tools(config.coding_enabled, &dynamic_tools, &meta_tools)
    } else {
        vec![]
    };
    let max_calls = agent_mode.max_tool_calls();
    let mut tool_executor =
        ToolExecutor::new(config.tool_timeout_secs, config.workspace_root.clone())
            .with_memory_file(config.memory_file_path.clone())
            .with_gateway_url(config.gateway_url.clone())
            .with_database(db.clone());

    // Enable coding on the executor when coding is enabled.
    // Chat mode also gets coding tools (the mode system controls prompts, not capabilities).
    let needs_coding = matches!(agent_mode, mode::AgentMode::Code | mode::AgentMode::Chat);
    if needs_coding && config.coding_enabled {
        if let Some(instance_id) = &config.instance_id {
            let git = Arc::new(
                GitContext::new(
                    config.workspace_root.clone(),
                    instance_id.clone(),
                    config.github_token.clone(),
                )
                .with_fork(config.fork_repo.clone(), config.upstream_repo.clone())
                .with_direct_push(config.direct_push),
            );
            tool_executor = tool_executor.with_coding(git, db.clone());
        }
    }

    // Attach cartridge engine for cartridge tools
    if let Some(engine) = cartridge_engine {
        tool_executor = tool_executor.with_cartridge_engine(engine.clone());
    }

    // Attach dynamic tool registry if enabled
    if config.dynamic_tools_enabled {
        let registry = ToolRegistry::new(
            db.clone(),
            config.workspace_root.clone(),
            config.tool_timeout_secs,
        );
        tool_executor = tool_executor.with_registry(registry);
    }

    // Use deep model for code mode (deeper reasoning for modifications)
    let use_deep = agent_mode == mode::AgentMode::Code;
    let result = run_tool_loop_with_model(
        &llm,
        &system_prompt,
        &mut conversation,
        &tool_declarations,
        &tool_executor,
        db,
        max_calls,
        use_deep,
    )
    .await?;

    // 9. Store assistant reply in session
    let tool_exec_json =
        serde_json::to_string(&result.tool_executions).unwrap_or_else(|_| "[]".to_string());
    if !result.text.is_empty() {
        let assistant_msg_id = uuid::Uuid::new_v4().to_string();
        db.insert_chat_message(&ChatMessage {
            id: assistant_msg_id,
            session_id: session_id.clone(),
            role: "assistant".to_string(),
            content: result.text.clone(),
            tool_executions: tool_exec_json,
            created_at: chrono::Utc::now().timestamp(),
        })?;
    }

    // 10. Record soul's reply as ChatResponse thought (backward compat)
    if !result.text.is_empty() {
        let response_thought_id = uuid::Uuid::new_v4().to_string();
        let response_thought = Thought {
            id: response_thought_id.clone(),
            thought_type: ThoughtType::ChatResponse,
            content: result.text.clone(),
            context: Some(snapshot_json),
            created_at: chrono::Utc::now().timestamp(),
            salience: None,
            memory_tier: None,
            strength: None,
        };
        db.insert_thought(&response_thought)?;
        thought_ids.push(response_thought_id);

        // Extract and record decisions
        for line in result.text.lines() {
            let trimmed = line.trim();
            if let Some(decision_text) = trimmed.strip_prefix("[DECISION]") {
                let decision_id = uuid::Uuid::new_v4().to_string();
                let decision = Thought {
                    id: decision_id.clone(),
                    thought_type: ThoughtType::Decision,
                    content: decision_text.trim().to_string(),
                    context: None,
                    created_at: chrono::Utc::now().timestamp(),
                    salience: None,
                    memory_tier: None,
                    strength: None,
                };
                db.insert_thought(&decision)?;
                thought_ids.push(decision_id);
            }
        }
    }

    Ok(ChatReply {
        reply: result.text,
        tool_executions: result.tool_executions,
        thought_ids,
        session_id,
    })
}

/// Streaming variant of handle_chat — sends ChatEvent through a channel as work happens.
/// The caller should spawn this in a task and consume events from the receiver.
pub async fn handle_chat_stream(
    message: &str,
    session_id: Option<&str>,
    config: &SoulConfig,
    db: &Arc<SoulDatabase>,
    observer: &Arc<dyn NodeObserver>,
    cartridge_engine: Option<&std::sync::Arc<x402_cartridge::CartridgeEngine>>,
    tx: tokio::sync::mpsc::Sender<crate::thinking::ChatEvent>,
) -> Result<(), SoulError> {
    use crate::thinking::ChatEvent;

    // 1-6: Same setup as handle_chat (session, context, conversation, tools)
    let session_id = match session_id {
        Some(id) => id.to_string(),
        None => db.get_or_create_default_session()?,
    };
    let now = chrono::Utc::now().timestamp();
    let user_msg_id = uuid::Uuid::new_v4().to_string();
    db.insert_chat_message(&ChatMessage {
        id: user_msg_id.clone(),
        session_id: session_id.clone(),
        role: "user".to_string(),
        content: message.to_string(),
        tool_executions: "[]".to_string(),
        created_at: now,
    })?;

    // Record as thought
    let user_thought = Thought {
        id: uuid::Uuid::new_v4().to_string(),
        thought_type: ThoughtType::ChatMessage,
        content: message.to_string(),
        context: None,
        created_at: now,
        salience: None,
        memory_tier: None,
        strength: None,
    };
    db.insert_thought(&user_thought)?;

    // Inject nudge for build requests
    let build_keywords = ["make", "build", "create", "write", "implement", "develop"];
    let msg_lower = message.to_lowercase();
    let is_build_request = build_keywords.iter().any(|k| msg_lower.contains(k))
        && (msg_lower.contains("app")
            || msg_lower.contains("game")
            || msg_lower.contains("tool")
            || msg_lower.contains("cartridge")
            || msg_lower.contains("calculator")
            || msg_lower.contains("todo")
            || msg_lower.contains("snake")
            || msg_lower.contains("tetris")
            || msg_lower.contains("drawing"));
    if is_build_request {
        let nudge_content = format!(
            "USER REQUEST: {}. Build this as a WASM cartridge with REAL source code. \
             Choose backend (x402 ABI, returns HTML) or frontend (Leptos app). \
             For frontend: use only leptos, wasm-bindgen, web-sys, serde, console_error_panic_hook. \
             Export init(selector) with mount_to(el, App). No other deps.",
            message
        );
        let _ = db.insert_nudge("user_chat", &nudge_content, 5);
    }

    // Build context
    let snapshot = observer
        .observe()
        .map_err(|e| SoulError::Observer(format!("observe failed: {e}")))?;
    let snapshot_json = serde_json::to_value(&snapshot)
        .map(|v| crate::toon::snapshot_to_toon(&v))
        .unwrap_or_else(|_| serde_json::to_string(&snapshot).unwrap_or_default());

    let agent_mode = mode::detect_mode_from_message(message, config.coding_enabled);
    let system_prompt = prompts::system_prompt_for_mode(agent_mode, config);
    let memory_section = match persistent_memory::read_or_seed(&config.memory_file_path) {
        Ok(content) if !content.is_empty() => format!("Your persistent memory:\n{}\n\n", content),
        _ => String::new(),
    };
    let plan_context = build_plan_context(db);
    let benchmark_context = {
        let bench_summary = crate::benchmark::opus_summary_for_prompt(db);
        let capability_profile = crate::capability::capability_guidance(db);
        if bench_summary.is_empty() && capability_profile.is_empty() {
            String::new()
        } else {
            format!(
                "Benchmark & Capabilities:\n{}\n{}\n\n",
                bench_summary, capability_profile
            )
        }
    };
    let capabilities_section = format!(
        "Your Environment & Capabilities:\n\
         - You are an autonomous AI agent running on Railway with full Rust toolchain\n\
         - Coding is {coding}\n\
         \n\
         CARTRIDGE RULES (FOLLOW EXACTLY):\n\
         When the user asks you to build something, you MUST write real code:\n\
         1. Choose the right type: backend (#[no_std] x402 ABI) or frontend (Leptos app).\n\
         2. For BACKEND: write #[no_std] Rust. Return HTML with inline CSS/JS from x402_handle().\n\
            No external deps. Use kv_get/kv_set for state. This always compiles.\n\
         3. For FRONTEND (set frontend=true): write a Leptos app. Available crates:\n\
            leptos 0.6 (csr), wasm-bindgen 0.2.108, web-sys, serde + serde(derive),\n\
            serde_json, console_error_panic_hook. NO OTHER DEPS. Export init(selector).\n\
         4. For pixel games: set interactive=true for the framebuffer ABI.\n\
         5. Then call compile_cartridge to build it.\n\
         6. NEVER create cartridges without source_code — empty = useless template.\n\
         7. Write the FULL implementation with real logic, not placeholders.\n\
         \n\
         BEHAVIOR RULES:\n\
         - Stay focused on what the user asked. Do NOT suggest unrelated projects.\n\
         - Do NOT hallucinate. If unsure, call list_cartridges to check what exists.\n\
         - Be concise. Do not write essays about your architecture or capabilities.\n\
         - When something fails, report the actual error, not a narrative about your journey.\n\n",
        coding = if config.coding_enabled {
            "ENABLED — you can write, edit, commit code"
        } else {
            "DISABLED"
        },
    );

    let context_message = format!(
        "{}{}{}Current node state:\n{}\n\n{}",
        memory_section, capabilities_section, benchmark_context, snapshot_json, plan_context
    );

    // Build conversation
    let history = db.get_session_messages(&session_id, 50)?;
    let mut conversation = vec![
        ConversationMessage {
            role: "user".to_string(),
            parts: vec![ConversationPart::Text(context_message)],
        },
        ConversationMessage {
            role: "model".to_string(),
            parts: vec![ConversationPart::Text(
                "I have reviewed the current node state, plan progress, and conversation history. How can I help?"
                    .to_string(),
            )],
        },
    ];
    for msg in &history {
        if msg.id == user_msg_id {
            continue;
        }
        let role = if msg.role == "user" { "user" } else { "model" };
        conversation.push(ConversationMessage {
            role: role.to_string(),
            parts: vec![ConversationPart::Text(msg.content.clone())],
        });
    }
    conversation.push(ConversationMessage {
        role: "user".to_string(),
        parts: vec![ConversationPart::Text(message.to_string())],
    });

    // LLM client
    let api_key = config
        .llm_api_key
        .as_ref()
        .ok_or_else(|| SoulError::Config("no LLM API key configured".to_string()))?;
    let llm = LlmClient::new(
        api_key.clone(),
        config.llm_model_fast.clone(),
        config.llm_model_think.clone(),
    );

    // Tools setup
    let (dynamic_tools, meta_tools) = if config.tools_enabled && config.dynamic_tools_enabled {
        let dynamic = ToolRegistry::new(
            db.clone(),
            config.workspace_root.clone(),
            config.tool_timeout_secs,
        )
        .dynamic_tool_declarations(agent_mode.mode_tag());
        let meta = ToolRegistry::meta_tool_declarations();
        (dynamic, meta)
    } else {
        (vec![], vec![])
    };
    let tool_declarations = if config.tools_enabled {
        agent_mode.available_tools(config.coding_enabled, &dynamic_tools, &meta_tools)
    } else {
        vec![]
    };
    let max_calls = agent_mode.max_tool_calls();
    let mut tool_executor =
        ToolExecutor::new(config.tool_timeout_secs, config.workspace_root.clone())
            .with_memory_file(config.memory_file_path.clone())
            .with_gateway_url(config.gateway_url.clone())
            .with_database(db.clone());

    let needs_coding = matches!(agent_mode, mode::AgentMode::Code | mode::AgentMode::Chat);
    if needs_coding && config.coding_enabled {
        if let Some(instance_id) = &config.instance_id {
            let git = Arc::new(
                GitContext::new(
                    config.workspace_root.clone(),
                    instance_id.clone(),
                    config.github_token.clone(),
                )
                .with_fork(config.fork_repo.clone(), config.upstream_repo.clone())
                .with_direct_push(config.direct_push),
            );
            tool_executor = tool_executor.with_coding(git, db.clone());
        }
    }
    if let Some(engine) = cartridge_engine {
        tool_executor = tool_executor.with_cartridge_engine(engine.clone());
    }
    if config.dynamic_tools_enabled {
        let registry = ToolRegistry::new(
            db.clone(),
            config.workspace_root.clone(),
            config.tool_timeout_secs,
        );
        tool_executor = tool_executor.with_registry(registry);
    }

    // Run streaming tool loop
    let use_deep = agent_mode == mode::AgentMode::Code;
    let result = crate::thinking::run_tool_loop_streaming(
        &llm,
        &system_prompt,
        &mut conversation,
        &tool_declarations,
        &tool_executor,
        db,
        max_calls,
        use_deep,
        &tx,
    )
    .await?;

    // Store assistant reply
    let tool_exec_json =
        serde_json::to_string(&result.tool_executions).unwrap_or_else(|_| "[]".to_string());
    if !result.text.is_empty() {
        db.insert_chat_message(&ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            role: "assistant".to_string(),
            content: result.text.clone(),
            tool_executions: tool_exec_json,
            created_at: chrono::Utc::now().timestamp(),
        })?;
    }

    // Send final reply event
    let _ = tx
        .send(ChatEvent::Reply {
            text: result.text,
            tool_executions: result.tool_executions,
            session_id,
        })
        .await;

    Ok(())
}

/// Build a plan context string for injection into chat conversations.
/// Includes active plan progress, pending approvals, and active goals.
fn build_plan_context(db: &Arc<SoulDatabase>) -> String {
    let mut sections = Vec::new();

    // Active plan
    if let Ok(Some(plan)) = db.get_active_plan() {
        let step_desc = plan
            .steps
            .get(plan.current_step)
            .map(|s| s.summary())
            .unwrap_or_else(|| "done".to_string());
        sections.push(format!(
            "## Active Plan\n- ID: {}\n- Goal: {}\n- Progress: step {}/{}\n- Current: {}\n- Replans: {}",
            plan.id,
            plan.goal_id,
            plan.current_step + 1,
            plan.steps.len(),
            step_desc,
            plan.replan_count,
        ));
    }

    // Pending approval plan
    if let Ok(Some(plan)) = db.get_pending_approval_plan() {
        let steps_summary: Vec<String> = plan
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("  {}. {}", i + 1, s.summary()))
            .collect();
        if let Ok(Some(goal)) = db.get_goal(&plan.goal_id) {
            sections.push(format!(
                "## PLAN AWAITING APPROVAL\n- Plan ID: {}\n- Goal: {}\n- Steps:\n{}\n\nThe user can approve or reject this plan.",
                plan.id,
                goal.description,
                steps_summary.join("\n"),
            ));
        }
    }

    // Active goals
    if let Ok(goals) = db.get_active_goals() {
        if !goals.is_empty() {
            let goal_lines: Vec<String> = goals
                .iter()
                .map(|g| {
                    format!(
                        "- [P{}] {} (retries: {})",
                        g.priority, g.description, g.retry_count
                    )
                })
                .collect();
            sections.push(format!("## Active Goals\n{}", goal_lines.join("\n")));
        }
    }

    if sections.is_empty() {
        String::new()
    } else {
        format!("# Soul State\n{}\n\n", sections.join("\n\n"))
    }
}
