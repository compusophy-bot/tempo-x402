//! Tool definitions and executor for the soul's function calling capabilities.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::coding;
use crate::db::{Mutation, SoulDatabase};
use crate::git::GitContext;
use crate::guard;
use crate::persistent_memory;
use crate::tool_registry::ToolRegistry;

mod beliefs;
mod cartridges;
mod deployment;
mod endpoints;
mod file_ops;
mod git;
mod memory;
mod planning;
mod shell;
mod social;

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
    pub(super) timeout_secs: u64,
    pub(super) workspace_root: PathBuf,
    pub(super) git: Option<Arc<GitContext>>,
    pub(super) db: Option<Arc<SoulDatabase>>,
    pub(super) coding_enabled: bool,
    pub(super) registry: Option<ToolRegistry>,
    pub(super) memory_file_path: String,
    pub(super) gateway_url: Option<String>,
    pub(super) railway_token: Option<String>,
    pub(super) railway_service_id: Option<String>,
    pub(super) railway_environment_id: Option<String>,
    /// Cartridge engine for cognitive cartridge execution (Phase 4).
    pub(super) cartridge_engine: Option<std::sync::Arc<x402_cartridge::CartridgeEngine>>,
}

/// Max output size per stream (stdout/stderr) to stay within LLM context limits.
pub(super) const MAX_OUTPUT_BYTES: usize = 4096;

/// Strip control characters from a string before JSON parsing.
/// Keeps \n, \r, \t which are valid in JSON strings but removes all other
/// control chars (0x00-0x1F, 0x7F) that cause serde_json parse failures.
pub(super) fn sanitize_json_body(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}

/// Max file size for read_file (256KB — large enough for even thinking.rs at 85KB).
pub(super) const MAX_READ_BYTES: usize = 262144;

/// Max entries for list_directory.
pub(super) const MAX_DIR_ENTRIES: usize = 200;

/// Max matches for search_files.
pub(super) const MAX_SEARCH_MATCHES: usize = 50;

/// Hard cap for execute_shell timeout.
pub(super) const SHELL_TIMEOUT_CAP: u64 = 300;

