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
    /// Flush all pending writes and reclaim disk space.
    ///
    /// Call periodically during housekeeping to prevent unbounded sled growth.
    /// Returns bytes reclaimed (approximate).
    pub fn flush_and_compact(&self) -> Result<(), SoulError> {
        self.db.flush()?;
        Ok(())
    }

    /// Size on disk in bytes (approximate — sled directory size).
    pub fn disk_size_bytes(&self) -> u64 {
        self.db.size_on_disk().unwrap_or(0)
    }

    /// Open (or create) the soul database at the given path.
    ///
    /// Path is treated as a directory for sled. If it ends in `.db` (legacy),
    /// the `.db` suffix is replaced with `.sled`.
    ///
    /// On startup, if the sled directory exceeds `SLED_COMPACT_THRESHOLD_MB`
    /// (default 500 MB), an export→delete→reimport compaction is performed
    /// to reclaim dead blob space that sled never frees on its own.
    pub fn new(path: &str) -> Result<Self, SoulError> {
        let sled_path = if path.ends_with(".db") {
            path.replace(".db", ".sled")
        } else {
            path.to_string()
        };

        // Compact on startup if bloated (sled never reclaims deleted blob space)
        Self::compact_if_needed(&sled_path)?;

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

    /// Compact the sled database if its on-disk size exceeds the threshold.
    ///
    /// Sled 0.34 never reclaims space from deleted keys — blob files grow
    /// monotonically. This performs a full export→delete→reimport cycle to
    /// rebuild the database with only live data, typically recovering 90%+
    /// of disk space.
    fn compact_if_needed(sled_path: &str) -> Result<(), SoulError> {
        let path = std::path::Path::new(sled_path);
        if !path.exists() {
            return Ok(());
        }

        let dir_size_mb = dir_size_bytes(path) / (1024 * 1024);
        let threshold_mb: u64 = std::env::var("SLED_COMPACT_THRESHOLD_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500);

        if dir_size_mb < threshold_mb {
            return Ok(());
        }

        tracing::warn!(
            size_mb = dir_size_mb,
            threshold_mb,
            "Sled database bloated — compacting via export/reimport"
        );

        // 1. Open the old database and export all live data
        let old_db = sled::open(sled_path)?;

        let export: Vec<(Vec<u8>, Vec<u8>, Vec<Vec<Vec<u8>>>)> = old_db
            .export()
            .into_iter()
            .map(|(col_type, col_name, iter)| {
                let data: Vec<Vec<Vec<u8>>> = iter.collect();
                (col_type, col_name, data)
            })
            .collect();

        let total_entries: usize = export.iter().map(|(_, _, d)| d.len()).sum();

        // 2. Drop the old database to release file locks
        drop(old_db);

        // 3. Delete the bloated directory
        std::fs::remove_dir_all(sled_path)?;

        // 4. Open a fresh database and import
        let new_db = sled::open(sled_path)?;

        new_db.import(
            export
                .into_iter()
                .map(|(ct, cn, data)| (ct, cn, data.into_iter()))
                .collect(),
        );

        new_db.flush()?;

        let new_size_mb = dir_size_bytes(path) / (1024 * 1024);
        tracing::warn!(
            old_size_mb = dir_size_mb,
            new_size_mb,
            entries = total_entries,
            "Sled compaction complete"
        );

        // 5. Drop the new DB so `new()` can reopen it normally
        drop(new_db);

        Ok(())
    }
}

/// Recursively sum file sizes in a directory.
fn dir_size_bytes(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if ft.is_dir() {
                total += dir_size_bytes(&entry.path());
            }
        }
    }
    total
}
