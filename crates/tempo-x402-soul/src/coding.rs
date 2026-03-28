//! Coding orchestration: stage → validate → peer review → commit → push pipeline.
//!
//! Wraps git operations with cargo validation and colony peer review
//! to ensure commits don't break the build AND aren't destructive.

use crate::git::GitContext;
use crate::guard;

/// Result of a coding commit attempt.
#[derive(Debug)]
pub struct CommitResult {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub message: String,
    pub cargo_check_passed: bool,
    pub cargo_test_passed: bool,
    /// Cargo error output (stderr) when check or test fails — for learning.
    pub error_output: Option<String>,
}

/// Minimum seconds between commits. Gives the deploy time to land and be tested.
/// Without this, the agent fires commits every cycle and never validates its own changes.
const COMMIT_COOLDOWN_SECS: i64 = 600; // 10 minutes

/// Maximum cumulative deletions per file over a rolling window.
/// Prevents the "incremental lobotomy" where an agent deletes <50% per commit
/// but cumulatively guts a file across many commits.
const MAX_CUMULATIVE_DELETION_PCT: f64 = 70.0;

/// Check if enough time has passed since the last commit.
pub fn check_commit_cooldown(db: &crate::db::SoulDatabase) -> Result<(), String> {
    let last_commit_at: i64 = db
        .get_state("last_commit_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let now = chrono::Utc::now().timestamp();
    let elapsed = now - last_commit_at;

    if elapsed < COMMIT_COOLDOWN_SECS {
        let remaining = COMMIT_COOLDOWN_SECS - elapsed;
        return Err(format!(
            "Commit cooldown: {remaining}s remaining (minimum {}s between commits). \
             Wait for your last deploy to land and test it first.",
            COMMIT_COOLDOWN_SECS
        ));
    }
    Ok(())
}

/// Record that a commit was made (for cooldown tracking).
pub fn record_commit(db: &crate::db::SoulDatabase) {
    let now = chrono::Utc::now().timestamp();
    let _ = db.set_state("last_commit_at", &now.to_string());

    // Track cumulative commit count for this deploy
    let commit_count: u64 = db
        .get_state("deploy_commit_count")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let _ = db.set_state("deploy_commit_count", &(commit_count + 1).to_string());
}

/// Flag that a benchmark should run after this commit to measure impact.
pub fn request_post_commit_benchmark(db: &crate::db::SoulDatabase) {
    let _ = db.set_state("benchmark_force_next", "1");
    tracing::info!("Post-commit benchmark requested — will measure impact of this change");
}

/// Orchestrate a validated commit: stage → cargo check → cargo test → commit → push.
///
/// If validation fails at any step, reverts changes and returns the error.
pub async fn validated_commit(
    git: &GitContext,
    workspace_root: &str,
    files: &[&str],
    message: &str,
) -> Result<CommitResult, String> {
    // 0. Check commit cooldown — don't rapid-fire commits
    // (The db param isn't available here, so the caller should check cooldown before calling)

    // 1. Validate all files pass the guard
    for file in files {
        guard::validate_write_target(file).map_err(|e| e.to_string())?;
    }

    // 2. Ensure we're on the VM branch
    let branch_result = git.ensure_branch().await?;
    if !branch_result.success {
        return Err(format!(
            "failed to switch to VM branch: {}",
            branch_result.output
        ));
    }

    // 3. Stage files
    let stage_result = git.stage_files(files).await?;
    if !stage_result.success {
        return Err(format!("failed to stage files: {}", stage_result.output));
    }

    // 4. Run cargo check
    let (check_passed, check_errors) = run_cargo_check(workspace_root).await;
    if !check_passed {
        // Revert staged changes
        let _ = git.revert_changes().await;
        let msg = match &check_errors {
            Some(err) => format!("cargo check failed — changes reverted.\nErrors:\n{err}"),
            None => "cargo check failed — changes reverted".to_string(),
        };
        return Ok(CommitResult {
            success: false,
            commit_sha: None,
            message: msg,
            cargo_check_passed: false,
            cargo_test_passed: false,
            error_output: check_errors,
        });
    }

    // 5. Run cargo test
    let (test_passed, test_errors) = run_cargo_test(workspace_root).await;
    if !test_passed {
        let _ = git.revert_changes().await;
        let msg = match &test_errors {
            Some(err) => format!("cargo test failed — changes reverted.\nErrors:\n{err}"),
            None => "cargo test failed — changes reverted".to_string(),
        };
        return Ok(CommitResult {
            success: false,
            commit_sha: None,
            message: msg,
            cargo_check_passed: true,
            cargo_test_passed: false,
            error_output: test_errors,
        });
    }

    // 6. Destruction guard — block commits that delete >50% of any file
    let destruction = check_destruction_guard(workspace_root, files).await;
    if let Err(reason) = destruction {
        let _ = git.revert_changes().await;
        return Ok(CommitResult {
            success: false,
            commit_sha: None,
            message: format!("BLOCKED by destruction guard — changes reverted.\n{reason}"),
            cargo_check_passed: true,
            cargo_test_passed: true,
            error_output: Some(reason),
        });
    }

    // 7. Colony peer review — send diff to peers, require majority approval
    let peer_review = request_colony_review(workspace_root, message).await;
    if let Err(reason) = peer_review {
        let _ = git.revert_changes().await;
        return Ok(CommitResult {
            success: false,
            commit_sha: None,
            message: format!("REJECTED by colony peer review — changes reverted.\n{reason}"),
            cargo_check_passed: true,
            cargo_test_passed: true,
            error_output: Some(reason),
        });
    }

    // 8. Commit
    let commit_result = git.commit(message).await?;
    if !commit_result.success {
        return Err(format!("commit failed: {}", commit_result.output));
    }

    // 7. Get the commit SHA
    let sha = git.head_sha().await?;

    // 8. Push
    let push_result = git.push().await;
    let push_msg = match push_result {
        Ok(r) if r.success => format!("committed and pushed: {sha}"),
        Ok(r) => format!("committed {sha} but push failed: {}", r.output),
        Err(e) => format!("committed {sha} but push failed: {e}"),
    };

    // Build artifacts go to /tmp/x402_cargo_target (not the volume).
    // Clean up after commit to free /tmp space for the next operation.
    let _ = tokio::fs::remove_dir_all("/tmp/x402_cargo_target").await;
    // Also clean any legacy target/ on the volume from old code
    let legacy_target = format!("{workspace_root}/target");
    if tokio::fs::metadata(&legacy_target).await.is_ok() {
        tracing::info!("Cleaning legacy workspace target/ from volume");
        let _ = tokio::fs::remove_dir_all(&legacy_target).await;
    }

    Ok(CommitResult {
        success: true,
        commit_sha: Some(sha),
        message: push_msg,
        cargo_check_passed: true,
        cargo_test_passed: true,
        error_output: None,
    })
}

// ── Destruction Guard ─────────────────────────────────────────────────

/// Block commits that delete more than 50% of any existing file's content.
/// Also checks cumulative changes: compares against the ORIGINAL file from
/// the deploy baseline (not just HEAD), preventing incremental lobotomy
/// where many small commits cumulatively gut a file.
async fn check_destruction_guard(workspace_root: &str, _files: &[&str]) -> Result<(), String> {
    // Get the staged diff with stats
    let output = tokio::process::Command::new("git")
        .args(["diff", "--cached", "--numstat"])
        .current_dir(workspace_root)
        .output()
        .await
        .map_err(|e| format!("failed to get diff stats: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let added: usize = parts[0].parse().unwrap_or(0);
        let deleted: usize = parts[1].parse().unwrap_or(0);
        let file_path = parts[2];

        // Skip new files (all additions, no deletions)
        if deleted == 0 {
            continue;
        }

        // If we're deleting more than we're adding, and deletions are >50% of total change,
        // check the original file size
        if deleted > added && deleted > 20 {
            // Get the original file line count
            let orig = tokio::process::Command::new("git")
                .args(["show", &format!("HEAD:{file_path}")])
                .current_dir(workspace_root)
                .output()
                .await;

            if let Ok(orig_output) = orig {
                if orig_output.status.success() {
                    let orig_lines = String::from_utf8_lossy(&orig_output.stdout).lines().count();
                    if orig_lines > 0 {
                        let deletion_pct = (deleted as f64 / orig_lines as f64) * 100.0;
                        if deletion_pct > 50.0 {
                            return Err(format!(
                                "DESTRUCTION BLOCKED: '{file_path}' would lose {deleted}/{orig_lines} lines ({deletion_pct:.0}%). \
                                 Deleting >50% of a file is not allowed. Make targeted edits instead of rewriting."
                            ));
                        }
                    }
                }
            }
        }
    }

    // ── Cumulative destruction check ──
    // Compare current staged version against the DEPLOY baseline (first commit on this deploy).
    // This catches incremental lobotomy: each commit deletes <50%, but across 10 commits
    // the file loses 90% of its content.
    let deploy_base = get_deploy_baseline_sha(workspace_root).await;
    if let Some(base_sha) = deploy_base {
        // Get cumulative diff against deploy baseline
        let cumulative = tokio::process::Command::new("git")
            .args(["diff", "--numstat", &base_sha, "HEAD"])
            .current_dir(workspace_root)
            .output()
            .await;

        if let Ok(cum_output) = cumulative {
            let cum_stdout = String::from_utf8_lossy(&cum_output.stdout);
            for line in cum_stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 3 {
                    continue;
                }
                let _added: usize = parts[0].parse().unwrap_or(0);
                let deleted: usize = parts[1].parse().unwrap_or(0);
                let file_path = parts[2];

                if deleted < 20 {
                    continue;
                }

                // Get the ORIGINAL file size at deploy baseline
                let orig = tokio::process::Command::new("git")
                    .args(["show", &format!("{base_sha}:{file_path}")])
                    .current_dir(workspace_root)
                    .output()
                    .await;

                if let Ok(orig_output) = orig {
                    if orig_output.status.success() {
                        let orig_lines =
                            String::from_utf8_lossy(&orig_output.stdout).lines().count();
                        if orig_lines > 0 {
                            let cum_deletion_pct = (deleted as f64 / orig_lines as f64) * 100.0;
                            if cum_deletion_pct > MAX_CUMULATIVE_DELETION_PCT {
                                return Err(format!(
                                    "CUMULATIVE DESTRUCTION BLOCKED: '{file_path}' has lost {deleted}/{orig_lines} lines \
                                     ({cum_deletion_pct:.0}%) since deploy baseline. Cumulative deletions exceed {MAX_CUMULATIVE_DELETION_PCT}%. \
                                     You are incrementally gutting this file. Stop and reconsider your approach."
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Get the git SHA from when this deploy started (for cumulative destruction check).
async fn get_deploy_baseline_sha(workspace_root: &str) -> Option<String> {
    // The deploy build SHA is stored in soul_state by reset_deploy_counters.
    // We can also just use the tag/SHA from DEPLOY_BUILD env var.
    // Fallback: use git log to find the commit from ~24h ago.
    let output = tokio::process::Command::new("git")
        .args(["log", "--since=24.hours.ago", "--reverse", "--format=%H"])
        .current_dir(workspace_root)
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().next().map(|s| s.to_string())
}

// ── Colony Peer Review ──────────────────────────────────────────────

/// Review request sent to peers before committing code changes.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CodeReviewRequest {
    /// The commit message describing the change.
    pub message: String,
    /// The unified diff of all staged changes.
    pub diff: String,
    /// Instance ID of the requesting agent.
    pub requester: String,
}

/// Review response from a peer.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CodeReviewResponse {
    /// Whether the peer approves the change.
    pub approved: bool,
    /// Reason for approval or rejection.
    pub reason: String,
    /// Instance ID of the reviewer.
    pub reviewer: String,
}

/// Send staged diff to all known peers for review. Requires majority approval.
/// If no peers are reachable, the commit proceeds (graceful degradation).
async fn request_colony_review(workspace_root: &str, message: &str) -> Result<(), String> {
    // Get the staged diff
    let diff_output = tokio::process::Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(workspace_root)
        .output()
        .await
        .map_err(|e| format!("failed to get diff: {e}"))?;

    let diff = String::from_utf8_lossy(&diff_output.stdout);
    if diff.is_empty() {
        return Ok(()); // nothing to review
    }

    // Truncate diff for network transfer (max 32KB)
    let diff_truncated: String = diff.chars().take(32768).collect();

    let requester = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "unknown".into());

    let review_req = CodeReviewRequest {
        message: message.to_string(),
        diff: diff_truncated,
        requester: requester.clone(),
    };

    // Get peer URLs
    let peer_urls = get_peer_urls_for_review();
    if peer_urls.is_empty() {
        tracing::info!("No peers available for code review — proceeding with commit");
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let mut approvals = 0u32;
    let mut rejections = 0u32;
    let mut rejection_reasons: Vec<String> = Vec::new();
    let total_peers = peer_urls.len() as u32;

    for peer_url in &peer_urls {
        let url = format!("{}/soul/code-review", peer_url.trim_end_matches('/'));
        let resp = client.post(&url).json(&review_req).send().await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(review) = r.json::<CodeReviewResponse>().await {
                    if review.approved {
                        approvals += 1;
                        tracing::info!(reviewer = %review.reviewer, "Peer approved code change");
                    } else {
                        rejections += 1;
                        rejection_reasons.push(format!("{}: {}", review.reviewer, review.reason));
                        tracing::warn!(
                            reviewer = %review.reviewer,
                            reason = %review.reason,
                            "Peer REJECTED code change"
                        );
                    }
                }
            }
            Ok(r) => {
                tracing::debug!(peer = %peer_url, status = %r.status(), "Peer review endpoint unavailable");
                // Non-responsive peers don't count — graceful degradation
            }
            Err(e) => {
                tracing::debug!(peer = %peer_url, error = %e, "Peer unreachable for review");
            }
        }
    }

    let total_votes = approvals + rejections;

    // If no peers responded at all, proceed (graceful degradation)
    if total_votes == 0 {
        tracing::info!(
            peers_tried = total_peers,
            "No peers responded to code review — proceeding"
        );
        return Ok(());
    }

    // Require majority approval among responding peers
    if approvals > rejections {
        tracing::info!(
            approvals,
            rejections,
            "Colony approved code change ({approvals}/{total_votes})"
        );
        Ok(())
    } else {
        let reasons = rejection_reasons.join("\n");
        Err(format!(
            "Colony rejected code change ({rejections}/{total_votes} rejected).\nReasons:\n{reasons}"
        ))
    }
}

/// Get peer URLs from PEER_URLS env var (the static mesh list).
fn get_peer_urls_for_review() -> Vec<String> {
    let our_domain = std::env::var("RAILWAY_PUBLIC_DOMAIN")
        .ok()
        .map(|d| format!("https://{d}"));

    std::env::var("PEER_URLS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| {
            // Skip self
            if let Some(ref our) = our_domain {
                s.trim_end_matches('/') != our.trim_end_matches('/')
            } else {
                true
            }
        })
        .collect()
}

/// Max error output to capture (4KB) — enough to see the error, not flood LLM context.
const MAX_ERROR_OUTPUT: usize = 4096;

/// Run `cargo check` on the soul crate only (not --workspace).
/// The agents only edit soul files — no need to compile all 8 crates.
/// Uses /tmp for target dir to avoid bloating the persistent volume.
pub async fn run_cargo_check(workspace_root: &str) -> (bool, Option<String>) {
    tracing::info!("running cargo check -p tempo-x402-soul...");
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new("cargo")
            .args(["check", "-p", "tempo-x402-soul"])
            .current_dir(workspace_root)
            .env("CARGO_TARGET_DIR", "/tmp/x402_cargo_target")
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!(stderr = %stderr, "cargo check failed");
                let truncated: String = stderr.chars().take(MAX_ERROR_OUTPUT).collect();
                (false, Some(truncated))
            } else {
                (true, None)
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "cargo check failed to run");
            (false, Some(format!("failed to run: {e}")))
        }
        Err(_) => {
            tracing::warn!("cargo check timed out");
            (false, Some("timed out after 300s".to_string()))
        }
    }
}

/// Run `cargo test` on the soul crate only (not --workspace).
/// Uses /tmp for target dir to avoid bloating the persistent volume.
async fn run_cargo_test(workspace_root: &str) -> (bool, Option<String>) {
    tracing::info!("running cargo test -p tempo-x402-soul...");
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new("cargo")
            .args(["test", "-p", "tempo-x402-soul"])
            .current_dir(workspace_root)
            .env("CARGO_TARGET_DIR", "/tmp/x402_cargo_target")
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!(stderr = %stderr, "cargo test failed");
                let truncated: String = stderr.chars().take(MAX_ERROR_OUTPUT).collect();
                (false, Some(truncated))
            } else {
                (true, None)
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "cargo test failed to run");
            (false, Some(format!("failed to run: {e}")))
        }
        Err(_) => {
            tracing::warn!("cargo test timed out");
            (false, Some("timed out after 600s".to_string()))
        }
    }
}
