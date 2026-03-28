//! Git operation tools: commit, propose PR, create issue.
use super::*;

impl ToolExecutor {
    /// Commit changes through the validated pipeline (stage -> cargo check -> cargo test -> commit -> push).
    pub(super) async fn commit_changes(
        &self,
        message: &str,
        files: &[&str],
    ) -> Result<ToolResult, String> {
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

        // Check commit cooldown — prevent rapid-fire commits
        coding::check_commit_cooldown(db)?;

        let workspace = self.workspace_root.to_string_lossy().to_string();
        let result = coding::validated_commit(git, &workspace, files, message).await?;

        // Record commit timestamp for cooldown tracking
        if result.success {
            coding::record_commit(db);
            // Request benchmark to run after this commit — measures impact
            coding::request_post_commit_benchmark(db);
        }

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

        // Lifecycle: track own commits and trigger Fork -> Branch transition
        if result.success {
            let phase = crate::lifecycle::current_phase(db);
            if phase == crate::lifecycle::LifecyclePhase::Fork {
                // First code commit: differentiate! Create own branch.
                let instance_id = git.instance_id();
                if let Some(new_branch) = crate::lifecycle::differentiate(db, instance_id) {
                    tracing::info!(
                        branch = %new_branch,
                        "Lifecycle: Fork → Branch — clone is now differentiating"
                    );
                }
            } else {
                crate::lifecycle::record_own_commit(db);
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: result.message,
            stderr: String::new(),
            exit_code: if result.success { 0 } else { 1 },
            duration_ms,
        })
    }

    /// Create a PR from the VM branch to main.
    pub(super) async fn propose_to_main(
        &self,
        title: &str,
        body: &str,
    ) -> Result<ToolResult, String> {
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

    pub(super) async fn create_issue(
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
}
