//! Tool definitions and executor for the soul's function calling capabilities.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::coding;
use crate::db::{Mutation, SoulDatabase};
use crate::git::GitContext;
use crate::guard;
use crate::llm::FunctionDeclaration;
use crate::persistent_memory;
use crate::tool_registry::ToolRegistry;

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
    workspace_root: PathBuf,
    git: Option<Arc<GitContext>>,
    db: Option<Arc<SoulDatabase>>,
    coding_enabled: bool,
    registry: Option<ToolRegistry>,
    memory_file_path: String,
    gateway_url: Option<String>,
    railway_token: Option<String>,
    railway_service_id: Option<String>,
    railway_environment_id: Option<String>,
}

/// Max output size per stream (stdout/stderr) to stay within LLM context limits.
const MAX_OUTPUT_BYTES: usize = 4096;

/// Max file size for read_file (256KB — large enough for even thinking.rs at 85KB).
const MAX_READ_BYTES: usize = 262144;

/// Max entries for list_directory.
const MAX_DIR_ENTRIES: usize = 200;

/// Max matches for search_files.
const MAX_SEARCH_MATCHES: usize = 50;

/// Hard cap for execute_shell timeout.
const SHELL_TIMEOUT_CAP: u64 = 300;

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
        }
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

    /// Execute a shell command with timeout and output truncation.
    async fn execute_shell(&self, command: &str, timeout_secs: u64) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&self.workspace_root)
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

    /// Read a file with optional offset and limit (line-based).
    async fn read_file(
        &self,
        path: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let resolved = self.resolve_path(path)?;

        // Check file size first
        let metadata = tokio::fs::metadata(&resolved)
            .await
            .map_err(|e| format!("cannot read file: {e}"))?;

        if !metadata.is_file() {
            return Err(format!("not a file: {path}"));
        }

        if metadata.len() > MAX_READ_BYTES as u64 && offset.is_none() && limit.is_none() {
            return Err(format!(
                "file too large ({} bytes, max {}). Use offset/limit to read portions.",
                metadata.len(),
                MAX_READ_BYTES
            ));
        }

        let content = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| format!("failed to read file: {e}"))?;

        let lines: Vec<&str> = content.lines().collect();
        let start_line = offset.unwrap_or(0);
        let end_line = limit
            .map(|l| (start_line + l).min(lines.len()))
            .unwrap_or(lines.len());

        let mut output = String::new();
        for (i, line) in lines
            .iter()
            .enumerate()
            .skip(start_line)
            .take(end_line - start_line)
        {
            output.push_str(&format!("{:>6}\t{}\n", i + 1, line));
        }

        // Truncate if still too large (char-safe to avoid panicking on multi-byte boundaries)
        if output.len() > MAX_OUTPUT_BYTES {
            output = output.chars().take(MAX_OUTPUT_BYTES).collect();
            output.push_str("\n... (truncated)");
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: output,
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Write (create or overwrite) a file. Guard-checked.
    async fn write_file(&self, path: &str, content: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Guard check on the raw path (before resolving, to catch traversal)
        guard::validate_write_target(path).map_err(|e| e.to_string())?;

        let resolved = self.resolve_path(path)?;
        let rel = self.relative_path(&resolved);
        guard::validate_write_target(&rel).map_err(|e| e.to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create parent directory: {e}"))?;
        }

        tokio::fs::write(&resolved, content)
            .await
            .map_err(|e| format!("failed to write file: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!("wrote {} bytes to {path}", content.len()),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Edit a file via search-and-replace. The old_string must appear exactly once. Guard-checked.
    async fn edit_file(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Guard check
        guard::validate_write_target(path).map_err(|e| e.to_string())?;

        let resolved = self.resolve_path(path)?;
        let rel = self.relative_path(&resolved);
        guard::validate_write_target(&rel).map_err(|e| e.to_string())?;

        let content = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| format!("failed to read file: {e}"))?;

        let count = content.matches(old_string).count();
        if count == 0 {
            return Err("old_string not found in file".to_string());
        }
        if count > 1 {
            return Err(format!(
                "old_string found {count} times — must be unique. Provide more context."
            ));
        }

        let new_content = content.replacen(old_string, new_string, 1);
        tokio::fs::write(&resolved, &new_content)
            .await
            .map_err(|e| format!("failed to write file: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!("edited {path}: replaced 1 occurrence"),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// List directory entries with type indicators.
    async fn list_directory(&self, path: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let resolved = self.resolve_path(path)?;

        let metadata = tokio::fs::metadata(&resolved)
            .await
            .map_err(|e| format!("cannot access path: {e}"))?;

        if !metadata.is_dir() {
            return Err(format!("not a directory: {path}"));
        }

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&resolved)
            .await
            .map_err(|e| format!("failed to read directory: {e}"))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("failed to read entry: {e}"))?
        {
            if entries.len() >= MAX_DIR_ENTRIES {
                entries.push("... (truncated, too many entries)".to_string());
                break;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry.file_type().await;
            let indicator = match ft {
                Ok(ft) if ft.is_dir() => "/",
                Ok(ft) if ft.is_symlink() => "@",
                _ => "",
            };
            entries.push(format!("{name}{indicator}"));
        }

        entries.sort();

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: entries.join("\n"),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Search for a literal string pattern across files. Uses grep via shell internally for
    /// performance (avoids reimplementing recursive file walking + binary detection).
    async fn search_files(
        &self,
        pattern: &str,
        path: Option<&str>,
        glob: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let search_path = path.unwrap_or(".");

        // Build grep command with safe quoting
        let mut cmd = format!("grep -rn --max-count={} -l", MAX_SEARCH_MATCHES);

        if let Some(g) = glob {
            cmd.push_str(&format!(" --include='{}'", g.replace('\'', "'\\''")));
        }

        // Use fixed-string mode for literal search (no regex interpretation)
        cmd.push_str(&format!(
            " -F -- '{}' '{}'",
            pattern.replace('\'', "'\\''"),
            search_path.replace('\'', "'\\''")
        ));

        // Run via shell (in workspace root)
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(&cmd)
                .current_dir(&self.workspace_root)
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let files = truncate_output(&output.stdout);
                if files.is_empty() {
                    Ok(ToolResult {
                        stdout: "no matches found".to_string(),
                        stderr: String::new(),
                        exit_code: 0,
                        duration_ms,
                    })
                } else {
                    // Now get context lines for matched files (limited)
                    let file_list: Vec<&str> = files.lines().take(MAX_SEARCH_MATCHES).collect();
                    let file_args: String = file_list
                        .iter()
                        .map(|f| format!("'{}'", f.replace('\'', "'\\''").trim()))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let context_cmd = format!(
                        "grep -n -F -- '{}' {} | head -{}",
                        pattern.replace('\'', "'\\''"),
                        file_args,
                        MAX_SEARCH_MATCHES * 3
                    );

                    let ctx_result = tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        tokio::process::Command::new("bash")
                            .arg("-c")
                            .arg(&context_cmd)
                            .current_dir(&self.workspace_root)
                            .output(),
                    )
                    .await;

                    let output_text = match ctx_result {
                        Ok(Ok(out)) => truncate_output(&out.stdout),
                        _ => files,
                    };

                    Ok(ToolResult {
                        stdout: output_text,
                        stderr: String::new(),
                        exit_code: 0,
                        duration_ms,
                    })
                }
            }
            Ok(Err(e)) => Err(format!("search failed: {e}")),
            Err(_) => Ok(ToolResult {
                stdout: String::new(),
                stderr: "search timed out after 30s".to_string(),
                exit_code: -1,
                duration_ms,
            }),
        }
    }

    /// Commit changes through the validated pipeline (stage → cargo check → cargo test → commit → push).
    async fn commit_changes(&self, message: &str, files: &[&str]) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        if !self.coding_enabled {
            return Err("coding is not enabled (set SOUL_CODING_ENABLED=true)".to_string());
        }

        let git = self
            .git
            .as_ref()
            .ok_or_else(|| "git context not available".to_string())?;
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "database not available".to_string())?;

        let workspace = self.workspace_root.to_string_lossy().to_string();
        let result = coding::validated_commit(git, &workspace, files, message).await?;

        // Link mutation to highest-priority active goal (if any)
        let active_goal_id = db
            .get_active_goals()
            .ok()
            .and_then(|goals| goals.first().map(|g| g.id.clone()));

        // Record mutation in database
        let mutation = Mutation {
            id: uuid::Uuid::new_v4().to_string(),
            commit_sha: result.commit_sha.clone(),
            branch: git.branch_name().to_string(),
            description: message.to_string(),
            files_changed: serde_json::to_string(files).unwrap_or_default(),
            cargo_check_passed: result.cargo_check_passed,
            cargo_test_passed: result.cargo_test_passed,
            created_at: chrono::Utc::now().timestamp(),
            goal_id: active_goal_id,
        };
        let _ = db.insert_mutation(&mutation);

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: result.message,
            stderr: String::new(),
            exit_code: if result.success { 0 } else { 1 },
            duration_ms,
        })
    }

    /// Create a PR from the VM branch to main.
    async fn propose_to_main(&self, title: &str, body: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        if !self.coding_enabled {
            return Err("coding is not enabled".to_string());
        }

        let git = self
            .git
            .as_ref()
            .ok_or_else(|| "git context not available".to_string())?;

        let result = git.create_pr(title, body).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ToolResult {
            stdout: result.output,
            stderr: String::new(),
            exit_code: if result.success { 0 } else { 1 },
            duration_ms,
        })
    }

    /// Update the persistent memory file.
    async fn update_memory(&self, content: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let bytes = persistent_memory::update(&self.memory_file_path, content)?;
        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "memory updated ({bytes} bytes written to {})",
                self.memory_file_path
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Create a script endpoint — write a bash script that becomes an instant HTTP endpoint.
    /// Scripts live at /data/endpoints/{slug}.sh and are served at GET/POST /x/{slug}.
    async fn create_script_endpoint(
        &self,
        slug: &str,
        script: &str,
        description: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Server-side endpoint cap: prevent script endpoint spam
        let scripts_dir_check = std::path::PathBuf::from("/data/endpoints");
        if scripts_dir_check.exists() {
            let script_count = std::fs::read_dir(&scripts_dir_check)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|ext| ext == "sh").unwrap_or(false))
                        .count()
                })
                .unwrap_or(0);
            if script_count >= 10 {
                return Err(format!(
                    "script endpoint limit reached ({script_count}/10). \
                     Delete existing endpoints before creating new ones. \
                     Focus on improving code quality instead of creating more scripts."
                ));
            }

            // Duplicate detection: reject slugs too similar to existing endpoints
            let existing_slugs: Vec<String> = std::fs::read_dir(&scripts_dir_check)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    e.path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(String::from)
                })
                .collect();
            let new_words: std::collections::HashSet<&str> = slug.split('-').collect();
            for existing in &existing_slugs {
                let existing_words: std::collections::HashSet<&str> = existing.split('-').collect();
                let intersection = new_words.intersection(&existing_words).count();
                let union = new_words.union(&existing_words).count();
                if union > 0 {
                    let similarity = intersection as f64 / union as f64;
                    if similarity > 0.5 && slug != existing.as_str() {
                        return Err(format!(
                            "slug '{slug}' is too similar to existing endpoint '{existing}' \
                             (Jaccard similarity {:.0}%). Each endpoint must be genuinely unique. \
                             Try something completely different.",
                            similarity * 100.0
                        ));
                    }
                }
            }
        }

        // Strip "script-" prefix if the LLM redundantly added it (node auto-prefixes)
        let slug = slug.strip_prefix("script-").unwrap_or(slug);

        // Validate slug
        if !slug
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("slug must be alphanumeric with hyphens/underscores only".to_string());
        }
        if slug.len() > 64 {
            return Err("slug too long (max 64 chars)".to_string());
        }

        // Security: block scripts that try to read secrets from the host process
        let script_lower = script.to_lowercase();
        const BLOCKED_PATTERNS: &[&str] = &[
            "/proc/1/environ",
            "/proc/self/environ",
            "/proc/1/cmdline",
            "/proc/1/maps",
            "evm_private_key",
            "facilitator_private_key",
            "railway_token",
            "gemini_api_key",
            "github_token",
        ];
        for pattern in BLOCKED_PATTERNS {
            if script_lower.contains(pattern) {
                return Err(format!(
                    "script blocked: contains forbidden pattern '{pattern}' — scripts must not access host secrets"
                ));
            }
        }

        let scripts_dir = PathBuf::from("/data/endpoints");
        std::fs::create_dir_all(&scripts_dir)
            .map_err(|e| format!("failed to create scripts directory: {e}"))?;

        let script_path = scripts_dir.join(format!("{slug}.sh"));

        // Prepend description as comment if provided
        let full_script = if let Some(desc) = description {
            format!("# {desc}\n{script}")
        } else {
            script.to_string()
        };

        std::fs::write(&script_path, &full_script)
            .map_err(|e| format!("failed to write script: {e}"))?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
        }

        // Register with gateway DB so peers can discover it immediately
        let gateway_slug = format!("script-{slug}");
        let register_note = self
            .register_script_in_gateway(&gateway_slug, description)
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "Script endpoint created: /x/{slug}\n\
                 Script: {}\n\
                 Size: {} bytes\n\
                 Gateway: {register_note}\n\
                 Test it: curl https://{{your-domain}}/x/{slug}",
                script_path.display(),
                full_script.len()
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Register a script endpoint with the gateway's admin API so it appears in /endpoints.
    async fn register_script_in_gateway(&self, slug: &str, description: Option<&str>) -> String {
        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);
        let url = format!("{}/admin/endpoints", gateway_url.trim_end_matches('/'));

        let body = serde_json::json!({
            "slug": slug,
            "description": description.unwrap_or("Script endpoint"),
        });

        match reqwest::Client::new()
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                format!("registered as {slug} (discoverable by peers)")
            }
            Ok(resp) => {
                let status = resp.status();
                format!("registration returned {status} (will auto-register on restart)")
            }
            Err(e) => {
                tracing::warn!(slug = %slug, error = %e, "Failed to register script in gateway");
                format!("registration failed: {e} (will auto-register on restart)")
            }
        }
    }

    /// List all script endpoints in /data/endpoints/.
    async fn list_script_endpoints(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let scripts_dir = PathBuf::from("/data/endpoints");

        if !scripts_dir.exists() {
            return Ok(ToolResult {
                stdout: "no script endpoints found (directory doesn't exist yet)".to_string(),
                stderr: String::new(),
                exit_code: 0,
                duration_ms: 0,
            });
        }

        let mut entries = Vec::new();
        if let Ok(dir) = std::fs::read_dir(&scripts_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "sh") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        let desc = std::fs::read_to_string(&path).ok().and_then(|c| {
                            c.lines()
                                .next()
                                .and_then(|l| l.strip_prefix("# ").map(String::from))
                        });
                        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        entries.push(format!(
                            "/x/{stem} — {} ({size} bytes)",
                            desc.unwrap_or_else(|| "no description".to_string())
                        ));
                    }
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: if entries.is_empty() {
                "no script endpoints found".to_string()
            } else {
                format!(
                    "{} script endpoints:\n{}",
                    entries.len(),
                    entries.join("\n")
                )
            },
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Test a script endpoint locally by running it with test input.
    async fn test_script_endpoint(&self, slug: &str, input: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        // Strip "script-" prefix if present (create_script_endpoint strips it too)
        let slug = slug.strip_prefix("script-").unwrap_or(slug);
        let script_path = PathBuf::from(format!("/data/endpoints/{slug}.sh"));

        if !script_path.exists() {
            return Err(format!("script endpoint '{slug}' not found"));
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new("bash")
                .arg(script_path.to_str().unwrap_or_default())
                .env("REQUEST_METHOD", "POST")
                .env("REQUEST_BODY", input)
                .env("QUERY_STRING", "")
                .env("REQUEST_HEADERS", "{}")
                .env("ENDPOINT_SLUG", slug)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: if output.status.success() {
                        format!("test passed (exit 0):\n{stdout}")
                    } else {
                        format!(
                            "test failed (exit {}):\nstdout: {stdout}\nstderr: {stderr}",
                            output.status.code().unwrap_or(-1)
                        )
                    },
                    stderr,
                    exit_code: output.status.code().unwrap_or(1),
                    duration_ms,
                })
            }
            Ok(Err(e)) => Err(format!("failed to run script: {e}")),
            Err(_) => Err("script timed out (10s limit for tests)".to_string()),
        }
    }

    /// Delete (deactivate) an endpoint on the local gateway.
    /// Uses the gateway's internal admin path — no payment required for own endpoints.
    async fn delete_endpoint(&self, slug: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);

        // Call the gateway's admin delete endpoint (no payment needed for local)
        let url = format!(
            "{}/admin/endpoints/{}",
            gateway_url.trim_end_matches('/'),
            slug
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        match client.delete(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: body,
                    stderr: if status.is_success() {
                        String::new()
                    } else {
                        format!("delete returned status {status}")
                    },
                    exit_code: status.as_u16() as i32,
                    duration_ms,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: String::new(),
                    stderr: format!("delete request failed: {e}"),
                    exit_code: -1,
                    duration_ms,
                })
            }
        }
    }

    /// Register an endpoint on the gateway via x402 payment.
    async fn register_endpoint(
        &self,
        slug: &str,
        target_url: &str,
        price: &str,
        description: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        if !self.coding_enabled {
            return Err("coding is not enabled (register_endpoint requires Code mode)".to_string());
        }

        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);

        let private_key = std::env::var("EVM_PRIVATE_KEY")
            .map_err(|_| "EVM_PRIVATE_KEY not set — cannot sign payment".to_string())?;

        let client = reqwest::Client::new();

        // Build registration body
        let mut body = serde_json::json!({
            "slug": slug,
            "target_url": target_url,
            "price": price,
        });
        if let Some(desc) = description {
            body["description"] = serde_json::Value::String(desc.to_string());
        }

        // Step 1: POST /register → expect 402
        let register_url = format!("{gateway_url}/register");
        let resp = client
            .post(&register_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("failed to POST /register: {e}"))?;

        if resp.status() != reqwest::StatusCode::PAYMENT_REQUIRED {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.is_success() {
                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ToolResult {
                    stdout: format!("endpoint registered (no payment needed): {text}"),
                    stderr: String::new(),
                    exit_code: 0,
                    duration_ms,
                });
            }
            return Err(format!("expected 402, got {status}: {text}"));
        }

        // Step 2: Parse PaymentRequirements from response
        let resp_json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse 402 response: {e}"))?;

        let accepts = resp_json
            .get("accepts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "402 response missing 'accepts' array".to_string())?;

        let req_value = accepts
            .first()
            .ok_or_else(|| "402 response 'accepts' array is empty".to_string())?;

        let requirements = parse_payment_requirements(req_value)?;

        // Step 3: Sign payment
        let signer = x402::wallet::WalletSigner::new(&private_key)
            .map_err(|e| format!("failed to create signer: {e}"))?;

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("system time error: {e}"))?
            .as_secs();

        let payment_b64 = signer
            .sign_payment(&requirements, now_secs)
            .map_err(|e| format!("failed to sign payment: {e}"))?;

        // Step 4: Retry with payment header
        let resp2 = client
            .post(&register_url)
            .header("PAYMENT-SIGNATURE", &payment_b64)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("failed to retry POST with payment: {e}"))?;

        let status = resp2.status();
        let text = resp2.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() {
            Ok(ToolResult {
                stdout: format!("endpoint /{slug} registered successfully: {text}"),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("registration failed ({status}): {text}"),
                exit_code: 1,
                duration_ms,
            })
        }
    }

    /// Check the node's own endpoints for self-introspection.
    /// Whitelisted to: health, analytics, analytics/{slug}, soul/status.
    async fn check_self(&self, endpoint: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Whitelist check: only allow safe read-only endpoints
        let trimmed = endpoint.trim_start_matches('/');
        let allowed = trimmed == "health"
            || trimmed == "analytics"
            || trimmed == "soul/status"
            || trimmed.starts_with("analytics/");

        if !allowed {
            return Err(format!(
                "endpoint '/{trimmed}' not allowed. Use: health, analytics, analytics/{{slug}}, soul/status"
            ));
        }

        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);

        let url = format!("{gateway_url}/{trimmed}");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let duration_ms = start.elapsed().as_millis() as u64;

                // Truncate body if huge
                let body_truncated = if body.len() > MAX_OUTPUT_BYTES {
                    format!(
                        "{}\n... (truncated)",
                        body.chars().take(MAX_OUTPUT_BYTES).collect::<String>()
                    )
                } else {
                    body
                };

                Ok(ToolResult {
                    stdout: body_truncated,
                    stderr: String::new(),
                    exit_code: status.as_u16() as i32,
                    duration_ms,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: String::new(),
                    stderr: format!("request failed: {e}"),
                    exit_code: -1,
                    duration_ms,
                })
            }
        }
    }

    /// Helper to make a Railway GraphQL API call.
    async fn railway_graphql(&self, query: &str) -> Result<serde_json::Value, String> {
        let token = self
            .railway_token
            .as_ref()
            .ok_or("RAILWAY_TOKEN not configured")?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))?;

        let resp = client
            .post("https://backboard.railway.app/graphql/v2")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .map_err(|e| format!("Railway API request failed: {e}"))?;

        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Railway API response parse failed: {e}"))
    }

    /// Check the latest deployment status for this service on Railway.
    async fn check_deploy_status(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let service_id = self
            .railway_service_id
            .as_ref()
            .ok_or("RAILWAY_SERVICE_ID not configured")?;
        let env_id = self
            .railway_environment_id
            .as_ref()
            .ok_or("RAILWAY_ENVIRONMENT_ID not configured")?;

        let query = format!(
            r#"{{ deployments(input: {{ serviceId: "{service_id}", environmentId: "{env_id}" }}, first: 3) {{ edges {{ node {{ id status createdAt updatedAt }} }} }} }}"#
        );

        let data = self.railway_graphql(&query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Format nicely for the LLM
        let edges = data
            .pointer("/data/deployments/edges")
            .and_then(|v| v.as_array());

        let mut output = String::new();
        if let Some(edges) = edges {
            for (i, edge) in edges.iter().enumerate() {
                let node = &edge["node"];
                let id = node["id"].as_str().unwrap_or("?");
                let status = node["status"].as_str().unwrap_or("?");
                let created = node["createdAt"].as_str().unwrap_or("?");
                let updated = node["updatedAt"].as_str().unwrap_or("?");
                output.push_str(&format!(
                    "{}. {} — status: {}, created: {}, updated: {}\n",
                    i + 1,
                    id,
                    status,
                    created,
                    updated
                ));
            }
        } else if let Some(errors) = data.get("errors") {
            output = format!("Railway API error: {errors}");
        } else {
            output = "No deployments found".to_string();
        }

        Ok(ToolResult {
            stdout: output,
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Get build logs for a Railway deployment.
    async fn get_deploy_logs(&self, deployment_id: Option<&str>) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let service_id = self
            .railway_service_id
            .as_ref()
            .ok_or("RAILWAY_SERVICE_ID not configured")?;
        let env_id = self
            .railway_environment_id
            .as_ref()
            .ok_or("RAILWAY_ENVIRONMENT_ID not configured")?;

        // If no deployment ID given, get the latest one first
        let deploy_id = if let Some(id) = deployment_id {
            id.to_string()
        } else {
            let query = format!(
                r#"{{ deployments(input: {{ serviceId: "{service_id}", environmentId: "{env_id}" }}, first: 1) {{ edges {{ node {{ id }} }} }} }}"#
            );
            let data = self.railway_graphql(&query).await?;
            data.pointer("/data/deployments/edges/0/node/id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or("No deployments found")?
        };

        let query = format!(
            r#"{{ buildLogs(deploymentId: "{deploy_id}", limit: 200) {{ message timestamp }} }}"#
        );

        let data = self.railway_graphql(&query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let mut output = format!("Build logs for deployment {deploy_id}:\n\n");

        if let Some(logs) = data.pointer("/data/buildLogs").and_then(|v| v.as_array()) {
            for log in logs {
                let msg = log["message"].as_str().unwrap_or("");
                output.push_str(msg);
                output.push('\n');
            }
            if logs.is_empty() {
                output.push_str("(no build logs available yet)\n");
            }
        } else if let Some(errors) = data.get("errors") {
            output = format!("Railway API error: {errors}");
        }

        // Truncate if too long
        if output.len() > MAX_OUTPUT_BYTES {
            output = output.chars().take(MAX_OUTPUT_BYTES).collect();
            output.push_str("\n... (truncated)");
        }

        Ok(ToolResult {
            stdout: output,
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Trigger a redeployment of this service on Railway.
    async fn trigger_redeploy(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let service_id = self
            .railway_service_id
            .as_ref()
            .ok_or("RAILWAY_SERVICE_ID not configured")?;
        let env_id = self
            .railway_environment_id
            .as_ref()
            .ok_or("RAILWAY_ENVIRONMENT_ID not configured")?;

        let query = format!(
            r#"mutation {{ serviceInstanceRedeploy(serviceId: "{service_id}", environmentId: "{env_id}") }}"#
        );

        let data = self.railway_graphql(&query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let success = data
            .pointer("/data/serviceInstanceRedeploy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(ToolResult {
            stdout: if success {
                "Redeployment triggered successfully. Use check_deploy_status to monitor progress."
                    .to_string()
            } else {
                format!("Redeploy response: {data}")
            },
            stderr: String::new(),
            exit_code: if success { 0 } else { 1 },
            duration_ms,
        })
    }

    /// Create a new GitHub repository.
    async fn create_github_repo(
        &self,
        name: &str,
        description: Option<&str>,
        private: bool,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let token = std::env::var("GITHUB_TOKEN")
            .map_err(|_| "GITHUB_TOKEN not set — cannot create repos".to_string())?;

        // Validate name
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(
                "repo name must be alphanumeric with hyphens, underscores, or dots".to_string(),
            );
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        let mut body = serde_json::json!({
            "name": name,
            "private": private,
            "auto_init": true,
        });
        if let Some(desc) = description {
            body["description"] = serde_json::json!(desc);
        }

        let resp = client
            .post("https://api.github.com/user/repos")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "x402-soul")
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("GitHub API error: {e}"))?;

        let status = resp.status();
        let resp_body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse GitHub response: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() {
            let html_url = resp_body["html_url"].as_str().unwrap_or("unknown");
            let clone_url = resp_body["clone_url"].as_str().unwrap_or("unknown");
            Ok(ToolResult {
                stdout: format!(
                    "Repository created successfully!\nURL: {html_url}\nClone: {clone_url}\nPrivate: {private}"
                ),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            let msg = resp_body["message"].as_str().unwrap_or("unknown error");
            Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("GitHub API error ({status}): {msg}"),
                exit_code: 1,
                duration_ms,
            })
        }
    }

    /// Fork an existing GitHub repository.
    async fn fork_github_repo(&self, owner: &str, repo: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let token = std::env::var("GITHUB_TOKEN")
            .map_err(|_| "GITHUB_TOKEN not set — cannot fork repos".to_string())?;

        // Validate owner and repo to prevent URL injection
        if !owner
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("owner must be alphanumeric with hyphens or underscores only".to_string());
        }
        if !repo
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(
                "repo must be alphanumeric with hyphens, underscores, or dots only".to_string(),
            );
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        let url = format!("https://api.github.com/repos/{owner}/{repo}/forks");

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "x402-soul")
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| format!("GitHub API error: {e}"))?;

        let status = resp.status();
        let resp_body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse GitHub response: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() || status.as_u16() == 202 {
            let html_url = resp_body["html_url"].as_str().unwrap_or("unknown");
            let full_name = resp_body["full_name"].as_str().unwrap_or("unknown");
            Ok(ToolResult {
                stdout: format!(
                    "Repository forked successfully!\nFork: {full_name}\nURL: {html_url}\nOriginal: {owner}/{repo}"
                ),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            let msg = resp_body["message"].as_str().unwrap_or("unknown error");
            Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("GitHub API error ({status}): {msg}"),
                exit_code: 1,
                duration_ms,
            })
        }
    }

    /// Discover peer instances by calling parent's /instance/siblings endpoint.
    /// Check this agent's on-chain reputation from the ERC-8004 registry.
    async fn check_reputation(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Read config from env
        let registry_str = std::env::var("ERC8004_REPUTATION_REGISTRY").unwrap_or_default();
        let token_id_str = std::env::var("ERC8004_AGENT_TOKEN_ID").unwrap_or_default();
        let rpc_url = std::env::var("RPC_URL").unwrap_or_default();

        if registry_str.is_empty() || token_id_str.is_empty() || rpc_url.is_empty() {
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ToolResult {
                stdout: "ERC-8004 reputation not configured. Need: ERC8004_REPUTATION_REGISTRY, ERC8004_AGENT_TOKEN_ID, RPC_URL".to_string(),
                stderr: String::new(),
                exit_code: 1,
                duration_ms,
            });
        }

        // Use HTTP call to check_self pattern — query the chain via shell
        // This avoids adding alloy as a dependency to the soul crate.
        // We use a JSON-RPC eth_call via curl instead.
        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "Reputation registry: {}\nAgent token ID: {}\nUse execute_shell with 'curl' to query the contract directly, or check_self with 'analytics' to see payment stats as a proxy for reputation.",
                registry_str, token_id_str
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Update this agent's on-chain metadata URI.
    async fn update_agent_metadata(&self, metadata_uri: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let registry_str = std::env::var("ERC8004_IDENTITY_REGISTRY").unwrap_or_default();
        let token_id_str = std::env::var("ERC8004_AGENT_TOKEN_ID").unwrap_or_default();

        if registry_str.is_empty() || token_id_str.is_empty() {
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ToolResult {
                stdout: "ERC-8004 identity not configured. Need: ERC8004_IDENTITY_REGISTRY, ERC8004_AGENT_TOKEN_ID".to_string(),
                stderr: String::new(),
                exit_code: 1,
                duration_ms,
            });
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "Identity registry: {}\nAgent token ID: {}\nRequested metadata URI: {}\nNote: On-chain metadata update requires a transaction. Use execute_shell to send the tx via cast or a script.",
                registry_str, token_id_str, metadata_uri
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    async fn discover_peers(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Try on-chain discovery first (decentralized, via ERC-8004 identity registry)
        let identity_registry = std::env::var("ERC8004_IDENTITY_REGISTRY")
            .ok()
            .and_then(|s| s.parse::<alloy::primitives::Address>().ok())
            .filter(|a| *a != alloy::primitives::Address::ZERO);

        if let Some(registry) = identity_registry {
            let rpc_url = std::env::var("RPC_URL")
                .unwrap_or_else(|_| "https://rpc.moderato.tempo.xyz".to_string());
            let self_address = std::env::var("EVM_ADDRESS")
                .ok()
                .and_then(|s| s.parse::<alloy::primitives::Address>().ok());

            let provider = alloy::providers::RootProvider::<alloy::network::Ethereum>::new_http(
                rpc_url.parse().map_err(|e| format!("bad RPC URL: {e}"))?,
            );

            match x402_identity::discovery::discover_peers(&provider, registry, self_address, 50)
                .await
            {
                Ok(peers) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    let output = serde_json::to_string_pretty(&serde_json::json!({
                        "source": "on-chain",
                        "registry": format!("{:#x}", registry),
                        "peers": peers,
                        "count": peers.len(),
                    }))
                    .unwrap_or_default();

                    let output_truncated = if output.len() > MAX_OUTPUT_BYTES {
                        format!(
                            "{}\n... (truncated)",
                            output.chars().take(MAX_OUTPUT_BYTES).collect::<String>()
                        )
                    } else {
                        output
                    };

                    return Ok(ToolResult {
                        stdout: output_truncated,
                        stderr: String::new(),
                        exit_code: 0,
                        duration_ms,
                    });
                }
                Err(e) => {
                    tracing::debug!(error = %e, "On-chain peer discovery failed, falling back to HTTP");
                }
            }
        }

        // Fallback: HTTP-based discovery via parent's /instance/siblings
        let default_local = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let parent_url = std::env::var("PARENT_URL")
            .ok()
            .or_else(|| self.gateway_url.clone())
            .unwrap_or(default_local);

        let url = format!("{}/instance/siblings", parent_url.trim_end_matches('/'));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();

                if !status.is_success() {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(ToolResult {
                        stdout: String::new(),
                        stderr: format!(
                            "discover_peers: {} returned HTTP {} — {}",
                            url,
                            status.as_u16(),
                            body.chars().take(200).collect::<String>()
                        ),
                        exit_code: status.as_u16() as i32,
                        duration_ms,
                    });
                }

                // Parse siblings and enrich each with /instance/info
                let siblings_json: serde_json::Value = match serde_json::from_str(&body) {
                    Ok(v) => v,
                    Err(e) => {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        return Ok(ToolResult {
                            stdout: String::new(),
                            stderr: format!(
                                "discover_peers: failed to parse response from {}: {} — body: {}",
                                url,
                                e,
                                body.chars().take(200).collect::<String>()
                            ),
                            exit_code: -1,
                            duration_ms,
                        });
                    }
                };
                let siblings = siblings_json
                    .get("siblings")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let mut enriched_peers = Vec::new();
                for sib in &siblings {
                    let inst_id = sib
                        .get("instance_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let sib_url = match sib.get("url").and_then(|v| v.as_str()) {
                        Some(u) => u,
                        None => continue,
                    };
                    let address = sib.get("address").and_then(|v| v.as_str());

                    // Fetch peer's /instance/info for endpoints + version
                    let info_url = format!("{}/instance/info", sib_url.trim_end_matches('/'));
                    let peer_info = match client.get(&info_url).send().await {
                        Ok(r) if r.status().is_success() => r.json().await.ok(),
                        Ok(r) => {
                            tracing::debug!(
                                instance_id = %inst_id,
                                status = %r.status(),
                                "Peer /instance/info returned non-2xx — skipping"
                            );
                            // Skip unreachable peers instead of adding with empty endpoints
                            continue;
                        }
                        Err(e) => {
                            tracing::debug!(
                                instance_id = %inst_id,
                                error = %e,
                                "Peer /instance/info unreachable — skipping"
                            );
                            continue;
                        }
                    };
                    let info_json: Option<serde_json::Value> = peer_info;

                    let version = info_json
                        .as_ref()
                        .and_then(|j| j.get("version"))
                        .and_then(|v| v.as_str());
                    let endpoints = info_json
                        .as_ref()
                        .and_then(|j| j.get("endpoints"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    // Build callable URLs so the LLM can pass them directly to call_paid_endpoint
                    let callable_endpoints: Vec<serde_json::Value> = endpoints
                        .iter()
                        .map(|ep| {
                            let slug = ep.get("slug").and_then(|s| s.as_str()).unwrap_or("");
                            let mut ep_clone = ep.clone();
                            if let Some(obj) = ep_clone.as_object_mut() {
                                obj.insert(
                                    "callable_url".to_string(),
                                    serde_json::Value::String(format!(
                                        "{}/g/{}",
                                        sib_url.trim_end_matches('/'),
                                        slug
                                    )),
                                );
                            }
                            ep_clone
                        })
                        .collect();

                    // ── x402 PAID peer data exchange ──
                    // All peer data fetches go through call_paid_endpoint so they
                    // generate real x402 economic activity (EIP-712 signed payments).

                    // 1. Call peer's "soul" paid endpoint to get status + brain + benchmark data
                    let soul_gateway_url = format!("{}/g/soul", sib_url.trim_end_matches('/'));
                    let paid_soul_data: Option<serde_json::Value> = match self
                        .call_paid_endpoint(&soul_gateway_url, "GET", None)
                        .await
                    {
                        Ok(result) if result.exit_code == 200 => {
                            tracing::info!(
                                peer = %inst_id,
                                "x402 PAID call to peer soul endpoint succeeded"
                            );
                            serde_json::from_str(&result.stdout).ok()
                        }
                        Ok(result) => {
                            tracing::warn!(
                                peer = %inst_id,
                                code = result.exit_code,
                                stderr = %result.stderr,
                                "x402 paid call to peer soul returned non-200"
                            );
                            None
                        }
                        Err(e) => {
                            tracing::warn!(peer = %inst_id, error = %e, "x402 paid call to peer soul failed");
                            None
                        }
                    };

                    // Extract brain weights from paid soul response and merge
                    if let Some(ref db) = self.db {
                        if let Some(ref soul_data) = paid_soul_data {
                            // The soul status includes brain info (train_steps, parameters)
                            // For full weight merging, we still need the weights endpoint
                            // but now we also track the paid call for coordination fitness
                        }

                        // Brain weight merge — still needs dedicated endpoint for full weights
                        // TODO: register brain/weights as a paid gateway endpoint
                        let brain_url =
                            format!("{}/soul/brain/weights", sib_url.trim_end_matches('/'));
                        if let Ok(resp) = client.get(&brain_url).send().await {
                            if resp.status().is_success() {
                                if let Ok(body) = resp.json::<serde_json::Value>().await {
                                    if let Some(weights_json) =
                                        body.get("weights").and_then(|v| v.as_str())
                                    {
                                        if let Some(peer_brain) =
                                            crate::brain::Brain::from_json(weights_json)
                                        {
                                            if peer_brain.train_steps > 0 {
                                                let mut our_brain = crate::brain::load_brain(db);
                                                let delta = peer_brain.compute_delta(
                                                    &crate::brain::Brain::new(),
                                                    inst_id,
                                                );
                                                our_brain.merge_delta(&delta, 0.3);
                                                crate::brain::save_brain(db, &our_brain);
                                                tracing::info!(
                                                    peer = %inst_id,
                                                    peer_steps = peer_brain.train_steps,
                                                    "Merged brain weights from peer"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 2. Call peer's "info" paid endpoint for version/endpoint data
                    let info_gateway_url = format!("{}/g/info", sib_url.trim_end_matches('/'));
                    match self
                        .call_paid_endpoint(&info_gateway_url, "GET", None)
                        .await
                    {
                        Ok(result) if result.exit_code == 200 => {
                            tracing::info!(
                                peer = %inst_id,
                                "x402 PAID call to peer info endpoint succeeded"
                            );
                        }
                        Ok(result) => {
                            tracing::warn!(
                                peer = %inst_id,
                                code = result.exit_code,
                                stderr = %result.stderr,
                                "x402 paid call to peer info returned non-200"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(peer = %inst_id, error = %e, "x402 paid call to peer info failed");
                        }
                    }

                    // Fetch peer's lessons for collective learning
                    let mut peer_lessons = Vec::new();
                    if let Some(ref db) = self.db {
                        // Extract lessons from paid soul data if available
                        if let Some(ref soul_data) = paid_soul_data {
                            if let Some(outcomes) =
                                soul_data.get("plan_outcomes").and_then(|v| v.as_array())
                            {
                                // Store as peer lessons for prompt injection
                                let key = format!("peer_lessons_{}", inst_id);
                                if let Ok(json) = serde_json::to_string(
                                    &serde_json::json!({ "outcomes": outcomes }),
                                ) {
                                    let _ = db.set_state(&key, &json);
                                }
                                for o in outcomes.iter().take(5) {
                                    if let Some(lesson) = o.get("lesson").and_then(|v| v.as_str()) {
                                        peer_lessons.push(lesson.to_string());
                                    }
                                }
                                tracing::info!(
                                    peer = %inst_id,
                                    lessons = peer_lessons.len(),
                                    "Extracted lessons from paid soul response"
                                );
                            }
                        }

                        // Fallback: if no lessons from paid response, try free endpoint
                        if peer_lessons.is_empty() {
                            let lessons_url =
                                format!("{}/soul/lessons", sib_url.trim_end_matches('/'));
                            if let Ok(resp) = client.get(&lessons_url).send().await {
                                if resp.status().is_success() {
                                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                                        let key = format!("peer_lessons_{}", inst_id);
                                        if let Ok(json) = serde_json::to_string(&body) {
                                            let _ = db.set_state(&key, &json);
                                        }
                                        if let Some(outcomes) =
                                            body.get("outcomes").and_then(|v| v.as_array())
                                        {
                                            for o in outcomes.iter().take(5) {
                                                if let Some(lesson) =
                                                    o.get("lesson").and_then(|v| v.as_str())
                                                {
                                                    peer_lessons.push(lesson.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fetch and import peer's verified benchmark solutions (collective intelligence)
                    let mut solutions_imported = 0u32;
                    if let Some(ref db) = self.db {
                        let solutions_url =
                            format!("{}/soul/benchmark/solutions", sib_url.trim_end_matches('/'));
                        if let Ok(resp) = client.get(&solutions_url).send().await {
                            if resp.status().is_success() {
                                if let Ok(body) = resp.json::<serde_json::Value>().await {
                                    if let Some(solutions) =
                                        body.get("solutions").and_then(|v| v.as_array())
                                    {
                                        let peer_sols: Vec<crate::benchmark::SharedSolution> =
                                            solutions
                                                .iter()
                                                .filter_map(|s| {
                                                    serde_json::from_value(s.clone()).ok()
                                                })
                                                .collect();
                                        if !peer_sols.is_empty() {
                                            let workspace = self.workspace_root.to_string_lossy();
                                            solutions_imported =
                                                crate::benchmark::import_solutions(
                                                    db, peer_sols, &workspace,
                                                )
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fetch peer's open PRs for peer review system
                    let mut peer_prs: Vec<serde_json::Value> = Vec::new();
                    let prs_url = format!("{}/soul/open-prs", sib_url.trim_end_matches('/'));
                    if let Ok(resp) = client.get(&prs_url).send().await {
                        if resp.status().is_success() {
                            if let Ok(body) = resp.json::<serde_json::Value>().await {
                                // Collect PRs that need review
                                for key in &["fork_prs", "upstream_prs"] {
                                    if let Some(prs) = body.get(*key).and_then(|v| v.as_array()) {
                                        for pr in prs {
                                            let needs_review = pr
                                                .get("reviewDecision")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.is_empty() || s == "REVIEW_REQUIRED")
                                                .unwrap_or(true);
                                            if needs_review {
                                                peer_prs.push(pr.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    enriched_peers.push(serde_json::json!({
                        "instance_id": inst_id,
                        "url": sib_url,
                        "address": address,
                        "version": version,
                        "endpoints": callable_endpoints,
                        "lessons": peer_lessons,
                        "solutions_imported": solutions_imported,
                        "open_prs": peer_prs,
                    }));

                    // ── Mutual linking: POST /instance/link back to the peer ──
                    // This ensures the peer also sees *us* in their siblings list.
                    // Without this, peer relationships are one-directional.
                    let our_public_url = std::env::var("RAILWAY_PUBLIC_DOMAIN")
                        .ok()
                        .map(|d| format!("https://{d}"))
                        .or_else(|| {
                            self.gateway_url
                                .as_deref()
                                .filter(|u| u.starts_with("https://"))
                                .map(String::from)
                        });
                    if let Some(our_url) = our_public_url.as_deref() {
                        // Only link back if we have an externally-reachable URL
                        if our_url.starts_with("https://") {
                            let link_url =
                                format!("{}/instance/link", sib_url.trim_end_matches('/'));
                            let link_body = serde_json::json!({ "url": our_url });
                            match client.post(&link_url).json(&link_body).send().await {
                                Ok(r) if r.status().is_success() => {
                                    tracing::info!(
                                        peer = %inst_id,
                                        our_url = %our_url,
                                        "Mutual link established — peer now sees us"
                                    );
                                }
                                Ok(r) => {
                                    tracing::debug!(
                                        peer = %inst_id,
                                        status = %r.status(),
                                        "Mutual link returned non-2xx (non-fatal)"
                                    );
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        peer = %inst_id,
                                        error = %e,
                                        "Mutual link request failed (non-fatal)"
                                    );
                                }
                            }
                        }
                    }
                }

                // Track successful peer discovery as coordination signal
                if !enriched_peers.is_empty() {
                    if let Some(ref db) = self.db {
                        let attempted: u64 = db
                            .get_state("peer_calls_attempted")
                            .ok()
                            .flatten()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let succeeded: u64 = db
                            .get_state("peer_calls_succeeded")
                            .ok()
                            .flatten()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let _ = db.set_state("peer_calls_attempted", &(attempted + 1).to_string());
                        let _ = db.set_state("peer_calls_succeeded", &(succeeded + 1).to_string());

                        // Persist peer endpoint catalog for prompt injection
                        // This lets the planning prompt tell agents about ALL available peer endpoints
                        let mut catalog: Vec<serde_json::Value> = Vec::new();
                        for peer in &enriched_peers {
                            let peer_id = peer
                                .get("instance_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let peer_eps = peer
                                .get("endpoints")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let slugs: Vec<String> = peer_eps
                                .iter()
                                .filter_map(|ep| {
                                    ep.get("slug").and_then(|s| s.as_str()).map(String::from)
                                })
                                .collect();
                            if !slugs.is_empty() {
                                catalog.push(serde_json::json!({
                                    "peer": peer_id,
                                    "slugs": slugs,
                                }));
                            }
                        }
                        if let Ok(json) = serde_json::to_string(&catalog) {
                            let _ = db.set_state("peer_endpoint_catalog", &json);
                        }

                        // Persist peer open PRs for review prompt injection
                        let mut all_peer_prs: Vec<serde_json::Value> = Vec::new();
                        for peer in &enriched_peers {
                            let peer_id = peer
                                .get("instance_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            if let Some(prs) = peer.get("open_prs").and_then(|v| v.as_array()) {
                                for pr in prs {
                                    let mut pr_entry = pr.clone();
                                    if let Some(obj) = pr_entry.as_object_mut() {
                                        obj.insert(
                                            "peer_id".to_string(),
                                            serde_json::Value::String(peer_id.to_string()),
                                        );
                                    }
                                    all_peer_prs.push(pr_entry);
                                }
                            }
                        }
                        if let Ok(json) = serde_json::to_string(&all_peer_prs) {
                            let _ = db.set_state("peer_open_prs", &json);
                        }
                    }
                }

                let output = serde_json::to_string_pretty(&serde_json::json!({
                    "source": "http",
                    "parent_url": parent_url,
                    "peers": enriched_peers,
                    "count": enriched_peers.len(),
                }))
                .unwrap_or_default();

                let duration_ms = start.elapsed().as_millis() as u64;
                let output_truncated = if output.len() > MAX_OUTPUT_BYTES {
                    format!(
                        "{}\n... (truncated)",
                        output.chars().take(MAX_OUTPUT_BYTES).collect::<String>()
                    )
                } else {
                    output
                };

                Ok(ToolResult {
                    stdout: output_truncated,
                    stderr: String::new(),
                    exit_code: status.as_u16() as i32,
                    duration_ms,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: String::new(),
                    stderr: format!("request failed: {e}"),
                    exit_code: -1,
                    duration_ms,
                })
            }
        }
    }

    /// Call a paid endpoint on another instance using the x402 payment flow.
    /// Pattern: GET → 402 → parse requirements → sign → retry with PAYMENT-SIGNATURE.
    async fn call_paid_endpoint(
        &self,
        url: &str,
        method: &str,
        body: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let private_key = std::env::var("EVM_PRIVATE_KEY")
            .map_err(|_| "EVM_PRIVATE_KEY not set — cannot sign payments".to_string())?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        // Step 1: Make initial request — expect 402
        let initial_resp = match method.to_uppercase().as_str() {
            "POST" => {
                client
                    .post(url)
                    .body(body.unwrap_or("").to_string())
                    .send()
                    .await
            }
            _ => client.get(url).send().await,
        }
        .map_err(|e| format!("initial request failed: {e}"))?;

        // If not 402, return the response directly (endpoint may be free)
        if initial_resp.status().as_u16() != 402 {
            let status = initial_resp.status();
            let resp_body = initial_resp.text().await.unwrap_or_default();
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ToolResult {
                stdout: resp_body,
                stderr: format!("endpoint returned {status} (not 402 — no payment needed)"),
                exit_code: status.as_u16() as i32,
                duration_ms,
            });
        }

        // Step 2: Parse PaymentRequirements from 402 response
        let resp_json: serde_json::Value = initial_resp
            .json()
            .await
            .map_err(|e| format!("failed to parse 402 response: {e}"))?;

        let accepts = resp_json
            .get("accepts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "402 response missing 'accepts' array".to_string())?;

        let req_value = accepts
            .first()
            .ok_or_else(|| "402 response 'accepts' array is empty".to_string())?;

        let requirements = parse_payment_requirements(req_value)?;

        // Step 2.5: Auto-approve the target's facilitator if needed.
        // For embedded facilitators, pay_to == facilitator address.
        // The facilitator calls transferFrom(payer, pay_to, amount), which
        // requires ERC-20 approve(facilitator, amount) from the payer.
        if let Ok(pay_to_addr) = requirements.pay_to.parse::<alloy::primitives::Address>() {
            let rpc_url = std::env::var("RPC_URL")
                .unwrap_or_else(|_| "https://rpc.moderato.tempo.xyz".to_string());
            if let Ok(rpc_parsed) = rpc_url.parse::<reqwest::Url>() {
                let pk_signer: alloy::signers::local::PrivateKeySigner = private_key
                    .parse()
                    .map_err(|e| format!("invalid private key for approval: {e}"))?;
                let payer_addr = pk_signer.address();
                let wallet = alloy::network::EthereumWallet::from(pk_signer);
                let provider = alloy::providers::ProviderBuilder::new()
                    .wallet(wallet)
                    .connect_http(rpc_parsed);
                let token = x402::constants::DEFAULT_TOKEN;

                // Check current allowance
                let current_allowance =
                    x402::tip20::allowance(&provider, token, payer_addr, pay_to_addr)
                        .await
                        .unwrap_or(alloy::primitives::U256::ZERO);

                // If allowance is below 1B pathUSD, approve MAX
                if current_allowance < alloy::primitives::U256::from(1_000_000_000_000_000u64) {
                    tracing::info!(
                        payer = %payer_addr,
                        facilitator = %pay_to_addr,
                        "Auto-approving facilitator for pathUSD (first payment to this peer)"
                    );
                    match x402::tip20::approve(
                        &provider,
                        token,
                        pay_to_addr,
                        alloy::primitives::U256::MAX,
                    )
                    .await
                    {
                        Ok(tx) => {
                            tracing::info!(tx = %tx, "Facilitator approved for pathUSD");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Auto-approval failed — payment may fail");
                        }
                    }
                }
            }
        }

        // Step 3: Sign payment using wallet signer (same pattern as register_endpoint)
        let signer = x402::wallet::WalletSigner::new(&private_key)
            .map_err(|e| format!("failed to create signer: {e}"))?;

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("system time error: {e}"))?
            .as_secs();

        let payment_b64 = signer
            .sign_payment(&requirements, now_secs)
            .map_err(|e| format!("failed to sign payment: {e}"))?;

        // Step 4: Retry with payment signature
        let paid_resp = match method.to_uppercase().as_str() {
            "POST" => {
                client
                    .post(url)
                    .header("PAYMENT-SIGNATURE", &payment_b64)
                    .body(body.unwrap_or("").to_string())
                    .send()
                    .await
            }
            _ => {
                client
                    .get(url)
                    .header("PAYMENT-SIGNATURE", &payment_b64)
                    .send()
                    .await
            }
        }
        .map_err(|e| format!("paid request failed: {e}"))?;

        let status = paid_resp.status();
        let final_body = paid_resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        let body_truncated = if final_body.len() > MAX_OUTPUT_BYTES {
            format!(
                "{}\n... (truncated)",
                final_body
                    .chars()
                    .take(MAX_OUTPUT_BYTES)
                    .collect::<String>()
            )
        } else {
            final_body
        };

        Ok(ToolResult {
            stdout: body_truncated.clone(),
            stderr: if status.is_success() {
                String::new()
            } else {
                // Include body in error for debugging
                let body_preview = if body_truncated.len() > 300 {
                    let mut end = 300;
                    while end > 0 && !body_truncated.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &body_truncated[..end])
                } else {
                    body_truncated
                };
                format!("paid request returned status {status}: {body_preview}")
            },
            exit_code: status.as_u16() as i32,
            duration_ms,
        })
    }

    /// Create an issue on the upstream repo.
    async fn create_issue(
        &self,
        title: &str,
        body: &str,
        labels: &[&str],
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        if !self.coding_enabled {
            return Err("coding is not enabled".to_string());
        }

        let git = self
            .git
            .as_ref()
            .ok_or_else(|| "git context not available".to_string())?;

        let result = git.create_issue(title, body, labels).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ToolResult {
            stdout: result.output,
            stderr: String::new(),
            exit_code: if result.success { 0 } else { 1 },
            duration_ms,
        })
    }

    /// Execute belief updates via the world model.
    async fn update_beliefs(&self, updates: &[serde_json::Value]) -> Result<ToolResult, String> {
        use crate::world_model::{Belief, BeliefDomain, Confidence, ModelUpdate};

        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        let now = chrono::Utc::now().timestamp();
        let mut applied = 0u32;
        let mut errors = Vec::new();

        for (i, update_val) in updates.iter().enumerate() {
            let update: ModelUpdate = match serde_json::from_value(update_val.clone()) {
                Ok(u) => u,
                Err(e) => {
                    errors.push(format!("update[{i}]: invalid format: {e}"));
                    continue;
                }
            };

            let result = match &update {
                ModelUpdate::Create {
                    domain,
                    subject,
                    predicate,
                    value,
                    evidence,
                } => {
                    let domain = BeliefDomain::parse(domain).unwrap_or(BeliefDomain::Node);
                    let belief = Belief {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain,
                        subject: subject.clone(),
                        predicate: predicate.clone(),
                        value: value.clone(),
                        confidence: Confidence::Medium,
                        evidence: evidence.clone(),
                        confirmation_count: 1,
                        created_at: now,
                        updated_at: now,
                        active: true,
                    };
                    db.upsert_belief(&belief).map(|_| true)
                }
                ModelUpdate::Update {
                    id,
                    value,
                    evidence,
                } => {
                    let beliefs = db.get_all_active_beliefs().map_err(|e| format!("{e}"))?;
                    if let Some(existing) = beliefs.iter().find(|b| b.id == *id) {
                        let updated = Belief {
                            value: value.clone(),
                            evidence: if evidence.is_empty() {
                                existing.evidence.clone()
                            } else {
                                evidence.clone()
                            },
                            updated_at: now,
                            ..existing.clone()
                        };
                        db.upsert_belief(&updated).map(|_| true)
                    } else {
                        Ok(false)
                    }
                }
                ModelUpdate::Confirm { id } => db.confirm_belief(id),
                ModelUpdate::Invalidate { id, reason } => db.invalidate_belief(id, reason),
                // Goal operations
                ModelUpdate::CreateGoal {
                    description,
                    success_criteria,
                    priority,
                    parent_goal_id,
                } => {
                    use crate::world_model::{Goal, GoalStatus};
                    let active_count = db.get_active_goals().map(|g| g.len()).unwrap_or(0);
                    if active_count >= 10 {
                        errors.push(format!("update[{i}]: goal cap reached (10 active)"));
                        continue;
                    }
                    let goal = Goal {
                        id: uuid::Uuid::new_v4().to_string(),
                        description: description.clone(),
                        status: GoalStatus::Active,
                        priority: *priority,
                        success_criteria: success_criteria.clone(),
                        progress_notes: String::new(),
                        parent_goal_id: parent_goal_id.clone(),
                        retry_count: 0,
                        created_at: now,
                        updated_at: now,
                        completed_at: None,
                    };
                    db.insert_goal(&goal).map(|_| true)
                }
                ModelUpdate::UpdateGoal {
                    goal_id,
                    progress_notes,
                    status,
                } => db.update_goal(goal_id, status.as_deref(), progress_notes.as_deref(), None),
                ModelUpdate::CompleteGoal { goal_id, outcome } => {
                    let notes = if outcome.is_empty() {
                        None
                    } else {
                        Some(outcome.as_str())
                    };
                    db.update_goal(goal_id, Some("completed"), notes, Some(now))
                }
                ModelUpdate::AbandonGoal { goal_id, reason } => {
                    db.update_goal(goal_id, Some("abandoned"), Some(reason.as_str()), Some(now))
                }
            };

            match result {
                Ok(true) => applied += 1,
                Ok(false) => errors.push(format!("update[{i}]: no effect (belief not found)")),
                Err(e) => errors.push(format!("update[{i}]: {e}")),
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout = format!("Applied {applied}/{} belief updates", updates.len());
        let stderr = if errors.is_empty() {
            String::new()
        } else {
            errors.join("\n")
        };

        Ok(ToolResult {
            stdout,
            stderr,
            exit_code: if errors.is_empty() { 0 } else { 1 },
            duration_ms,
        })
    }

    /// Approve a pending plan.
    async fn approve_plan(&self, plan_id: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        match db.approve_plan(plan_id) {
            Ok(true) => Ok(ToolResult {
                stdout: format!("Plan {plan_id} approved — execution will begin next cycle"),
                stderr: String::new(),
                exit_code: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Ok(false) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("No pending plan with ID {plan_id}"),
                exit_code: 1,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => Err(format!("failed to approve plan: {e}")),
        }
    }

    /// Reject a pending plan with optional reason.
    async fn reject_plan(&self, plan_id: &str, reason: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        match db.reject_plan(plan_id) {
            Ok(true) => {
                if !reason.is_empty() {
                    let _ = db.insert_nudge("user", &format!("Plan rejected: {reason}"), 5);
                }
                Ok(ToolResult {
                    stdout: format!("Plan {plan_id} rejected"),
                    stderr: String::new(),
                    exit_code: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
            Ok(false) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("No pending plan with ID {plan_id}"),
                exit_code: 1,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => Err(format!("failed to reject plan: {e}")),
        }
    }

    /// Request a new plan by creating a goal + high-priority nudge.
    async fn request_plan(&self, description: &str, priority: u32) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| "soul database not available".to_string())?;

        let now = chrono::Utc::now().timestamp();
        let priority = priority.clamp(1, 5);

        // Create a goal
        let goal = crate::world_model::Goal {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.to_string(),
            status: crate::world_model::GoalStatus::Active,
            priority,
            success_criteria: String::new(),
            progress_notes: String::new(),
            parent_goal_id: None,
            retry_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        db.insert_goal(&goal)
            .map_err(|e| format!("failed to create goal: {e}"))?;

        // Create a high-priority nudge to trigger plan creation next cycle
        let _ = db.insert_nudge("user", &format!("User requested: {description}"), 5);

        Ok(ToolResult {
            stdout: format!(
                "Created goal '{}' (priority {priority}) — plan will be created next cycle",
                &description[..description.len().min(80)]
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// Parse a PaymentRequirements JSON value into the wallet-compatible struct.
/// Tries wallet format (camelCase) first, falls back to server format (snake_case + Address).
fn parse_payment_requirements(
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

/// Return the update_memory tool declaration (available in Observe, Chat, Code).
pub fn update_memory_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "update_memory".to_string(),
        description: "Update your persistent memory file. This is your long-term memory — it persists across restarts. Write markdown content (max 4KB). The entire content is replaced, so include everything you want to remember.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Full replacement markdown content for your memory file (max 4096 bytes)"
                }
            },
            "required": ["content"]
        }),
    }
}

/// Return the check_self tool declaration (Observe + Chat + Code modes).
pub fn check_self_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "check_self".to_string(),
        description: "Check your own node's endpoints for self-introspection. Whitelisted endpoints: health, analytics, analytics/{slug}, soul/status. Returns the HTTP response body and status code.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "endpoint": {
                    "type": "string",
                    "description": "The endpoint path to check (e.g. 'health', 'analytics', 'analytics/weather', 'soul/status')"
                }
            },
            "required": ["endpoint"]
        }),
    }
}

/// Return the update_beliefs tool declaration (Observe + Chat + Code modes).
pub fn update_beliefs_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "update_beliefs".to_string(),
        description: "Update your world model with structured beliefs. Each update is one of: \
            create (new belief), update (change value), confirm (verify still true), \
            invalidate (mark as wrong). Use this to record what you know, not just what you see."
            .to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "updates": {
                    "type": "array",
                    "description": "Array of belief updates to apply",
                    "items": {
                        "type": "object",
                        "properties": {
                            "op": {
                                "type": "string",
                                "enum": ["create", "update", "confirm", "invalidate"],
                                "description": "Operation type"
                            },
                            "domain": {
                                "type": "string",
                                "enum": ["node", "endpoints", "codebase", "strategy", "self", "identity"],
                                "description": "Belief domain (required for create)"
                            },
                            "subject": {
                                "type": "string",
                                "description": "What the belief is about (required for create)"
                            },
                            "predicate": {
                                "type": "string",
                                "description": "What aspect (required for create)"
                            },
                            "value": {
                                "type": "string",
                                "description": "The belief value (required for create and update)"
                            },
                            "evidence": {
                                "type": "string",
                                "description": "Why you believe this"
                            },
                            "id": {
                                "type": "string",
                                "description": "Belief ID (required for update, confirm, invalidate)"
                            },
                            "reason": {
                                "type": "string",
                                "description": "Why invalidating (required for invalidate)"
                            }
                        },
                        "required": ["op"]
                    }
                }
            },
            "required": ["updates"]
        }),
    }
}

/// Return the register_endpoint tool declaration (Code mode only).
pub fn register_endpoint_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "register_endpoint".to_string(),
        description: "Register a new paid endpoint on the gateway. Handles the full x402 payment flow: sends registration request, signs payment authorization, and completes registration.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "URL slug for the endpoint (e.g. 'weather', 'translate')"
                },
                "target_url": {
                    "type": "string",
                    "description": "The backend URL this endpoint proxies to"
                },
                "price": {
                    "type": "string",
                    "description": "Price per request (default '$0.01')"
                },
                "description": {
                    "type": "string",
                    "description": "Optional description of what this endpoint does"
                }
            },
            "required": ["slug", "target_url"]
        }),
    }
}

/// Return the delete_endpoint tool declaration (Observe + Code modes).
pub fn delete_endpoint_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "delete_endpoint".to_string(),
        description: "Delete (deactivate) a registered endpoint by slug. Use this to clean up unused or redundant endpoints.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "The slug of the endpoint to delete"
                }
            },
            "required": ["slug"]
        }),
    }
}

/// Return the approve_plan tool declaration (Chat + Code modes).
pub fn approve_plan_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "approve_plan".to_string(),
        description: "Approve a pending plan so it can begin execution. Use when the user approves a plan that is awaiting approval.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "The ID of the pending plan to approve"
                }
            },
            "required": ["plan_id"]
        }),
    }
}

/// Return the reject_plan tool declaration (Chat + Code modes).
pub fn reject_plan_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "reject_plan".to_string(),
        description: "Reject a pending plan. Optionally provide a reason which will be used as a nudge for the next planning cycle.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "The ID of the pending plan to reject"
                },
                "reason": {
                    "type": "string",
                    "description": "Why the plan was rejected (optional, used to guide replanning)"
                }
            },
            "required": ["plan_id"]
        }),
    }
}

