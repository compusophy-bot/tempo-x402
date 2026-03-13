//! External SWE benchmark integration: HumanEval from OpenAI via HuggingFace.
//!
//! Uses the real HumanEval dataset (164 Python coding problems with unit tests)
//! as an external reference for intelligence measurement. Problems are fetched
//! from HuggingFace, solved by the agent's LLM, validated by running Python tests,
//! and scores are tracked over time.
//!
//! Published reference scores for comparison:
//! - GPT-4: 67.0% pass@1
//! - Claude 3.5 Sonnet: 92.0% pass@1
//! - Gemini 1.5 Pro: 71.9% pass@1
//! - GPT-3.5: 48.1% pass@1
//!
//! This gives us an objective, third-party measure of our agent's coding ability
//! that we can track over time and compare against established baselines.

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;
use crate::llm::LlmClient;

/// A HumanEval problem fetched from HuggingFace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanEvalProblem {
    /// e.g. "HumanEval/0"
    pub task_id: String,
    /// The function signature + docstring (the prompt given to the model).
    pub prompt: String,
    /// The canonical solution (for reference, not shown to model).
    pub canonical_solution: String,
    /// Python test code that validates the solution.
    pub test: String,
    /// The function name being tested.
    pub entry_point: String,
}

/// Result of running a single HumanEval problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub id: String,
    pub task_id: String,
    pub entry_point: String,
    pub passed: bool,
    /// The solution the LLM generated.
    pub generated_solution: String,
    /// Error output if the test failed.
    pub error_output: String,
    /// Time to generate + validate in ms.
    pub total_ms: u64,
    pub created_at: i64,
}

/// Aggregated benchmark scores over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScore {
    /// pass@1: fraction of problems solved correctly.
    pub pass_at_1: f64,
    /// Total problems attempted in this scoring window.
    pub problems_attempted: u32,
    /// Total problems passed in this scoring window.
    pub problems_passed: u32,
    /// When this score was computed.
    pub measured_at: i64,
    /// Historical scores for trend tracking.
    pub history: Vec<HistoricalScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalScore {
    pub pass_at_1: f64,
    pub problems_attempted: u32,
    pub measured_at: i64,
}

/// Reference scores from published benchmarks for comparison.
pub const REFERENCE_SCORES: &[(&str, f64)] = &[
    ("GPT-4", 67.0),
    ("GPT-4o", 90.2),
    ("Claude 3.5 Sonnet", 92.0),
    ("Claude 3 Opus", 84.9),
    ("Gemini 1.5 Pro", 71.9),
    ("Gemini 1.5 Flash", 71.5),
    ("GPT-3.5 Turbo", 48.1),
    ("Llama 3 70B", 81.7),
    ("CodeLlama 34B", 48.8),
];

/// HuggingFace datasets API base URL.
/// Dataset was renamed from `openai_humaneval` to `openai/openai_humaneval`.
/// API limits length to 100 rows per request, so we paginate.
const HUMANEVAL_DATASET_BASE: &str =
    "https://datasets-server.huggingface.co/rows?dataset=openai/openai_humaneval&config=openai_humaneval&split=test";

/// Fetch HumanEval problems from HuggingFace (paginated, max 100 per request).
pub async fn fetch_problems() -> Result<Vec<HumanEvalProblem>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let mut problems = Vec::new();
    let mut offset = 0u64;
    let page_size = 100u64;

    loop {
        let url = format!(
            "{}&offset={}&length={}",
            HUMANEVAL_DATASET_BASE, offset, page_size
        );
        let resp = client
            .get(&url)
            .header("User-Agent", "tempo-x402-soul/1.8")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch HumanEval dataset: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("HuggingFace API returned {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse HumanEval response: {e}"))?;

        let rows = body
            .get("rows")
            .and_then(|v| v.as_array())
            .ok_or("Missing 'rows' in HumanEval response")?;

        if rows.is_empty() {
            break;
        }

        for row in rows {
            let row_data = row.get("row").unwrap_or(row);
            let task_id = row_data
                .get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let prompt = row_data
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let canonical_solution = row_data
                .get("canonical_solution")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let test = row_data
                .get("test")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let entry_point = row_data
                .get("entry_point")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !task_id.is_empty() && !prompt.is_empty() && !test.is_empty() {
                problems.push(HumanEvalProblem {
                    task_id,
                    prompt,
                    canonical_solution,
                    test,
                    entry_point,
                });
            }
        }

        offset += page_size;
        // HumanEval has 164 problems — stop after we have them all
        if offset >= 200 {
            break;
        }
    }

    if problems.is_empty() {
        return Err("No problems parsed from HumanEval dataset".into());
    }

    tracing::info!(count = problems.len(), "Fetched HumanEval problems");
    Ok(problems)
}

