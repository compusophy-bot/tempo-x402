//! Structured feedback loop: record plan outcomes, classify errors, extract lessons.
//!
//! Every completed or failed plan generates a structured outcome record.
//! Lessons are extracted from outcomes and fed back into goal creation and planning
//! prompts, creating a genuine learning loop.

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;
#[cfg(test)]
use crate::plan::PlanStep;
use crate::plan::{Plan, PlanStatus};

/// Structured outcome of a completed or failed plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOutcome {
    pub id: String,
    pub plan_id: String,
    pub goal_id: String,
    pub goal_description: String,
    pub status: String,
    /// Which step types succeeded.
    pub steps_succeeded: Vec<String>,
    /// Which step types failed (if any).
    pub steps_failed: Vec<String>,
    /// Classified error category (if failed).
    pub error_category: Option<ErrorCategory>,
    /// Raw error message (if failed).
    pub error_message: Option<String>,
    /// Extracted lesson from this outcome.
    pub lesson: String,
    /// Total steps in the plan.
    pub total_steps: usize,
    /// Steps completed before finish/failure.
    pub steps_completed: usize,
    /// Replan count.
    pub replan_count: u32,
    pub created_at: i64,
}

/// Error classification for structured learning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Compilation error — Rust code didn't compile.
    CompileError,
    /// Test failure — code compiled but tests failed.
    TestFailure,
    /// File not found or wrong path.
    FileNotFound,
    /// Shell command failed.
    ShellError,
    /// Peer/network operation failed.
    NetworkError,
    /// Protected file — tried to edit a guarded file.
    ProtectedFile,
    /// Endpoint creation error (duplicate, max cap, etc).
    EndpointError,
    /// Git error (branch, commit, push).
    GitError,
    /// LLM produced unparseable output.
    LlmParseError,
    /// Unsolvable — no peers, missing config, etc.
    Unsolvable,
    /// Unknown / uncategorized.
    Unknown,
}

impl ErrorCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CompileError => "compile_error",
            Self::TestFailure => "test_failure",
            Self::FileNotFound => "file_not_found",
            Self::ShellError => "shell_error",
            Self::NetworkError => "network_error",
            Self::ProtectedFile => "protected_file",
            Self::EndpointError => "endpoint_error",
            Self::GitError => "git_error",
            Self::LlmParseError => "llm_parse_error",
            Self::Unsolvable => "unsolvable",
            Self::Unknown => "unknown",
        }
    }
}

/// Classify an error message into a category.
pub fn classify_error(error: &str) -> ErrorCategory {
    let e = error.to_lowercase();

    if e.contains("cargo check")
        || e.contains("compile")
        || e.contains("cannot find")
        || e.contains("unresolved import")
        || e.contains("expected") && e.contains("found")
        || e.contains("e0")
    // Rust error codes
    {
        ErrorCategory::CompileError
    } else if e.contains("test") && (e.contains("fail") || e.contains("panic")) {
        ErrorCategory::TestFailure
    } else if e.contains("no such file") || e.contains("not found") || e.contains("does not exist")
    {
        ErrorCategory::FileNotFound
    } else if e.contains("protected") || e.contains("guard") || e.contains("cannot modify") {
        ErrorCategory::ProtectedFile
    } else if e.contains("peers found: 0")
        || e.contains("unable to auto-detect")
        || e.contains("unsolvable")
    {
        ErrorCategory::Unsolvable
    } else if e.contains("peer")
        || e.contains("network")
        || e.contains("connection")
        || e.contains("timeout")
        || e.contains("reqwest")
    {
        ErrorCategory::NetworkError
    } else if e.contains("endpoint")
        || e.contains("duplicate")
        || e.contains("max cap")
        || e.contains("similar slug")
    {
        ErrorCategory::EndpointError
    } else if e.contains("git")
        || e.contains("branch")
        || e.contains("commit")
        || e.contains("push")
    {
        ErrorCategory::GitError
    } else if e.contains("parse")
        || e.contains("json")
        || e.contains("invalid")
        || e.contains("unexpected token")
    {
        ErrorCategory::LlmParseError
    } else if e.contains("command")
        || e.contains("exit code")
        || e.contains("shell")
        || e.contains("permission denied")
    {
        ErrorCategory::ShellError
    } else if e.contains("429")
        || e.contains("rate limit")
        || e.contains("resource_exhausted")
        || e.contains("too many requests")
    {
        ErrorCategory::NetworkError
    } else if e.contains("replan")
        || e.contains("failed")
        || e.contains("error")
        || e.contains("could not")
    {
        // Catch-all for error messages that don't match specific patterns
        // but clearly indicate failure — better than "unknown"
        ErrorCategory::ShellError
    } else {
        ErrorCategory::Unknown
    }
}

