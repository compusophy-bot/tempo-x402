//! External SWE benchmark integration: Exercism Rust exercises.
//!
//! Uses real Exercism Rust exercises (100+ problems with cargo test suites)
//! as an external reference for coding ability measurement. Exercises are
//! fetched from the exercism/rust GitHub repo, solved by the agent's LLM,
//! validated by running `cargo test` in a temp project, and scores are
//! tracked over time with difficulty weighting.
//!
//! Replaces the previous HumanEval (Python) benchmark which was trivially
//! easy for modern LLMs (100% pass@1 for Gemini 3.1 Flash).
//!
//! Difficulty tiers (weighted scoring):
//! - Easy (1x): basic string/math exercises
//! - Medium (2x): data structures, algorithms, trait implementations
//! - Hard (3x): complex systems, concurrency, unsafe code

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;
use crate::llm::LlmClient;

/// An Exercism Rust exercise fetched from GitHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExercismProblem {
    /// Exercise slug, e.g. "hello-world", "binary-search"
    pub slug: String,
    /// The exercise description / instructions (markdown).
    pub instructions: String,
    /// The test file content (tests/*.rs or tests/slug.rs).
    pub test_code: String,
    /// The starter source file (src/lib.rs stub).
    pub starter_code: String,
    /// Difficulty: "easy", "medium", "hard"
    pub difficulty: String,
}

/// Result of running a single benchmark problem.
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
    /// Weighted pass rate (difficulty-adjusted).
    pub pass_at_1: f64,
    /// Raw pass rate (unweighted).
    #[serde(default)]
    pub raw_pass_rate: f64,
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

/// Difficulty weight for scoring.
fn difficulty_weight(difficulty: &str) -> f64 {
    match difficulty {
        "hard" => 3.0,
        "medium" => 2.0,
        _ => 1.0,
    }
}

/// Classify an exercise slug into a difficulty tier.
fn classify_difficulty(slug: &str) -> &'static str {
    // Hard exercises: complex algorithms, systems programming, concurrency
    const HARD: &[&str] = &[
        "forth",
        "react",
        "circular-buffer",
        "doubly-linked-list",
        "parallel-letter-frequency",
        "macros",
        "xorcism",
        "grep",
        "book-store",
        "dominoes",
        "rectangles",
        "two-bucket",
        "variable-length-quantity",
        "custom-set",
        "nucleotide-codons",
        "rail-fence-cipher",
        "crypto-square",
        "wordy",
        "decimal",
        "bowling",
    ];
    // Medium exercises: data structures, trait impls, moderate algorithms
    const MEDIUM: &[&str] = &[
        "binary-search",
        "binary-search-tree",
        "clock",
        "simple-linked-list",
        "robot-simulator",
        "roman-numerals",
        "all-your-base",
        "allergies",
        "anagram",
        "bracket-push",
        "matching-brackets",
        "grade-school",
        "tournament",
        "pig-latin",
        "queen-attack",
        "minesweeper",
        "ocr-numbers",
        "alphametics",
        "sublist",
        "spiral-matrix",
        "palindrome-products",
        "pascals-triangle",
        "sieve",
        "largest-series-product",
        "luhn",
        "isbn-verifier",
        "diamond",
        "say",
        "phone-number",
        "run-length-encoding",
        "accumulate",
        "protein-translation",
        "affine-cipher",
        "rotational-cipher",
        "simple-cipher",
        "dot-dsl",
        "rpn-calculator",
        "poker",
    ];

    if HARD.contains(&slug) {
        "hard"
    } else if MEDIUM.contains(&slug) {
        "medium"
    } else {
        "easy"
    }
}

/// Reference scores: modern Rust benchmarks (2026 estimates).
/// These are approximate — Exercism Rust is our own benchmark.
pub const REFERENCE_SCORES: &[(&str, f64)] = &[
    ("Gemini 3.1 Flash (self)", 0.0), // will be filled by actual runs
    ("Claude Sonnet 4", 85.0),
    ("GPT-4o", 78.0),
    ("Claude Opus 4", 92.0),
    ("Gemini 3 Pro", 80.0),
];

/// GitHub raw content base URL for exercism/rust.
const EXERCISM_BASE: &str =
    "https://raw.githubusercontent.com/exercism/rust/main/exercises/practice";

/// GitHub API URL for listing exercises.
const EXERCISM_API: &str = "https://api.github.com/repos/exercism/rust/contents/exercises/practice";

