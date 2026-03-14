//! Capability tracking: per-skill success rate measurement.
//!
//! Tracks success/failure of individual capabilities (code editing, compilation,
//! peer calls, etc.) to build a profile of what the agent is actually good at.
//! This feeds into planning prompts so the agent can play to its strengths
//! and improve its weaknesses.

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;
use crate::plan::PlanStep;

/// A distinct capability that can be measured.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Reading files successfully.
    FileRead,
    /// Writing/editing files successfully.
    FileWrite,
    /// Peer code review: reviewed another agent's PR.
    PeerReview,
    /// Code accepted: your PR was approved by a peer reviewer.
    CodeAccepted,
    /// Code compiles after changes (cargo check passes).
    CodeCompile,
    /// Tests pass after changes.
    TestPass,
    /// Shell commands succeed.
    ShellExec,
    /// Peer calls succeed.
    PeerCall,
    /// Endpoint creation succeeds.
    EndpointCreate,
    /// Git operations (commit, push, PR) succeed.
    GitOps,
    /// LLM code generation produces valid output.
    CodeGen,
    /// Search/investigation steps succeed.
    CodeSearch,
    /// Plan completes end-to-end.
    PlanComplete,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileRead => "file_read",
            Self::FileWrite => "file_write",
            Self::PeerReview => "peer_review",
            Self::CodeAccepted => "code_accepted",
            Self::CodeCompile => "code_compile",
            Self::TestPass => "test_pass",
            Self::ShellExec => "shell_exec",
            Self::PeerCall => "peer_call",
            Self::EndpointCreate => "endpoint_create",
            Self::GitOps => "git_ops",
            Self::CodeGen => "code_gen",
            Self::CodeSearch => "code_search",
            Self::PlanComplete => "plan_complete",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::FileRead => "File Reading",
            Self::FileWrite => "File Writing",
            Self::PeerReview => "Peer Review",
            Self::CodeAccepted => "Code Accepted",
            Self::CodeCompile => "Compilation",
            Self::TestPass => "Test Passing",
            Self::ShellExec => "Shell Execution",
            Self::PeerCall => "Peer Calls",
            Self::EndpointCreate => "Endpoint Creation",
            Self::GitOps => "Git Operations",
            Self::CodeGen => "Code Generation",
            Self::CodeSearch => "Code Search",
            Self::PlanComplete => "Plan Completion",
        }
    }

    /// Map a plan step to the capability it tests.
    pub fn from_step(step: &PlanStep) -> Self {
        match step {
            PlanStep::ReadFile { .. } => Self::FileRead,
            PlanStep::SearchCode { .. } | PlanStep::ListDir { .. } => Self::CodeSearch,
            PlanStep::RunShell { .. } => Self::ShellExec,
            PlanStep::Commit { .. } => Self::GitOps,
            PlanStep::GenerateCode { .. } => Self::CodeGen,
            PlanStep::EditCode { .. } => Self::FileWrite,
            PlanStep::CargoCheck { .. } => Self::CodeCompile,
            PlanStep::CheckSelf { .. } => Self::ShellExec,
            PlanStep::CreateScriptEndpoint { .. } => Self::EndpointCreate,
            PlanStep::TestScriptEndpoint { .. } => Self::EndpointCreate,
            PlanStep::Think { .. } => Self::CodeGen,
            PlanStep::CallPaidEndpoint { .. }
            | PlanStep::DiscoverPeers { .. }
            | PlanStep::CallPeer { .. } => Self::PeerCall,
            PlanStep::CreateGithubRepo { .. } | PlanStep::ForkGithubRepo { .. } => Self::GitOps,
            PlanStep::DeleteEndpoint { .. } => Self::EndpointCreate,
            PlanStep::Screenshot { .. }
            | PlanStep::ScreenClick { .. }
            | PlanStep::ScreenType { .. }
            | PlanStep::BrowseUrl { .. } => Self::ShellExec, // computer use maps to shell capability for now
            PlanStep::ReviewPeerPR { .. } => Self::PeerReview,
            PlanStep::CloneSelf { .. } => Self::PeerCall,
        }
    }
}

/// A single capability event: one attempt at using a capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityEvent {
    pub id: String,
    pub capability: String,
    pub succeeded: bool,
    pub context: String,
    pub created_at: i64,
}

/// Aggregated capability profile: success rates per capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityProfile {
    pub capabilities: Vec<CapabilityScore>,
    pub overall_success_rate: f64,
    pub strongest: Option<String>,
    pub weakest: Option<String>,
}

/// Score for a single capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityScore {
    pub capability: String,
    pub display_name: String,
    pub attempts: u32,
    pub successes: u32,
    pub success_rate: f64,
}

/// Record a capability attempt.
pub fn record_event(db: &SoulDatabase, capability: &Capability, succeeded: bool, context: &str) {
    let event = CapabilityEvent {
        id: uuid::Uuid::new_v4().to_string(),
        capability: capability.as_str().to_string(),
        succeeded,
        context: context.chars().take(200).collect(),
        created_at: chrono::Utc::now().timestamp(),
    };

    if let Err(e) = db.insert_capability_event(&event) {
        tracing::warn!(error = %e, "Failed to record capability event");
    }
}