/// Pick a random subset of N problems for a benchmark run.
pub fn sample_problems(problems: &[HumanEvalProblem], n: usize) -> Vec<HumanEvalProblem> {
    use std::collections::HashSet;

    if problems.len() <= n {
        return problems.to_vec();
    }

    // Deterministic-ish shuffle using timestamp as seed
    let seed = chrono::Utc::now().timestamp() as usize;
    let mut indices: HashSet<usize> = HashSet::new();
    let mut i = seed;
    while indices.len() < n {
        i = (i.wrapping_mul(6364136223846793005).wrapping_add(1)) % problems.len();
        indices.insert(i);
    }

    indices.into_iter().map(|i| problems[i].clone()).collect()
}

/// Generate a solution for a HumanEval problem using the LLM.
pub async fn generate_solution(
    llm: &LlmClient,
    problem: &HumanEvalProblem,
) -> Result<String, String> {
    let system = "You are a Python coding expert. Complete the function implementation. \
        Output ONLY the Python code — no markdown, no explanation, no ```python blocks. \
        The code must be valid Python that can be directly executed. \
        Include the function signature from the prompt and your implementation.";

    let prompt = format!(
        "Complete this Python function:\n\n{}\n\n\
        Output ONLY the complete function implementation (including the def line). \
        No markdown, no explanation.",
        problem.prompt
    );

    let response = llm
        .think(system, &prompt)
        .await
        .map_err(|e| format!("LLM generation failed: {e}"))?;

    // Clean up: strip markdown code blocks if the LLM included them anyway
    let cleaned = strip_code_blocks(&response);
    Ok(cleaned)
}

/// Validate a solution by running it against HumanEval tests.
/// Returns (passed, error_output).
pub async fn validate_solution(
    problem: &HumanEvalProblem,
    solution: &str,
    workspace_root: &str,
) -> (bool, String) {
    // Build the test script: solution + test code + test runner
    let test_script = format!(
        "{solution}\n\n{test}\n\ncheck({entry_point})\nprint(\"HUMANEVAL_PASS\")\n",
        solution = solution,
        test = problem.test,
        entry_point = problem.entry_point,
    );

    // Write to temp file
    let test_path = format!("/tmp/humaneval_{}.py", problem.entry_point);
    if let Err(e) = tokio::fs::write(&test_path, &test_script).await {
        return (false, format!("Failed to write test file: {e}"));
    }

    // Run with timeout
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new("python3")
            .arg(&test_path)
            .current_dir(workspace_root)
            .output(),
    )
    .await;

    // Clean up
    let _ = tokio::fs::remove_file(&test_path).await;

    match output {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if stdout.contains("HUMANEVAL_PASS") {
                (true, String::new())
            } else {
                let error = if stderr.is_empty() {
                    format!("stdout: {}", stdout.chars().take(500).collect::<String>())
                } else {
                    stderr.chars().take(500).collect()
                };
                (false, error)
            }
        }
        Ok(Err(e)) => (false, format!("exec error: {e}")),
        Err(_) => (false, "timeout (10s)".into()),
    }
}

