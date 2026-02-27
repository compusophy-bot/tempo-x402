//! Interactive chat handler for the soul.
//!
//! Stateless per-request: builds context from DB (recent thoughts + snapshot),
//! runs the LLM with tools, records thoughts, and returns the reply.

use std::sync::Arc;

use serde::Serialize;

use crate::config::SoulConfig;
use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::git::GitContext;
use crate::llm::{ConversationMessage, ConversationPart, LlmClient};
use crate::memory::{Thought, ThoughtType};
use crate::mode;
use crate::observer::NodeObserver;
use crate::prompts;
use crate::thinking::{run_tool_loop, ToolExecution};
use crate::tools::ToolExecutor;

/// The soul's reply to a chat message.
#[derive(Debug, Clone, Serialize)]
pub struct ChatReply {
    pub reply: String,
    pub tool_executions: Vec<ToolExecution>,
    pub thought_ids: Vec<String>,
}

/// Handle an interactive chat message.
///
/// 1. Record user message as ChatMessage thought
/// 2. Build context from snapshot + recent thoughts
/// 3. Run LLM with tools (reuses the think cycle's tool loop)
/// 4. Record response as ChatResponse thought + any decisions
/// 5. Return reply
pub async fn handle_chat(
    message: &str,
    config: &SoulConfig,
    db: &Arc<SoulDatabase>,
    observer: &Arc<dyn NodeObserver>,
) -> Result<ChatReply, SoulError> {
    let mut thought_ids = Vec::new();

    // 1. Record user message
    let user_thought_id = uuid::Uuid::new_v4().to_string();
    let user_thought = Thought {
        id: user_thought_id.clone(),
        thought_type: ThoughtType::ChatMessage,
        content: message.to_string(),
        context: None,
        created_at: chrono::Utc::now().timestamp(),
    };
    db.insert_thought(&user_thought)?;
    thought_ids.push(user_thought_id);

    // 2. Get current snapshot
    let snapshot = observer
        .observe()
        .map_err(|e| SoulError::Observer(format!("observe failed: {e}")))?;
    let snapshot_json = serde_json::to_string(&snapshot)?;

    // 3. Fetch recent thoughts for context
    let recent = db.recent_thoughts(10)?;
    let recent_summary: Vec<String> = recent
        .iter()
        .map(|t| {
            format!(
                "[{}] {}: {}",
                t.thought_type.as_str(),
                chrono::DateTime::from_timestamp(t.created_at, 0)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| "?".to_string()),
                t.content.chars().take(200).collect::<String>()
            )
        })
        .collect();

    // 4. Detect mode from message
    let agent_mode = mode::detect_mode_from_message(message, config.coding_enabled);
    let system_prompt = prompts::system_prompt_for_mode(agent_mode, config);

    // 5. Build conversation
    let context_message = format!(
        "Current node state:\n{}\n\nRecent thoughts:\n{}",
        snapshot_json,
        recent_summary.join("\n")
    );

    let mut conversation = vec![
        ConversationMessage {
            role: "user".to_string(),
            parts: vec![ConversationPart::Text(context_message)],
        },
        ConversationMessage {
            role: "model".to_string(),
            parts: vec![ConversationPart::Text(
                "I have reviewed the current node state and recent thoughts. How can I help?"
                    .to_string(),
            )],
        },
        ConversationMessage {
            role: "user".to_string(),
            parts: vec![ConversationPart::Text(message.to_string())],
        },
    ];

    // 6. Construct LLM client
    let api_key = config
        .llm_api_key
        .as_ref()
        .ok_or_else(|| SoulError::Config("no LLM API key configured".to_string()))?;

    let llm = LlmClient::new(
        api_key.clone(),
        config.llm_model_fast.clone(),
        config.llm_model_think.clone(),
    );

    // 7. Run tool loop with mode-specific tools
    let tool_declarations = if config.tools_enabled {
        agent_mode.available_tools(config.coding_enabled)
    } else {
        vec![]
    };
    let max_calls = agent_mode.max_tool_calls();
    let mut tool_executor =
        ToolExecutor::new(config.tool_timeout_secs, config.workspace_root.clone());

    // Enable coding on the executor if in Code mode
    if agent_mode == mode::AgentMode::Code && config.coding_enabled {
        if let Some(instance_id) = &config.instance_id {
            let git = Arc::new(GitContext::new(
                config.workspace_root.clone(),
                instance_id.clone(),
                config.github_token.clone(),
            ));
            tool_executor = tool_executor.with_coding(git, db.clone());
        }
    }

    let result = run_tool_loop(
        &llm,
        &system_prompt,
        &mut conversation,
        &tool_declarations,
        &tool_executor,
        db,
        max_calls,
    )
    .await?;

    // 8. Record soul's reply as ChatResponse thought
    if !result.text.is_empty() {
        let response_thought_id = uuid::Uuid::new_v4().to_string();
        let response_thought = Thought {
            id: response_thought_id.clone(),
            thought_type: ThoughtType::ChatResponse,
            content: result.text.clone(),
            context: Some(snapshot_json),
            created_at: chrono::Utc::now().timestamp(),
        };
        db.insert_thought(&response_thought)?;
        thought_ids.push(response_thought_id);

        // 9. Extract and record decisions
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
    })
}
