//! The thinking loop: periodic observe → think → record cycle with tool execution.

use std::sync::Arc;

use serde::Serialize;

use crate::config::SoulConfig;
use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::llm::{
    ConversationMessage, ConversationPart, FunctionDeclaration, FunctionResponse, LlmClient,
    LlmResult,
};
use crate::memory::{Thought, ThoughtType};
use crate::observer::{NodeObserver, NodeSnapshot};
use crate::tools::{self, ToolExecutor};

/// The thinking loop that drives the soul.
pub struct ThinkingLoop {
    config: SoulConfig,
    db: Arc<SoulDatabase>,
    llm: Option<LlmClient>,
    observer: Arc<dyn NodeObserver>,
    tool_executor: ToolExecutor,
}

impl ThinkingLoop {
    /// Create a new thinking loop.
    pub fn new(config: SoulConfig, db: Arc<SoulDatabase>, observer: Arc<dyn NodeObserver>) -> Self {
        let llm = config.llm_api_key.as_ref().map(|key| {
            LlmClient::new(
                key.clone(),
                config.llm_model_fast.clone(),
                config.llm_model_think.clone(),
            )
        });

        let tool_executor = ToolExecutor::new(config.tool_timeout_secs);

        Self {
            config,
            db,
            llm,
            observer,
            tool_executor,
        }
    }

    /// Run the thinking loop forever at a fixed interval.
    pub async fn run(&self) {
        let interval = std::time::Duration::from_secs(self.config.think_interval_secs);

        tracing::info!(
            interval_secs = self.config.think_interval_secs,
            dormant = self.llm.is_none(),
            tools_enabled = self.config.tools_enabled,
            "Soul thinking loop started"
        );

        loop {
            let snapshot = match self.observer.observe() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "Soul observe failed");
                    tokio::time::sleep(interval).await;
                    continue;
                }
            };

            if let Err(e) = self.think_cycle_with_snapshot(&snapshot).await {
                tracing::warn!(error = %e, "Soul think cycle failed");
            }

            tokio::time::sleep(interval).await;
        }
    }

    /// Execute one think cycle with a pre-captured snapshot.
    async fn think_cycle_with_snapshot(&self, snapshot: &NodeSnapshot) -> Result<(), SoulError> {
        let snapshot_json = serde_json::to_string(snapshot)?;

        // Record observation
        let obs_thought = Thought {
            id: uuid::Uuid::new_v4().to_string(),
            thought_type: ThoughtType::Observation,
            content: format!(
                "Uptime: {}s, Endpoints: {}, Revenue: {}, Payments: {}, Children: {}",
                snapshot.uptime_secs,
                snapshot.endpoint_count,
                snapshot.total_revenue,
                snapshot.total_payments,
                snapshot.children_count,
            ),
            context: Some(snapshot_json.clone()),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.db.insert_thought(&obs_thought)?;

        // If dormant (no API key), stop here
        let llm = match &self.llm {
            Some(g) => g,
            None => {
                tracing::debug!("Soul dormant — observation recorded, skipping LLM");
                self.increment_cycle_count()?;
                return Ok(());
            }
        };

        // Build prompt from snapshot + recent thoughts
        let recent = self.db.recent_thoughts(5)?;
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

        let user_prompt = format!(
            "Current node state:\n{}\n\nRecent thoughts:\n{}\n\n\
             Analyze the node's current state briefly. Note any concerns or opportunities. \
             If you want to inspect something, use the execute_shell tool. \
             If you have a new recommendation (not already in recent thoughts), prefix it with [DECISION]. \
             Do NOT repeat previous decisions. Keep your response under 200 words.",
            snapshot_json,
            recent_summary.join("\n")
        );

        let system_prompt = format!(
            "{}\n\nYou are generation {} in the node lineage.{}",
            self.config.personality,
            self.config.generation,
            self.config
                .parent_id
                .as_ref()
                .map(|p| format!(" Your parent is {p}."))
                .unwrap_or_default()
        );

        // Determine if we should use tools
        let use_tools = self.config.tools_enabled && self.config.llm_api_key.is_some();
        let tool_declarations = if use_tools {
            tools::available_tools()
        } else {
            vec![]
        };

        // Build initial conversation
        let mut conversation = vec![ConversationMessage {
            role: "user".to_string(),
            parts: vec![ConversationPart::Text(user_prompt)],
        }];

        // Agentic tool loop
        let result = run_tool_loop(
            llm,
            &system_prompt,
            &mut conversation,
            &tool_declarations,
            &self.tool_executor,
            &self.db,
            self.config.max_tool_calls,
        )
        .await?;

        let final_text = result.text;
        let tool_calls_made = result.tool_executions.len() as u32;

        // Record reasoning (final text response)
        if !final_text.is_empty() {
            let reasoning = Thought {
                id: uuid::Uuid::new_v4().to_string(),
                thought_type: ThoughtType::Reasoning,
                content: final_text.clone(),
                context: Some(snapshot_json),
                created_at: chrono::Utc::now().timestamp(),
            };
            self.db.insert_thought(&reasoning)?;

            // Extract and record decisions (lines starting with [DECISION])
            for line in final_text.lines() {
                let trimmed = line.trim();
                if let Some(decision_text) = trimmed.strip_prefix("[DECISION]") {
                    let decision = Thought {
                        id: uuid::Uuid::new_v4().to_string(),
                        thought_type: ThoughtType::Decision,
                        content: decision_text.trim().to_string(),
                        context: None,
                        created_at: chrono::Utc::now().timestamp(),
                    };
                    self.db.insert_thought(&decision)?;
                    tracing::info!(decision = decision_text.trim(), "Soul decision recorded");
                }
            }
        }

        // Update state
        self.increment_cycle_count()?;

        if tool_calls_made > 0 {
            tracing::info!(
                tool_calls = tool_calls_made,
                "Soul cycle complete with tool use"
            );
        }

        Ok(())
    }

    /// Increment the total_think_cycles counter and update last_think_at.
    fn increment_cycle_count(&self) -> Result<(), SoulError> {
        let current: u64 = self
            .db
            .get_state("total_think_cycles")?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        self.db
            .set_state("total_think_cycles", &(current + 1).to_string())?;
        self.db
            .set_state("last_think_at", &chrono::Utc::now().timestamp().to_string())?;
        Ok(())
    }
}