/// Extract a human-readable lesson from a plan outcome.
pub fn extract_lesson(plan: &Plan, goal_desc: &str, error: Option<&str>) -> String {
    match &plan.status {
        PlanStatus::Completed => {
            let step_types: Vec<String> = plan.steps.iter().map(|s| s.summary()).collect();
            format!(
                "SUCCESS: '{}' completed in {} steps ({}). Approach worked.",
                truncate(goal_desc, 80),
                plan.steps.len(),
                step_types.join(" → ")
            )
        }
        PlanStatus::Failed => {
            let category = error.map(classify_error).unwrap_or(ErrorCategory::Unknown);
            let failed_at = if plan.current_step < plan.steps.len() {
                plan.steps[plan.current_step].summary()
            } else {
                "unknown step".to_string()
            };
            format!(
                "FAILURE: '{}' failed at step {} ({}) — {} error: {}. Had {} replans. Avoid this approach.",
                truncate(goal_desc, 60),
                plan.current_step,
                failed_at,
                category.as_str(),
                truncate(error.unwrap_or("unknown"), 100),
                plan.replan_count,
            )
        }
        _ => format!(
            "INCOMPLETE: '{}' — status: {:?}",
            truncate(goal_desc, 80),
            plan.status
        ),
    }
}

/// Record a plan outcome to the database.
pub fn record_outcome(db: &SoulDatabase, plan: &Plan, goal_desc: &str, error: Option<&str>) {
    let now = chrono::Utc::now().timestamp();
    let lesson = extract_lesson(plan, goal_desc, error);

    let steps_succeeded: Vec<String> = plan.steps[..plan.current_step.min(plan.steps.len())]
        .iter()
        .map(|s| s.summary())
        .collect();

    let steps_failed: Vec<String> =
        if plan.current_step < plan.steps.len() && matches!(plan.status, PlanStatus::Failed) {
            vec![plan.steps[plan.current_step].summary()]
        } else {
            vec![]
        };

    let error_category = error.map(classify_error);

    let outcome = PlanOutcome {
        id: uuid::Uuid::new_v4().to_string(),
        plan_id: plan.id.clone(),
        goal_id: plan.goal_id.clone(),
        goal_description: goal_desc.to_string(),
        status: plan.status.as_str().to_string(),
        steps_succeeded,
        steps_failed,
        error_category,
        error_message: error.map(|e| truncate(e, 500).to_string()),
        lesson,
        total_steps: plan.steps.len(),
        steps_completed: plan.current_step,
        replan_count: plan.replan_count,
        created_at: now,
    };

    if let Err(e) = db.insert_plan_outcome(&outcome) {
        tracing::warn!(error = %e, "Failed to record plan outcome");
    }
}

/// Consult past experience before creating a new plan.
/// Returns a formatted string of relevant lessons for the prompt.
pub fn consult_experience(db: &SoulDatabase, goal_desc: &str) -> String {
    let outcomes = match db.get_recent_plan_outcomes(20) {
        Ok(o) => o,
        Err(_) => return String::new(),
    };

    if outcomes.is_empty() {
        return String::new();
    }

    // Compute word overlap to find relevant outcomes
    let goal_lower = goal_desc.to_lowercase();
    let goal_words: std::collections::HashSet<&str> = goal_lower.split_whitespace().collect();

    let mut relevant: Vec<(f64, &PlanOutcome)> = outcomes
        .iter()
        .map(|o| {
            let desc_lower = o.goal_description.to_lowercase();
            let outcome_words: std::collections::HashSet<&str> =
                desc_lower.split_whitespace().collect();
            // We need owned strings for comparison since goal_words borrows from a local
            let intersection = goal_words
                .iter()
                .filter(|w| outcome_words.contains(w.to_owned()))
                .count();
            let union = goal_words.len() + outcome_words.len() - intersection;
            let similarity = if union > 0 {
                intersection as f64 / union as f64
            } else {
                0.0
            };
            (similarity, o)
        })
        .collect();

    // Sort by relevance, take top 5
    relevant.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<&PlanOutcome> = relevant.iter().take(5).map(|(_, o)| *o).collect();

    // Also include all failures (even if not "relevant") — they're always useful
    let failures: Vec<&PlanOutcome> = outcomes
        .iter()
        .filter(|o| o.status == "failed")
        .take(5)
        .collect();

    let mut lines = vec!["# Past Experience (learn from this)".to_string()];

    // Error pattern summary
    let error_counts = count_error_categories(&outcomes);
    if !error_counts.is_empty() {
        lines.push("## Error Patterns".to_string());
        for (cat, count) in &error_counts {
            lines.push(format!("- {cat}: {count} occurrences"));
        }
    }

    // Success rate
    let total = outcomes.len();
    let successes = outcomes.iter().filter(|o| o.status == "completed").count();
    let fail_count = outcomes.iter().filter(|o| o.status == "failed").count();
    lines.push(format!(
        "## Track Record: {successes}/{total} plans succeeded, {fail_count} failed"
    ));

    // Relevant lessons
    if !top.is_empty() {
        lines.push("## Relevant Lessons".to_string());
        for o in &top {
            lines.push(format!("- {}", o.lesson));
        }
    }

    // All failure lessons
    if !failures.is_empty() {
        lines.push("## Recent Failures (DO NOT REPEAT)".to_string());
        for o in &failures {
            lines.push(format!("- {}", o.lesson));
        }
    }

    lines.join("\n")
}