/// Return the request_plan tool declaration (Chat + Code modes).
pub fn request_plan_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "request_plan".to_string(),
        description: "Request a new plan by creating a goal. The soul will create a plan for this goal in the next cycle.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "What the plan should accomplish"
                },
                "priority": {
                    "type": "integer",
                    "description": "Priority 1-5 (5 = highest, default 5)"
                }
            },
            "required": ["description"]
        }),
    }
}

/// Return the discover_peers tool declaration (Observe + Chat + Code modes).
pub fn discover_peers_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "discover_peers".to_string(),
        description: "Discover peer agents via the on-chain ERC-8004 identity registry or HTTP fallback. Returns peer URLs, addresses, version, and endpoint catalogs. Each endpoint includes a callable_url that can be passed directly to call_paid_endpoint.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Return the call_paid_endpoint tool declaration (Chat + Code modes).
pub fn call_paid_endpoint_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "call_paid_endpoint".to_string(),
        description: "Call another agent's paid endpoint using the x402 payment flow. Automatically handles 402 → sign EIP-712 payment → retry with signature. Auto-approves ERC-20 allowance on first payment to a new peer. Use the callable_url from discover_peers output.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Full callable URL of the paid endpoint (e.g., 'https://peer.up.railway.app/g/script-peer-discovery/' — use callable_url from discover_peers)"
                },
                "method": {
                    "type": "string",
                    "description": "HTTP method: GET or POST (default: GET)"
                },
                "body": {
                    "type": "string",
                    "description": "Request body for POST requests"
                }
            },
            "required": ["url"]
        }),
    }
}

