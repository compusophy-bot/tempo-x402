//! Dynamic tool registry — register, list, unregister, and execute tools at runtime.
//!
//! Tools are persisted in the soul's SQLite database and executed via shell.
//! Three meta-tools (`register_tool`, `list_tools`, `unregister_tool`) let the
//! soul manage its own toolset.

use std::path::PathBuf;
use std::sync::Arc;

use crate::db::{DynamicTool, SoulDatabase};
use crate::llm::FunctionDeclaration;
use crate::tools::ToolResult;

/// Maximum number of dynamic tools that can be registered.
const MAX_DYNAMIC_TOOLS: u32 = 20;

/// Max output bytes per stream (matches tools.rs).
const MAX_OUTPUT_BYTES: usize = 16384;

/// The dynamic tool registry.
pub struct ToolRegistry {
    db: Arc<SoulDatabase>,
    workspace_root: PathBuf,
    timeout_secs: u64,
}

impl ToolRegistry {
    /// Create a new tool registry backed by the soul database.
    pub fn new(db: Arc<SoulDatabase>, workspace_root: String, timeout_secs: u64) -> Self {
        Self {
            db,
            workspace_root: PathBuf::from(workspace_root),
            timeout_secs,
        }
    }

    /// Names of the three meta-tools.
    pub const META_TOOL_NAMES: [&'static str; 3] =
        ["register_tool", "list_tools", "unregister_tool"];

    /// Check if a tool name is a meta-tool.
    pub fn is_meta_tool(name: &str) -> bool {
        Self::META_TOOL_NAMES.contains(&name)
    }

    /// Check if a tool name is a registered dynamic tool.
    pub fn is_dynamic_tool(&self, name: &str) -> bool {
        self.db.get_tool(name).ok().flatten().is_some()
    }

    /// Execute a meta-tool by name.
    pub async fn execute_meta_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<ToolResult, String> {
        match name {
            "register_tool" => self.handle_register(args).await,
            "list_tools" => self.handle_list().await,
            "unregister_tool" => self.handle_unregister(args).await,
            _ => Err(format!("not a meta-tool: {name}")),
        }
    }

    /// Execute a dynamic tool by name.
    pub async fn execute_dynamic_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<ToolResult, String> {
        let tool = self
            .db
            .get_tool(name)
            .map_err(|e| format!("db error: {e}"))?
            .ok_or_else(|| format!("dynamic tool not found: {name}"))?;

        if !tool.enabled {
            return Err(format!("tool '{name}' is disabled"));
        }

        self.execute_tool_handler(&tool, args).await
    }

    /// Get function declarations for all enabled dynamic tools, filtered by mode tag.
    pub fn dynamic_tool_declarations(&self, mode_tag: &str) -> Vec<FunctionDeclaration> {
        let tools = match self.db.list_tools(true) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load dynamic tools");
                return vec![];
            }
        };

