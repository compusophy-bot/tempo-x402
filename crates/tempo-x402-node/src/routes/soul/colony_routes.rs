//! Colony coordination endpoints — worker registration, benchmark distribution, work assignment.
//!
//! These endpoints are served by the QUEEN node. Workers call them to participate
//! in the collective consciousness.

use super::*;

/// POST /soul/colony/register — Worker registers or heartbeats.
/// Body: { "instance_id": "...", "url": "..." }
pub(super) async fn colony_register(
    state: web::Data<NodeState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let instance_id = body
        .get("instance_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let url = body.get("url").and_then(|v| v.as_str()).unwrap_or("");

    if instance_id.is_empty() || url.is_empty() {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"error": "instance_id and url required"}));
    }

    x402_soul::collective::register_worker(soul_db, instance_id, url);

    let workers = x402_soul::collective::get_live_workers(soul_db);
    HttpResponse::Ok().json(serde_json::json!({
        "status": "registered",
        "colony_size": workers.len() + 1, // +1 for queen
        "workers": workers,
    }))
}

/// GET /soul/colony/peers — Get live colony members.
///
/// If this node is a queen, returns its registered workers.
/// If this node is a worker, proxies to the queen and includes self.
pub(super) async fn colony_peers(state: web::Data<NodeState>) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Check if we're a worker (have a queen URL)
    if let Some(queen_url) = x402_soul::collective::queen_url() {
        // Worker: proxy to queen's /soul/colony/peers
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        if let Ok(resp) = client
            .get(format!("{}/soul/colony/peers", queen_url))
            .send()
            .await
        {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                return HttpResponse::Ok().json(data);
            }
        }
        // Fallback if queen unreachable: show self + queen
        return HttpResponse::Ok().json(serde_json::json!({
            "colony_size": 2,
            "colony_iq": "--",
            "workers": [{"instance_id": "self", "url": "this node"}],
        }));
    }

    // Queen: return registered workers
    let workers = x402_soul::collective::get_live_workers(soul_db);

    let colony_iq = soul_db
        .get_state("colony_iq")
        .ok()
        .flatten()
        .unwrap_or_else(|| "--".to_string());

    HttpResponse::Ok().json(serde_json::json!({
        "colony_size": workers.len() + 1,
        "colony_iq": colony_iq,
        "workers": workers,
    }))
}

/// GET /soul/colony/benchmark/assignment?worker_id=xxx — Get benchmark problems for this worker.
pub(super) async fn colony_benchmark_assignment(
    state: web::Data<NodeState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let worker_id = query.get("worker_id").cloned().unwrap_or_default();
    if worker_id.is_empty() {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"error": "worker_id required"}));
    }

    match x402_soul::collective::load_assignment(soul_db, &worker_id) {
        Some(assignment) => HttpResponse::Ok().json(assignment),
        None => HttpResponse::NoContent().finish(),
    }
}

/// POST /soul/colony/benchmark/result — Worker reports benchmark results.
/// Body: { "session_id": "...", "worker_id": "...", "results": [...] }
pub(super) async fn colony_benchmark_result(
    state: web::Data<NodeState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let worker_id = body
        .get("worker_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let results = body
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut passed = 0u32;
    let mut attempted = 0u32;

    for result_val in &results {
        if let Ok(result) =
            serde_json::from_value::<x402_soul::collective::BenchmarkResult>(result_val.clone())
        {
            attempted += 1;
            if result.passed {
                passed += 1;
                // Store solution for codegen training
                if !result.solution.is_empty() {
                    x402_soul::codegen::record_training_example(
                        soul_db,
                        &result.solution,
                        &format!("worker/{}/{}", worker_id, result.slug),
                    );
                }
            }
            // Record the run in queen's DB
            let problem = x402_soul::benchmark::ExercismProblem {
                slug: result.slug.clone(),
                instructions: String::new(),
                test_code: String::new(),
                starter_code: String::new(),
                difficulty: String::new(),
                cargo_toml: String::new(),
            };
            let run = x402_soul::benchmark::BenchmarkRun {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: format!("opus/{}", result.slug),
                entry_point: result.slug,
                passed: result.passed,
                generated_solution: result.solution.chars().take(2000).collect(),
                error_output: result.error_output.chars().take(500).collect(),
                total_ms: result.total_ms,
                created_at: chrono::Utc::now().timestamp(),
            };
            if let Err(e) = soul_db.insert_benchmark_run(&run) {
                tracing::warn!(error = %e, "Failed to record worker benchmark run");
            }
        }
    }

    tracing::info!(
        worker = worker_id,
        passed,
        attempted,
        "Received benchmark results from worker"
    );

    HttpResponse::Ok().json(serde_json::json!({
        "status": "accepted",
        "passed": passed,
        "attempted": attempted,
    }))
}

/// POST /soul/colony/train — Worker submits training examples for the queen's brain.
/// Body: { "examples": [...] }
pub(super) async fn colony_train(
    state: web::Data<NodeState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let examples = body
        .get("examples")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Store training data for next brain training cycle
    let mut queue: Vec<serde_json::Value> = soul_db
        .get_state("colony_training_queue")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    queue.extend(examples.iter().cloned());
    // Cap at 500 queued examples
    if queue.len() > 500 {
        queue.drain(..queue.len() - 500);
    }

    if let Ok(json) = serde_json::to_string(&queue) {
        let _ = soul_db.set_state("colony_training_queue", &json);
    }

    HttpResponse::Ok().json(serde_json::json!({
        "status": "queued",
        "queued": queue.len(),
    }))
}

/// POST /soul/colony/work — Worker requests a task assignment.
/// Body: { "worker_id": "..." }
pub(super) async fn colony_work(
    state: web::Data<NodeState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    let _worker_id = body
        .get("worker_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // For now, return 204 No Content — work distribution will be added
    // as the plan execution pipeline matures. The distributed benchmark
    // is the primary work distribution mechanism for now.
    HttpResponse::NoContent().finish()
}

/// POST /soul/colony/report — Worker reports task completion.
pub(super) async fn colony_report(
    state: web::Data<NodeState>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let _soul_db = match &state.soul_db {
        Some(db) => db,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({"error": "soul not active"}));
        }
    };

    // Accept and log — plan step distribution will be implemented
    // after distributed benchmark proves the coordination protocol works.
    tracing::info!(body = %body, "Work report received from worker");
    HttpResponse::Ok().json(serde_json::json!({"status": "accepted"}))
}
