//! Tool definitions and executor for the soul's function calling capabilities.

use serde::{Deserialize, Serialize};

use crate::llm::FunctionDeclaration;

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

/// Executes tools requested by the LLM.
pub struct ToolExecutor {
    timeout_secs: u64,
}

/// Max output size per stream (stdout/stderr) to stay within LLM context limits.
const MAX_OUTPUT_BYTES: usize = 4096;

impl ToolExecutor {
    /// Create a new tool executor with the given per-command timeout.
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    /// Execute a tool by name with the given arguments.
    pub async fn execute(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<ToolResult, String> {
        match name {
            "execute_shell" => {
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'command' argument".to_string())?;

                let timeout = args
                    .get("timeout_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(self.timeout_secs)
                    .min(120); // hard cap at 120s

                self.execute_shell(command, timeout).await
            }
            _ => Err(format!("unknown tool: {name}")),
        }
    }

    /// Execute a shell command with timeout and output truncation.
    async fn execute_shell(&self, command: &str, timeout_secs: u64) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = truncate_output(&output.stdout);
                let stderr = truncate_output(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                Ok(ToolResult {
                    stdout,
                    stderr,
                    exit_code,
                    duration_ms,
                })
            }
            Ok(Err(e)) => Err(format!("command failed to execute: {e}")),
            Err(_) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("command timed out after {timeout_secs}s"),
                exit_code: -1,
                duration_ms,
            }),
        }
    }
}

/// Return the list of function declarations for the LLM's tools parameter.
pub fn available_tools() -> Vec<FunctionDeclaration> {
    vec![FunctionDeclaration {
        name: "execute_shell".to_string(),
        description: "Execute a shell command in the node's container. Use for inspection (env, ls, curl, df) not destruction.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Max seconds to wait (default 30, max 120)"
                }
            },
            "required": ["command"]
        }),
    }]
}

/// Truncate raw output bytes to a UTF-8 string, capping at MAX_OUTPUT_BYTES.
fn truncate_output(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    if s.len() <= MAX_OUTPUT_BYTES {
        s.into_owned()
    } else {
        let truncated: String = s.chars().take(MAX_OUTPUT_BYTES).collect();
        format!("{truncated}\n... (truncated)")
    }
}