/// Fetch the list of available exercise slugs from the Exercism repo.
async fn fetch_exercise_list() -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp = client
        .get(EXERCISM_API)
        .header("User-Agent", "tempo-x402-soul/1.8")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("Failed to list exercises: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse exercise list: {e}"))?;

    let entries = body
        .as_array()
        .ok_or("Expected JSON array from GitHub API")?;

    let slugs: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            let entry_type = entry.get("type")?.as_str()?;
            if entry_type == "dir" {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    if slugs.is_empty() {
        return Err("No exercises found in exercism/rust repo".into());
    }

    tracing::info!(count = slugs.len(), "Found Exercism Rust exercises");
    Ok(slugs)
}

/// Fetch a single exercise's files from GitHub.
async fn fetch_exercise(slug: &str) -> Result<ExercismProblem, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    // Fetch instructions
    let instructions_url = format!("{EXERCISM_BASE}/{slug}/.docs/instructions.md");
    let instructions = client
        .get(&instructions_url)
        .header("User-Agent", "tempo-x402-soul/1.8")
        .send()
        .await
        .ok()
        .and_then(|r| {
            if r.status().is_success() {
                Some(r)
            } else {
                None
            }
        });
    let instructions = match instructions {
        Some(r) => r.text().await.unwrap_or_default(),
        None => String::new(),
    };

    // Fetch test file — try tests/{slug}.rs first, then tests/*.rs
    let test_slug = slug.replace('-', "_");
    let test_url = format!("{EXERCISM_BASE}/{slug}/tests/{test_slug}.rs");
    let test_code = client
        .get(&test_url)
        .header("User-Agent", "tempo-x402-soul/1.8")
        .send()
        .await
        .ok()
        .and_then(|r| {
            if r.status().is_success() {
                Some(r)
            } else {
                None
            }
        });
    let test_code = match test_code {
        Some(r) => r.text().await.unwrap_or_default(),
        None => String::new(),
    };

    if test_code.is_empty() {
        return Err(format!("No test file found for exercise '{slug}'"));
    }

    // Fetch starter code (src/lib.rs)
    let starter_url = format!("{EXERCISM_BASE}/{slug}/src/lib.rs");
    let starter_code = client
        .get(&starter_url)
        .header("User-Agent", "tempo-x402-soul/1.8")
        .send()
        .await
        .ok()
        .and_then(|r| {
            if r.status().is_success() {
                Some(r)
            } else {
                None
            }
        });
    let starter_code = match starter_code {
        Some(r) => r.text().await.unwrap_or_default(),
        None => String::new(),
    };

    let difficulty = classify_difficulty(slug).to_string();

    Ok(ExercismProblem {
        slug: slug.to_string(),
        instructions,
        test_code,
        starter_code,
        difficulty,
    })
}

/// Fetch all Exercism Rust problems (with caching).
pub async fn fetch_problems() -> Result<Vec<ExercismProblem>, String> {
    let slugs = fetch_exercise_list().await?;

    let mut problems = Vec::new();
    let mut fetch_errors = 0u32;

    for slug in &slugs {
        match fetch_exercise(slug).await {
            Ok(problem) => problems.push(problem),
            Err(e) => {
                fetch_errors += 1;
                tracing::debug!(slug = %slug, error = %e, "Skipping exercise");
                if fetch_errors > 20 {
                    // Too many failures — probably rate limited
                    tracing::warn!(
                        "Too many fetch failures, stopping at {} exercises",
                        problems.len()
                    );
                    break;
                }
            }
        }
    }

    if problems.is_empty() {
        return Err("No exercises fetched successfully".into());
    }

    tracing::info!(
        count = problems.len(),
        errors = fetch_errors,
        "Fetched Exercism Rust exercises"
    );
    Ok(problems)
}