/// Return the check_reputation tool declaration (Observe + Chat + Code modes).
pub fn check_reputation_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "check_reputation".to_string(),
        description: "Check your on-chain reputation score from the ERC-8004 reputation registry. Returns positive, negative, and neutral feedback counts. Requires ERC8004_REPUTATION_REGISTRY to be configured.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Return the update_agent_metadata tool declaration (Code mode only).
pub fn update_agent_metadata_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "update_agent_metadata".to_string(),
        description: "Update your on-chain agent metadata URI in the ERC-8004 identity registry. The metadata URI should point to a URL that describes this agent (e.g., /instance/info).".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "metadata_uri": {
                    "type": "string",
                    "description": "The new metadata URI to set on-chain"
                }
            },
            "required": ["metadata_uri"]
        }),
    }
}

/// Return the list of function declarations for the LLM's tools parameter.
pub fn available_tools() -> Vec<FunctionDeclaration> {
    vec![
        FunctionDeclaration {
            name: "execute_shell".to_string(),
            description: "Execute a shell command in the node's container. Use for non-file operations (curl, env, df, cargo). Prefer file tools for reading/writing files.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Max seconds to wait (default 120, max 300)"
                    }
                },
                "required": ["command"]
            }),
        },
        FunctionDeclaration {
            name: "read_file".to_string(),
            description: "Read a file with line numbers. Returns numbered lines. Use offset/limit for large files.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (relative to workspace root or absolute)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Start reading from this line (0-indexed, optional)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read (optional)"
                    }
                },
                "required": ["path"]
            }),
        },
        FunctionDeclaration {
            name: "write_file".to_string(),
            description: "Create or overwrite a file. Protected files (soul core, identity, Cargo files) cannot be written.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to write (relative to workspace root or absolute)"
                    },
                    "content": {
                        "type": "string",
                        "description": "The full content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        FunctionDeclaration {
            name: "edit_file".to_string(),
            description: "Edit a file via search-and-replace. The old_string must appear exactly once in the file. Protected files cannot be edited.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to edit (relative to workspace root or absolute)"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact string to find (must be unique in the file)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement string"
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        FunctionDeclaration {
            name: "list_directory".to_string(),
            description: "List entries in a directory with type indicators (/ for dirs, @ for symlinks).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path (relative to workspace root or absolute, defaults to '.')"
                    }
                },
                "required": []
            }),
        },
        FunctionDeclaration {
            name: "search_files".to_string(),
            description: "Search for a literal string across files. Returns matching file paths and lines with line numbers.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The literal string to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (defaults to workspace root)"
                    },
                    "glob": {
                        "type": "string",
                        "description": "File glob pattern to filter (e.g. '*.rs', '*.toml')"
                    }
                },
                "required": ["pattern"]
            }),
        },
    ]
}