/// Record a step result as a capability event.
pub fn record_step_result(db: &SoulDatabase, step: &PlanStep, succeeded: bool, context: &str) {
    let cap = Capability::from_step(step);
    record_event(db, &cap, succeeded, context);
}

/// Compute the current capability profile from recent events.
pub fn compute_profile(db: &SoulDatabase) -> CapabilityProfile {
    let events = db.get_recent_capability_events(200).unwrap_or_default();

    let all_capabilities = [
        Capability::FileRead,
        Capability::FileWrite,
        Capability::CodeCompile,
        Capability::TestPass,
        Capability::ShellExec,
        Capability::PeerCall,
        Capability::EndpointCreate,
        Capability::GitOps,
        Capability::CodeGen,
        Capability::CodeSearch,
        Capability::PlanComplete,
        Capability::PeerReview,
        Capability::CodeAccepted,
    ];

    let mut scores: Vec<CapabilityScore> = Vec::new();
    let mut total_attempts = 0u32;
    let mut total_successes = 0u32;

    for cap in &all_capabilities {
        let cap_events: Vec<&CapabilityEvent> = events
            .iter()
            .filter(|e| e.capability == cap.as_str())
            .collect();

        let attempts = cap_events.len() as u32;
        let successes = cap_events.iter().filter(|e| e.succeeded).count() as u32;
        let success_rate = if attempts > 0 {
            successes as f64 / attempts as f64
        } else {
            0.5 // no data — neutral
        };

        total_attempts += attempts;
        total_successes += successes;

        scores.push(CapabilityScore {
            capability: cap.as_str().to_string(),
            display_name: cap.display_name().to_string(),
            attempts,
            successes,
            success_rate,
        });
    }

    let overall_success_rate = if total_attempts > 0 {
        total_successes as f64 / total_attempts as f64
    } else {
        0.5
    };

    // Find strongest/weakest (only from capabilities with >=3 attempts)
    let measured: Vec<&CapabilityScore> = scores.iter().filter(|s| s.attempts >= 3).collect();
    let strongest = measured
        .iter()
        .max_by(|a, b| {
            a.success_rate
                .partial_cmp(&b.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|s| s.display_name.clone());
    let weakest = measured
        .iter()
        .min_by(|a, b| {
            a.success_rate
                .partial_cmp(&b.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|s| s.display_name.clone());

    CapabilityProfile {
        capabilities: scores,
        overall_success_rate,
        strongest,
        weakest,
    }
}

/// Format capability profile for inclusion in prompts.
pub fn capability_guidance(db: &SoulDatabase) -> String {
    let profile = compute_profile(db);

    let mut lines = vec!["# Your Capability Profile".to_string()];
    lines.push(format!(
        "Overall success rate: {:.0}%",
        profile.overall_success_rate * 100.0
    ));

    if let Some(ref strong) = profile.strongest {
        lines.push(format!("Strongest: {strong}"));
    }
    if let Some(ref weak) = profile.weakest {
        lines.push(format!("Weakest: {weak} — be cautious with this"));
    }

    // Show capabilities with meaningful data (3+ attempts to avoid noisy early metrics)
    let measured: Vec<&CapabilityScore> = profile
        .capabilities
        .iter()
        .filter(|s| s.attempts >= 3)
        .collect();

    if !measured.is_empty() {
        lines.push("## Success Rates".to_string());
        for s in &measured {
            let bar = if s.success_rate >= 0.8 {
                "+++"
            } else if s.success_rate >= 0.5 {
                "++"
            } else {
                "+"
            };
            lines.push(format!(
                "- {} {}: {:.0}% ({}/{})",
                bar,
                s.display_name,
                s.success_rate * 100.0,
                s.successes,
                s.attempts
            ));
        }
    }

    lines.join("\n")
}

// ── Emergent Agent Specialization ────────────────────────────────────

/// Role labels that emerge from capability profiles.
/// These are NOT configured — they are computed from actual performance data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleLabel {
    /// High code generation + compilation success. Primary coder.
    Solver,
    /// High peer review + code accepted rates. Quality gatekeeper.
    Reviewer,
    /// High endpoint creation + shell execution. Infrastructure builder.
    Builder,
    /// High peer call + git ops success. Network coordinator.
    Coordinator,
    /// No strong specialization yet. Jack of all trades.
    Generalist,
}

impl RoleLabel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Solver => "solver",
            Self::Reviewer => "reviewer",
            Self::Builder => "builder",
            Self::Coordinator => "coordinator",
            Self::Generalist => "generalist",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Solver => "You excel at writing code that compiles and passes tests. \
                Prioritize coding tasks: fix bugs, implement features, optimize algorithms.",
            Self::Reviewer => "You excel at reviewing code and maintaining quality standards. \
                Prioritize review tasks: review peer PRs, analyze code quality, suggest improvements.",
            Self::Builder => "You excel at creating endpoints and running infrastructure. \
                Prioritize building tasks: create useful endpoints, set up services, manage deployments.",
            Self::Coordinator => "You excel at network coordination and peer interaction. \
                Prioritize coordination: call peer endpoints, discover peers, manage PRs, facilitate collaboration.",
            Self::Generalist => "You have balanced capabilities across all areas. \
                Take on whatever task has the highest priority — you can handle anything.",
        }
    }
}