/// Run a benchmark session: fetch problems, solve N, validate, record results.
/// Returns the pass@1 score for this session.
pub async fn run_benchmark_session(
    llm: &LlmClient,
    db: &SoulDatabase,
    workspace_root: &str,
    sample_size: usize,
) -> Result<f64, String> {
    tracing::info!(
        sample_size = sample_size,
        "Starting HumanEval benchmark session"
    );

    // Try to load cached problems, fetch if not cached
    let problems = match load_cached_problems(db) {
        Some(p) if p.len() >= 100 => p,
        _ => {
            let fetched = fetch_problems().await?;
            cache_problems(db, &fetched);
            fetched
        }
    };

    let sample = sample_problems(&problems, sample_size);
    let mut passed = 0u32;
    let mut attempted = 0u32;
    let now = chrono::Utc::now().timestamp();

    for problem in &sample {
        attempted += 1;

        // Generate solution
        let solution = match generate_solution(llm, problem).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    task_id = %problem.task_id,
                    error = %e,
                    "Benchmark: failed to generate solution"
                );
                record_run(db, problem, false, "", &e, 0);
                continue;
            }
        };

        // Validate
        let start = std::time::Instant::now();
        let (success, error_output) = validate_solution(problem, &solution, workspace_root).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        if success {
            passed += 1;
            tracing::info!(
                task_id = %problem.task_id,
                entry_point = %problem.entry_point,
                "Benchmark: PASS"
            );
        } else {
            tracing::info!(
                task_id = %problem.task_id,
                entry_point = %problem.entry_point,
                error = %error_output.chars().take(100).collect::<String>(),
                "Benchmark: FAIL"
            );
        }

        record_run(db, problem, success, &solution, &error_output, elapsed_ms);
    }

    let pass_at_1 = if attempted > 0 {
        passed as f64 / attempted as f64 * 100.0
    } else {
        0.0
    };

    // Store score
    update_score(db, pass_at_1, attempted, passed, now);

    tracing::info!(
        pass_at_1 = format!("{:.1}%", pass_at_1),
        passed = passed,
        attempted = attempted,
        "HumanEval benchmark session complete"
    );

    Ok(pass_at_1)
}

/// Record a single benchmark run.
fn record_run(
    db: &SoulDatabase,
    problem: &HumanEvalProblem,
    passed: bool,
    solution: &str,
    error: &str,
    total_ms: u64,
) {
    let run = BenchmarkRun {
        id: uuid::Uuid::new_v4().to_string(),
        task_id: problem.task_id.clone(),
        entry_point: problem.entry_point.clone(),
        passed,
        generated_solution: solution.chars().take(2000).collect(),
        error_output: error.chars().take(500).collect(),
        total_ms,
        created_at: chrono::Utc::now().timestamp(),
    };

    if let Err(e) = db.insert_benchmark_run(&run) {
        tracing::warn!(error = %e, "Failed to record benchmark run");
    }
}

/// Update the stored benchmark score with a new measurement.
fn update_score(db: &SoulDatabase, pass_at_1: f64, attempted: u32, passed: u32, measured_at: i64) {
    let mut score = load_score(db).unwrap_or(BenchmarkScore {
        pass_at_1: 0.0,
        problems_attempted: 0,
        problems_passed: 0,
        measured_at: 0,
        history: Vec::new(),
    });

    // Push previous score to history (if it had data)
    if score.problems_attempted > 0 {
        score.history.push(HistoricalScore {
            pass_at_1: score.pass_at_1,
            problems_attempted: score.problems_attempted,
            measured_at: score.measured_at,
        });
        // Keep last 20 historical scores
        if score.history.len() > 20 {
            score.history.drain(..score.history.len() - 20);
        }
    }

    score.pass_at_1 = pass_at_1;
    score.problems_attempted = attempted;
    score.problems_passed = passed;
    score.measured_at = measured_at;

    if let Ok(json) = serde_json::to_string(&score) {
        let _ = db.set_state("benchmark_score", &json);
    }

    // Also save to disk for analysis
    save_benchmark_to_disk(&score);
}