/// Count error categories across outcomes.
fn count_error_categories(outcomes: &[PlanOutcome]) -> Vec<(String, u32)> {
    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for o in outcomes {
        if let Some(ref cat) = o.error_category {
            *counts.entry(cat.as_str().to_string()).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<(String, u32)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted
}

/// Collect lessons from peer agents (stored during discover_peers).
/// Returns a formatted string for prompt injection.
pub fn collect_peer_lessons(db: &SoulDatabase) -> String {
    // Scan soul_state for peer_lessons_* keys
    let all_state = db.get_all_state().unwrap_or_default();
    let mut peer_sections = Vec::new();

    for (key, value) in &all_state {
        if !key.starts_with("peer_lessons_") {
            continue;
        }
        let peer_id = key.strip_prefix("peer_lessons_").unwrap_or(key);
        let parsed: serde_json::Value = match serde_json::from_str(value) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mut lines = Vec::new();

        // Extract lessons from outcomes
        if let Some(outcomes) = parsed.get("outcomes").and_then(|v| v.as_array()) {
            for o in outcomes.iter().take(5) {
                let status = o.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                let lesson = o.get("lesson").and_then(|v| v.as_str()).unwrap_or("");
                if !lesson.is_empty() {
                    lines.push(format!("  - [{status}] {lesson}"));
                }
            }
        }

        // Extract capability strengths
        if let Some(profile) = parsed.get("capability_profile") {
            if let Some(caps) = profile.as_object() {
                let strong: Vec<String> = caps
                    .iter()
                    .filter_map(|(k, v)| {
                        let rate = v.get("success_rate").and_then(|r| r.as_f64())?;
                        if rate > 0.7 {
                            Some(format!("{k}({rate:.0}%)"))
                        } else {
                            None
                        }
                    })
                    .collect();
                if !strong.is_empty() {
                    lines.push(format!("  Strengths: {}", strong.join(", ")));
                }
            }
        }

        // Extract benchmark score
        if let Some(bench) = parsed.get("benchmark") {
            let pass = bench
                .get("pass_at_1")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if pass > 0.0 {
                lines.push(format!("  HumanEval: {pass:.1}%"));
            }
        }

        if !lines.is_empty() {
            peer_sections.push(format!("Peer {peer_id}:\n{}", lines.join("\n")));
        }
    }

    if peer_sections.is_empty() {
        return String::new();
    }

    format!(
        "# Peer Intelligence (lessons from sibling agents)\n{}",
        peer_sections.join("\n")
    )
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_error() {
        assert_eq!(
            classify_error("cargo check failed: unresolved import"),
            ErrorCategory::CompileError
        );
        assert_eq!(
            classify_error("test xyz failed with panic"),
            ErrorCategory::TestFailure
        );
        assert_eq!(
            classify_error("Peers found: 0 — no siblings available"),
            ErrorCategory::Unsolvable
        );
        assert_eq!(
            classify_error("No such file or directory: /foo/bar"),
            ErrorCategory::FileNotFound
        );
        assert_eq!(
            classify_error("protected by guard: soul core file"),
            ErrorCategory::ProtectedFile
        );
        assert_eq!(
            classify_error("something random happened"),
            ErrorCategory::Unknown
        );
    }

    #[test]
    fn test_extract_lesson_completed() {
        let plan = Plan {
            id: "p1".into(),
            goal_id: "g1".into(),
            steps: vec![PlanStep::ReadFile {
                path: "foo.rs".into(),
                store_as: Some("src".into()),
            }],
            current_step: 1,
            status: PlanStatus::Completed,
            context: Default::default(),
            replan_count: 0,
            created_at: 0,
            updated_at: 0,
        };
        let lesson = extract_lesson(&plan, "improve error handling", None);
        assert!(lesson.starts_with("SUCCESS:"));
        assert!(lesson.contains("improve error handling"));
    }

    #[test]
    fn test_extract_lesson_failed() {
        let plan = Plan {
            id: "p1".into(),
            goal_id: "g1".into(),
            steps: vec![
                PlanStep::ReadFile {
                    path: "foo.rs".into(),
                    store_as: None,
                },
                PlanStep::CargoCheck {
                    store_as: Some("check".into()),
                },
            ],
            current_step: 1,
            status: PlanStatus::Failed,
            context: Default::default(),
            replan_count: 2,
            created_at: 0,
            updated_at: 0,
        };
        let lesson = extract_lesson(&plan, "fix compilation", Some("cargo check failed: E0433"));
        assert!(lesson.starts_with("FAILURE:"));
        assert!(lesson.contains("compile_error"));
    }
}
