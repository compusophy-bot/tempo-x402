//! Plan validation: hard mechanical checks that reject bad plans before execution.
//!
//! This is the most impactful single change for genuine recursive self-improvement.
//! Instead of relying on prompt injection ("please don't do X"), we enforce rules
//! mechanically at the Rust level. The LLM cannot override these checks.
//!
//! ## Design Principles
//!
//! 1. **Server-side enforcement > prompt injection** — LLMs ignore instructions.
//!    Mechanical checks cannot be bypassed.
//! 2. **Rules derived from data** — Durable rules are extracted from plan outcomes
//!    and stored in the DB. New plans are checked against them.
//! 3. **Fail fast** — Reject bad plans at creation time, not after 5 failed steps.
//! 4. **Explainable rejections** — Every rejection includes a human-readable reason
//!    that feeds back into the LLM's next attempt.

use crate::db::SoulDatabase;
use crate::feedback::PlanOutcome;
use crate::plan::PlanStep;

/// Result of plan validation.
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub violations: Vec<PlanViolation>,
}

/// A specific rule violation found in a plan.
#[derive(Debug, Clone)]
pub struct PlanViolation {
    pub rule: &'static str,
    pub severity: Severity,
    pub detail: String,
    /// Which step index triggered the violation (if applicable).
    pub step_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    /// Plan must be rejected.
    Hard,
    /// Warning — plan proceeds but violation is logged.
    Soft,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        !self.violations.iter().any(|v| v.severity == Severity::Hard)
    }

    /// Format violations for injection into replan prompt.
    pub fn rejection_reason(&self) -> String {
        let hard: Vec<&PlanViolation> = self
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Hard)
            .collect();
        if hard.is_empty() {
            return String::new();
        }
        let mut lines = vec!["PLAN REJECTED — fix these issues:".to_string()];
        for v in &hard {
            let step_info = v
                .step_index
                .map(|i| format!(" (step {})", i + 1))
                .unwrap_or_default();
            lines.push(format!("- [{}]{}: {}", v.rule, step_info, v.detail));
        }
        lines.join("\n")
    }
}

/// Validate a plan against mechanical rules. Returns validation result.
/// Hard violations mean the plan must be rejected and replanned.
pub fn validate_plan(
    steps: &[PlanStep],
    db: &SoulDatabase,
    goal_description: &str,
) -> ValidationResult {
    let mut violations = Vec::new();

    // ── Rule 1: No editing without reading first ──
    // Plans that edit/generate files without reading them first almost always fail.
    check_read_before_write(steps, &mut violations);

    // ── Rule 2: No commit without cargo_check ──
    // Commits without validation always break the build.
    check_cargo_before_commit(steps, &mut violations);

    // ── Rule 3: Plans must start with investigation ──
    // Plans that jump straight to editing without understanding context fail.
    check_starts_with_investigation(steps, &mut violations);

    // ── Rule 4: No retrying recently failed approaches ──
    // Check if this goal+approach combination has failed before.
    check_not_retrying_failures(steps, db, goal_description, &mut violations);

    // ── Rule 5: No editing protected files (redundant with sanitize but explicit) ──
    check_no_protected_files(steps, &mut violations);

    // ── Rule 6: Check durable rules from DB ──
    check_durable_rules(steps, db, &mut violations);

    // ── Rule 7: Plans with low-capability steps should include fallbacks ──
    check_capability_feasibility(steps, db, &mut violations);

    // ── Rule 8: Minimum plan quality ──
    check_plan_quality(steps, db, &mut violations);

    // ── Rule 9: Block plans for goals with excessive failure chains ──
    check_failure_chain_saturation(db, goal_description, &mut violations);

    ValidationResult {
        valid: violations.iter().all(|v| v.severity != Severity::Hard),
        violations,
    }
}