/// Save benchmark score snapshot to /data/benchmark_history/.
fn save_benchmark_to_disk(score: &BenchmarkScore) {
    let dir = std::path::Path::new("/data/benchmark_history");
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let filename = format!("benchmark_{}.json", score.measured_at);
    let path = dir.join(filename);
    if let Ok(json) = serde_json::to_string_pretty(score) {
        if let Err(e) = std::fs::write(&path, &json) {
            tracing::warn!(error = %e, "Failed to save benchmark to disk");
        } else {
            tracing::info!(
                pass_at_1 = format!("{:.1}%", score.pass_at_1),
                path = %path.display(),
                "Benchmark snapshot saved to disk"
            );
        }
    }
}

/// Load the current benchmark score.
pub fn load_score(db: &SoulDatabase) -> Option<BenchmarkScore> {
    db.get_state("benchmark_score")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Cache fetched HumanEval problems in soul_state (avoid re-fetching).
fn cache_problems(db: &SoulDatabase, problems: &[HumanEvalProblem]) {
    if let Ok(json) = serde_json::to_string(problems) {
        let _ = db.set_state("humaneval_problems_cache", &json);
    }
}

/// Load cached problems.
fn load_cached_problems(db: &SoulDatabase) -> Option<Vec<HumanEvalProblem>> {
    db.get_state("humaneval_problems_cache")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Strip markdown code blocks from LLM output.
fn strip_code_blocks(s: &str) -> String {
    let s = s.trim();
    // Strip ```python ... ``` or ``` ... ```
    if s.starts_with("```") {
        let without_start = if let Some(rest) = s.strip_prefix("```python") {
            rest
        } else if let Some(rest) = s.strip_prefix("```py") {
            rest
        } else if let Some(rest) = s.strip_prefix("```") {
            rest
        } else {
            s
        };
        let trimmed = without_start.trim();
        if trimmed.ends_with("```") {
            return trimmed[..trimmed.len() - 3].trim().to_string();
        }
        return trimmed.to_string();
    }
    s.to_string()
}

/// Check if it's time to run a benchmark (every N cycles).
pub fn should_run_benchmark(db: &SoulDatabase, interval: u64) -> bool {
    let total_cycles: u64 = db
        .get_state("total_think_cycles")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Don't benchmark in first 20 cycles — let agent warm up
    if total_cycles < 20 {
        return false;
    }

    // Check cooldown — don't run more than once per hour
    let last_benchmark: i64 = db
        .get_state("last_benchmark_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let now = chrono::Utc::now().timestamp();
    if now - last_benchmark < 3600 {
        return false;
    }

    // Check if we've passed an interval boundary since last benchmark cycle
    let last_benchmark_cycle: u64 = db
        .get_state("last_benchmark_cycle")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Trigger if we've crossed an interval boundary (handles skipped cycles)
    total_cycles / interval > last_benchmark_cycle / interval
}

/// Default: run benchmark every 50 cycles.
pub const DEFAULT_BENCHMARK_INTERVAL: u64 = 50;
/// Default: sample 20 problems per session (balance speed vs. statistical significance).
pub const DEFAULT_SAMPLE_SIZE: usize = 20;

/// Format benchmark score for prompt injection.
pub fn benchmark_summary_for_prompt(db: &SoulDatabase) -> String {
    let score = match load_score(db) {
        Some(s) => s,
        None => return String::new(),
    };

    let mut lines = vec![format!(
        "# HumanEval Benchmark: {:.1}% pass@1 ({}/{} problems)",
        score.pass_at_1, score.problems_passed, score.problems_attempted
    )];

    // Show trend
    if score.history.len() >= 2 {
        let prev = &score.history[score.history.len() - 1];
        let delta = score.pass_at_1 - prev.pass_at_1;
        let direction = if delta > 0.5 {
            "IMPROVING"
        } else if delta < -0.5 {
            "DECLINING"
        } else {
            "STABLE"
        };
        lines.push(format!(
            "Trend: {} ({:+.1}% from last session)",
            direction, delta
        ));
    }

    // Compare against reference scores
    lines.push("## vs. Published Baselines".into());
    for (model, ref_score) in REFERENCE_SCORES {
        let comparison = if score.pass_at_1 > *ref_score + 1.0 {
            "above"
        } else if score.pass_at_1 < *ref_score - 1.0 {
            "below"
        } else {
            "~equal"
        };
        lines.push(format!("- {model}: {ref_score:.1}% ({comparison})"));
    }

    lines.join("\n")
}

// ── Solution Sharing (Collective Intelligence) ──────────────────────

/// A verified solution that can be shared between peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSolution {
    pub task_id: String,
    pub entry_point: String,
    pub solution: String,
    /// Who solved it (instance_id or "self").
    pub solved_by: String,
}

/// Export all verified (passed) solutions for peer sharing.
pub fn export_solutions(db: &SoulDatabase) -> Vec<SharedSolution> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "self".into());

    // Get our own solved problems
    let mut solutions: Vec<SharedSolution> = db
        .get_all_benchmark_runs()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.passed && !r.generated_solution.is_empty())
        .map(|r| SharedSolution {
            task_id: r.task_id,
            entry_point: r.entry_point,
            solution: r.generated_solution,
            solved_by: instance_id.clone(),
        })
        .collect();

    // Also include imported peer solutions
    if let Some(imported) = db
        .get_state("imported_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<SharedSolution>>(&s).ok())
    {
        solutions.extend(imported);
    }

    // Deduplicate by task_id (keep first/our own)
    let mut seen = std::collections::HashSet::new();
    solutions.retain(|s| seen.insert(s.task_id.clone()));

    solutions
}

