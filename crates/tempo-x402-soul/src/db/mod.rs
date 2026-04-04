//! Soul database: sled-backed lock-free embedded KV store.
//!
//! Replaces SQLite (Mutex<Connection>) to eliminate deadlocks between
//! the codegen training thread and the async thinking loop.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::SoulError;
use crate::events::{ErrorCodeCount, EventFilter, SoulEvent};
use crate::memory::{Thought, ThoughtType};
use crate::plan::{Plan, PlanStatus};
use crate::world_model::{Belief, BeliefDomain, Confidence, Goal, GoalStatus};

mod beliefs;
mod benchmark;
mod chat;
mod events;
mod feedback;
mod goals;
mod housekeeping;
mod mutations;
mod nudges;
mod plans;
mod state;
mod thoughts;
mod tools;

/// Stats from a pruning cycle.
#[derive(Debug, Default)]
pub struct PruneStats {
    pub thoughts: u32,
    pub goals: u32,
    pub plans: u32,
    pub mutations: u32,
    pub nudges: u32,
    pub beliefs: u32,
    pub messages: u32,
    pub sessions: u32,
    pub events: u32,
}

impl PruneStats {
    pub fn total(&self) -> u32 {
        self.thoughts
            + self.goals
            + self.plans
            + self.mutations
            + self.nudges
            + self.beliefs
            + self.messages
            + self.sessions
            + self.events
    }
}

/// A dynamically registered tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicTool {
    pub name: String,
    pub description: String,
    /// JSON schema for parameters.
    pub parameters: String,
    /// "shell_command" or "shell_script"
    pub handler_type: String,
    /// The command or script path.
    pub handler_config: String,
    pub enabled: bool,
    /// JSON array of mode tags, e.g. ["code","chat"]
    pub mode_tags: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// An external signal injected into the thinking loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nudge {
    pub id: String,
    /// "user", "system", or "stagnation"
    pub source: String,
    pub content: String,
    pub priority: u32,
    pub created_at: i64,
    pub processed_at: Option<i64>,
    pub active: bool,
}

/// A chat session for multi-turn conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub active: bool,
}

/// A message within a chat session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    /// "user" or "assistant"
    pub role: String,
    pub content: String,
    /// JSON array of tool execution summaries.
    pub tool_executions: String,
    pub created_at: i64,
}

/// A recorded code mutation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutation {
    pub id: String,
    pub commit_sha: Option<String>,
    pub branch: String,
    pub description: String,
    /// JSON array of file paths.
    pub files_changed: String,
    pub cargo_check_passed: bool,
    pub cargo_test_passed: bool,
    pub created_at: i64,
    /// The goal this mutation advances (if any).
    pub goal_id: Option<String>,
}

/// The soul's dedicated database — pure sled, lock-free.
///
/// Every former SQLite table is now a named sled Tree.
/// All reads/writes are lock-free and safe from any thread.
pub struct SoulDatabase {
    pub(super) db: sled::Db,
    pub(super) state: sled::Tree,
    pub(super) thoughts: sled::Tree,
    pub(super) goals: sled::Tree,
    pub(super) plans: sled::Tree,
    pub(super) beliefs: sled::Tree,
    pub(super) events: sled::Tree,
    pub(super) benchmark_runs: sled::Tree,
    pub(super) chat_sessions: sled::Tree,
    pub(super) chat_messages: sled::Tree,
    pub(super) nudges: sled::Tree,
    pub(super) mutations: sled::Tree,
    pub(super) tools: sled::Tree,
    pub(super) plan_outcomes: sled::Tree,
    pub(super) capability_events: sled::Tree,
    pub(super) pattern_counts: sled::Tree,
}

impl SoulDatabase {
    /// Open (or create) the soul database at the given path.
    ///
    /// Path is treated as a directory for sled. If it ends in `.db` (legacy),
    /// the `.db` suffix is replaced with `.sled`.
    pub fn new(path: &str) -> Result<Self, SoulError> {
        let sled_path = if path.ends_with(".db") {
            path.replace(".db", ".sled")
        } else {
            path.to_string()
        };
        let db = sled::open(&sled_path)?;

        Ok(Self {
            state: db.open_tree("soul_state")?,
            thoughts: db.open_tree("thoughts")?,
            goals: db.open_tree("goals")?,
            plans: db.open_tree("plans")?,
            beliefs: db.open_tree("beliefs")?,
            events: db.open_tree("events")?,
            benchmark_runs: db.open_tree("benchmark_runs")?,
            chat_sessions: db.open_tree("chat_sessions")?,
            chat_messages: db.open_tree("chat_messages")?,
            nudges: db.open_tree("nudges")?,
            mutations: db.open_tree("mutations")?,
            tools: db.open_tree("tools")?,
            plan_outcomes: db.open_tree("plan_outcomes")?,
            capability_events: db.open_tree("capability_events")?,
            pattern_counts: db.open_tree("pattern_counts")?,
            db,
        })
    }
}
