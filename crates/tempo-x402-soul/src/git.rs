//! Git operations for branch-per-VM workflow.
//!
//! Each VM operates on a `vm/<instance-id>` branch. Changes are committed
//! and pushed to this branch, never directly to main.

/// Git context for a specific VM instance.
pub struct GitContext {
    workspace_root: String,
    instance_id: String,
    branch_name: String,
    github_token: Option<String>,
}

/// Result of a git operation.
#[derive(Debug)]
pub struct GitResult {
    pub success: bool,
    pub output: String,
}

impl GitContext {
    /// Create a new git context for the given instance.
    pub fn new(workspace_root: String, instance_id: String, github_token: Option<String>) -> Self {
        let branch_name = format!("vm/{instance_id}");
        Self {
            workspace_root,
            instance_id,
            branch_name,
            github_token,
        }
    }

    /// Get the branch name for this VM.
    pub fn branch_name(&self) -> &str {
        &self.branch_name
    }

    /// Get the instance ID.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Ensure the VM branch exists and is checked out.
    /// Creates from current HEAD if it doesn't exist.
    pub async fn ensure_branch(&self) -> Result<GitResult, String> {
        // Check current branch
        let current = self.run_git(&["rev-parse", "--abbrev-ref", "HEAD"]).await?;
        if current.output.trim() == self.branch_name {
            return Ok(GitResult {
                success: true,
                output: format!("already on branch {}", self.branch_name),
            });
        }

        // Try to checkout existing branch
        let checkout = self.run_git(&["checkout", &self.branch_name]).await;

        match checkout {
            Ok(r) if r.success => Ok(r),
            _ => {
                // Create new branch from current HEAD
                let result = self.run_git(&["checkout", "-b", &self.branch_name]).await?;
                if result.success {
                    tracing::info!(branch = %self.branch_name, "created new VM branch");
                }
                Ok(result)
            }
        }
    }

    /// Stage specific files for commit.
    pub async fn stage_files(&self, files: &[&str]) -> Result<GitResult, String> {
        if files.is_empty() {
            return Err("no files to stage".to_string());
        }

        let mut args = vec!["add", "--"];
        args.extend(files);
        self.run_git(&args).await
    }

    /// Get the diff of staged changes.
    pub async fn diff_staged(&self) -> Result<String, String> {
        let result = self.run_git(&["diff", "--cached", "--stat"]).await?;
        Ok(result.output)
    }

    /// Commit staged changes with a message.
    pub async fn commit(&self, message: &str) -> Result<GitResult, String> {
        self.run_git(&["commit", "-m", message]).await
    }

    /// Push the VM branch to origin.
    pub async fn push(&self) -> Result<GitResult, String> {
        // Set up auth if we have a token
        if let Some(token) = &self.github_token {
            self.configure_auth(token).await?;
        }

        self.run_git(&["push", "-u", "origin", &self.branch_name])
            .await
    }

    /// Get the current HEAD SHA.
    pub async fn head_sha(&self) -> Result<String, String> {
        let result = self.run_git(&["rev-parse", "HEAD"]).await?;
        Ok(result.output.trim().to_string())
    }

    /// Reset to a known good SHA (hard reset + force push).
    pub async fn reset_to_last_good(&self, sha: &str) -> Result<GitResult, String> {
        let reset = self.run_git(&["reset", "--hard", sha]).await?;
        if !reset.success {
            return Ok(reset);
        }

        // Force push only the VM branch
        if self.github_token.is_some() {
            self.run_git(&["push", "--force", "origin", &self.branch_name])
                .await
        } else {
            Ok(reset)
        }
    }

    /// Revert all unstaged changes.
    pub async fn revert_changes(&self) -> Result<GitResult, String> {
        self.run_git(&["checkout", "--", "."]).await
    }