        tools
            .into_iter()
            .filter(|t| {
                // Parse mode_tags JSON array and check if it contains the mode
                serde_json::from_str::<Vec<String>>(&t.mode_tags)
                    .unwrap_or_default()
                    .iter()
                    .any(|tag| tag == mode_tag)
            })
            .map(|t| {
                let parameters: serde_json::Value =
                    serde_json::from_str(&t.parameters).unwrap_or_default();
                FunctionDeclaration {
                    name: t.name,
                    description: t.description,
                    parameters,
                }
            })
            .collect()
    }

    /// Get function declarations for the three meta-tools.
    pub fn meta_tool_declarations() -> Vec<FunctionDeclaration> {
        vec![
            FunctionDeclaration {
                name: "register_tool".to_string(),
                description: "Register a new dynamic tool. The tool will be executable via shell. Max 20 dynamic tools.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Tool name (alphanumeric + underscores, no spaces)"
                        },
                        "description": {
                            "type": "string",
                            "description": "What the tool does (shown to the LLM)"
                        },
                        "parameters": {
                            "type": "object",
                            "description": "JSON Schema for the tool's parameters"
                        },
                        "handler_type": {
                            "type": "string",
                            "enum": ["shell_command", "shell_script"],
                            "description": "shell_command: inline command; shell_script: path to script in /data/tools/"
                        },
                        "handler_config": {
                            "type": "string",
                            "description": "The shell command or script filename"
                        },
                        "mode_tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Modes where this tool is available (default: [\"code\",\"chat\"])"
                        }
                    },
                    "required": ["name", "description", "handler_type", "handler_config"]
                }),
            },
            FunctionDeclaration {
                name: "list_tools".to_string(),
                description: "List all registered dynamic tools with their status and configuration.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            FunctionDeclaration {
                name: "unregister_tool".to_string(),
                description: "Remove a dynamic tool by name.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Name of the tool to remove"
                        }
                    },
                    "required": ["name"]
                }),
            },
        ]
    }

    // ── Internal handlers ────────────────────────────────────────────────

    async fn handle_register(&self, args: &serde_json::Value) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'name' argument".to_string())?;

        // Validate name: alphanumeric + underscores only
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.is_empty() {
            return Err(
                "tool name must be non-empty and contain only alphanumeric characters and underscores"
                    .to_string(),
            );
        }

        // Prevent collision with static tools
        let static_names = [
            "execute_shell",
            "read_file",
            "write_file",
            "edit_file",
            "list_directory",
            "search_files",
            "commit_changes",
            "propose_to_main",
        ];
        if static_names.contains(&name) || Self::META_TOOL_NAMES.contains(&name) {
            return Err(format!("cannot register tool with reserved name: {name}"));
        }

        // Check count limit
        let count = self
            .db
            .count_tools()
            .map_err(|e| format!("db error: {e}"))?;
        if count >= MAX_DYNAMIC_TOOLS && self.db.get_tool(name).ok().flatten().is_none() {
            return Err(format!(
                "max dynamic tools reached ({MAX_DYNAMIC_TOOLS}). Unregister one first."
            ));
        }

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'description' argument".to_string())?;

        let handler_type = args
            .get("handler_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'handler_type' argument".to_string())?;

        if handler_type != "shell_command" && handler_type != "shell_script" {
            return Err("handler_type must be 'shell_command' or 'shell_script'".to_string());
        }

        let handler_config = args
            .get("handler_config")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'handler_config' argument".to_string())?;

        let parameters = args
            .get("parameters")
            .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| r#"{"type":"object","properties":{},"required":[]}"#.to_string());

        let mode_tags = args
            .get("mode_tags")
            .map(|v| serde_json::to_string(v).unwrap_or_else(|_| r#"["code","chat"]"#.to_string()))
            .unwrap_or_else(|| r#"["code","chat"]"#.to_string());

        let now = chrono::Utc::now().timestamp();
        let tool = DynamicTool {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
            handler_type: handler_type.to_string(),
            handler_config: handler_config.to_string(),
            enabled: true,
            mode_tags,
            created_at: now,
            updated_at: now,
        };

        self.db
            .insert_tool(&tool)
            .map_err(|e| format!("failed to register tool: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!("registered tool '{name}' (handler: {handler_type})"),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    async fn handle_list(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let tools = self
            .db
            .list_tools(false)
            .map_err(|e| format!("db error: {e}"))?;

        if tools.is_empty() {
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ToolResult {
                stdout: "no dynamic tools registered".to_string(),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            });
        }

        let mut output = String::new();
        for t in &tools {
            output.push_str(&format!(
                "- {} [{}] (handler: {}, enabled: {}, modes: {})\n  {}\n",
                t.name, t.handler_type, t.handler_config, t.enabled, t.mode_tags, t.description
            ));
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: output,
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    async fn handle_unregister(&self, args: &serde_json::Value) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'name' argument".to_string())?;

        let deleted = self
            .db
            .delete_tool(name)
            .map_err(|e| format!("db error: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        if deleted {
            Ok(ToolResult {
                stdout: format!("unregistered tool '{name}'"),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            Ok(ToolResult {
                stdout: format!("tool '{name}' not found"),
                stderr: String::new(),
                exit_code: 1,
                duration_ms,
            })
        }
    }

    /// Execute a dynamic tool's handler via shell.
    async fn execute_tool_handler(
        &self,
        tool: &DynamicTool,
        args: &serde_json::Value,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let command = match tool.handler_type.as_str() {
            "shell_command" => tool.handler_config.clone(),
            "shell_script" => {
                let script_dir = PathBuf::from("/data/tools");
                let script_path = script_dir.join(&tool.handler_config);
                format!("bash {}", script_path.display())
            }
            _ => return Err(format!("unknown handler_type: {}", tool.handler_type)),
        };

        // Build environment with tool args
        let args_json = serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string());
        let mut env_vars: Vec<(String, String)> = vec![("TOOL_ARGS".to_string(), args_json)];

        // Add individual parameters as TOOL_PARAM_{NAME}
        if let Some(obj) = args.as_object() {
            for (key, value) in obj {
                let env_key = format!("TOOL_PARAM_{}", key.to_uppercase());
                let env_val = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                env_vars.push((env_key, env_val));
            }
        }

        let timeout = std::time::Duration::from_secs(self.timeout_secs.min(300));

        let result = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(&command)
                .current_dir(&self.workspace_root)
                .envs(env_vars)
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
                stderr: format!("command timed out after {}s", self.timeout_secs.min(300)),
                exit_code: -1,
                duration_ms,
            }),
        }
    }
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
