//! Agentic tool loop: repeatedly call the LLM, execute tool calls, return final text.
use std::sync::Arc;

use serde::Serialize;

use crate::db::SoulDatabase;
use crate::error::SoulError;
use crate::llm::{
    ConversationMessage, ConversationPart, FunctionDeclaration, FunctionResponse, LlmClient,
    LlmResult,
};
use crate::tools::{self, ToolExecutor};

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
/// When `use_deep` is true, uses the deeper/think model (e.g. Gemini Pro).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_tool_loop_with_model(
    llm: &LlmClient,
    system_prompt: &str,
    conversation: &mut Vec<ConversationMessage>,
    tool_declarations: &[FunctionDeclaration],
    tool_executor: &ToolExecutor,
    _db: &Arc<SoulDatabase>,
    max_tool_calls: u32,
    use_deep: bool,
) -> Result<ToolLoopResult, SoulError> {
    let mut tool_calls_made = 0u32;
    let mut final_text = String::new();
    let mut tool_executions = Vec::new();

    for _ in 0..=max_tool_calls {
        // Hard timeout per LLM call to prevent infinite hangs
        let llm_result = if use_deep {
            tokio::time::timeout(
                std::time::Duration::from_secs(120),
                llm.think_deep_with_tools(system_prompt, conversation, tool_declarations),
            )
            .await
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(120),
                llm.think_with_tools(system_prompt, conversation, tool_declarations),
            )
            .await
        };
        let result = match llm_result {
            Ok(r) => r?,
            Err(_) => {
                tracing::warn!("LLM call timed out after 120s");
                break;
            }
        };

        match result {
            LlmResult::Text(text) => {
                final_text = text;
                break;
            }
            LlmResult::FunctionCall(fc) => {
                if tool_calls_made >= max_tool_calls {
                    tracing::warn!("Hit max tool calls ({max_tool_calls}), requesting summary");
                    // Give the LLM one final chance to summarize instead of hard-stopping
                    conversation.push(ConversationMessage {
                        role: "user".to_string(),
                        parts: vec![ConversationPart::Text(format!(
                            "You've used {tool_calls_made} tool calls (limit: {max_tool_calls}). \
                             Summarize your progress and provide your final answer now."
                        ))],
                    });
                    // One more LLM call to get the summary
                    let summary_result = tokio::time::timeout(
                        std::time::Duration::from_secs(60),
                        llm.think_with_tools(system_prompt, conversation, &[]),
                    )
                    .await;
                    if let Ok(Ok(LlmResult::Text(text))) = summary_result {
                        final_text = text;
                    }
                    break;
                }

                tracing::info!(tool = %fc.name, args = %fc.args, "Executing tool");

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

                let tool_summary = summarize_tool_call(&fc.name, &fc.args);
                tool_executions.push(ToolExecution {
                    command: tool_summary,
                    stdout: tool_result.stdout.clone(),
                    stderr: tool_result.stderr.clone(),
                    exit_code: tool_result.exit_code,
                    duration_ms: tool_result.duration_ms,
                });

                conversation.push(ConversationMessage {
                    role: "model".to_string(),
                    parts: vec![ConversationPart::FunctionCall(fc.clone())],
                });

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

/// Create a human-readable summary of a tool call for logging.
fn summarize_tool_call(name: &str, args: &serde_json::Value) -> String {
    match name {
        "execute_shell" => args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("read_file: {path}")
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("write_file: {path}")
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("edit_file: {path}")
        }
        "list_directory" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("list_directory: {path}")
        }
        "search_files" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            format!("search_files: {pattern}")
        }
        "check_self" => {
            let endpoint = args.get("endpoint").and_then(|v| v.as_str()).unwrap_or("?");
            format!("check_self: /{endpoint}")
        }
        "update_memory" => "update_memory".to_string(),
        "register_endpoint" => {
            let slug = args.get("slug").and_then(|v| v.as_str()).unwrap_or("?");
            format!("register_endpoint: /{slug}")
        }
        _ => format!("{name}: {args}"),
    }
}