impl ToolExecutor {
    /// Create a new tool executor with the given per-command timeout and workspace root.
    pub fn new(timeout_secs: u64, workspace_root: String) -> Self {
        Self {
            timeout_secs,
            workspace_root: PathBuf::from(workspace_root),
            git: None,
            db: None,
            coding_enabled: false,
            registry: None,
            memory_file_path: "/data/soul_memory.md".to_string(),
            gateway_url: None,
            railway_token: std::env::var("RAILWAY_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            railway_service_id: std::env::var("RAILWAY_SERVICE_ID")
                .ok()
                .filter(|s| !s.is_empty()),
            railway_environment_id: std::env::var("RAILWAY_ENVIRONMENT_ID")
                .ok()
                .filter(|s| !s.is_empty()),
            cartridge_engine: None,
        }
    }

    /// Set the cartridge engine for cognitive cartridge execution.
    pub fn with_cartridge_engine(
        mut self,
        engine: std::sync::Arc<x402_cartridge::CartridgeEngine>,
    ) -> Self {
        self.cartridge_engine = Some(engine);
        self
    }

    /// Set the persistent memory file path.
    pub fn with_memory_file(mut self, path: String) -> Self {
        self.memory_file_path = path;
        self
    }

    /// Set the gateway URL for endpoint registration.
    pub fn with_gateway_url(mut self, url: Option<String>) -> Self {
        self.gateway_url = url;
        self
    }

    /// Attach the soul database (needed for update_beliefs in all modes).
    pub fn with_database(mut self, db: Arc<SoulDatabase>) -> Self {
        self.db = Some(db);
        self
    }

    /// Enable coding capabilities with git context and database.
    pub fn with_coding(mut self, git: Arc<GitContext>, db: Arc<SoulDatabase>) -> Self {
        self.git = Some(git);
        self.db = Some(db);
        self.coding_enabled = true;
        self
    }

    /// Attach a dynamic tool registry.
    pub fn with_registry(mut self, registry: ToolRegistry) -> Self {
        self.registry = Some(registry);
        self
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
                    .min(SHELL_TIMEOUT_CAP);

                self.execute_shell(command, timeout).await
            }
            "read_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'path' argument".to_string())?;
                let offset = args
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                self.read_file(path, offset, limit).await
            }
            "write_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'path' argument".to_string())?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'content' argument".to_string())?;
                self.write_file(path, content).await
            }
            "edit_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'path' argument".to_string())?;
                let old_string = args
                    .get("old_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'old_string' argument".to_string())?;
                let new_string = args
                    .get("new_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'new_string' argument".to_string())?;
                self.edit_file(path, old_string, new_string).await
            }
            "list_directory" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                self.list_directory(path).await
            }
            "search_files" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'pattern' argument".to_string())?;
                let path = args.get("path").and_then(|v| v.as_str());
                let glob = args.get("glob").and_then(|v| v.as_str());
                self.search_files(pattern, path, glob).await
            }
            "commit_changes" => {
                // Track commits for fitness evolution score
                if let Some(ref db) = self.db {
                    let total_commits: u64 = db
                        .get_state("total_commits")
                        .ok()
                        .flatten()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let _ = db.set_state("total_commits", &(total_commits + 1).to_string());
                }
                let message = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'message' argument".to_string())?;
                // If files not provided, auto-detect all changed files via git
                let file_strs: Vec<String> =
                    if let Some(files) = args.get("files").and_then(|v| v.as_array()) {
                        files
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    } else {
                        // Auto-detect: git diff --name-only + git ls-files --others --exclude-standard
                        let ws = self.workspace_root.to_string_lossy();
                        let modified = tokio::process::Command::new("git")
                            .args(["diff", "--name-only"])
                            .current_dir(&*ws)
                            .output()
                            .await
                            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                            .unwrap_or_default();
                        let untracked = tokio::process::Command::new("git")
                            .args(["ls-files", "--others", "--exclude-standard"])
                            .current_dir(&*ws)
                            .output()
                            .await
                            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                            .unwrap_or_default();
                        modified
                            .lines()
                            .chain(untracked.lines())
                            .filter(|l| !l.is_empty())
                            .map(String::from)
                            .collect()
                    };
                if file_strs.is_empty() {
                    return Ok(ToolResult {
                        stdout: "nothing to commit — no changed files detected".to_string(),
                        stderr: String::new(),
                        exit_code: 0,
                        duration_ms: 0,
                    });
                }
                let refs: Vec<&str> = file_strs.iter().map(|s| s.as_str()).collect();
                self.commit_changes(message, &refs).await
            }
            "propose_to_main" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'title' argument".to_string())?;
                let body = args
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Automated PR from soul agent");
                self.propose_to_main(title, body).await
            }
            "create_issue" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'title' argument".to_string())?;
                let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
                let labels: Vec<&str> = args
                    .get("labels")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                self.create_issue(title, body, &labels).await
            }
            "update_memory" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'content' argument".to_string())?;
                self.update_memory(content).await
            }
            "check_self" => {
                let endpoint = args
                    .get("endpoint")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'endpoint' argument".to_string())?;
                self.check_self(endpoint).await
            }
            "delete_endpoint" => {
                let slug = args
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'slug' argument".to_string())?;
                self.delete_endpoint(slug).await
            }
            "register_endpoint" => {
                let slug = args
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'slug' argument".to_string())?;
                let target_url = args
                    .get("target_url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'target_url' argument".to_string())?;
                let price = args
                    .get("price")
                    .and_then(|v| v.as_str())
                    .unwrap_or("$0.01");
                let description = args.get("description").and_then(|v| v.as_str());
                self.register_endpoint(slug, target_url, price, description)
                    .await
            }
            "update_beliefs" => {
                let updates = args
                    .get("updates")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| "missing 'updates' argument (must be array)".to_string())?;
                self.update_beliefs(updates).await
            }
            "approve_plan" => {
                let plan_id = args
                    .get("plan_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'plan_id' argument".to_string())?;
                self.approve_plan(plan_id).await
            }
            "reject_plan" => {
                let plan_id = args
                    .get("plan_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'plan_id' argument".to_string())?;
                let reason = args.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                self.reject_plan(plan_id, reason).await
            }
            "request_plan" => {
                let description = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'description' argument".to_string())?;
                let priority = args.get("priority").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
                self.request_plan(description, priority).await
            }
            "create_script_endpoint" => {
                let slug = args
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'slug' argument".to_string())?;
                let script = args
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'script' argument".to_string())?;
                let description = args.get("description").and_then(|v| v.as_str());
                self.create_script_endpoint(slug, script, description).await
            }
            "discover_peers" => self.discover_peers().await,
            "clone_self" => self.clone_self().await,
            "spawn_specialist" => {
                let specialization = args
                    .get("specialization")
                    .and_then(|v| v.as_str())
                    .unwrap_or("generalist")
                    .to_string();
                let initial_goal = args
                    .get("initial_goal")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                self.spawn_specialist(&specialization, initial_goal.as_deref())
                    .await
            }
            "delegate_task" => {
                let target = args
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let task_desc = args
                    .get("task_description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let priority = args.get("priority").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
                self.delegate_task(&target, &task_desc, priority).await
            }
            "call_paid_endpoint" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'url' argument".to_string())?;
                let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
                let body = args.get("body").and_then(|v| v.as_str());
                // Track peer call attempts for fitness scoring
                if let Some(ref db) = self.db {
                    let attempted: u64 = db
                        .get_state("peer_calls_attempted")
                        .ok()
                        .flatten()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let _ = db.set_state("peer_calls_attempted", &(attempted + 1).to_string());
                }
                let result = self.call_paid_endpoint(url, method, body).await;
                if let Ok(ref r) = result {
                    // Accept any 2xx status or exit_code 0 as success
                    let is_success = r.exit_code == 0 || (200..300).contains(&r.exit_code);
                    if is_success {
                        if let Some(ref db) = self.db {
                            let succeeded: u64 = db
                                .get_state("peer_calls_succeeded")
                                .ok()
                                .flatten()
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            let _ =
                                db.set_state("peer_calls_succeeded", &(succeeded + 1).to_string());
                        }
                    }
                }
                result
            }
            "check_reputation" => self.check_reputation().await,
            "update_agent_metadata" => {
                let uri = args
                    .get("metadata_uri")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'metadata_uri' argument".to_string())?;
                self.update_agent_metadata(uri).await
            }
            "check_deploy_status" => self.check_deploy_status().await,
            "get_deploy_logs" => {
                let deployment_id = args.get("deployment_id").and_then(|v| v.as_str());
                self.get_deploy_logs(deployment_id).await
            }
            "trigger_redeploy" => self.trigger_redeploy().await,
            "create_github_repo" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'name' argument".to_string())?;
                let description = args.get("description").and_then(|v| v.as_str());
                let private = args
                    .get("private")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.create_github_repo(name, description, private).await
            }
            "fork_github_repo" => {
                let owner = args
                    .get("owner")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'owner' argument".to_string())?;
                let repo = args
                    .get("repo")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'repo' argument".to_string())?;
                self.fork_github_repo(owner, repo).await
            }
            "list_script_endpoints" => self.list_script_endpoints().await,
            "test_script_endpoint" => {
                let slug = args
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'slug' argument".to_string())?;
                let input = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
                self.test_script_endpoint(slug, input).await
            }
            // ── WASM Cartridge tools ──
            "create_cartridge" => {
                let slug = args
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'slug' argument".to_string())?;
                let source_code = args.get("source_code").and_then(|v| v.as_str());
                let description = args.get("description").and_then(|v| v.as_str());
                let interactive = args.get("interactive").and_then(|v| v.as_bool()).unwrap_or(false);
                let frontend = args.get("frontend").and_then(|v| v.as_bool()).unwrap_or(false);
                self.create_cartridge(slug, source_code, description, interactive, frontend).await
            }
            "compile_cartridge" => {
                let slug = args
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'slug' argument".to_string())?;
                self.compile_cartridge(slug).await
            }
            "test_cartridge" => {
                let slug = args
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'slug' argument".to_string())?;
                let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
                self.test_cartridge(slug, method, path, body).await
            }
            "list_cartridges" => self.list_cartridges().await,
            "screenshot" => {
                let executor = crate::computer_use::ComputerExecutor::new(
                    std::path::PathBuf::from("/tmp/screenshots"),
                );
                if !executor.is_available() {
                    return Ok(ToolResult {
                        stdout: "No display available — computer use requires DISPLAY env var"
                            .into(),
                        stderr: String::new(),
                        exit_code: 1,
                        duration_ms: 0,
                    });
                }
                let action = crate::computer_use::ComputerAction::Screenshot { region: None };
                let result = executor.execute(&action).await;
                Ok(ToolResult {
                    stdout: if let Some(ss) = &result.screenshot {
                        format!(
                            "Screenshot captured: {}x{} at {}",
                            ss.width, ss.height, ss.path
                        )
                    } else {
                        result
                            .error
                            .clone()
                            .unwrap_or_else(|| "Screenshot failed".into())
                    },
                    stderr: result.error.unwrap_or_default(),
                    exit_code: if result.success { 0 } else { 1 },
                    duration_ms: result.duration_ms,
                })
            }
            "mouse_click" => {
                let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let executor = crate::computer_use::ComputerExecutor::new(
                    std::path::PathBuf::from("/tmp/screenshots"),
                );
                let action = crate::computer_use::ComputerAction::MouseClick {
                    point: crate::computer_use::Point { x, y },
                    button: crate::computer_use::MouseButton::Left,
                    double: args
                        .get("double")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                };
                let result = executor.execute(&action).await;
                Ok(ToolResult {
                    stdout: if result.success {
                        format!("Clicked at ({x}, {y})")
                    } else {
                        String::new()
                    },
                    stderr: result.error.unwrap_or_default(),
                    exit_code: if result.success { 0 } else { 1 },
                    duration_ms: result.duration_ms,
                })
            }
            "type_text" => {
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'text' argument".to_string())?;
                let executor = crate::computer_use::ComputerExecutor::new(
                    std::path::PathBuf::from("/tmp/screenshots"),
                );
                let action = crate::computer_use::ComputerAction::TypeText {
                    text: text.to_string(),
                };
                let result = executor.execute(&action).await;
                Ok(ToolResult {
                    stdout: if result.success {
                        format!("Typed: {}", text.chars().take(50).collect::<String>())
                    } else {
                        String::new()
                    },
                    stderr: result.error.unwrap_or_default(),
                    exit_code: if result.success { 0 } else { 1 },
                    duration_ms: result.duration_ms,
                })
            }
            "key_press" => {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'key' argument".to_string())?;
                let executor = crate::computer_use::ComputerExecutor::new(
                    std::path::PathBuf::from("/tmp/screenshots"),
                );
                let action = crate::computer_use::ComputerAction::KeyPress {
                    key: key.to_string(),
                    modifiers: vec![],
                };
                let result = executor.execute(&action).await;
                Ok(ToolResult {
                    stdout: if result.success {
                        format!("Pressed: {key}")
                    } else {
                        String::new()
                    },
                    stderr: result.error.unwrap_or_default(),
                    exit_code: if result.success { 0 } else { 1 },
                    duration_ms: result.duration_ms,
                })
            }
            "open_url" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'url' argument".to_string())?;
                let executor = crate::computer_use::ComputerExecutor::new(
                    std::path::PathBuf::from("/tmp/screenshots"),
                );
                let action = crate::computer_use::ComputerAction::OpenUrl {
                    url: url.to_string(),
                };
                let result = executor.execute(&action).await;
                Ok(ToolResult {
                    stdout: if result.success {
                        format!("Opened URL: {url}")
                    } else {
                        String::new()
                    },
                    stderr: result.error.unwrap_or_default(),
                    exit_code: if result.success { 0 } else { 1 },
                    duration_ms: result.duration_ms,
                })
            }
            "brain_predict" => {
                let step_type = args
                    .get("step_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let brain =
                    crate::brain::load_brain(self.db.as_ref().ok_or("brain requires database")?);
                let features = vec![0.0f32; 32]; // minimal features
                let prediction = brain.predict(&features);
                Ok(ToolResult {
                    stdout: format!(
                        "Brain prediction for '{}': success_prob={:.1}%, likely_error={:?}",
                        step_type,
                        prediction.success_prob * 100.0,
                        prediction.likely_error,
                    ),
                    stderr: String::new(),
                    exit_code: 0,
                    duration_ms: 0,
                })
            }
            _ => {
                // Check meta-tools and dynamic tools via registry
                if let Some(ref registry) = self.registry {
                    if ToolRegistry::is_meta_tool(name) {
                        return registry.execute_meta_tool(name, args).await;
                    }
                    if registry.is_dynamic_tool(name) {
                        return registry.execute_dynamic_tool(name, args).await;
                    }
                }
                Err(format!("unknown tool: {name}"))
            }
        }
    }

    /// Resolve a path relative to workspace root, preventing traversal outside it.
    fn resolve_path(&self, path: &str) -> Result<PathBuf, String> {
        let candidate = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.workspace_root.join(path)
        };

        // Canonicalize what exists; for new files, walk up to find the nearest
        // existing ancestor and resolve relative to it.
        let resolved = if candidate.exists() {
            candidate
                .canonicalize()
                .map_err(|e| format!("failed to resolve path: {e}"))?
        } else {
            let filename = candidate
                .file_name()
                .ok_or_else(|| "invalid path: no filename".to_string())?;

            // Walk up to find the nearest existing ancestor for canonicalization.
            // This allows writing to not-yet-created directories (write_file creates them).
            let mut ancestor = candidate.parent().map(PathBuf::from);
            let mut tail_segments: Vec<std::ffi::OsString> = vec![filename.to_os_string()];
            while let Some(ref a) = ancestor {
                if a.exists() {
                    break;
                }
                if let Some(seg) = a.file_name() {
                    tail_segments.push(seg.to_os_string());
                }
                ancestor = a.parent().map(PathBuf::from);
            }

            let base = match ancestor {
                Some(a) if a.exists() => a
                    .canonicalize()
                    .map_err(|e| format!("failed to resolve ancestor: {e}"))?,
                _ => self.workspace_root.clone(),
            };

            // Rebuild the path from the canonicalized base + non-existent segments
            tail_segments.reverse();
            let mut result = base;
            for seg in tail_segments {
                result = result.join(seg);
            }
            result
        };

        Ok(resolved)
    }

    /// Get the relative path from workspace root for guard checking.
    fn relative_path(&self, resolved: &Path) -> String {
        resolved
            .strip_prefix(&self.workspace_root)
            .unwrap_or(resolved)
            .to_string_lossy()
            .to_string()
    }
}

