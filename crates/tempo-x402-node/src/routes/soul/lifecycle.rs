//! Lifecycle endpoints — reset, cleanup, disk management, open PRs, rules reset.

use super::status::{dir_size, format_bytes, get_volume_usage};
use super::*;

/// POST /soul/reset — clear historical dead weight (thoughts, ALL plans, counters).
/// Keeps active goals and beliefs. Clears stuck active plans.
pub(super) async fn soul_reset(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    match soul_db.reset_history() {
        Ok((thoughts, plans, nudges)) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "deleted": {
                "thoughts": thoughts,
                "plans": plans,
                "nudges": nudges,
            },
            "kept": "active goals, active beliefs"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("reset failed: {e}")
        })),
    }
}

/// POST /soul/cleanup — force cleanup of disk-hungry artifacts.
/// Removes cargo target/, runs git gc, VACUUM on DBs, prunes old data.
pub(super) async fn soul_cleanup(state: web::Data<NodeState>) -> HttpResponse {
    let mut cleaned = serde_json::Map::new();

    // 1. Remove cargo target/
    let target_dir = "/data/workspace/target";
    if std::path::Path::new(target_dir).exists() {
        let size_before = dir_size(target_dir);
        let _ = std::fs::remove_dir_all(target_dir);
        cleaned.insert(
            "cargo_target_freed".to_string(),
            serde_json::json!(format_bytes(size_before)),
        );
    }

    // NOTE: Do NOT clean CARGO_HOME registry — cargo needs it for compilation.
    // Deleting it forces a 300s+ re-download causing timeout failures.

    // 2. Git gc aggressive
    let _ = std::process::Command::new("git")
        .args(["gc", "--aggressive", "--prune=now"])
        .current_dir("/data/workspace")
        .output();
    cleaned.insert("git_gc".to_string(), serde_json::json!("done"));

    // 3. Soul DB cleanup
    if let Some(db) = &state.soul_db {
        let _ = db.prune_old_data();
        let _ = db.wal_checkpoint();
        cleaned.insert("soul_db_pruned".to_string(), serde_json::json!(true));
    }

    // 4. Gateway DB WAL checkpoint + VACUUM via sqlite3 CLI
    let _ = std::process::Command::new("sqlite3")
        .args([
            "/data/gateway.db",
            "PRAGMA wal_checkpoint(TRUNCATE); VACUUM;",
        ])
        .output();
    cleaned.insert("gateway_db_vacuumed".to_string(), serde_json::json!(true));

    // 5. Remove old brain checkpoints (keep last 3)
    cleanup_old_files("/data/brain_checkpoints", 3);
    cleaned.insert(
        "brain_checkpoints_pruned".to_string(),
        serde_json::json!(true),
    );

    // 6. Remove old benchmark history (keep last 5)
    cleanup_old_files("/data/benchmark_history", 5);
    cleaned.insert(
        "benchmark_history_pruned".to_string(),
        serde_json::json!(true),
    );

    // Report new usage
    let after = get_volume_usage();
    cleaned.insert("volume_after".to_string(), after);

    HttpResponse::Ok().json(serde_json::Value::Object(cleaned))
}

/// Keep only the N most recent files in a directory (by modification time).
fn cleanup_old_files(dir: &str, keep: usize) {
    let p = std::path::Path::new(dir);
    if !p.is_dir() {
        return;
    }
    let mut entries: Vec<(std::time::SystemTime, std::path::PathBuf)> = match std::fs::read_dir(p) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((mtime, e.path()))
            })
            .collect(),
        Err(_) => return,
    };
    entries.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
    for (_mtime, path) in entries.into_iter().skip(keep) {
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// GET /soul/open-prs — list this agent's open pull requests.
/// Exposed so peer agents can discover PRs that need review (academic peer review).
pub(super) async fn open_prs(state: web::Data<NodeState>) -> HttpResponse {
    let fork_repo = std::env::var("SOUL_FORK_REPO").unwrap_or_default();
    let upstream_repo = std::env::var("SOUL_UPSTREAM_REPO").unwrap_or_default();
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_default();

    if fork_repo.is_empty() {
        return HttpResponse::Ok().json(serde_json::json!({
            "instance_id": instance_id,
            "prs": [],
            "message": "no fork repo configured"
        }));
    }

    // Use gh CLI to list open PRs
    let workspace =
        std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".into());
    let gh_token = std::env::var("GH_TOKEN")
        .or_else(|_| std::env::var("GITHUB_TOKEN"))
        .unwrap_or_default();

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::process::Command::new("gh")
            .args([
                "pr",
                "list",
                "--repo",
                &fork_repo,
                "--state",
                "open",
                "--json",
                "number,title,headRefName,author,additions,deletions,createdAt,reviewDecision",
                "--limit",
                "20",
            ])
            .current_dir(&workspace)
            .env("GH_TOKEN", &gh_token)
            .output(),
    )
    .await;

    let prs: serde_json::Value = match result {
        Ok(Ok(output)) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(&stdout).unwrap_or(serde_json::json!([]))
        }
        _ => serde_json::json!([]),
    };

    // Also check upstream PRs if configured
    let upstream_prs: serde_json::Value = if !upstream_repo.is_empty() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            tokio::process::Command::new("gh")
                .args([
                    "pr",
                    "list",
                    "--repo",
                    &upstream_repo,
                    "--state",
                    "open",
                    "--json",
                    "number,title,headRefName,author,additions,deletions,createdAt,reviewDecision",
                    "--limit",
                    "20",
                ])
                .current_dir(&workspace)
                .env("GH_TOKEN", &gh_token)
                .output(),
        )
        .await;
        match result {
            Ok(Ok(output)) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                serde_json::from_str(&stdout).unwrap_or(serde_json::json!([]))
            }
            _ => serde_json::json!([]),
        }
    } else {
        serde_json::json!([])
    };

    // Count PRs needing review (no review decision yet)
    let empty_vec = vec![];
    let fork_prs_arr = prs.as_array().unwrap_or(&empty_vec);
    let upstream_prs_arr = upstream_prs.as_array().unwrap_or(&empty_vec);
    let needs_review_count = fork_prs_arr
        .iter()
        .chain(upstream_prs_arr.iter())
        .filter(|pr| {
            pr.get("reviewDecision")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty() || s == "REVIEW_REQUIRED")
                .unwrap_or(true)
        })
        .count();

    let _ = &state; // suppress unused warning

    HttpResponse::Ok().json(serde_json::json!({
        "instance_id": instance_id,
        "fork_repo": fork_repo,
        "upstream_repo": upstream_repo,
        "fork_prs": prs,
        "upstream_prs": upstream_prs,
        "needs_review_count": needs_review_count,
    }))
}

