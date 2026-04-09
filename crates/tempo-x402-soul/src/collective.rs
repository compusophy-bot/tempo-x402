//! Collective consciousness — the colony as ONE distributed mind.
//!
//! The queen coordinates. Workers execute. The colony thinks together.
//!
//! Protocol:
//!   1. Workers register with queen on startup (POST /soul/colony/register)
//!   2. Workers heartbeat every 5 min (re-register)
//!   3. Queen partitions benchmark problems across workers
//!   4. Workers solve their partition, report results
//!   5. Queen aggregates into ONE Colony IQ
//!   6. Workers submit training data, queen trains brain on all
//!   7. Workers fetch updated weights from queen
//!
//! Failure modes:
//!   - Worker can't reach queen → falls back to standalone
//!   - Worker dies → queen prunes after 10 min, reassigns work
//!   - Queen restarts → workers re-register within 5 min

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;

/// Colony role — determines thinking loop behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColonyRole {
    /// No colony — acts exactly as before.
    Standalone,
    /// Coordinator: holds canonical brain, goals, plans, distributes benchmark.
    Queen,
    /// Fungible compute: pulls tasks from queen, reports results.
    Worker,
}

impl ColonyRole {
    pub fn from_env() -> Self {
        match std::env::var("COLONY_ROLE")
            .unwrap_or_default()
            .trim()
            .to_lowercase()
            .as_str()
        {
            "queen" => ColonyRole::Queen,
            "worker" => ColonyRole::Worker,
            _ => ColonyRole::Standalone,
        }
    }
}

/// A registered worker in the colony.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyWorker {
    pub instance_id: String,
    pub url: String,
    pub registered_at: i64,
    pub last_heartbeat: i64,
    pub fitness: f64,
}

/// Benchmark assignment for a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkAssignment {
    pub session_id: String,
    pub problem_slugs: Vec<String>,
    pub assigned_at: i64,
}

/// Benchmark result from a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub session_id: String,
    pub slug: String,
    pub passed: bool,
    pub solution: String,
    pub error_output: String,
    pub total_ms: u64,
}

/// Work assignment for a worker (plan step execution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkAssignment {
    pub plan_id: String,
    pub goal_description: String,
    pub step_index: usize,
    pub step_json: serde_json::Value,
    pub context: serde_json::Value,
}

/// Work result from a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResult {
    pub plan_id: String,
    pub step_index: usize,
    pub success: bool,
    pub output: String,
    pub training_examples: Vec<serde_json::Value>,
}

// ── Queen: Worker Registry ──────────────────────────────────────────

const HEARTBEAT_TIMEOUT_SECS: i64 = 600; // 10 minutes

/// Register or update a worker. Called by POST /soul/colony/register.
pub fn register_worker(db: &SoulDatabase, instance_id: &str, url: &str) {
    let now = chrono::Utc::now().timestamp();
    let mut workers = load_workers(db);

    if let Some(w) = workers.iter_mut().find(|w| w.instance_id == instance_id) {
        w.url = url.to_string();
        w.last_heartbeat = now;
    } else {
        workers.push(ColonyWorker {
            instance_id: instance_id.to_string(),
            url: url.to_string(),
            registered_at: now,
            last_heartbeat: now,
            fitness: 0.0,
        });
        tracing::info!(instance_id, url, "New worker registered in colony");
    }

    save_workers(db, &workers);
}

/// Get live workers (prune dead ones). Called by GET /soul/colony/peers.
pub fn get_live_workers(db: &SoulDatabase) -> Vec<ColonyWorker> {
    let now = chrono::Utc::now().timestamp();
    let mut workers = load_workers(db);
    let before = workers.len();
    workers.retain(|w| now - w.last_heartbeat < HEARTBEAT_TIMEOUT_SECS);
    if workers.len() < before {
        tracing::info!(
            pruned = before - workers.len(),
            remaining = workers.len(),
            "Pruned dead workers from colony"
        );
        save_workers(db, &workers);
    }
    workers
}

/// Update a worker's fitness (called after benchmark results come in).
pub fn update_worker_fitness(db: &SoulDatabase, instance_id: &str, fitness: f64) {
    let mut workers = load_workers(db);
    if let Some(w) = workers.iter_mut().find(|w| w.instance_id == instance_id) {
        w.fitness = fitness;
        save_workers(db, &workers);
    }
}

// ── Queen: Benchmark Distribution ───────────────────────────────────

/// Partition benchmark problems across N workers + queen.
/// Returns: (queen_problems, vec of (worker_id, worker_url, worker_problems))
pub fn partition_benchmark(
    db: &SoulDatabase,
    all_slugs: &[String],
    sample_size: usize,
) -> (Vec<String>, Vec<(String, String, Vec<String>)>) {
    let workers = get_live_workers(db);
    let n_nodes = workers.len() + 1; // workers + queen
    let per_node = sample_size / n_nodes;
    let remainder = sample_size % n_nodes;

    // Take first `sample_size` problems (caller already sampled/shuffled)
    let problems: Vec<&String> = all_slugs.iter().take(sample_size).collect();

    let mut queen_problems = Vec::new();
    let mut worker_assignments = Vec::new();
    let mut idx = 0;

    // Queen gets first batch + any remainder
    let queen_count = per_node + remainder;
    for _ in 0..queen_count.min(problems.len() - idx) {
        queen_problems.push(problems[idx].clone());
        idx += 1;
    }

    // Each worker gets per_node problems
    for worker in &workers {
        let mut worker_probs = Vec::new();
        for _ in 0..per_node {
            if idx >= problems.len() {
                break;
            }
            worker_probs.push(problems[idx].clone());
            idx += 1;
        }
        if !worker_probs.is_empty() {
            worker_assignments.push((worker.instance_id.clone(), worker.url.clone(), worker_probs));
        }
    }

    tracing::info!(
        queen = queen_problems.len(),
        workers = worker_assignments.len(),
        total_nodes = n_nodes,
        "Benchmark partitioned across colony"
    );

    (queen_problems, worker_assignments)
}

