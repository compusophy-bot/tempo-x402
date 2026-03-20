//! Clone Lifecycle: Fork → Branch → Birth
//!
//! Models biological cell division. Every clone starts identical to its parent
//! ("fork" phase) and progressively differentiates through source code changes
//! until it becomes an independent organism with its own repo ("birth").
//!
//! ## Phases
//!
//! ### Phase 1: Fork (undifferentiated)
//! - Deploys from `main` on the colony fork
//! - Receives ALL updates automatically when main is pushed
//! - Differentiates only through data (brain weights, cortex, genesis, etc.)
//! - `SOUL_DIRECT_PUSH=true` — pushes code changes to fork's main
//! - Railway rebuilds whenever main is pushed
//!
//! ### Phase 2: Branch (differentiating)
//! - Clone has made its first source code change
//! - Creates own branch: `vm/{instance-id}` on the fork
//! - Railway switches to watch this branch
//! - Still syncs FROM main (merges upstream changes)
//! - But has its OWN unique code modifications
//! - Can create PRs back to upstream
//!
//! ### Phase 3: Birth (independent organism)
//! - Clone creates its own GitHub repo (fork of the fork)
//! - Fully independent — own CI, own releases, own evolution
//! - Can OPTIONALLY sync from canonical upstream
//! - Is its own entity with its own trajectory
//! - "Born" — no longer a clone
//!
//! ## Transition Triggers
//!
//! Fork → Branch: First successful `cargo check` + commit of a code change
//! Branch → Birth: Manual trigger or when branch diverges significantly from main

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;

/// The three lifecycle phases of a clone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecyclePhase {
    /// Phase 1: identical to parent, deploys from main
    #[default]
    Fork,
    /// Phase 2: own branch, differentiating through code
    Branch,
    /// Phase 3: own repo, fully independent
    Birth,
}

impl std::fmt::Display for LifecyclePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fork => write!(f, "fork"),
            Self::Branch => write!(f, "branch"),
            Self::Birth => write!(f, "birth"),
        }
    }
}

/// Full lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleStatus {
    pub phase: LifecyclePhase,
    /// Number of unique source code commits (not synced from main)
    pub own_commits: u64,
    /// Branch name if in Branch or Birth phase
    pub branch: Option<String>,
    /// Own repo URL if in Birth phase
    pub own_repo: Option<String>,
    /// Total lines changed vs canonical main
    pub lines_diverged: u64,
}

/// Load the current lifecycle phase from soul_state.
pub fn current_phase(db: &SoulDatabase) -> LifecyclePhase {
    db.get_state("lifecycle_phase")
        .ok()
        .flatten()
        .and_then(|s| match s.as_str() {
            "fork" => Some(LifecyclePhase::Fork),
            "branch" => Some(LifecyclePhase::Branch),
            "birth" => Some(LifecyclePhase::Birth),
            _ => None,
        })
        .unwrap_or_else(|| {
            // Check env var for initial phase
            std::env::var("SOUL_LIFECYCLE_PHASE")
                .ok()
                .and_then(|s| match s.as_str() {
                    "fork" => Some(LifecyclePhase::Fork),
                    "branch" => Some(LifecyclePhase::Branch),
                    "birth" => Some(LifecyclePhase::Birth),
                    _ => None,
                })
                .unwrap_or(LifecyclePhase::Fork)
        })
}

/// Save the lifecycle phase.
pub fn set_phase(db: &SoulDatabase, phase: LifecyclePhase) {
    let _ = db.set_state("lifecycle_phase", &phase.to_string());
}

/// Increment the own-commits counter (called after a code change commit).
pub fn record_own_commit(db: &SoulDatabase) {
    let count: u64 = db
        .get_state("lifecycle_own_commits")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let _ = db.set_state("lifecycle_own_commits", &(count + 1).to_string());
}

/// Get the own-commits count.
pub fn own_commits(db: &SoulDatabase) -> u64 {
    db.get_state("lifecycle_own_commits")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Get full lifecycle status.
pub fn status(db: &SoulDatabase) -> LifecycleStatus {
    let phase = current_phase(db);
    let own = own_commits(db);
    let branch = db
        .get_state("lifecycle_branch")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty());
    let own_repo = db
        .get_state("lifecycle_own_repo")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty());
    let lines_diverged: u64 = db
        .get_state("lifecycle_lines_diverged")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    LifecycleStatus {
        phase,
        own_commits: own,
        branch,
        own_repo,
        lines_diverged,
    }
}

/// Attempt to transition from Fork → Branch.
/// Called when the soul makes its first code commit.
/// Returns the new branch name if transition occurred.
pub fn differentiate(db: &SoulDatabase, instance_id: &str) -> Option<String> {
    let phase = current_phase(db);
    if phase != LifecyclePhase::Fork {
        return None; // Already differentiated
    }

    let branch = format!("vm/{}", instance_id);
    set_phase(db, LifecyclePhase::Branch);
    let _ = db.set_state("lifecycle_branch", &branch);
    record_own_commit(db);

    tracing::info!(
        branch = %branch,
        "Lifecycle: Fork → Branch — clone is differentiating"
    );

    Some(branch)
}

/// Format lifecycle info for injection into prompts.
pub fn prompt_section(db: &SoulDatabase) -> String {
    let status = status(db);
    match status.phase {
        LifecyclePhase::Fork => "# Lifecycle: Fork (Phase 1)\n\
             You are running identical code to all other clones.\n\
             To differentiate, make SOURCE CODE changes — not just data.\n\
             Your first successful code commit will create your own branch.\n\
             Think about what makes YOU unique. What can you build that no other clone has?"
            .to_string(),
        LifecyclePhase::Branch => {
            format!(
                "# Lifecycle: Branch (Phase 2) — DIFFERENTIATING\n\
                 You are on branch `{}` with {} own commits ({} lines diverged).\n\
                 You have your own code. Keep evolving it.\n\
                 When ready, you can request Birth (Phase 3) — your own repo, fully independent.",
                status.branch.as_deref().unwrap_or("unknown"),
                status.own_commits,
                status.lines_diverged,
            )
        }
        LifecyclePhase::Birth => {
            format!(
                "# Lifecycle: Birth (Phase 3) — INDEPENDENT ORGANISM\n\
                 You have your own repo: {}\n\
                 You are a fully independent entity with {} own commits.\n\
                 You can still sync from upstream if you choose.",
                status.own_repo.as_deref().unwrap_or("unknown"),
                status.own_commits,
            )
        }
    }
}
