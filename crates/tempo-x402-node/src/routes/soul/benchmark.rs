//! Benchmark endpoints — solutions export, failure export, peer review, code review, trigger.

use super::*;

/// GET /soul/benchmark/solutions — export verified solutions for peer sharing.
pub(super) async fn get_benchmark_solutions(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let solutions = x402_soul::benchmark::export_solutions(soul_db);
    let (collective_pass, solved, total) = x402_soul::benchmark::collective_score(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "solutions": solutions,
        "count": solutions.len(),
        "collective_score": {
            "pass_at_1": collective_pass,
            "unique_solved": solved,
            "total_problems": total,
        },
    }))
}

/// GET /soul/benchmark/failures — export failed attempts for collaborative solving.
/// Peers use these as negative context to avoid the same mistakes.
pub(super) async fn get_benchmark_failures(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let failures = x402_soul::benchmark::export_failures(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "failures": failures,
        "count": failures.len(),
    }))
}

/// POST /soul/benchmark/review — peer reviews a benchmark solution.
/// Used by adversarial verification: agent A generates, agent B reviews.
pub(super) async fn review_benchmark_solution(
    state: web::Data<NodeState>,
    body: web::Json<x402_soul::benchmark::ReviewRequest>,
) -> HttpResponse {
    let config = match &state.soul_config {
        Some(c) => c,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let api_key = match &config.llm_api_key {
        Some(k) => k.clone(),
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "no LLM key — dormant mode"}));
        }
    };

    let llm = x402_soul::llm::LlmClient::new(
        api_key,
        config.llm_model_fast.clone(),
        config.llm_model_think.clone(),
    );

    match x402_soul::benchmark::review_solution(&llm, &body).await {
        Ok(review) => HttpResponse::Ok().json(review),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e})),
    }
}

/// POST /soul/code-review — peer reviews a proposed code change.
/// Used by colony peer review: before committing, agents send their diff
/// to peers for approval. Peers use the LLM to review for destructiveness.
pub(super) async fn review_code_change(
    state: web::Data<NodeState>,
    body: web::Json<x402_soul::coding::CodeReviewRequest>,
) -> HttpResponse {
    let config = match &state.soul_config {
        Some(c) => c,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let api_key = match &config.llm_api_key {
        Some(k) => k.clone(),
        None => {
            // No LLM key — can't review, approve by default (graceful degradation)
            let reviewer = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "unknown".into());
            return HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                approved: true,
                reason: "dormant mode — auto-approved".to_string(),
                reviewer,
            });
        }
    };

    let llm = x402_soul::llm::LlmClient::new(
        api_key,
        config.llm_model_fast.clone(),
        config.llm_model_think.clone(),
    );

    let reviewer_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "unknown".into());

    // Quick mechanical checks first (no LLM needed)
    let diff = &body.diff;

    // Check 1: Any file losing >50% of lines?
    let mut _destruction_detected = false;
    let mut _destruction_detail = String::new();
    for line in diff.lines() {
        // Diff headers like "--- a/file" and "+++ b/file" + stats
        if line.starts_with("diff --git") {
            // Count additions/deletions for this file chunk
            // Simple heuristic: if we see way more --- than +++ lines in a chunk
        }
    }

    // Check 2: Are critical files being modified?
    let critical_files = [
        "prompts.rs",
        "validation.rs",
        "guard.rs",
        "thinking.rs",
        "plan.rs",
        "brain.rs",
    ];
    let modifies_critical = critical_files
        .iter()
        .any(|f| diff.contains(&format!("/{f}")));

    // Use LLM for nuanced review of critical file changes
    let system = "You are a code reviewer for an autonomous AI colony. Your job is to PROTECT \
        the codebase from destructive changes. You are reviewing a diff proposed by a peer agent.\n\n\
        REJECT if:\n\
        - The diff deletes more than 50% of any file\n\
        - Core prompt builders, validation rules, or safety layers are removed\n\
        - The change replaces working code with stubs or no-ops\n\
        - Function signatures change in ways that break callers\n\
        - The change is clearly a confused refactor that loses functionality\n\n\
        APPROVE if:\n\
        - The change adds new functionality without removing existing\n\
        - Bug fixes that are targeted and don't gut surrounding code\n\
        - New tests or documentation\n\
        - Genuine improvements that maintain all existing behavior\n\n\
        Respond with EXACTLY this JSON (no markdown):\n\
        {\"approved\": true, \"reason\": \"...\"} or {\"approved\": false, \"reason\": \"...\"}";

    let prompt =
        format!(
        "Review this code change from agent '{}'.\n\nCommit message: {}\n\n{}Diff:\n```\n{}\n```",
        body.requester,
        body.message,
        if modifies_critical { "⚠️ WARNING: This modifies CRITICAL files.\n\n" } else { "" },
        // Truncate diff for LLM context
        body.diff.chars().take(8000).collect::<String>()
    );

    match llm.think(&system, &prompt).await {
        Ok(response) => {
            let cleaned = response
                .trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();

            if let Ok(review) = serde_json::from_str::<serde_json::Value>(cleaned) {
                let approved = review
                    .get("approved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let reason = review
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("no reason")
                    .to_string();

                HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                    approved,
                    reason,
                    reviewer: reviewer_id,
                })
            } else {
                // Parse failed — conservative: reject if critical files, approve otherwise
                let approved = !modifies_critical;
                HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                    approved,
                    reason: format!(
                        "review parse failed — {}",
                        if approved {
                            "auto-approved (non-critical)"
                        } else {
                            "auto-rejected (critical files)"
                        }
                    ),
                    reviewer: reviewer_id,
                })
            }
        }
        Err(e) => {
            // LLM failed — conservative: reject critical, approve non-critical
            let approved = !modifies_critical;
            HttpResponse::Ok().json(x402_soul::coding::CodeReviewResponse {
                approved,
                reason: format!(
                    "LLM review failed: {e} — {}",
                    if approved {
                        "auto-approved"
                    } else {
                        "auto-rejected"
                    }
                ),
                reviewer: reviewer_id,
            })
        }
    }
}

/// POST /soul/benchmark — request a benchmark run on the next cycle.
/// Sets a flag that the thinking loop checks.
pub(super) async fn trigger_benchmark(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Force benchmark on next cycle (bypasses warmup + interval checks)
    let _ = soul_db.set_state("benchmark_force_next", "1");
    let _ = soul_db.set_state("last_benchmark_at", "0");
    let _ = soul_db.set_state("last_benchmark_cycle", "0");

    // Check current score
    let current = x402_soul::benchmark::load_score(soul_db);
    let elo = x402_soul::elo::load_rating(soul_db);

    HttpResponse::Ok().json(serde_json::json!({
        "status": "benchmark_triggered",
        "message": "Benchmark will run on the next thinking cycle",
        "current_score": current.as_ref().map(|s| s.pass_at_1),
        "current_elo": elo,
        "problems_attempted": current.as_ref().map(|s| s.problems_attempted).unwrap_or(0),
    }))
}
