//! Soul database: separate SQLite for thoughts and state.

use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::SoulError;
use crate::events::{ErrorCodeCount, EventFilter, SoulEvent};
use crate::memory::{Thought, ThoughtType};
use crate::plan::{Plan, PlanStatus, PlanStep};
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

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS thoughts (
    id TEXT PRIMARY KEY,
    thought_type TEXT NOT NULL,
    content TEXT NOT NULL,
    context TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_thoughts_type ON thoughts(thought_type);
CREATE INDEX IF NOT EXISTS idx_thoughts_created ON thoughts(created_at);

CREATE TABLE IF NOT EXISTS soul_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tools (
    name TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    parameters TEXT NOT NULL,
    handler_type TEXT NOT NULL,
    handler_config TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    mode_tags TEXT NOT NULL DEFAULT '["code","chat"]',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mutations (
    id TEXT PRIMARY KEY,
    commit_sha TEXT,
    branch TEXT NOT NULL,
    description TEXT NOT NULL,
    files_changed TEXT NOT NULL,
    cargo_check_passed INTEGER NOT NULL DEFAULT 0,
    cargo_test_passed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mutations_created ON mutations(created_at);
"#;

/// The soul's dedicated SQLite database.
pub struct SoulDatabase {
    pub(super) conn: Mutex<Connection>,
}

impl SoulDatabase {
    /// Open (or create) the soul database at the given path.
    pub fn new(path: &str) -> Result<Self, SoulError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        // Limit WAL file growth — checkpoint after 1000 pages (~4MB)
        conn.execute_batch("PRAGMA wal_autocheckpoint=1000;")?;
        conn.execute_batch(SCHEMA)?;
        Self::run_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Run incremental schema migrations using PRAGMA user_version.
    fn run_migrations(conn: &Connection) -> Result<(), SoulError> {
        let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if version < 1 {
            // v1: neuroplastic memory columns + pattern_counts table
            // Each ALTER TABLE must be a separate statement (SQLite limitation).
            // Use execute_batch with individual error handling — columns may already exist
            // if a previous migration was partially applied.
            let alters = [
                "ALTER TABLE thoughts ADD COLUMN salience REAL",
                "ALTER TABLE thoughts ADD COLUMN salience_factors TEXT",
                "ALTER TABLE thoughts ADD COLUMN memory_tier TEXT",
                "ALTER TABLE thoughts ADD COLUMN strength REAL",
                "ALTER TABLE thoughts ADD COLUMN prediction_error REAL",
            ];
            for alter in &alters {
                // Ignore "duplicate column" errors for idempotency
                let _ = conn.execute_batch(alter);
            }

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS pattern_counts (
                    fingerprint TEXT PRIMARY KEY,
                    count INTEGER NOT NULL DEFAULT 1,
                    last_seen_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_thoughts_salience ON thoughts(salience);
                CREATE INDEX IF NOT EXISTS idx_thoughts_tier_strength ON thoughts(memory_tier, strength);",
            )?;

            conn.execute_batch("PRAGMA user_version = 1;")?;
        }

        if version < 2 {
            // v2: world model beliefs table
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS beliefs (
                    id TEXT PRIMARY KEY,
                    domain TEXT NOT NULL,
                    subject TEXT NOT NULL,
                    predicate TEXT NOT NULL,
                    value TEXT NOT NULL,
                    confidence TEXT NOT NULL DEFAULT 'medium',
                    evidence TEXT NOT NULL DEFAULT '',
                    confirmation_count INTEGER NOT NULL DEFAULT 1,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    active INTEGER NOT NULL DEFAULT 1
                );
                CREATE UNIQUE INDEX IF NOT EXISTS idx_beliefs_unique
                    ON beliefs(domain, subject, predicate) WHERE active = 1;
                CREATE INDEX IF NOT EXISTS idx_beliefs_domain ON beliefs(domain);
                PRAGMA user_version = 2;",
            )?;
        }

        if version < 3 {
            // v3: goals table + goal_id on mutations
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS goals (
                    id TEXT PRIMARY KEY,
                    description TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'active',
                    priority INTEGER NOT NULL DEFAULT 3,
                    success_criteria TEXT NOT NULL DEFAULT '',
                    progress_notes TEXT NOT NULL DEFAULT '',
                    parent_goal_id TEXT,
                    retry_count INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    completed_at INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_goals_status ON goals(status);
                CREATE INDEX IF NOT EXISTS idx_goals_priority ON goals(priority);",
            )?;
            // Add goal_id to mutations (ignore if already exists)
            let _ = conn.execute_batch("ALTER TABLE mutations ADD COLUMN goal_id TEXT");
            conn.execute_batch("PRAGMA user_version = 3;")?;
        }

        if version < 4 {
            // v4: plans table for plan-driven execution
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS plans (
                    id TEXT PRIMARY KEY,
                    goal_id TEXT NOT NULL,
                    steps TEXT NOT NULL,
                    current_step INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL DEFAULT 'active',
                    context TEXT NOT NULL DEFAULT '{}',
                    replan_count INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_plans_status ON plans(status);
                CREATE INDEX IF NOT EXISTS idx_plans_goal ON plans(goal_id);
                PRAGMA user_version = 4;",
            )?;
        }

        if version < 5 {
            // v5: nudges table for external signal injection
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS nudges (
                    id TEXT PRIMARY KEY,
                    source TEXT NOT NULL,
                    content TEXT NOT NULL,
                    priority INTEGER NOT NULL DEFAULT 3,
                    created_at INTEGER NOT NULL,
                    processed_at INTEGER,
                    active INTEGER NOT NULL DEFAULT 1
                );
                CREATE INDEX IF NOT EXISTS idx_nudges_active ON nudges(active, priority DESC, created_at ASC);
                PRAGMA user_version = 5;",
            )?;
        }

        if version < 6 {
            // v6: chat sessions + messages for multi-turn conversation
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS chat_sessions (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL DEFAULT '',
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    active INTEGER NOT NULL DEFAULT 1
                );
                CREATE INDEX IF NOT EXISTS idx_chat_sessions_active ON chat_sessions(active, updated_at DESC);

                CREATE TABLE IF NOT EXISTS chat_messages (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    tool_executions TEXT NOT NULL DEFAULT '[]',
                    created_at INTEGER NOT NULL,
                    FOREIGN KEY (session_id) REFERENCES chat_sessions(id)
                );
                CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at ASC);
                PRAGMA user_version = 6;",
            )?;
        }

        if version < 7 {
            // v7: feedback loop — plan outcomes + capability events
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS plan_outcomes (
                    id TEXT PRIMARY KEY,
                    plan_id TEXT NOT NULL,
                    goal_id TEXT NOT NULL,
                    goal_description TEXT NOT NULL,
                    status TEXT NOT NULL,
                    steps_succeeded TEXT NOT NULL DEFAULT '[]',
                    steps_failed TEXT NOT NULL DEFAULT '[]',
                    error_category TEXT,
                    error_message TEXT,
                    lesson TEXT NOT NULL DEFAULT '',
                    total_steps INTEGER NOT NULL DEFAULT 0,
                    steps_completed INTEGER NOT NULL DEFAULT 0,
                    replan_count INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_plan_outcomes_created ON plan_outcomes(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_plan_outcomes_status ON plan_outcomes(status);

                CREATE TABLE IF NOT EXISTS capability_events (
                    id TEXT PRIMARY KEY,
                    capability TEXT NOT NULL,
                    succeeded INTEGER NOT NULL,
                    context TEXT NOT NULL DEFAULT '',
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_capability_events_cap ON capability_events(capability, created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_capability_events_created ON capability_events(created_at DESC);

                PRAGMA user_version = 7;",
            )?;
        }

        if version < 8 {
            // v8: benchmark runs (originally HumanEval, now Exercism Rust)
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS benchmark_runs (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    entry_point TEXT NOT NULL,
                    passed INTEGER NOT NULL,
                    generated_solution TEXT NOT NULL DEFAULT '',
                    error_output TEXT NOT NULL DEFAULT '',
                    total_ms INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_benchmark_runs_task ON benchmark_runs(task_id);
                CREATE INDEX IF NOT EXISTS idx_benchmark_runs_created ON benchmark_runs(created_at DESC);

                PRAGMA user_version = 8;",
            )?;
        }

        if version < 9 {
            // v9: structured events table for observability
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS events (
                    id TEXT PRIMARY KEY,
                    level TEXT NOT NULL,
                    code TEXT NOT NULL,
                    category TEXT NOT NULL,
                    message TEXT NOT NULL,
                    context TEXT NOT NULL DEFAULT '{}',
                    plan_id TEXT,
                    goal_id TEXT,
                    step_index INTEGER,
                    tool_name TEXT,
                    peer_url TEXT,
                    resolved INTEGER NOT NULL DEFAULT 0,
                    resolved_at INTEGER,
                    resolution TEXT,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_events_level ON events(level, created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_events_code ON events(code, created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_events_category ON events(category, created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_events_unresolved ON events(resolved, level, created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_events_plan ON events(plan_id);

                PRAGMA user_version = 9;",
            )?;
        }

        Ok(())
    }
}
