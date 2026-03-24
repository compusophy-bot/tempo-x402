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
            PlanStep::CloneSelf { .. }
            | PlanStep::SpawnSpecialist { .. }
            | PlanStep::DelegateTask { .. } => Self::PeerCall,
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

/// Periodically called by the orchestration layer or cortex consolidation
pub fn log_fitness_trends(db: &SoulDatabase) {
    let profile = compute_profile(db);
    tracing::info!(
        target: "fitness_trends",
        overall_success_rate = profile.overall_success_rate,
        strongest = ?profile.strongest,
        weakest = ?profile.weakest,
        "Fitness trend snapshot logged"
    );
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

    // Highlight capabilities with 0 attempts — the agent doesn't know what it hasn't tried
    let unexplored: Vec<&CapabilityScore> = profile
        .capabilities
        .iter()
        .filter(|s| s.attempts == 0)
        .collect();

    if !unexplored.is_empty() {
        lines.push("## UNEXPLORED CAPABILITIES (never attempted — try these!)".to_string());
        for s in &unexplored {
            lines.push(format!(
                "- {} — 0 attempts, unknown potential",
                s.display_name
            ));
        }
        lines.push(
            "Pick one of these unexplored capabilities and include it in your next plan."
                .to_string(),
        );
    }

    lines.join("\n")
}

// Hardcoded role labels (Solver/Reviewer/Builder/Coordinator/Generalist) were removed.
// They were fake emergence — just classification into predetermined bins.
// Real differentiation comes from colony.rs niche recommendations based on
// what capabilities peers DON'T cover (competitive exclusion).

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