/// Store a benchmark assignment for a specific worker.
pub fn store_assignment(db: &SoulDatabase, worker_id: &str, assignment: &BenchmarkAssignment) {
    let key = format!("colony_bench_assignment_{}", worker_id);
    if let Ok(json) = serde_json::to_string(assignment) {
        let _ = db.set_state(&key, &json);
    }
}

/// Load a worker's benchmark assignment.
pub fn load_assignment(db: &SoulDatabase, worker_id: &str) -> Option<BenchmarkAssignment> {
    let key = format!("colony_bench_assignment_{}", worker_id);
    db.get_state(&key)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Store aggregated colony benchmark results.
pub fn record_colony_benchmark(
    db: &SoulDatabase,
    total_passed: u32,
    total_attempted: u32,
    weighted_score: f64,
) {
    let iq = crate::opus_bench::weighted_score_to_iq(weighted_score);
    let _ = db.set_state("colony_iq", &format!("{:.0}", iq));
    let _ = db.set_state("colony_pass_at_1", &format!("{:.2}", weighted_score));
    let _ = db.set_state(
        "colony_benchmark_summary",
        &serde_json::json!({
            "colony_iq": iq,
            "pass_at_1": weighted_score,
            "total_passed": total_passed,
            "total_attempted": total_attempted,
            "measured_at": chrono::Utc::now().timestamp(),
            "workers": get_live_workers(db).len(),
        })
        .to_string(),
    );
    tracing::info!(
        iq = format!("{:.0}", iq),
        pass_at_1 = format!("{:.1}%", weighted_score),
        passed = total_passed,
        attempted = total_attempted,
        "Colony IQ computed from distributed benchmark"
    );
}

// ── Worker: Queen Communication ─────────────────────────────────────

/// Get the queen URL from env.
pub fn queen_url() -> Option<String> {
    std::env::var("COLONY_QUEEN_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Register this worker with the queen. Returns true if successful.
pub async fn register_with_queen(queen: &str, instance_id: &str, self_url: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    match client
        .post(format!(
            "{}/soul/colony/register",
            queen.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "instance_id": instance_id,
            "url": self_url,
        }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(queen, "Registered with queen");
            true
        }
        Ok(resp) => {
            tracing::warn!(queen, status = %resp.status(), "Queen registration failed");
            false
        }
        Err(e) => {
            tracing::warn!(queen, error = %e, "Could not reach queen");
            false
        }
    }
}

/// Fetch benchmark assignment from queen. Returns None if no assignment.
pub async fn fetch_benchmark_assignment(
    queen: &str,
    instance_id: &str,
) -> Option<BenchmarkAssignment> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let resp = client
        .get(format!(
            "{}/soul/colony/benchmark/assignment?worker_id={}",
            queen.trim_end_matches('/'),
            instance_id,
        ))
        .send()
        .await
        .ok()?;

    if resp.status().is_success() {
        resp.json().await.ok()
    } else {
        None
    }
}

/// Report benchmark results to queen.
pub async fn report_benchmark_results(
    queen: &str,
    results: &[BenchmarkResult],
    session_id: &str,
    instance_id: &str,
) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    match client
        .post(format!(
            "{}/soul/colony/benchmark/result",
            queen.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "session_id": session_id,
            "worker_id": instance_id,
            "results": results,
        }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                queen,
                results = results.len(),
                "Reported benchmark results to queen"
            );
            true
        }
        _ => {
            tracing::warn!(queen, "Failed to report benchmark results");
            false
        }
    }
}

/// Submit training examples to queen.
pub async fn submit_training_data(queen: &str, examples: &[serde_json::Value]) -> bool {
    if examples.is_empty() {
        return true;
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    match client
        .post(format!("{}/soul/colony/train", queen.trim_end_matches('/')))
        .json(&serde_json::json!({ "examples": examples }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => true,
        _ => false,
    }
}

/// Fetch work assignment from queen. Returns None if no work available.
pub async fn fetch_work(queen: &str, instance_id: &str) -> Option<WorkAssignment> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let resp = client
        .post(format!("{}/soul/colony/work", queen.trim_end_matches('/')))
        .json(&serde_json::json!({ "worker_id": instance_id }))
        .send()
        .await
        .ok()?;

    if resp.status() == 204 {
        return None; // No work available
    }
    if resp.status().is_success() {
        resp.json().await.ok()
    } else {
        None
    }
}

/// Report work result to queen.
pub async fn report_work(queen: &str, result: &WorkResult) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    match client
        .post(format!(
            "{}/soul/colony/report",
            queen.trim_end_matches('/')
        ))
        .json(result)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => true,
        _ => false,
    }
}

/// Fetch latest brain weights from queen.
pub async fn fetch_queen_brain(queen: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;

    let resp = client
        .get(format!(
            "{}/soul/brain/weights",
            queen.trim_end_matches('/')
        ))
        .send()
        .await
        .ok()?;

    if resp.status().is_success() {
        let data: serde_json::Value = resp.json().await.ok()?;
        data.get("weights")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}

// ── Persistence ─────────────────────────────────────────────────────

fn load_workers(db: &SoulDatabase) -> Vec<ColonyWorker> {
    db.get_state("colony_workers")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_workers(db: &SoulDatabase, workers: &[ColonyWorker]) {
    if let Ok(json) = serde_json::to_string(workers) {
        let _ = db.set_state("colony_workers", &json);
    }
}