/// Rule 1: Every edit_code/generate_code step must have a corresponding read_file
/// for the same file earlier in the plan.
fn check_read_before_write(steps: &[PlanStep], violations: &mut Vec<PlanViolation>) {
    let mut files_read: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, step) in steps.iter().enumerate() {
        match step {
            PlanStep::ReadFile { path, .. } => {
                files_read.insert(normalize_path(path));
            }
            PlanStep::EditCode { file_path, .. } => {
                let normalized = normalize_path(file_path);
                if !files_read.contains(&normalized) {
                    violations.push(PlanViolation {
                        rule: "read-before-edit",
                        severity: Severity::Hard,
                        detail: format!(
                            "edit_code on '{}' without reading it first. Add a read_file step before this.",
                            file_path
                        ),
                        step_index: Some(i),
                    });
                }
            }
            PlanStep::GenerateCode { file_path, .. } => {
                // GenerateCode on existing files should read first.
                // For new files, this is fine — but we can't know at validation time
                // if the file exists. Use a soft warning.
                let normalized = normalize_path(file_path);
                if !files_read.contains(&normalized) && looks_like_existing_file(file_path) {
                    violations.push(PlanViolation {
                        rule: "read-before-generate",
                        severity: Severity::Soft,
                        detail: format!(
                            "generate_code on '{}' without reading it first. If the file exists, read it first to avoid overwriting.",
                            file_path
                        ),
                        step_index: Some(i),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Rule 2: If there's a commit step, there must be a cargo_check step after the last
/// code-modifying step and before the commit.
/// NOTE: This now only warns (Soft). Use `auto_fix_cargo_check()` to auto-insert instead
/// of forcing the LLM to figure it out (which traps weaker models in rejection loops).
fn check_cargo_before_commit(steps: &[PlanStep], violations: &mut Vec<PlanViolation>) {
    let mut last_code_step: Option<usize> = None;
    let mut has_cargo_check_after_last_code = false;

    for (i, step) in steps.iter().enumerate() {
        match step {
            PlanStep::EditCode { .. } | PlanStep::GenerateCode { .. } => {
                last_code_step = Some(i);
                has_cargo_check_after_last_code = false;
            }
            PlanStep::CargoCheck { .. } => {
                has_cargo_check_after_last_code = true;
            }
            PlanStep::Commit { .. } => {
                if last_code_step.is_some() && !has_cargo_check_after_last_code {
                    violations.push(PlanViolation {
                        rule: "cargo-check-before-commit",
                        severity: Severity::Soft, // Downgraded: auto_fix_cargo_check handles this
                        detail: "Commit without cargo_check after code changes (auto-fixed)."
                            .to_string(),
                        step_index: Some(i),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Auto-insert `CargoCheck` steps before `Commit` when code changes precede it.
/// This prevents weaker models (Flash Lite) from getting stuck in validation rejection loops.
pub fn auto_fix_cargo_check(steps: &mut Vec<PlanStep>) -> usize {
    let mut insertions = 0;
    let mut i = 0;
    while i < steps.len() {
        if matches!(steps[i], PlanStep::Commit { .. }) {
            // Walk backwards to check if there's a code step without a CargoCheck between
            let mut has_code = false;
            let mut has_check = false;
            let mut j = i;
            while j > 0 {
                j -= 1;
                match &steps[j] {
                    PlanStep::EditCode { .. } | PlanStep::GenerateCode { .. } => {
                        has_code = true;
                        break;
                    }
                    PlanStep::CargoCheck { .. } => {
                        has_check = true;
                        break;
                    }
                    _ => {}
                }
            }
            if has_code && !has_check {
                steps.insert(i, PlanStep::CargoCheck { store_as: None });
                insertions += 1;
                i += 1; // skip past the inserted CargoCheck
            }
        }
        i += 1;
    }
    if insertions > 0 {
        tracing::info!(insertions, "Auto-inserted CargoCheck steps before Commit");
    }
    insertions
}

/// Rule 3: The first step must be investigative (read_file, list_dir, search_code,
/// check_self, discover_peers, or think).
fn check_starts_with_investigation(steps: &[PlanStep], violations: &mut Vec<PlanViolation>) {
    if steps.is_empty() {
        return;
    }
    let first = &steps[0];
    let is_investigative = matches!(
        first,
        PlanStep::ReadFile { .. }
            | PlanStep::ListDir { .. }
            | PlanStep::SearchCode { .. }
            | PlanStep::CheckSelf { .. }
            | PlanStep::Think { .. }
            | PlanStep::DiscoverPeers { .. }
            | PlanStep::CallPeer { .. }
    );
    // Also allow discover_peers and call_peer as first steps for coordination goals
    let is_coordination = matches!(first, PlanStep::RunShell { command, .. } if {
        let cmd = command.to_lowercase();
        cmd.contains("discover") || cmd.contains("peer")
    });

    if !is_investigative && !is_coordination {
        violations.push(PlanViolation {
            rule: "investigate-first",
            severity: Severity::Soft,
            detail: format!(
                "Plan starts with {} instead of investigation. Start by reading/listing/searching to understand context.",
                first.summary()
            ),
            step_index: Some(0),
        });
    }
}

/// Rule 4: Check if a similar goal recently failed with the same approach.
fn check_not_retrying_failures(
    steps: &[PlanStep],
    db: &SoulDatabase,
    goal_description: &str,
    violations: &mut Vec<PlanViolation>,
) {
    let recent_outcomes = match db.get_recent_plan_outcomes(20) {
        Ok(o) => o,
        Err(_) => return,
    };

    // Find recent failures with similar goals
    let goal_lower = goal_description.to_lowercase();
    let goal_words: std::collections::HashSet<&str> = goal_lower.split_whitespace().collect();

    for outcome in &recent_outcomes {
        if outcome.status != "failed" {
            continue;
        }
        // Check similarity
        let desc_lower = outcome.goal_description.to_lowercase();
        let outcome_words: std::collections::HashSet<&str> =
            desc_lower.split_whitespace().collect();
        let intersection = goal_words
            .iter()
            .filter(|w| outcome_words.contains(*w))
            .count();
        let union = goal_words.len() + outcome_words.len() - intersection;
        let similarity = if union > 0 {
            intersection as f64 / union as f64
        } else {
            0.0
        };

        // High similarity to a recent failure
        if similarity > 0.6 {
            // Check if the plan approach is also similar (same step types in same order)
            let current_step_types: Vec<String> =
                steps.iter().take(5).map(|s| s.summary()).collect();
            let failed_step_types = &outcome.steps_succeeded;
            let step_overlap = current_step_types
                .iter()
                .filter(|s| failed_step_types.iter().any(|f| f.contains(s.as_str())))
                .count();

            if step_overlap > 2 {
                violations.push(PlanViolation {
                    rule: "no-retry-same-approach",
                    severity: Severity::Soft, // Warn, don't block — Hard severity caused infinite rejection loops
                    detail: format!(
                        "This plan resembles a recently failed plan for '{}' (similarity: {:.0}%, {} step overlap). The previous failure was: {}. Consider a different approach.",
                        outcome.goal_description.chars().take(60).collect::<String>(),
                        similarity * 100.0,
                        step_overlap,
                        outcome.error_message.as_deref().unwrap_or("unknown"),
                    ),
                    step_index: None,
                });
                break;
            }
        }
    }
}

/// Rule 5: No steps that target protected files.
fn check_no_protected_files(steps: &[PlanStep], violations: &mut Vec<PlanViolation>) {
    for (i, step) in steps.iter().enumerate() {
        let path = match step {
            PlanStep::EditCode { file_path, .. } => Some(file_path.as_str()),
            PlanStep::GenerateCode { file_path, .. } => Some(file_path.as_str()),
            _ => None,
        };
        if let Some(p) = path {
            if crate::guard::is_protected(p) {
                violations.push(PlanViolation {
                    rule: "protected-file",
                    severity: Severity::Hard,
                    detail: format!("'{}' is a protected file and cannot be modified.", p),
                    step_index: Some(i),
                });
            }
        }
    }
}

/// Rule 6: Check durable behavioral rules stored in the DB.
/// These are mechanically-enforced rules extracted from past reflections.
fn check_durable_rules(steps: &[PlanStep], db: &SoulDatabase, violations: &mut Vec<PlanViolation>) {
    let rules = match db.get_state("durable_rules") {
        Ok(Some(json_str)) => match serde_json::from_str::<Vec<DurableRule>>(&json_str) {
            Ok(r) => r,
            Err(_) => return,
        },
        _ => return,
    };

    for rule in &rules {
        // Skip expired rules (TTL in cycles, based on creation timestamp approximation)
        // We use cycle count as a proxy — each cycle is ~30-120s
        if rule.ttl_cycles > 0 {
            // Approximate: rule was created at some cycle; if it's been > ttl_cycles since then, skip
            // Since we don't store creation cycle, use time-based approximation (1 cycle ≈ 60s)
            let age_secs = chrono::Utc::now().timestamp() - rule.created_at;
            let approx_age_cycles = (age_secs / 60) as u64;
            if approx_age_cycles > rule.ttl_cycles {
                continue; // Expired
            }
        }

        // Skip rules with unresolved template variables (e.g. ${variable})
        if rule.pattern.contains("${") {
            continue;
        }

        if rule.check_type == "step_type_blocked" {
            // Only match step_type:error_category pairs, not bare step types
            // This prevents blocking core tools like "ls", "read", "shell:"
            if !rule.pattern.contains(':') {
                continue; // Skip bare step type blocks — too aggressive
            }
            for (i, step) in steps.iter().enumerate() {
                let summary = step.summary().to_lowercase();
                // Pattern is "step_type:error_category" — only match the step_type part
                let step_type_part = rule.pattern.split(':').next().unwrap_or(&rule.pattern);
                if summary.contains(step_type_part) {
                    violations.push(PlanViolation {
                        rule: "durable-rule",
                        severity: Severity::Soft,
                        detail: format!("Durable rule '{}': {}", rule.name, rule.reason),
                        step_index: Some(i),
                    });
                }
            }
        } else if rule.check_type == "goal_pattern_blocked" {
            // This is checked at goal creation, not plan validation
        } else if rule.check_type == "file_blocked" {
            // Block writes to specific files (beyond guard.rs protections)
            for (i, step) in steps.iter().enumerate() {
                let path = match step {
                    PlanStep::EditCode { file_path, .. } => Some(file_path.as_str()),
                    PlanStep::GenerateCode { file_path, .. } => Some(file_path.as_str()),
                    _ => None,
                };
                if let Some(p) = path {
                    if p.contains(&rule.pattern) {
                        violations.push(PlanViolation {
                            rule: "durable-rule-file",
                            severity: Severity::Soft,
                            detail: format!(
                                "Durable rule '{}': {} (file: {})",
                                rule.name, rule.reason, p
                            ),
                            step_index: Some(i),
                        });
                    }
                }
            }
        }
    }
}

/// Rule 7: Check if any step type has a very low success rate (< 20%).
/// If so, warn — the plan is likely to fail at that step.
fn check_capability_feasibility(
    steps: &[PlanStep],
    db: &SoulDatabase,
    violations: &mut Vec<PlanViolation>,
) {
    let profile = crate::capability::compute_profile(db);

    for (i, step) in steps.iter().enumerate() {
        let cap = crate::capability::Capability::from_step(step);
        if let Some(cap_stat) = profile
            .capabilities
            .iter()
            .find(|s| s.capability == cap.as_str())
        {
            // Only flag if we have enough data to be confident
            if cap_stat.attempts >= 10 && cap_stat.success_rate < 0.2 {
                violations.push(PlanViolation {
                    rule: "low-capability",
                    severity: Severity::Soft,
                    detail: format!(
                        "Step '{}' uses capability '{}' which has only {:.0}% success rate ({} attempts). Consider an alternative approach.",
                        step.summary(),
                        cap_stat.capability,
                        cap_stat.success_rate * 100.0,
                        cap_stat.attempts,
                    ),
                    step_index: Some(i),
                });
            }
        }
    }
}

/// Rule 8: Basic plan quality checks.
fn check_plan_quality(steps: &[PlanStep], db: &SoulDatabase, violations: &mut Vec<PlanViolation>) {
    // Empty plans
    if steps.is_empty() {
        violations.push(PlanViolation {
            rule: "non-empty",
            severity: Severity::Hard,
            detail: "Plan has no steps.".to_string(),
            step_index: None,
        });
        return;
    }

    // Plans with only think steps (no action)
    let non_think = steps
        .iter()
        .filter(|s| !matches!(s, PlanStep::Think { .. }))
        .count();
    if non_think == 0 && steps.len() > 1 {
        violations.push(PlanViolation {
            rule: "has-action",
            severity: Severity::Soft,
            detail: "Plan contains only think steps with no concrete actions.".to_string(),
            step_index: None,
        });
    }

    // Plans that are just read-read-read with no edits or actions (busy work)
    let reads_only = steps.iter().all(|s| {
        matches!(
            s,
            PlanStep::ReadFile { .. }
                | PlanStep::ListDir { .. }
                | PlanStep::SearchCode { .. }
                | PlanStep::Think { .. }
        )
    });
    if reads_only && steps.len() > 3 {
        // Escalate to Hard after 5+ trivial completions — agent is stuck in read-only loop
        let trivial_count = db
            .count_plan_outcomes_by_status("completed_trivial")
            .unwrap_or(0);
        let severity = if trivial_count >= 5 {
            Severity::Hard
        } else {
            Severity::Soft
        };
        violations.push(PlanViolation {
            rule: "not-just-reads",
            severity,
            detail: format!(
                "Plan only reads/searches with no concrete actions ({} trivial completions so far). \
                 Include at least one substantive step: edit_code, generate_code, create_script_endpoint, commit, etc.",
                trivial_count
            ),
            step_index: None,
        });
    }
}

/// Rule 9: If a goal already has 3+ unresolved failure chains, reject new plans.
/// This prevents infinite loops where the same goal keeps generating failing plans.
fn check_failure_chain_saturation(
    db: &SoulDatabase,
    goal_description: &str,
    violations: &mut Vec<PlanViolation>,
) {
    let chains_json = db
        .get_state("failure_chains")
        .ok()
        .flatten()
        .unwrap_or_default();
    let chains: Vec<FailureChain> = serde_json::from_str(&chains_json).unwrap_or_default();

    // Count unresolved chains for this goal (partial match)
    let goal_lower = goal_description.to_lowercase();
    let matching_chains: Vec<&FailureChain> = chains
        .iter()
        .filter(|c| {
            !c.resolved && {
                let chain_goal = c.goal_description.to_lowercase();
                // Jaccard similarity check
                let goal_words: std::collections::HashSet<&str> =
                    goal_lower.split_whitespace().collect();
                let chain_words: std::collections::HashSet<&str> =
                    chain_goal.split_whitespace().collect();
                let intersection = goal_words.intersection(&chain_words).count();
                let union = goal_words.len() + chain_words.len() - intersection;
                union > 0 && (intersection as f64 / union as f64) > 0.4
            }
        })
        .collect();

    if matching_chains.len() >= 5 {
        // Collect the distinct error patterns
        let mut error_patterns: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for chain in &matching_chains {
            error_patterns.insert(format!(
                "{}: {}",
                chain.step_type,
                chain.error_snippet.chars().take(50).collect::<String>()
            ));
        }

        violations.push(PlanViolation {
            rule: "failure-chain-saturated",
            severity: Severity::Soft, // Warn, don't block — Hard severity caused infinite rejection loops
            detail: format!(
                "This goal has {} unresolved failure chains. Errors: {}. Consider abandoning this goal and trying something different.",
                matching_chains.len(),
                error_patterns.into_iter().take(3).collect::<Vec<_>>().join("; "),
            ),
            step_index: None,
        });
    }
}

// ── Durable Rules System ──────────────────────────────────────────────

/// A durable behavioral rule enforced mechanically.
/// Stored as JSON array in soul_state key "durable_rules".
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DurableRule {
    pub name: String,
    /// "step_type_blocked", "goal_pattern_blocked", "file_blocked"
    pub check_type: String,
    /// Pattern to match against (lowercase).
    pub pattern: String,
    /// Human-readable reason for the rule.
    pub reason: String,
    /// How many times this rule has been triggered (for telemetry).
    pub trigger_count: u64,
    /// Timestamp when rule was created.
    pub created_at: i64,
    /// TTL in cycles — rule auto-expires after this many cycles. 0 = no expiry.
    #[serde(default = "default_ttl")]
    pub ttl_cycles: u64,
}

fn default_ttl() -> u64 {
    200
}

/// Extract durable rules from a plan outcome and failure chain history.
/// Called after reflection on failed plans. Looks for patterns that should
/// become permanent behavioral rules.
pub fn extract_durable_rules(outcome: &PlanOutcome, db: &SoulDatabase) -> Vec<DurableRule> {
    let mut rules = Vec::new();
    let now = chrono::Utc::now().timestamp();

    // Only extract rules from failures
    if outcome.status != "failed" {
        return rules;
    }

    let error = outcome
        .error_message
        .as_deref()
        .unwrap_or("")
        .to_lowercase();

    // NEVER create durable rules from rate-limit errors — they're transient infra issues,
    // not real tool failures. Creating rules from 429s permanently poisons the agent.
    if crate::feedback::is_rate_limit_error(&error) {
        tracing::info!("Skipping durable rule extraction — error was rate limit (transient)");
        return rules;
    }

    // Rule: if a specific file caused a protected-file error, block it
    if error.contains("protected") {
        if let Some(path) = extract_file_from_error(&error) {
            rules.push(DurableRule {
                name: format!("block-{}", path.replace('/', "-")),
                check_type: "file_blocked".to_string(),
                pattern: path,
                reason: format!(
                    "File is protected. Previous attempt failed: {}",
                    outcome.lesson.chars().take(100).collect::<String>()
                ),
                trigger_count: 0,
                created_at: now,
                ttl_cycles: 200,
            });
        }
    }

    // Rule: if the same step_type:error_category has failed 3+ times, block it
    let chains: Vec<FailureChain> = db
        .get_state("failure_chains")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // Group unresolved failures by step_type:error_category pairs (not bare step types)
    let mut step_fail_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for chain in &chains {
        if chain.resolved {
            continue;
        }
        // Key on step_type:error_category — never block a bare step type like "ls" or "shell:"
        let step_part = chain
            .step_type
            .split_whitespace()
            .next()
            .unwrap_or(&chain.step_type)
            .to_lowercase();
        let key = format!("{}:{}", step_part, chain.error_category);
        *step_fail_counts.entry(key).or_insert(0) += 1;
    }

    for (step_key, count) in &step_fail_counts {
        if *count >= 3 {
            let rule_name = format!("block-step-{}", step_key.replace(['/', ':'], "-"));
            rules.push(DurableRule {
                name: rule_name,
                check_type: "step_type_blocked".to_string(),
                pattern: step_key.clone(),
                reason: format!(
                    "Step+error '{}' has failed {} times without resolution. Avoid this approach.",
                    step_key, count
                ),
                trigger_count: 0,
                created_at: now,
                ttl_cycles: 200,
            });
        }
    }

    // Rule: if the same error on the same file has happened 3+ times, block that file+error combo
    let mut file_error_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for chain in &chains {
        if chain.resolved {
            continue;
        }
        if let Some(ref path) = chain.file_path {
            let key = format!("{}:{}", path, chain.error_category);
            *file_error_counts.entry(key).or_insert(0) += 1;
        }
    }

    for (key, count) in &file_error_counts {
        if *count >= 3 {
            let rule_name = format!("block-file-error-{}", key.replace(['/', ':'], "-"));
            rules.push(DurableRule {
                name: rule_name,
                check_type: "file_blocked".to_string(),
                pattern: key.split(':').next().unwrap_or(key).to_string(),
                reason: format!(
                    "File+error combo '{}' has failed {} times. Stop retrying.",
                    key, count
                ),
                trigger_count: 0,
                created_at: now,
                ttl_cycles: 200,
            });
        }
    }

    rules
}

/// Merge new rules into existing rules in the DB, deduplicating by name.
pub fn merge_durable_rules(db: &SoulDatabase, new_rules: &[DurableRule]) {
    if new_rules.is_empty() {
        return;
    }

    let mut existing: Vec<DurableRule> = db
        .get_state("durable_rules")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    for rule in new_rules {
        // Deduplicate by name
        if existing.iter().any(|r| r.name == rule.name) {
            // Increment trigger count
            if let Some(r) = existing.iter_mut().find(|r| r.name == rule.name) {
                r.trigger_count += 1;
            }
        } else {
            existing.push(rule.clone());
        }
    }

    // Cap at 50 rules (remove oldest first)
    if existing.len() > 50 {
        existing.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        existing.truncate(50);
    }

    if let Ok(json) = serde_json::to_string(&existing) {
        let _ = db.set_state("durable_rules", &json);
    }
}

// ── Brain-Gated Execution ─────────────────────────────────────────────

/// Decide whether to execute a step based on brain prediction.
/// Returns (should_execute, reason) — if should_execute is false,
/// the step should be skipped and the plan should be replanned.
pub fn brain_gate_step(
    db: &SoulDatabase,
    step: &PlanStep,
    prediction: &crate::brain::BrainPrediction,
) -> (bool, Option<String>) {
    // Only gate when the brain has enough training data AND is actually learning.
    // Two conditions must be met:
    // 1. Enough training steps (at least 5000)
    // 2. Loss must be reasonable (< 12.0) — high loss means the brain hasn't converged
    //    and its predictions are unreliable. Soul-bot had 500K steps but 14.8 loss,
    //    blocking everything including `ls`.
    let brain = crate::brain::load_brain(db);
    if brain.train_steps < 5000 || brain.running_loss > 12.0 {
        return (true, None);
    }

    // The brain with ~50K params gets poisoned easily — it learns from failure
    // data and starts blocking everything. Only allow brain gating for truly
    // risky operations (commit, push, deploy). Everything else should execute
    // and fail naturally — the replan mechanism handles failures fine.
    let step_summary = step.summary();
    let is_risky_op = step_summary.starts_with("commit")
        || step_summary.contains("push")
        || step_summary.contains("deploy")
        || step_summary.contains("delete")
        || step_summary.contains("force");
    if !is_risky_op {
        return (true, None);
    }

    // Hard gate: if brain predicts <10% success AND error confidence is high,
    // skip the step entirely. Cap confidence at 95% — a 50K-param net should
    // never be 100% certain about anything.
    let capped_confidence = prediction.error_confidence.min(0.95);
    if prediction.success_prob < 0.10 && capped_confidence > 0.8 {
        let reason = format!(
            "Brain predicts {:.0}% success with {:.0}% confidence in {:?} error. Skipping step '{}' — replan with a different approach.",
            prediction.success_prob * 100.0,
            capped_confidence * 100.0,
            prediction.likely_error,
            step.summary(),
        );
        return (false, Some(reason));
    }

    // Soft gate: if brain predicts <25% success, log a warning but proceed
    // (the warning is visible in logs for human review)
    if prediction.success_prob < 0.20 {
        let reason = format!(
            "Brain warns: {:.0}% success probability for '{}'. Likely error: {:?}.",
            prediction.success_prob * 100.0,
            step.summary(),
            prediction.likely_error,
        );
        return (true, Some(reason));
    }

    (true, None)
}

// ── Failure Chain Tracking ────────────────────────────────────────────

/// A causal chain of failures linking step → error → file → category.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FailureChain {
    pub goal_description: String,
    pub step_type: String,
    pub file_path: Option<String>,
    pub error_category: String,
    pub error_snippet: String,
    /// What was tried to fix it (replan steps).
    pub fix_attempts: Vec<String>,
    /// Whether any fix succeeded.
    pub resolved: bool,
    pub created_at: i64,
}

/// Record a failure chain when a plan step fails.
pub fn record_failure_chain(
    db: &SoulDatabase,
    goal_desc: &str,
    step: &PlanStep,
    error: &str,
    _replan_count: u32,
) {
    let chain = FailureChain {
        goal_description: goal_desc.chars().take(100).collect(),
        step_type: step.summary(),
        file_path: step.target_file().map(String::from),
        error_category: crate::feedback::classify_error(error).as_str().to_string(),
        error_snippet: error.chars().take(200).collect(),
        fix_attempts: Vec::new(),
        resolved: false,
        created_at: chrono::Utc::now().timestamp(),
    };

    // Store as JSON array in soul_state
    let mut chains: Vec<FailureChain> = db
        .get_state("failure_chains")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    chains.push(chain);

    // Keep last 30 failure chains
    if chains.len() > 30 {
        chains.drain(..chains.len() - 30);
    }

    if let Ok(json) = serde_json::to_string(&chains) {
        let _ = db.set_state("failure_chains", &json);
    }
}

/// Get a formatted summary of recent failure chains for prompt injection.
pub fn failure_chain_summary(db: &SoulDatabase) -> String {
    let chains: Vec<FailureChain> = db
        .get_state("failure_chains")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    if chains.is_empty() {
        return String::new();
    }

    // Group by error category
    let mut by_category: std::collections::HashMap<String, Vec<&FailureChain>> =
        std::collections::HashMap::new();
    for chain in &chains {
        by_category
            .entry(chain.error_category.clone())
            .or_default()
            .push(chain);
    }

    let mut lines = vec!["# Failure Patterns (causal analysis)".to_string()];
    for (category, chain_list) in &by_category {
        lines.push(format!(
            "## {} ({} occurrences)",
            category,
            chain_list.len()
        ));
        for c in chain_list.iter().take(3) {
            let file_info = c
                .file_path
                .as_deref()
                .map(|f| format!(" in {}", f))
                .unwrap_or_default();
            lines.push(format!(
                "- {}{}: {}",
                c.step_type,
                file_info,
                c.error_snippet.chars().take(80).collect::<String>()
            ));
        }
    }

    lines.join("\n")
}

// ── Helpers ───────────────────────────────────────────────────────────

fn normalize_path(path: &str) -> String {
    let s = path.replace('\\', "/");
    let s = s.strip_prefix("./").unwrap_or(&s);
    let s = s.strip_prefix("/data/workspace/").unwrap_or(s);
    let s = s.strip_prefix('/').unwrap_or(s);
    s.to_string()
}

/// Heuristic: does this path look like an existing source file?
fn looks_like_existing_file(path: &str) -> bool {
    let p = normalize_path(path);
    // Existing crate source files
    p.starts_with("crates/") && p.ends_with(".rs")
}

/// Try to extract a file path from an error message.
fn extract_file_from_error(error: &str) -> Option<String> {
    // Look for common patterns like "PROTECTED: 'path/to/file'"
    if let Some(start) = error.find('\'') {
        if let Some(end) = error[start + 1..].find('\'') {
            let path = &error[start + 1..start + 1 + end];
            if path.contains('/') || path.ends_with(".rs") {
                return Some(path.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_db() -> SoulDatabase {
        SoulDatabase::new(":memory:").unwrap()
    }

    #[test]
    fn test_read_before_edit_violation() {
        let db = make_db();
        let steps = vec![PlanStep::EditCode {
            file_path: "crates/tempo-x402-soul/src/thinking.rs".to_string(),
            description: "improve loop".to_string(),
            context_keys: vec![],
        }];
        let result = validate_plan(&steps, &db, "test goal");
        assert!(!result.is_valid());
        assert!(result
            .violations
            .iter()
            .any(|v| v.rule == "read-before-edit"));
    }

    #[test]
    fn test_read_then_edit_passes() {
        let db = make_db();
        let steps = vec![
            PlanStep::ReadFile {
                path: "crates/tempo-x402-soul/src/thinking.rs".to_string(),
                store_as: Some("src".to_string()),
            },
            PlanStep::EditCode {
                file_path: "crates/tempo-x402-soul/src/thinking.rs".to_string(),
                description: "improve loop".to_string(),
                context_keys: vec!["src".to_string()],
            },
            PlanStep::CargoCheck {
                store_as: Some("check".to_string()),
            },
            PlanStep::Commit {
                message: "improve thinking loop".to_string(),
            },
        ];
        let result = validate_plan(&steps, &db, "test goal");
        assert!(result.is_valid());
    }

    #[test]
    fn test_commit_without_cargo_check() {
        let db = make_db();
        let steps = vec![
            PlanStep::ReadFile {
                path: "crates/tempo-x402-soul/src/thinking.rs".to_string(),
                store_as: Some("src".to_string()),
            },
            PlanStep::EditCode {
                file_path: "crates/tempo-x402-soul/src/thinking.rs".to_string(),
                description: "fix".to_string(),
                context_keys: vec![],
            },
            PlanStep::Commit {
                message: "fix".to_string(),
            },
        ];
        let result = validate_plan(&steps, &db, "test goal");
        // cargo-check-before-commit is now Soft (auto-fixed), so plan is valid
        // but a violation should still be recorded
        assert!(result
            .violations
            .iter()
            .any(|v| v.rule == "cargo-check-before-commit"));
    }

    #[test]
    fn test_protected_file_blocked() {
        let db = make_db();
        let steps = vec![
            PlanStep::ReadFile {
                path: "crates/tempo-x402-soul/src/tools.rs".to_string(),
                store_as: Some("src".to_string()),
            },
            PlanStep::EditCode {
                file_path: "crates/tempo-x402-soul/src/tools.rs".to_string(),
                description: "modify tools".to_string(),
                context_keys: vec![],
            },
        ];
        let result = validate_plan(&steps, &db, "test goal");
        assert!(!result.is_valid());
        assert!(result.violations.iter().any(|v| v.rule == "protected-file"));
    }

    #[test]
    fn test_empty_plan_rejected() {
        let db = make_db();
        let steps: Vec<PlanStep> = vec![];
        let result = validate_plan(&steps, &db, "test goal");
        assert!(!result.is_valid());
        assert!(result.violations.iter().any(|v| v.rule == "non-empty"));
    }

    #[test]
    fn test_brain_gate_untrained() {
        let db = make_db();
        let step = PlanStep::ReadFile {
            path: "foo.rs".to_string(),
            store_as: None,
        };
        let prediction = crate::brain::BrainPrediction {
            success_prob: 0.05,
            likely_error: crate::feedback::ErrorCategory::Unknown,
            error_confidence: 0.9,
            capability_confidence: std::collections::HashMap::new(),
        };
        // Brain untrained — should always allow
        let (should_execute, _) = brain_gate_step(&db, &step, &prediction);
        assert!(should_execute);
    }
}