/// Import solutions from a peer. Validates them by re-running tests.
/// Returns the number of new solutions imported.
pub async fn import_solutions(
    db: &SoulDatabase,
    peer_solutions: Vec<SharedSolution>,
    workspace_root: &str,
) -> u32 {
    let mut imported = 0u32;

    // Load existing imported solutions
    let mut existing: Vec<SharedSolution> = db
        .get_state("imported_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // Get our already-solved task IDs
    let our_solved: std::collections::HashSet<String> = db
        .get_all_benchmark_runs()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.passed)
        .map(|r| r.task_id)
        .collect();

    let already_imported: std::collections::HashSet<String> =
        existing.iter().map(|s| s.task_id.clone()).collect();

    // Load cached problems for test validation
    let problems = load_cached_problems(db).unwrap_or_default();
    let problem_map: std::collections::HashMap<&str, &HumanEvalProblem> =
        problems.iter().map(|p| (p.task_id.as_str(), p)).collect();

    for sol in peer_solutions {
        // Skip if we already have this solution
        if our_solved.contains(&sol.task_id) || already_imported.contains(&sol.task_id) {
            continue;
        }

        // Validate the peer's solution by running it
        if let Some(problem) = problem_map.get(sol.task_id.as_str()) {
            let (passed, _error) = validate_solution(problem, &sol.solution, workspace_root).await;
            if passed {
                tracing::info!(
                    task_id = %sol.task_id,
                    solved_by = %sol.solved_by,
                    "Imported verified solution from peer"
                );
                existing.push(sol);
                imported += 1;
            } else {
                tracing::debug!(
                    task_id = %sol.task_id,
                    "Peer solution failed validation — skipping"
                );
            }
        }
    }

    // Save updated imports
    if imported > 0 {
        if let Ok(json) = serde_json::to_string(&existing) {
            let _ = db.set_state("imported_solutions", &json);
        }
        tracing::info!(
            imported,
            total_imported = existing.len(),
            "Peer solutions imported"
        );
    }

    imported
}

/// Compute the collective pass@1 score: our solutions + verified peer solutions.
/// This is the "swarm intelligence" metric — higher than any individual agent.
pub fn collective_score(db: &SoulDatabase) -> (f64, u32, u32) {
    let all_solutions = export_solutions(db);
    let total_problems = 164u32; // HumanEval has 164 problems

    let unique_solved: std::collections::HashSet<&str> =
        all_solutions.iter().map(|s| s.task_id.as_str()).collect();

    let solved = unique_solved.len() as u32;
    let pass_at_1 = if total_problems > 0 {
        solved as f64 / total_problems as f64 * 100.0
    } else {
        0.0
    };

    (pass_at_1, solved, total_problems)
}
