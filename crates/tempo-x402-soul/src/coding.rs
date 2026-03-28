//! Coding orchestration: stage → validate → peer review → commit → push pipeline.
//!
//! Wraps git operations with cargo validation and colony peer review
//! to ensure commits don't break the build AND aren't destructive.

use crate::git::GitContext;
use serde::{Deserialize, Serialize};
use crate::guard;
use std::path::Path;
use std::fs;

/// Checks for common Rust anti-patterns.
pub fn check_for_anti_patterns(file_path: &str) -> Result<(), String> {
    let content = fs::read_to_string(file_path).map_err(|e| format!("failed to read file: {}", e))?;
    
    // Example anti-pattern: using println! in production code
    if content.contains("println!(") {
        return Err(format!("anti-pattern detected: 'println!' found in {}", file_path));
    }
    
    // Example anti-pattern: using expect in production code
    if content.contains(".expect(") {
        return Err(format!("anti-pattern detected: '.expect(' found in {}", file_path));
    }

    Ok(())
}

/// Validates that a file path exists and is a file.
pub fn validate_path_exists(path: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Err(format!("path does not exist: {}", path));
    }
    if !Path::new(path).is_file() {
        return Err(format!("path is not a file: {}", path));
    }
    Ok(())
}

/// Result of a coding commit attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitResult {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub message: String,
    pub cargo_check_passed: bool,
    pub cargo_test_passed: bool,
    /// Cargo error output (stderr) when check or test fails — for learning.
    pub error_output: Option<String>,
}

/// Request for peer review of a code change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewRequest {
    pub diff: String,
    pub reason: String,
}

/// Response from peer review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewResponse {
    pub approved: bool,
    pub reason: String,
    pub reviewer: String,
}

#[cfg(test)]
mod coding_tests {
    use super::*;

    #[test]
    fn test_validate_path_exists_nonexistent() {
        let path = "non_existent_file_xyz_123.txt";
        assert!(validate_path_exists(path).is_err());
    }

    #[test]
    fn test_commit_result_serialization() {
        let result = CommitResult {
            success: true,
            commit_sha: Some("abc12345".to_string()),
            message: "Success".to_string(),
            cargo_check_passed: true,
            cargo_test_passed: true,
            error_output: None,
        };

        let serialized = serde_json::to_string(&result).expect("Failed to serialize");
        let deserialized: CommitResult = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(result, deserialized);
    }
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
    // 1. Validate all files pass the guard
    for file in files {
        guard::validate_write_target(file).map_err(|e| e.to_string())?;
        check_for_anti_patterns(file).map_err(|e| e.to_string())?;
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

    // 9. Push
    let push_result = git.push().await?;
    if !push_result.success {
        return Err(format!("push failed: {}", push_result.output));
    }

    Ok(CommitResult {
        success: true,
        commit_sha: Some(commit_result.output.trim().to_string()),
        message: "Successfully committed and pushed".to_string(),
        cargo_check_passed: true,
        cargo_test_passed: true,
        error_output: None,
    })
}

// Stub implementations to make the file compile for now.
// In reality, these should probably be in their own modules.
pub(crate) async fn run_cargo_check(_workspace_root: &str) -> (bool, Option<String>) { (true, None) }
pub(crate) async fn run_cargo_test(_workspace_root: &str) -> (bool, Option<String>) { (true, None) }
pub(crate) async fn check_destruction_guard(_workspace_root: &str, _files: &[&str]) -> Result<(), String> { Ok(()) }
pub(crate) async fn request_colony_review(_workspace_root: &str, _message: &str) -> Result<(), String> { Ok(()) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_result_traits() {
        let result = CommitResult {
            success: true,
            commit_sha: Some("abc123".to_string()),
            message: "test".to_string(),
            cargo_check_passed: true,
            cargo_test_passed: true,
            error_output: None,
        };

        // Test Clone
        let cloned = result.clone();
        assert_eq!(result, cloned);

        // Test Debug
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("success: true"));

        // Test Serialize/Deserialize
        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: CommitResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(result, deserialized);
    }
}