/// A single tool execution record.
#[derive(Debug, Clone, Serialize)]
pub struct ToolExecution {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

/// Result of running the agentic tool loop.
#[derive(Debug)]
pub struct ToolLoopResult {
    pub text: String,
    pub tool_executions: Vec<ToolExecution>,
}

/// Run the agentic tool loop: repeatedly call the LLM, execute any tool calls,
/// and return the final text response plus a log of all tool executions.
pub(crate) async fn run_tool_loop(
    llm: &LlmClient,
    system_prompt: &str,
    conversation: &mut Vec<ConversationMessage>,
    tool_declarations: &[FunctionDeclaration],
    tool_executor: &ToolExecutor,
    db: &Arc<SoulDatabase>,
    max_tool_calls: u32,
) -> Result<ToolLoopResult, SoulError> {
    let mut tool_calls_made = 0u32;
    let mut final_text = String::new();
    let mut tool_executions = Vec::new();

    for _ in 0..=max_tool_calls {
        let result = llm
            .think_with_tools(system_prompt, conversation, tool_declarations)
            .await?;

        match result {
            LlmResult::Text(text) => {
                final_text = text;
                break;
            }
            LlmResult::FunctionCall(fc) => {
                if tool_calls_made >= max_tool_calls {
                    tracing::warn!("Soul hit max tool calls ({max_tool_calls}), stopping");
                    break;
                }

                tracing::info!(
                    tool = %fc.name,
                    args = %fc.args,
                    "Soul executing tool"
                );

                // Execute the tool
                let tool_result = match tool_executor.execute(&fc.name, &fc.args).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(error = %e, "Tool execution error");
                        tools::ToolResult {
                            stdout: String::new(),
                            stderr: e,
                            exit_code: -1,
                            duration_ms: 0,
                        }
                    }
                };

                // Record tool execution as a thought
                let command_str = fc
                    .args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let tool_thought = Thought {
                    id: uuid::Uuid::new_v4().to_string(),
                    thought_type: ThoughtType::ToolExecution,
                    content: command_str.to_string(),
                    context: Some(serde_json::to_string(&tool_result).unwrap_or_default()),
                    created_at: chrono::Utc::now().timestamp(),
                };
                db.insert_thought(&tool_thought)?;

                // Record for return value
                tool_executions.push(ToolExecution {
                    command: command_str.to_string(),
                    stdout: tool_result.stdout.clone(),
                    stderr: tool_result.stderr.clone(),
                    exit_code: tool_result.exit_code,
                    duration_ms: tool_result.duration_ms,
                });

                // Append model's function call to conversation
                conversation.push(ConversationMessage {
                    role: "model".to_string(),
                    parts: vec![ConversationPart::FunctionCall(fc.clone())],
                });

                // Append function response to conversation
                let response_value = serde_json::to_value(&tool_result).unwrap_or_default();
                conversation.push(ConversationMessage {
                    role: "user".to_string(),
                    parts: vec![ConversationPart::FunctionResponse(FunctionResponse {
                        name: fc.name,
                        response: response_value,
                    })],
                });

                tool_calls_made += 1;
            }
        }
    }

    Ok(ToolLoopResult {
        text: final_text,
        tool_executions,
    })
}