    /// Create a PR from the VM branch to main using `gh`.
    pub async fn create_pr(&self, title: &str, body: &str) -> Result<GitResult, String> {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::process::Command::new("gh")
                .args([
                    "pr",
                    "create",
                    "--base",
                    "main",
                    "--head",
                    &self.branch_name,
                    "--title",
                    title,
                    "--body",
                    body,
                ])
                .current_dir(&self.workspace_root)
                .env("GH_TOKEN", self.github_token.as_deref().unwrap_or(""))
                .output(),
        )
        .await
        .map_err(|_| "gh pr create timed out after 30s".to_string())?
        .map_err(|e| format!("gh pr create failed: {e}"))?;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();

        Ok(GitResult {
            success: result.status.success(),
            output: if result.status.success() {
                stdout
            } else {
                format!("{stdout}\n{stderr}")
            },
        })
    }

    /// Configure git auth using the GitHub token.
    async fn configure_auth(&self, token: &str) -> Result<(), String> {
        // Get current remote URL
        let remote = self.run_git(&["remote", "get-url", "origin"]).await?;
        let url = remote.output.trim();

        // If already using token URL, skip
        if url.contains("x-access-token") {
            return Ok(());
        }

        // Convert to token-based URL
        let new_url = if url.starts_with("https://github.com/") {
            url.replace(
                "https://github.com/",
                &format!("https://x-access-token:{token}@github.com/"),
            )
        } else if url.starts_with("git@github.com:") {
            let path = url.strip_prefix("git@github.com:").unwrap_or(url);
            format!("https://x-access-token:{token}@github.com/{path}")
        } else {
            return Err(format!("unexpected remote URL format: {url}"));
        };

        self.run_git(&["remote", "set-url", "origin", &new_url])
            .await?;
        Ok(())
    }

    /// Run a git command in the workspace directory.
    async fn run_git(&self, args: &[&str]) -> Result<GitResult, String> {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            tokio::process::Command::new("git")
                .args(args)
                .current_dir(&self.workspace_root)
                .output(),
        )
        .await
        .map_err(|_| format!("git {} timed out", args.first().unwrap_or(&"")))?
        .map_err(|e| format!("git command failed: {e}"))?;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();

        if !result.status.success() {
            tracing::debug!(
                args = ?args,
                stderr = %stderr,
                "git command failed"
            );
        }

        Ok(GitResult {
            success: result.status.success(),
            output: if stdout.is_empty() { stderr } else { stdout },
        })
    }
}

/// Check if git is available and we're in a git repo.
pub async fn is_git_repo(workspace_root: &str) -> bool {
    let result = tokio::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(workspace_root)
        .output()
        .await;

    matches!(result, Ok(output) if output.status.success())
}

/// Check if the `gh` CLI is available.
pub async fn is_gh_available() -> bool {
    let result = tokio::process::Command::new("gh")
        .arg("--version")
        .output()
        .await;

    matches!(result, Ok(output) if output.status.success())
}

/// Validate that a branch name is safe (vm/<id> pattern only).
pub fn is_valid_vm_branch(branch: &str) -> bool {
    if let Some(id) = branch.strip_prefix("vm/") {
        !id.is_empty()
            && id.len() <= 64
            && id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    } else {
        false
    }
}

/// Check if a path would write to the protected main branch.
pub fn targets_main_branch(refspec: &str) -> bool {
    let r = refspec.to_lowercase();
    r == "main" || r == "master" || r.ends_with(":main") || r.ends_with(":master")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_vm_branches() {
        assert!(is_valid_vm_branch("vm/node-abc123"));
        assert!(is_valid_vm_branch("vm/test_instance"));
        assert!(is_valid_vm_branch("vm/a"));
    }

    #[test]
    fn invalid_vm_branches() {
        assert!(!is_valid_vm_branch("main"));
        assert!(!is_valid_vm_branch("vm/"));
        assert!(!is_valid_vm_branch("feature/foo"));
        assert!(!is_valid_vm_branch("vm/path/with/slashes"));
    }

    #[test]
    fn main_branch_detection() {
        assert!(targets_main_branch("main"));
        assert!(targets_main_branch("master"));
        assert!(targets_main_branch("HEAD:main"));
        assert!(!targets_main_branch("vm/node-abc"));
    }
}