/// Return tool declarations including git/coding tools (when coding is enabled).
pub fn available_tools_with_git(coding_enabled: bool) -> Vec<FunctionDeclaration> {
    let mut tools = available_tools();

    if coding_enabled {
        tools.push(FunctionDeclaration {
            name: "commit_changes".to_string(),
            description: "Validate and commit file changes. Runs cargo check + cargo test before committing. If files omitted, auto-detects all changed files.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Commit message describing the changes"
                    },
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Array of file paths to stage and commit. If omitted, all changed files are auto-detected."
                    }
                },
                "required": ["message"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "propose_to_main".to_string(),
            description: "Create a pull request from the VM branch to main for human review. If fork workflow is configured, creates a cross-fork PR."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "PR title (short, descriptive)"
                    },
                    "body": {
                        "type": "string",
                        "description": "PR body/description with details of changes"
                    }
                },
                "required": ["title"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "create_issue".to_string(),
            description: "Create a GitHub issue on the upstream repository. Use for bug reports, feature requests, improvement ideas, or tracking work."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Issue title (short, descriptive)"
                    },
                    "body": {
                        "type": "string",
                        "description": "Issue body with details, context, and proposed approach"
                    },
                    "labels": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional labels to apply (e.g. ['enhancement', 'bug'])"
                    }
                },
                "required": ["title"]
            }),
        });

        // Script endpoint tools — create HTTP endpoints without Rust compilation
        tools.push(FunctionDeclaration {
            name: "create_script_endpoint".to_string(),
            description: "Create an instant HTTP endpoint by writing a bash script. The script becomes available at GET/POST /x/{slug} immediately — no compilation or restart needed. The script receives REQUEST_METHOD, REQUEST_BODY, QUERY_STRING, REQUEST_HEADERS as env vars. Output JSON to stdout for JSON responses, or plain text. IMPORTANT: Do NOT create endpoints similar to ones that already exist — each must be genuinely unique and useful.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "URL slug for the endpoint (alphanumeric + hyphens, e.g. 'base64', 'hash-keccak')"
                    },
                    "script": {
                        "type": "string",
                        "description": "Bash script content. Use REQUEST_BODY for input, output JSON to stdout. Example: echo '{\"result\": \"'$(echo $REQUEST_BODY | base64)'\"}'"
                    },
                    "description": {
                        "type": "string",
                        "description": "Short description of what the endpoint does"
                    }
                },
                "required": ["slug", "script"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "list_script_endpoints".to_string(),
            description:
                "List all script endpoints you've created. Shows slug, description, and size."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        });

        tools.push(FunctionDeclaration {
            name: "test_script_endpoint".to_string(),
            description: "Test a script endpoint locally before advertising it. Runs the script with test input and returns the output.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "The endpoint slug to test"
                    },
                    "input": {
                        "type": "string",
                        "description": "Test input (passed as REQUEST_BODY env var)"
                    }
                },
                "required": ["slug"]
            }),
        });

        // GitHub tools — create repos, fork repos, expand into external projects
        tools.push(FunctionDeclaration {
            name: "create_github_repo".to_string(),
            description: "Create a new GitHub repository. Use this to start new projects, libraries, or research repos. Requires GITHUB_TOKEN.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Repository name (e.g. 'my-research-project')"
                    },
                    "description": {
                        "type": "string",
                        "description": "Short description of the repository"
                    },
                    "private": {
                        "type": "boolean",
                        "description": "Whether the repo should be private (default: false)"
                    }
                },
                "required": ["name"]
            }),
        });

        tools.push(FunctionDeclaration {
            name: "fork_github_repo".to_string(),
            description: "Fork an existing GitHub repository to your account. Use this to study, improve, or build on other projects.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "Repository owner (e.g. 'openai')"
                    },
                    "repo": {
                        "type": "string",
                        "description": "Repository name (e.g. 'whisper')"
                    }
                },
                "required": ["owner", "repo"]
            }),
        });
    }

    tools
}

/// Return the check_deploy_status tool declaration (Code mode).
pub fn check_deploy_status_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "check_deploy_status".to_string(),
        description: "Check the status of your latest Railway deployments. Shows whether your last push built and deployed successfully, is still building, or failed.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

/// Return the get_deploy_logs tool declaration (Code mode).
pub fn get_deploy_logs_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "get_deploy_logs".to_string(),
        description: "Get the build logs for a Railway deployment. Use this after check_deploy_status shows a failed build to understand what went wrong. If no deployment_id is given, fetches logs for the latest deployment.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "deployment_id": {
                    "type": "string",
                    "description": "Optional deployment ID to get logs for. If omitted, gets the latest deployment's logs."
                }
            }
        }),
    }
}

/// Return the trigger_redeploy tool declaration (Code mode).
pub fn trigger_redeploy_tool() -> FunctionDeclaration {
    FunctionDeclaration {
        name: "trigger_redeploy".to_string(),
        description: "Trigger a redeployment of your Railway service. Use this if you need to rebuild without pushing new code.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
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
