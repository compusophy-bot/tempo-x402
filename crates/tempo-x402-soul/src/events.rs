//! Structured event system for agent observability.
//!
//! Events are the nervous system of the colony. Every significant action,
//! error, or state change gets recorded with a structured code, severity level,
//! and optional context. This enables:
//!
//! - **Self-diagnosis**: agents reason about their own error patterns
//! - **Peer monitoring**: agents pay (via x402) to read each other's event streams
//! - **Operator visibility**: queryable event log with filtering
//!
//! ## Event codes
//!
//! String-based, hierarchical: `category.subcategory.specific`.
//! Agents can define new codes without recompiling — just emit a new string.
//!
//! Common codes:
//! - `peer.discovery.success` / `peer.discovery.empty` / `peer.discovery.failed`
//! - `peer.call.success` / `peer.call.payment_failed` / `peer.call.unreachable`
//! - `plan.completed` / `plan.failed` / `plan.step.succeeded` / `plan.step.failed`
//! - `plan.step.brain_gated` / `plan.replanned`
//! - `payment.received` / `payment.sent` / `payment.failed`
//! - `tool.execution.failed` / `tool.shell.error` / `tool.file.not_found`
//! - `goal.created` / `goal.abandoned` / `goal.completed`
//! - `system.startup` / `system.deploy` / `system.stagnation`

use crate::db::SoulDatabase;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A structured event in the soul's event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulEvent {
    pub id: String,
    pub level: String,
    pub code: String,
    pub category: String,
    pub message: String,
    #[serde(default = "default_context")]
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_url: Option<String>,
    pub resolved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    pub created_at: i64,
}

fn default_context() -> String {
    "{}".to_string()
}

/// Optional references to attach to an event.
#[derive(Debug, Default, Clone)]
pub struct EventRefs {
    pub plan_id: Option<String>,
    pub goal_id: Option<String>,
    pub step_index: Option<i32>,
    pub tool_name: Option<String>,
    pub peer_url: Option<String>,
}

/// Filters for querying events.
#[derive(Debug, Default, Deserialize)]
pub struct EventFilter {
    pub level: Option<String>,
    pub code_prefix: Option<String>,
    pub category: Option<String>,
    pub plan_id: Option<String>,
    pub resolved: Option<bool>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    50
}

/// Computed health summary from event data.
#[derive(Debug, Serialize)]
pub struct HealthSummary {
    pub status: String,
    pub blockers: Vec<SoulEvent>,
    pub warnings: Vec<SoulEvent>,
    pub error_count_1h: u64,
    pub warn_count_1h: u64,
    pub last_successful_plan: Option<i64>,
    pub last_error: Option<SoulEvent>,
    pub top_error_codes: Vec<ErrorCodeCount>,
}

#[derive(Debug, Serialize)]
pub struct ErrorCodeCount {
    pub code: String,
    pub count: u64,
}

/// Emit a structured event to the soul database.
///
/// This is the primary entry point for recording events. It:
/// 1. Extracts category from the code's first segment
/// 2. Generates a UUID
/// 3. Inserts into the events table
/// 4. Logs via tracing at the appropriate level
/// 5. Auto-resolves related events when applicable
pub fn emit_event(
    db: &Arc<SoulDatabase>,
    level: &str,
    code: &str,
    message: &str,
    context: Option<serde_json::Value>,
    refs: EventRefs,
) {
    let category = code.split('.').next().unwrap_or("unknown").to_string();
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let ctx_str = context
        .map(|v| v.to_string())
        .unwrap_or_else(|| "{}".to_string());

    let event = SoulEvent {
        id,
        level: level.to_string(),
        code: code.to_string(),
        category,
        message: message.chars().take(1000).collect(),
        context: ctx_str,
        plan_id: refs.plan_id,
        goal_id: refs.goal_id,
        step_index: refs.step_index,
        tool_name: refs.tool_name,
        peer_url: refs.peer_url,
        resolved: false,
        resolved_at: None,
        resolution: None,
        created_at: now,
    };

    // Log via tracing
    match level {
        "error" => tracing::error!(code = code, "{}", message),
        "warn" => tracing::warn!(code = code, "{}", message),
        "info" => tracing::info!(code = code, "{}", message),
        _ => tracing::debug!(code = code, "{}", message),
    }

    // Insert into DB
    if let Err(e) = db.insert_event(&event) {
        tracing::warn!("Failed to insert event: {e}");
        return;
    }

    // Auto-resolution: success events resolve prior failures of the same category
    match code {
        "peer.discovery.success" => {
            let _ = db.resolve_events_by_code("peer.discovery.empty", "Peers discovered");
            let _ = db.resolve_events_by_code("peer.discovery.failed", "Peers discovered");
        }
        "peer.call.success" => {
            let _ = db.resolve_events_by_code("peer.call.payment_failed", "Paid call succeeded");
            let _ = db.resolve_events_by_code("peer.call.unreachable", "Peer now reachable");
        }
        "plan.completed" => {
            // Resolve step failures for this specific plan
            if let Some(ref plan_id) = event.plan_id {
                let _ = db.resolve_events_by_plan("plan.step.failed", plan_id, "Plan completed");
            }
        }
        _ => {}
    }
}