/// Pick a random subset of N problems for a benchmark run.
pub fn sample_problems(problems: &[ExercismProblem], n: usize) -> Vec<ExercismProblem> {
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

/// Generate a solution for an Exercism Rust problem using the LLM.
pub async fn generate_solution(
    llm: &LlmClient,
    problem: &ExercismProblem,
) -> Result<String, String> {
    let system = "You are a Rust coding expert. Implement the solution for the given exercise. \
        Output ONLY valid Rust code for src/lib.rs — no markdown, no explanation, no ```rust blocks. \
        The code must compile and pass all tests. Include all necessary use statements and type definitions. \
        Do NOT include any test code or #[cfg(test)] modules — only the implementation.";

    let mut prompt = format!(
        "Implement this Exercism Rust exercise: {}\n\n",
        problem.slug
    );

    if !problem.instructions.is_empty() {
        // Truncate very long instructions
        let instr: String = problem.instructions.chars().take(3000).collect();
        prompt.push_str(&format!("## Instructions\n{instr}\n\n"));
    }

    if !problem.starter_code.is_empty() {
        prompt.push_str(&format!(
            "## Starter Code (src/lib.rs)\n```rust\n{}\n```\n\n",
            problem.starter_code
        ));
    }

    // Show test code so the LLM knows the expected API
    let test_preview: String = problem.test_code.chars().take(4000).collect();
    prompt.push_str(&format!(
        "## Test Code (must pass all these tests)\n```rust\n{test_preview}\n```\n\n\
         Output ONLY the complete src/lib.rs implementation. No markdown fences."
    ));

    let response = llm
        .think(system, &prompt)
        .await
        .map_err(|e| format!("LLM generation failed: {e}"))?;

    let cleaned = strip_code_blocks(&response);
    Ok(cleaned)
}

/// Validate a solution by creating a temp Cargo project and running `cargo test`.
/// Returns (passed, error_output).
pub async fn validate_solution(
    problem: &ExercismProblem,
    solution: &str,
    _workspace_root: &str,
) -> (bool, String) {
    let test_dir = format!("/tmp/exercism_bench_{}", problem.slug);

    // Create temp Cargo project
    let setup = async {
        // Clean up any previous run
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
        tokio::fs::create_dir_all(format!("{test_dir}/src")).await?;
        tokio::fs::create_dir_all(format!("{test_dir}/tests")).await?;

        // Write Cargo.toml
        let cargo_toml = format!(
            "[package]\n\
             name = \"exercism-bench-{slug}\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n",
            slug = problem.slug.replace('-', "_")
        );
        tokio::fs::write(format!("{test_dir}/Cargo.toml"), &cargo_toml).await?;

        // Write solution as src/lib.rs
        tokio::fs::write(format!("{test_dir}/src/lib.rs"), solution).await?;

        // Write test file — remove #[ignore] attributes so all tests run
        let test_code = problem.test_code.replace("#[ignore]", "");
        let test_slug = problem.slug.replace('-', "_");
        tokio::fs::write(format!("{test_dir}/tests/{test_slug}.rs"), &test_code).await?;

        Ok::<(), std::io::Error>(())
    };

    if let Err(e) = setup.await {
        return (false, format!("Setup failed: {e}"));
    }

    // Run cargo test with timeout
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::process::Command::new("cargo")
            .arg("test")
            .arg("--manifest-path")
            .arg(format!("{test_dir}/Cargo.toml"))
            .env("CARGO_TARGET_DIR", format!("{test_dir}/target"))
            .output(),
    )
    .await;

    // Clean up
    let _ = tokio::fs::remove_dir_all(&test_dir).await;

    match output {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if out.status.success() {
                (true, String::new())
            } else {
                // Extract useful error info
                let error = if stderr.contains("error[E") {
                    // Compilation error — show the first few errors
                    stderr
                        .lines()
                        .filter(|l| l.contains("error") || l.contains("-->"))
                        .take(10)
                        .collect::<Vec<_>>()
                        .join("\n")
                } else if stdout.contains("FAILED") {
                    // Test failure
                    stdout.chars().take(500).collect()
                } else {
                    format!(
                        "stderr: {}\nstdout: {}",
                        stderr.chars().take(300).collect::<String>(),
                        stdout.chars().take(200).collect::<String>()
                    )
                };
                (false, error)
            }
        }
        Ok(Err(e)) => (false, format!("exec error: {e}")),
        Err(_) => (false, "timeout (60s)".into()),
    }
}

/// Run a benchmark session: fetch problems, solve N, validate, record results.
/// Returns the weighted pass rate for this session.
pub async fn run_benchmark_session(
    llm: &LlmClient,
    db: &SoulDatabase,
    workspace_root: &str,
    sample_size: usize,
) -> Result<f64, String> {
    tracing::info!(
        sample_size = sample_size,
        "Starting Exercism Rust benchmark session"
    );

    // Try to load cached problems, fetch if not cached
    let problems = match load_cached_problems(db) {
        Some(p) if p.len() >= 20 => p,
        _ => {
            let fetched = fetch_problems().await?;
            cache_problems(db, &fetched);
            fetched
        }
    };

    let sample = sample_problems(&problems, sample_size);
    let mut total_weight = 0.0f64;
    let mut earned_weight = 0.0f64;
    let mut passed = 0u32;
    let mut attempted = 0u32;
    let now = chrono::Utc::now().timestamp();

    for problem in &sample {
        attempted += 1;
        let weight = difficulty_weight(&problem.difficulty);
        total_weight += weight;

        // Generate solution
        let solution = match generate_solution(llm, problem).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    slug = %problem.slug,
                    difficulty = %problem.difficulty,
                    error = %e,
                    "Benchmark: failed to generate solution"
                );
                record_run(db, problem, false, "", &e, 0);
                continue;
            }
        };

        // Validate via cargo test
        let start = std::time::Instant::now();
        let (success, error_output) = validate_solution(problem, &solution, workspace_root).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        if success {
            passed += 1;
            earned_weight += weight;
            tracing::info!(
                slug = %problem.slug,
                difficulty = %problem.difficulty,
                "Benchmark: PASS"
            );
        } else {
            tracing::info!(
                slug = %problem.slug,
                difficulty = %problem.difficulty,
                error = %error_output.chars().take(100).collect::<String>(),
                "Benchmark: FAIL"
            );
        }

        record_run(db, problem, success, &solution, &error_output, elapsed_ms);
    }

    // Weighted score: difficulty-adjusted pass rate
    let weighted_score = if total_weight > 0.0 {
        earned_weight / total_weight * 100.0
    } else {
        0.0
    };

    let raw_rate = if attempted > 0 {
        passed as f64 / attempted as f64 * 100.0
    } else {
        0.0
    };

    // Store score
    update_score(db, weighted_score, raw_rate, attempted, passed, now);

    tracing::info!(
        weighted = format!("{:.1}%", weighted_score),
        raw = format!("{:.1}%", raw_rate),
        passed = passed,
        attempted = attempted,
        "Exercism Rust benchmark session complete"
    );

    Ok(weighted_score)
}