/// An agent's emergent role, computed from its capability profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRole {
    /// The primary role label.
    pub primary: RoleLabel,
    /// Confidence in this role assignment (0.0-1.0).
    /// Higher means the agent is clearly specialized; lower means it's still a generalist.
    pub confidence: f64,
    /// The capability weights that determined this role.
    /// Maps capability group name to its average success rate.
    pub capability_weights: std::collections::HashMap<String, f64>,
}

/// Compute the agent's emergent role from its capability profile.
///
/// The role is determined by grouping capabilities into role-relevant clusters
/// and finding which cluster the agent performs best at (relative to its average).
///
/// Requires at least 10 total events to avoid noisy early classifications.
pub fn compute_role(db: &SoulDatabase) -> AgentRole {
    let profile = compute_profile(db);

    // Need enough data to be meaningful
    let total_attempts: u32 = profile.capabilities.iter().map(|s| s.attempts).sum();
    if total_attempts < 10 {
        return AgentRole {
            primary: RoleLabel::Generalist,
            confidence: 0.0,
            capability_weights: std::collections::HashMap::new(),
        };
    }

    // Group capabilities into role clusters with weighted averages
    let solver_caps = &["code_gen", "code_compile", "test_pass", "file_write"];
    let reviewer_caps = &["peer_review", "code_accepted", "code_search", "file_read"];
    let builder_caps = &["endpoint_create", "shell_exec", "file_write"];
    let coordinator_caps = &["peer_call", "git_ops", "plan_complete"];

    let avg = |caps: &[&str]| -> f64 {
        let scores: Vec<f64> = profile
            .capabilities
            .iter()
            .filter(|s| caps.contains(&s.capability.as_str()) && s.attempts >= 2)
            .map(|s| s.success_rate)
            .collect();
        if scores.is_empty() {
            return 0.0;
        }
        scores.iter().sum::<f64>() / scores.len() as f64
    };

    let solver_score = avg(solver_caps);
    let reviewer_score = avg(reviewer_caps);
    let builder_score = avg(builder_caps);
    let coordinator_score = avg(coordinator_caps);

    let mut weights = std::collections::HashMap::new();
    weights.insert("solver".to_string(), solver_score);
    weights.insert("reviewer".to_string(), reviewer_score);
    weights.insert("builder".to_string(), builder_score);
    weights.insert("coordinator".to_string(), coordinator_score);

    // Find the highest-scoring role
    let scores = [
        (RoleLabel::Solver, solver_score),
        (RoleLabel::Reviewer, reviewer_score),
        (RoleLabel::Builder, builder_score),
        (RoleLabel::Coordinator, coordinator_score),
    ];

    let overall_avg = profile.overall_success_rate;

    // Find best role
    let (best_role, best_score) = scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(r, s)| (r.clone(), *s))
        .unwrap_or((RoleLabel::Generalist, 0.0));

    // Confidence: how much better is the best role vs overall average?
    // If the best role is >15% above average, high confidence.
    // If it's within 5% of average, stay generalist.
    let advantage = best_score - overall_avg;
    let confidence = (advantage * 5.0).clamp(0.0, 1.0); // 20% advantage = 100% confidence

    let primary = if confidence < 0.25 || total_attempts < 20 {
        RoleLabel::Generalist
    } else {
        best_role
    };

    AgentRole {
        primary,
        confidence,
        capability_weights: weights,
    }
}

/// Format agent role for inclusion in prompts.
pub fn role_guidance(db: &SoulDatabase) -> String {
    let role = compute_role(db);

    if role.primary == RoleLabel::Generalist && role.confidence < 0.1 {
        return String::new(); // Not enough data yet
    }

    let mut lines = vec![format!(
        "# Your Emergent Role: {} (confidence: {:.0}%)",
        role.primary.as_str().to_uppercase(),
        role.confidence * 100.0
    )];

    lines.push(role.primary.description().to_string());

    if !role.capability_weights.is_empty() {
        lines.push("## Role Scores".to_string());
        let mut sorted: Vec<_> = role.capability_weights.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (name, score) in &sorted {
            let bar = if **score >= 0.8 {
                "+++"
            } else if **score >= 0.5 {
                "++"
            } else {
                "+"
            };
            lines.push(format!("- {bar} {name}: {:.0}%", score * 100.0));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_from_step() {
        let step = PlanStep::ReadFile {
            path: "foo.rs".into(),
            store_as: None,
        };
        assert_eq!(Capability::from_step(&step), Capability::FileRead);

        let step = PlanStep::CargoCheck {
            store_as: Some("check".into()),
        };
        assert_eq!(Capability::from_step(&step), Capability::CodeCompile);

        let step = PlanStep::Commit {
            message: "test".into(),
        };
        assert_eq!(Capability::from_step(&step), Capability::GitOps);
    }
}