/// Convenience: emit an info-level event with no refs.
pub fn emit_info(db: &Arc<SoulDatabase>, code: &str, message: &str) {
    emit_event(db, "info", code, message, None, EventRefs::default());
}

/// Convenience: emit a warning with refs.
pub fn emit_warn(db: &Arc<SoulDatabase>, code: &str, message: &str, refs: EventRefs) {
    emit_event(db, "warn", code, message, None, refs);
}

/// Convenience: emit an error with refs.
pub fn emit_error(db: &Arc<SoulDatabase>, code: &str, message: &str, refs: EventRefs) {
    emit_event(db, "error", code, message, None, refs);
}

/// Compute a health summary from the events table.
pub fn compute_health(db: &Arc<SoulDatabase>) -> HealthSummary {
    let now = chrono::Utc::now().timestamp();
    let one_hour_ago = now - 3600;

    let blockers = db.get_unresolved_events("error", 20).unwrap_or_default();
    let warnings = db.get_unresolved_events("warn", 10).unwrap_or_default();
    let error_count_1h = db.count_events_since("error", one_hour_ago).unwrap_or(0);
    let warn_count_1h = db.count_events_since("warn", one_hour_ago).unwrap_or(0);
    let last_error = db.get_latest_event_by_level("error").unwrap_or(None);
    let top_error_codes = db
        .top_event_codes_since("error", now - 86400, 5)
        .unwrap_or_default();

    // Get last successful plan from plans table
    let last_successful_plan = db
        .get_state("last_plan_completed_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok());

    let status = if !blockers.is_empty() {
        "unhealthy"
    } else if warnings.len() > 5 || error_count_1h > 2 {
        "degraded"
    } else {
        "healthy"
    }
    .to_string();

    HealthSummary {
        status,
        blockers,
        warnings,
        error_count_1h,
        warn_count_1h,
        last_successful_plan,
        last_error,
        top_error_codes,
    }
}

/// Format health data for injection into planning/goal prompts.
/// Returns empty string if healthy (no noise in prompts).
pub fn format_health_for_prompt(db: &Arc<SoulDatabase>) -> String {
    let health = compute_health(db);

    if health.status == "healthy" && health.top_error_codes.is_empty() {
        return String::new();
    }

    let mut out = String::from("# System Health\n");
    out.push_str(&format!("Status: {}\n", health.status));

    if !health.blockers.is_empty() {
        out.push_str("\nBlockers (unresolved errors):\n");
        for b in &health.blockers {
            let ago = format_ago(b.created_at);
            out.push_str(&format!("- [{}] {} ({})\n", b.code, b.message, ago));
        }
    }

    if !health.warnings.is_empty() {
        out.push_str(&format!(
            "\nWarnings: {} unresolved\n",
            health.warnings.len()
        ));
        for w in health.warnings.iter().take(5) {
            out.push_str(&format!("- [{}] {}\n", w.code, w.message));
        }
    }

    if health.error_count_1h > 0 {
        out.push_str(&format!(
            "\nError rate (1h): {}/hr\n",
            health.error_count_1h
        ));
    }

    if !health.top_error_codes.is_empty() {
        out.push_str("\nTop errors (24h):\n");
        for ec in &health.top_error_codes {
            out.push_str(&format!("- {} ({}x)\n", ec.code, ec.count));
        }
    }

    out
}

fn format_ago(timestamp: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let diff = now - timestamp;
    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}