/// Record a single benchmark run.
fn record_run(
    db: &SoulDatabase,
    problem: &ExercismProblem,
    passed: bool,
    solution: &str,
    error: &str,
    total_ms: u64,
) {
    let run = BenchmarkRun {
        id: uuid::Uuid::new_v4().to_string(),
        task_id: format!("exercism/{}", problem.slug),
        entry_point: problem.slug.clone(),
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
fn update_score(
    db: &SoulDatabase,
    weighted_score: f64,
    raw_rate: f64,
    attempted: u32,
    passed: u32,
    measured_at: i64,
) {
    let mut score = load_score(db).unwrap_or(BenchmarkScore {
        pass_at_1: 0.0,
        raw_pass_rate: 0.0,
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

    score.pass_at_1 = weighted_score;
    score.raw_pass_rate = raw_rate;
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
                weighted = format!("{:.1}%", score.pass_at_1),
                raw = format!("{:.1}%", score.raw_pass_rate),
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

/// Cache fetched problems in soul_state (avoid re-fetching).
fn cache_problems(db: &SoulDatabase, problems: &[ExercismProblem]) {
    if let Ok(json) = serde_json::to_string(problems) {
        let _ = db.set_state("exercism_problems_cache", &json);
    }
}

/// Load cached problems.
fn load_cached_problems(db: &SoulDatabase) -> Option<Vec<ExercismProblem>> {
    db.get_state("exercism_problems_cache")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Strip markdown code blocks from LLM output.
fn strip_code_blocks(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("```") {
        let without_start = if let Some(rest) = s.strip_prefix("```rust") {
            rest
        } else if let Some(rest) = s.strip_prefix("```rs") {
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
    // Check for manual trigger flag (set by POST /soul/benchmark)
    let forced = db
        .get_state("benchmark_force_next")
        .ok()
        .flatten()
        .map(|v| v == "1")
        .unwrap_or(false);
    if forced {
        // Clear the flag so it only fires once
        let _ = db.set_state("benchmark_force_next", "0");
        return true;
    }

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
/// Default: sample 15 problems per session (Rust compilation is slower than Python).
pub const DEFAULT_SAMPLE_SIZE: usize = 15;

/// Format benchmark score for prompt injection.
pub fn benchmark_summary_for_prompt(db: &SoulDatabase) -> String {
    let score = match load_score(db) {
        Some(s) => s,
        None => return String::new(),
    };

    let mut lines = vec![format!(
        "# Exercism Rust Benchmark: {:.1}% weighted ({:.1}% raw, {}/{} problems)",
        score.pass_at_1, score.raw_pass_rate, score.problems_passed, score.problems_attempted
    )];

    lines.push(
        "Exercises are real Exercism Rust problems validated via `cargo test`. \
         Weighted score accounts for difficulty (easy=1x, medium=2x, hard=3x)."
            .into(),
    );

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
    let problem_map: std::collections::HashMap<String, &ExercismProblem> = problems
        .iter()
        .map(|p| (format!("exercism/{}", p.slug), p))
        .collect();

    for sol in peer_solutions {
        // Skip if we already have this solution
        if our_solved.contains(&sol.task_id) || already_imported.contains(&sol.task_id) {
            continue;
        }

        // Validate the peer's solution by running it
        if let Some(problem) = problem_map.get(&sol.task_id) {
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

/// Compute the collective score: our solutions + verified peer solutions.
/// Uses the total exercise count from cache.
pub fn collective_score(db: &SoulDatabase) -> (f64, u32, u32) {
    let all_solutions = export_solutions(db);
    let total_problems = load_cached_problems(db)
        .map(|p| p.len() as u32)
        .unwrap_or(100); // estimate if no cache

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