// Tool declaration functions have been moved to `crate::tool_decl`.
// Re-export them here for backward compatibility.
pub use crate::tool_decl::*;

/// Truncate raw output bytes to a UTF-8 string, capping at MAX_OUTPUT_BYTES.
pub(super) fn truncate_output(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    if s.len() <= MAX_OUTPUT_BYTES {
        s.into_owned()
    } else {
        let truncated: String = s.chars().take(MAX_OUTPUT_BYTES).collect();
        format!("{truncated}\n... (truncated)")
    }
}

pub(super) fn parse_payment_requirements(
    value: &serde_json::Value,
) -> Result<x402::wallet::PaymentRequirements, String> {
    // Try wallet format first (camelCase fields, String types)
    if let Ok(r) = serde_json::from_value::<x402::wallet::PaymentRequirements>(value.clone()) {
        return Ok(r);
    }
    // Fall back to server format (snake_case fields, Address types) and convert
    let server_req: x402::payment::PaymentRequirements = serde_json::from_value(value.clone())
        .map_err(|e| format!("failed to parse PaymentRequirements: {e}"))?;
    Ok(x402::wallet::PaymentRequirements {
        scheme: server_req.scheme,
        network: server_req.network,
        price: server_req.price,
        asset: format!("{}", server_req.asset),
        amount: server_req.amount,
        pay_to: format!("{}", server_req.pay_to),
        max_timeout_seconds: server_req.max_timeout_seconds,
        description: server_req.description,
    })
}