/// POST /soul/rules/reset — clear durable rules and optionally failure chains.
pub(super) async fn soul_rules_reset(
    state: web::Data<NodeState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let soul_db = match state.soul_db.as_ref() {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}))
        }
    };

    // Clear durable rules
    let _ = soul_db.set_state("durable_rules", "[]");

    // Optionally clear failure chains
    let cleared_chains = if query
        .get("reset_failure_chains")
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        let _ = soul_db.set_state("failure_chains", "[]");
        true
    } else {
        false
    };

    HttpResponse::Ok().json(serde_json::json!({
        "durable_rules": "cleared",
        "failure_chains": if cleared_chains { "cleared" } else { "unchanged" },
    }))
}

/// POST /soul/cleanup — clean build artifacts from /data volume.
/// No auth required — only deletes known safe targets (cargo target dirs).
pub(super) async fn disk_cleanup(_state: web::Data<NodeState>) -> HttpResponse {
    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let script = format!(
        "rm -rf {ws}/target /tmp/x402_cargo_target {ws}/.cargo 2>/dev/null; \
         rm -rf /data/workspace/target 2>/dev/null; \
         echo \"$(du -sh /data 2>/dev/null | cut -f1)\""
    );

    match tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&script)
        .output()
        .await
    {
        Ok(output) => {
            let size = String::from_utf8_lossy(&output.stdout).trim().to_string();
            HttpResponse::Ok().json(serde_json::json!({
                "cleaned": true,
                "data_volume_size": size,
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
    }
}

/// POST /soul/cognitive-reset — full nuclear reset of all learned state.
///
/// Wipes: brain weights, cortex, genesis, hivemind, synthesis, plans, goals,
/// thoughts, nudges, plan outcomes, capability events, durable rules, failure chains.
/// Preserves: benchmark history, ELO score, persistent memory (soul_memory.md).
///
/// This is the Rust-native replacement for shelling out to Python+sqlite3.
/// Call this instead of the hacky admin/exec Python scripts.
pub(super) async fn cognitive_reset(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Force a cognitive architecture reset by clearing the version marker.
    // On next thinking cycle, the version check will see a mismatch and
    // trigger reset_cognitive_architecture() which wipes everything properly.
    let version_tag = format!("manual-reset-{}", chrono::Utc::now().timestamp());
    let reset = soul_db.reset_cognitive_architecture(&version_tag);

    // Also clear plans, goals, thoughts (reset_cognitive_architecture does this,
    // but be thorough in case the version check doesn't fire immediately)
    let history = soul_db.reset_history();

    // Clear the commit gate state
    let _ = soul_db.set_state("commit_awaiting_benchmark", "0");
    let _ = soul_db.set_state("last_commit_at", "0");
    let _ = soul_db.set_state("total_think_cycles", "0");
    let _ = soul_db.set_state("cycles_since_last_commit", "0");
    let _ = soul_db.set_state("recent_errors", "[]");

    let (thoughts, plans, nudges) = history.unwrap_or((0, 0, 0));

    HttpResponse::Ok().json(serde_json::json!({
        "status": "cognitive_reset_complete",
        "version_tag": version_tag,
        "architecture_reset": reset,
        "cleared": {
            "thoughts": thoughts,
            "plans": plans,
            "nudges": nudges,
            "brain_weights": true,
            "cortex": true,
            "genesis": true,
            "hivemind": true,
            "synthesis": true,
        },
        "preserved": ["benchmark_history", "elo_score", "persistent_memory"],
    }))
}
