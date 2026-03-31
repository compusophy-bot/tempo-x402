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

        // Commit readiness gate — blocks until benchmark has measured last commit's impact
        coding::check_commit_readiness(db)?;

        let workspace = self.workspace_root.to_string_lossy().to_string();

        // Code quality gate — predict whether this diff improves the codebase
        // Only gates if model has been trained (>10 steps). Otherwise let commits through.
        {
            let quality_model = crate::code_quality::load_model(db);
            if quality_model.train_steps >= 10 {
                match crate::code_quality::evaluate_diff(db, &workspace).await {
                    Ok(prediction) if prediction.score < -0.1 && prediction.confidence > 0.3 => {
                        return Err(format!(
                            "Code quality gate: predicted regression (score={:.2}, confidence={:.0}%). \
                             The model predicts this change will hurt benchmark performance. \
                             Review your changes and try a more targeted approach.",
                            prediction.score,
                            prediction.confidence * 100.0,
                        ));
                    }
                    Ok(prediction) if prediction.score < 0.0 => {
                        tracing::warn!(
                            score = format!("{:.3}", prediction.score),
                            "Code quality warning: predicted neutral/slight regression, proceeding"
                        );
                    }
                    Ok(_) => {} // Predicted improvement — proceed
                    Err(e) => {
                        tracing::debug!(error = %e, "Code quality evaluation skipped");
                    }
                }
            }
        }

        let result = coding::validated_commit(git, &workspace, files, message).await?;

        // Enter AWAITING_BENCHMARK state — next commit blocked until benchmark runs
        if result.success {
            coding::record_commit(db);

            // Feed committed code to codegen model as training data
            // Get the diff of what was just committed
            if let Ok(diff_out) = tokio::process::Command::new("git")
                .args(["diff", "HEAD~1", "HEAD", "--", "*.rs"])
                .current_dir(&workspace)
                .output()
                .await
            {
                let diff = String::from_utf8_lossy(&diff_out.stdout);
                if !diff.is_empty() {
                    crate::codegen::record_training_example(db, &diff, "commit");
                }
            }
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
