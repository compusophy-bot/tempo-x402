//! Opus IQ Benchmark — embedded benchmark for measuring coding intelligence.
//!
//! Uses 182 custom problems from the `opus_bench/` module, designed by Claude Opus 4.6.
//! Problems are solved by the agent's LLM, validated by running `cargo test` in a temp
//! project, and scores are tracked over time with difficulty weighting.
//!
//! Five difficulty tiers measuring distinct cognitive capabilities:
//! - Tier 1 (1x): Multi-constraint Rust coding (generation)
//! - Tier 2 (2x): Find + fix bugs from failing tests (debugging)
//! - Tier 3 (3x): Infer algorithm from I/O examples only (induction)
//! - Tier 4 (4x): Logic puzzles + constraint satisfaction (reasoning)
//! - Tier 5 (5x): Exploit known LLM failure modes (adversarial)
//! - Tier 6 (8x): Multi-step algorithms, precision-critical (brutal)

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;
use crate::llm::LlmClient;

/// A benchmark problem (embedded from opus_bench).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProblem {
    /// Exercise slug, e.g. "opus-stack", "opus-fsm"
    pub slug: String,
    /// The exercise description / instructions (markdown).
    pub instructions: String,
    /// The test file content (tests/*.rs or tests/slug.rs).
    pub test_code: String,
    /// The starter source file (src/lib.rs stub).
    pub starter_code: String,
    /// Difficulty: "tier1", "tier2", "tier3", "tier4", "tier5", "tier6"
    pub difficulty: String,
    /// Cargo.toml content (std-only for Opus problems, may have deps for others).
    #[serde(default)]
    pub cargo_toml: String,
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
        "tier1" => 1.0,
        "tier2" => 2.0,
        "tier3" => 3.0,
        "tier4" => 4.0,
        "tier5" => 5.0,
        "tier6" => 8.0,
        _ => 1.0,
    }
}

/// Reference scores: modern Rust benchmarks (2026 estimates).
pub const REFERENCE_SCORES: &[(&str, f64)] = &[
    ("Gemini 3.1 Flash (self)", 0.0), // will be filled by actual runs
    ("Claude Sonnet 4", 85.0),
    ("GPT-4o", 78.0),
    ("Claude Opus 4", 92.0),
    ("Gemini 3 Pro", 80.0),
];

/// Shared target directory for all benchmark compilations.
/// Deps compile once, then each exercise only recompiles its own lib + tests.
const BENCHMARK_TARGET_DIR: &str = "/tmp/bench_target";

/// Generate a solution for a benchmark problem using the LLM.
/// If `peer_failures` is provided, they're injected as negative context —
/// the LLM sees what was tried before and why it failed, making it more
/// likely to find a different, working approach. This is the core mechanism
/// for proving collective intelligence (2 agents > 1 agent).
pub async fn generate_solution(
    llm: &LlmClient,
    db: &SoulDatabase,
    problem: &BenchmarkProblem,
    peer_failures: &[SharedFailure],
) -> Result<String, String> {
    // Phase 3 weaning: try local codegen model first (saves Gemini credits).
    // Feed it real problem context, not just the slug.
    {
        let codegen_prompt = format!(
            "// Rust solution for: {}\n// Instructions: {}\n// Tests:\n{}\n\n",
            problem.slug,
            problem.instructions.chars().take(500).collect::<String>(),
            problem.test_code.chars().take(1000).collect::<String>(),
        );
        let attempts: u64 = db.get_state("codegen_benchmark_attempts").ok().flatten()
            .and_then(|s| s.parse().ok()).unwrap_or(0);
        let successes: u64 = db.get_state("codegen_benchmark_successes").ok().flatten()
            .and_then(|s| s.parse().ok()).unwrap_or(0);

        match crate::codegen::generate(db, &codegen_prompt, 512) {
            Some(local_code) if local_code.len() > 30 => {
                let _ = db.set_state("codegen_benchmark_attempts", &(attempts + 1).to_string());
                // Log every attempt — we need visibility into codegen quality
                tracing::info!(
                    slug = %problem.slug,
                    chars = local_code.len(),
                    has_fn = local_code.contains("fn "),
                    total_attempts = attempts + 1,
                    total_successes = successes,
                    "Codegen: local model produced output (weaning attempt)"
                );
                // Let cargo test be the judge — any non-trivial output gets a chance.
                // The whole point of the benchmark is to measure real capability.
                // Pattern matching was rejecting potentially valid code.
                let _ = db.set_state("codegen_last_used", "1");
                return Ok(local_code);
            }
            Some(local_code) => {
                tracing::debug!(
                    slug = %problem.slug,
                    chars = local_code.len(),
                    "Codegen: output too short, falling back to Gemini"
                );
            }
            None => {
                tracing::debug!(slug = %problem.slug, "Codegen: model not ready or produced nothing");
            }
        }
    }

    // Extract accumulated lessons from past benchmark runs
    let benchmark_hints = load_benchmark_hints(db);

    // Per-problem context: what specifically failed last time for THIS problem
    let problem_context = {
        let key = format!("problem_context_{}", problem.slug);
        db.get_state(&key).ok().flatten().map(|ctx| {
            format!("\n## Previous Failure Analysis for '{}'\n{}\n\
                     Use this knowledge to avoid the same mistakes.\n", problem.slug, ctx)
        }).unwrap_or_default()
    };

    let base_system = format!(
        "You are a Rust coding expert solving benchmark exercises. \
         Output ONLY valid Rust code for src/lib.rs — no markdown, no explanation, no ```rust blocks. \
         The code must compile and pass ALL tests.\n\n\
         ## CRITICAL Rules\n\
         1. Read EVERY test case carefully — the tests ARE the spec. Match function signatures, return types, trait bounds EXACTLY.\n\
         2. If tests use `assert_eq!` or `assert_ne!`, your types MUST derive or implement `Debug` and `PartialEq`.\n\
         3. If a type parameter `T` is used, check what trait bounds the tests require (Clone, Debug, PartialEq, Ord, etc.).\n\
         4. Export everything the tests import with `pub`. Check `use` statements in tests.\n\
         5. Never write prose or explanations — ONLY Rust code.\n\
         6. Handle ALL edge cases: empty inputs, zero, negative numbers, unicode, overflow.\n\n\
         ## Common Mistakes to AVOID\n\
         - Missing `#[derive(Debug)]` — tests use `assert_eq!` which requires Debug\n\
         - Missing `Clone` bound on generic types — if you call `.clone()` on `T`, add `T: Clone`\n\
         - Wrong return type: `&str` vs `String`, `Option<T>` vs `Result<T,E>` — match the test expectations\n\
         - Forgetting to handle the empty/zero case\n\
         - Writing English text instead of Rust code\n\
         - Not making structs/enums `pub` when tests import them\n\
         - Integer overflow: use `i64`/`u64` or checked arithmetic for large numbers\n\
         - Off-by-one errors in ranges and slicing\n\
         - For exercises with `Display` trait: check if tests call `.to_string()` or use `format!`\n\
         - For exercises with custom errors: check if tests pattern-match on error variants\n\
         {}{}", benchmark_hints, problem_context);

    let system = if peer_failures.is_empty() {
        base_system
    } else {
        format!(
            "{}\n\nIMPORTANT: Previous attempt(s) at this problem FAILED. \
                 Study the failed solution(s) and error output below carefully. \
                 Your solution MUST take a DIFFERENT approach to avoid the same mistake.",
            base_system
        )
    };

    let mut prompt = format!(
        "Implement this Rust exercise: {}\n\n",
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

    // Show available dependencies so LLM knows what crates it can use
    if !problem.cargo_toml.is_empty() && problem.cargo_toml.contains("[dependencies]") {
        let deps_section: String = problem
            .cargo_toml
            .lines()
            .skip_while(|l| !l.contains("[dependencies]"))
            .take_while(|l| !l.starts_with('[') || l.contains("[dependencies]"))
            .collect::<Vec<_>>()
            .join("\n");
        if !deps_section.is_empty() {
            prompt.push_str(&format!(
                "## Available Dependencies (Cargo.toml)\n```toml\n{deps_section}\n```\n\n"
            ));
        }
    }

    // Inject peer failure context — the core collaborative intelligence mechanism
    let task_id = format!("opus/{}", problem.slug);
    let relevant_failures: Vec<&SharedFailure> = peer_failures
        .iter()
        .filter(|f| f.task_id == task_id)
        .collect();
    if !relevant_failures.is_empty() {
        prompt.push_str(
            "## FAILED PREVIOUS ATTEMPTS — study these carefully\n\n",
        );
        for (i, failure) in relevant_failures.iter().enumerate().take(2) {
            let sol_preview: String = failure.failed_solution.chars().take(1500).collect();
            // Parse test output to extract specific failing tests and assertions
            let focused_errors = parse_test_failures(&failure.error_output);
            let err_section = if focused_errors.is_empty() {
                let raw: String = failure.error_output.chars().take(500).collect();
                format!("Raw error:\n```\n{raw}\n```")
            } else {
                format!("Failing tests:\n{focused_errors}")
            };
            prompt.push_str(&format!(
                "### Attempt {} (by {})\n```rust\n{}\n```\n{}\n\n",
                i + 1,
                failure.attempted_by,
                sol_preview,
                err_section,
            ));
        }
        prompt.push_str(
            "Your solution MUST fix these specific test failures. \
             Focus on the EXACT assertion mismatches above.\n\n",
        );
    }

    // Show test code so the LLM knows the expected API
    let test_preview: String = problem.test_code.chars().take(4000).collect();
    prompt.push_str(&format!(
        "## Test Code (must pass all these tests)\n```rust\n{test_preview}\n```\n\n\
         Output ONLY the complete src/lib.rs implementation. No markdown fences."
    ));

    let response = llm
        .think(&system, &prompt)
        .await
        .map_err(|e| format!("LLM generation failed: {e}"))?;

    let cleaned = strip_code_blocks(&response);
    Ok(cleaned)
}

/// Request for peer review of a benchmark solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub slug: String,
    pub instructions: String,
    pub test_code: String,
    pub solution: String,
}

/// Response from peer review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResponse {
    pub bugs_found: Vec<String>,
    pub suggested_fix: String,
    /// Whether the reviewer thinks the solution will pass tests.
    pub likely_passes: bool,
}

/// Review a solution generated by a peer agent.
/// Uses a fundamentally different prompt (critic, not creator) to break
/// the correlation of errors that makes self-review weak.
pub async fn review_solution(
    llm: &LlmClient,
    req: &ReviewRequest,
) -> Result<ReviewResponse, String> {
    let system = "You are a meticulous Rust code reviewer. Your job is to find bugs in a solution \
        to a benchmark exercise. You are NOT the author — you are an independent reviewer. \
        Be adversarial: assume the code has bugs and look hard for them. \
        Check: type mismatches, off-by-one errors, missing edge cases, wrong algorithm, \
        incorrect trait implementations, panic-prone code, integer overflow.\n\n\
        Respond in this EXACT JSON format (no markdown fences):\n\
        {\"bugs_found\": [\"description of bug 1\", ...], \"suggested_fix\": \"complete corrected src/lib.rs code\", \"likely_passes\": false}\n\n\
        If you find NO bugs and the code looks correct, respond:\n\
        {\"bugs_found\": [], \"suggested_fix\": \"\", \"likely_passes\": true}";

    let mut prompt = format!("Review this Rust solution for: {}\n\n", req.slug);

    if !req.instructions.is_empty() {
        let instr: String = req.instructions.chars().take(2000).collect();
        prompt.push_str(&format!("## Problem\n{instr}\n\n"));
    }

    let test_preview: String = req.test_code.chars().take(3000).collect();
    prompt.push_str(&format!(
        "## Tests (solution must pass all)\n```rust\n{test_preview}\n```\n\n"
    ));

    let sol_preview: String = req.solution.chars().take(3000).collect();
    prompt.push_str(&format!(
        "## Solution to Review\n```rust\n{sol_preview}\n```\n\n\
         Find all bugs. If the code is correct, say so. Respond in JSON only."
    ));

    let response = llm
        .think(system, &prompt)
        .await
        .map_err(|e| format!("Review LLM call failed: {e}"))?;

    // Parse the review response
    let cleaned = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    match serde_json::from_str::<ReviewResponse>(cleaned) {
        Ok(review) => Ok(review),
        Err(_) => {
            // If parsing fails, try to extract from the response
            if response.to_lowercase().contains("no bugs")
                || response.to_lowercase().contains("looks correct")
            {
                Ok(ReviewResponse {
                    bugs_found: vec![],
                    suggested_fix: String::new(),
                    likely_passes: true,
                })
            } else {
                // Assume there might be issues
                Ok(ReviewResponse {
                    bugs_found: vec![response.chars().take(200).collect()],
                    suggested_fix: String::new(),
                    likely_passes: false,
                })
            }
        }
    }
}

/// Request peer review from a live peer agent.
/// Returns the review response, or None if the peer is unreachable.
pub async fn request_peer_review(peer_url: &str, req: &ReviewRequest) -> Option<ReviewResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()?;

    let url = format!("{}/soul/benchmark/review", peer_url.trim_end_matches('/'));
    let resp = client.post(&url).json(req).send().await.ok()?;

    if !resp.status().is_success() {
        return None;
    }

    resp.json::<ReviewResponse>().await.ok()
}

/// Validate a solution by creating a temp Cargo project and running `cargo test`.
/// Returns (passed, error_output).
pub async fn validate_solution(
    problem: &BenchmarkProblem,
    solution: &str,
    _workspace_root: &str,
) -> (bool, String) {
    let test_dir = format!("/tmp/bench_{}", problem.slug);

    // Create temp Cargo project
    let setup = async {
        // Clean up any previous run of THIS exercise (not the shared target)
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
        tokio::fs::create_dir_all(format!("{test_dir}/src")).await?;
        tokio::fs::create_dir_all(format!("{test_dir}/tests")).await?;

        // Write Cargo.toml — use the provided one if available, fall back to minimal
        let cargo_toml = if !problem.cargo_toml.is_empty() {
            problem.cargo_toml.clone()
        } else {
            // Use the exercise slug as the crate name — tests import `use {slug}::*`
            format!(
                "[package]\n\
                 name = \"{slug}\"\n\
                 version = \"0.1.0\"\n\
                 edition = \"2021\"\n",
                slug = problem.slug
            )
        };
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

    // Run cargo test with shared target dir (deps compile once across all exercises)
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        tokio::process::Command::new("cargo")
            .arg("test")
            .arg("--manifest-path")
            .arg(format!("{test_dir}/Cargo.toml"))
            .env("CARGO_TARGET_DIR", BENCHMARK_TARGET_DIR)
            .output(),
    )
    .await;

    // Clean up exercise dir (keep shared target for next exercise)
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
        Err(_) => {
            // Clean shared target dir on timeout to prevent cascade failures
            let _ = tokio::fs::remove_dir_all(BENCHMARK_TARGET_DIR).await;
            (false, "timeout (120s)".into())
        }
    }
}

/// Get the URL of a live peer for adversarial review.
/// Returns the first reachable peer URL, or None.
/// Checks: discovered_peers, peer_endpoint_catalog, and PARENT_URL (for clones).
pub fn get_peer_url(db: &SoulDatabase) -> Option<String> {
    let self_url = std::env::var("RAILWAY_PUBLIC_DOMAIN")
        .ok()
        .map(|d| format!("https://{d}"))
        .or_else(|| std::env::var("GATEWAY_URL").ok())
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();

    // Check peer_endpoint_catalog for known peers
    let catalog: Vec<serde_json::Value> = db
        .get_state("peer_endpoint_catalog")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // Also check children/peers from node state if available
    let peers: Vec<serde_json::Value> = db
        .get_state("discovered_peers")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // Try to find a peer URL from either source (skip self)
    for peer in peers.iter().chain(catalog.iter()) {
        if let Some(url) = peer.get("url").and_then(|v| v.as_str()) {
            let normalized = url.trim_end_matches('/');
            if !normalized.is_empty() && normalized != self_url {
                return Some(normalized.to_string());
            }
        }
    }

    // Fallback: PARENT_URL is a valid peer for clones
    if let Ok(parent_url) = std::env::var("PARENT_URL") {
        let normalized = parent_url.trim_end_matches('/').to_string();
        if !normalized.is_empty() && normalized != self_url {
            return Some(normalized);
        }
    }

    None
}

/// Record a single benchmark run.
fn record_run(
    db: &SoulDatabase,
    problem: &BenchmarkProblem,
    passed: bool,
    solution: &str,
    error: &str,
    total_ms: u64,
    task_id: &str,
) {
    let run = BenchmarkRun {
        id: uuid::Uuid::new_v4().to_string(),
        task_id: task_id.to_string(),
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
        if let Some(stripped) = trimmed.strip_suffix("```") {
            return stripped.trim().to_string();
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

    // Don't benchmark in first 3 cycles — minimal warmup
    if total_cycles < 3 {
        return false;
    }

    // Check cooldown — don't run more than once per 5 minutes
    // The benchmark IS the training loop. Run it frequently.
    let last_benchmark: i64 = db
        .get_state("last_benchmark_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let now = chrono::Utc::now().timestamp();
    if now - last_benchmark < 300 {
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

/// Run benchmark every 5 cycles — benchmarking IS the core training loop.
/// Each run samples 15 problems x 1 Gemini call each = 15 API calls per session.
pub const DEFAULT_BENCHMARK_INTERVAL: u64 = 5;
/// Sample 15 problems per session (was 10) — broader coverage per run.
pub const DEFAULT_SAMPLE_SIZE: usize = 15;

/// Run a benchmark session using Opus IQ problems (embedded, no network).
/// Solve via LLM, validate via cargo test, record, train brain.
pub async fn run_opus_benchmark_session(
    llm: &LlmClient,
    db: &SoulDatabase,
    workspace_root: &str,
    sample_size: usize,
) -> Result<f64, String> {
    tracing::info!(
        sample_size = sample_size,
        "Starting Opus IQ benchmark session"
    );

    // Pre-flight: check disk space — cargo test needs ~500MB for compilation
    // Clean any stale benchmark artifacts first
    let _ = tokio::fs::remove_dir_all(BENCHMARK_TARGET_DIR).await;
    let disk_ok = tokio::process::Command::new("df")
        .args(["--output=pcent", "/tmp"])
        .output()
        .await
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            let pct: u64 = s.lines().last().unwrap_or("0")
                .trim().trim_end_matches('%').parse().unwrap_or(0);
            pct < 90
        })
        .unwrap_or(true); // if df fails, try anyway
    if !disk_ok {
        return Err("Disk usage above 90% — skipping benchmark to avoid hang".into());
    }

    let problems = crate::opus_bench::load_embedded_problems();
    if problems.is_empty() {
        return Err("No Opus benchmark problems loaded".into());
    }

    // Build set of already-solved problems to prioritize unsolved
    let all_runs = db.get_all_benchmark_runs().unwrap_or_default();
    let solved_slugs: std::collections::HashSet<String> = all_runs
        .iter()
        .filter(|r| r.passed)
        .map(|r| r.entry_point.clone())
        .collect();

    // Count consecutive failures per problem (only for never-solved problems).
    // Problems with 3+ consecutive failures are "stuck" — deprioritize them.
    let stuck_slugs: std::collections::HashSet<String> = {
        let mut fail_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        // Runs are ordered by created_at DESC, so we iterate recent-first
        for run in &all_runs {
            if solved_slugs.contains(&run.entry_point) {
                continue; // Skip problems that have been solved at least once
            }
            if !run.task_id.starts_with("opus/") {
                continue; // Only count Opus runs
            }
            let count = fail_counts.entry(run.entry_point.clone()).or_insert(0);
            if !run.passed {
                *count += 1;
            }
        }
        fail_counts
            .into_iter()
            .filter(|(_, count)| *count >= 3) // 3 consecutive failures = stuck, move on
            .map(|(slug, _)| slug)
            .collect()
    };

    if !stuck_slugs.is_empty() {
        tracing::info!(
            stuck = stuck_slugs.len(),
            slugs = %stuck_slugs.iter().cloned().collect::<Vec<_>>().join(", "),
            "Opus: deprioritizing stuck problems (3+ consecutive failures, never solved)"
        );
    }

    // Stratified sampling: guarantee at least 1 problem from EACH tier,
    // then fill remaining slots. Skip stuck problems (except 1 retry slot).
    let sample = {
        let mut by_tier: std::collections::HashMap<String, Vec<&BenchmarkProblem>> =
            std::collections::HashMap::new();
        for p in &problems {
            by_tier.entry(p.difficulty.clone()).or_default().push(p);
        }

        let mut selected = Vec::new();
        let seed = chrono::Utc::now().timestamp() as usize;
        let mut rng = seed;

        // Phase 1: one from each tier (guaranteed representation, prefer non-stuck)
        let mut tiers: Vec<String> = by_tier.keys().cloned().collect();
        tiers.sort();
        for tier in &tiers {
            if let Some(tier_problems) = by_tier.get(tier) {
                // Prefer non-stuck problems for tier guarantee
                let non_stuck: Vec<&&BenchmarkProblem> = tier_problems
                    .iter()
                    .filter(|p| !stuck_slugs.contains(&p.slug))
                    .collect();
                let pool = if non_stuck.is_empty() {
                    tier_problems.iter().collect::<Vec<_>>()
                } else {
                    non_stuck
                };
                if !pool.is_empty() {
                    rng = (rng.wrapping_mul(6364136223846793005).wrapping_add(1))
                        % pool.len();
                    selected.push((*pool[rng]).clone());
                }
            }
        }

        // Phase 2: fill remaining slots from unsolved NON-STUCK problems, easier first
        let selected_slugs: std::collections::HashSet<String> =
            selected.iter().map(|p| p.slug.clone()).collect();
        let mut remaining: Vec<&BenchmarkProblem> = problems
            .iter()
            .filter(|p| {
                !selected_slugs.contains(&p.slug)
                    && !solved_slugs.contains(&p.slug)
                    && !stuck_slugs.contains(&p.slug)
            })
            .collect();
        // Sort easier problems first — solve the solvable ones to generate training data.
        remaining.sort_by(|a, b| a.difficulty.cmp(&b.difficulty));

        let slots_left = sample_size.saturating_sub(selected.len() + 1); // reserve 1 for retry
        for p in remaining.iter().take(slots_left) {
            selected.push((*p).clone());
        }

        // Phase 3: 1 retry slot for a stuck problem (occasional re-attempt)
        if !stuck_slugs.is_empty() {
            let stuck_problems: Vec<&BenchmarkProblem> = problems
                .iter()
                .filter(|p| stuck_slugs.contains(&p.slug))
                .collect();
            if !stuck_problems.is_empty() {
                rng = (rng.wrapping_mul(6364136223846793005).wrapping_add(1))
                    % stuck_problems.len();
                selected.push(stuck_problems[rng].clone());
            }
        }

        selected
    };

    tracing::info!(
        total_problems = problems.len(),
        solved = solved_slugs.len(),
        sampled = sample.len(),
        tiers = sample
            .iter()
            .map(|p| p.difficulty.as_str())
            .collect::<Vec<_>>()
            .join(","),
        "Opus IQ: stratified sampling (all tiers guaranteed)"
    );

    let mut total_weight = 0.0f64;
    let mut earned_weight = 0.0f64;
    let mut passed = 0u32;
    let mut attempted = 0u32;
    let now = chrono::Utc::now().timestamp();

    // Brain training data
    let mut brain_attempts: Vec<crate::brain::BenchmarkAttemptContext> = Vec::new();
    let current_elo = crate::elo::load_rating(db) as f32;
    let current_pass_at_1 = db
        .get_state("opus_benchmark_score")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<BenchmarkScore>(&s).ok())
        .map(|s| s.pass_at_1 as f32)
        .unwrap_or(0.0);

    // Load peer failures for collaborative solving
    let peer_failures = load_peer_failures(db);

    for problem in &sample {
        attempted += 1;
        let weight = difficulty_weight(&problem.difficulty);
        total_weight += weight;
        let task_id = format!("opus/{}", problem.slug);

        // Load own past failures for this problem
        let own_failures: Vec<SharedFailure> = db
            .get_all_benchmark_runs()
            .unwrap_or_default()
            .iter()
            .filter(|r| !r.passed && r.task_id == task_id && !r.generated_solution.is_empty())
            .take(2)
            .map(|r| SharedFailure {
                task_id: r.task_id.clone(),
                entry_point: r.entry_point.clone(),
                failed_solution: r.generated_solution.clone(),
                error_output: r.error_output.clone(),
                attempted_by: "self (previous attempt)".to_string(),
            })
            .collect();

        let mut all_failures = own_failures;
        all_failures.extend(
            peer_failures
                .iter()
                .filter(|f| f.task_id == task_id)
                .cloned(),
        );

        // Generate solution
        let solution = match generate_solution(llm, db, problem, &all_failures).await {
            Ok(s) => s,
            Err(e) => {
                let is_api_error = e.contains("429")
                    || e.contains("quota")
                    || e.contains("rate limit")
                    || e.contains("Too Many Requests")
                    || e.contains("503")
                    || e.contains("500");
                if is_api_error {
                    attempted -= 1;
                    total_weight -= weight;
                    tracing::warn!(slug = %problem.slug, error = %e, "Opus: LLM API error — NOT counting");
                } else {
                    tracing::warn!(slug = %problem.slug, tier = %problem.difficulty, error = %e, "Opus: gen failed");
                    record_run(db, problem, false, "", &e, 0, &task_id);
                }
                continue;
            }
        };

        // Validate via cargo test with self-play retries
        let start = std::time::Instant::now();
        let (mut success, mut error_output) =
            validate_solution(problem, &solution, workspace_root).await;

        // Only 1 retry for Opus — 3 retries made tiers 1-5 trivially easy.
        // One retry catches compilation typos; more than that inflates scores.
        let max_retries = 1;
        let mut retry_count = 0;
        let mut last_solution = solution.clone();
        let mut retry_context = all_failures.clone();
        while !success && !error_output.is_empty() && retry_count < max_retries {
            retry_count += 1;
            tracing::info!(slug = %problem.slug, retry = retry_count, "Opus: retrying");
            retry_context.push(SharedFailure {
                task_id: task_id.clone(),
                entry_point: problem.slug.clone(),
                failed_solution: last_solution.clone(),
                error_output: error_output.clone(),
                attempted_by: format!("self (retry {})", retry_count),
            });
            if let Ok(retry_solution) = generate_solution(llm, db, problem, &retry_context).await {
                let (retry_ok, retry_err) =
                    validate_solution(problem, &retry_solution, workspace_root).await;
                if retry_ok {
                    success = true;
                    error_output = String::new();
                } else {
                    error_output = retry_err;
                    last_solution = retry_solution;
                }
            } else {
                break;
            }
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Check if codegen was used for this solution
        let codegen_used = db.get_state("codegen_last_used").ok().flatten()
            .map(|v| v == "1").unwrap_or(false);
        let _ = db.set_state("codegen_last_used", "0"); // Reset flag

        if success {
            passed += 1;
            earned_weight += weight;

            // Phase 3: store passing solution for codegen training
            crate::codegen::record_training_example(
                db,
                &last_solution,
                &format!("opus/{}", problem.slug),
            );

            // DATA MULTIPLIER: generate 2 alternative solutions for already-solved
            // problems. Each verified solution = more training data. The tests are
            // the oracle — any code that passes cargo test is ground truth.
            // Only do this for problems we already solved (don't waste API on unsolved).
            for alt_i in 0..2u32 {
                // Ask for a DIFFERENT approach by injecting the first solution as context
                let alt_context = vec![SharedFailure {
                    task_id: task_id.clone(),
                    entry_point: problem.slug.clone(),
                    failed_solution: last_solution.clone(),
                    error_output: "This solution PASSED but generate a COMPLETELY DIFFERENT \
                        implementation using a different algorithm or data structure. \
                        Do NOT copy this approach.".to_string(),
                    attempted_by: format!("self (alt {alt_i})"),
                }];
                match generate_solution(llm, db, problem, &alt_context).await {
                    Ok(alt_solution) if alt_solution != last_solution => {
                        let (alt_ok, _) =
                            validate_solution(problem, &alt_solution, workspace_root).await;
                        if alt_ok {
                            crate::codegen::record_training_example(
                                db,
                                &alt_solution,
                                &format!("opus/{}/alt{alt_i}", problem.slug),
                            );
                            tracing::info!(
                                slug = %problem.slug,
                                alt = alt_i,
                                "Opus: alternative solution VERIFIED — more training data"
                            );
                        }
                        // Clean up after each alt to prevent /tmp growth
                        let _ = tokio::fs::remove_dir_all(BENCHMARK_TARGET_DIR).await;
                    }
                    _ => {}
                }
            }

            // Track codegen success
            if codegen_used {
                let s: u64 = db.get_state("codegen_benchmark_successes").ok().flatten()
                    .and_then(|s| s.parse().ok()).unwrap_or(0);
                let _ = db.set_state("codegen_benchmark_successes", &(s + 1).to_string());
                tracing::info!(
                    slug = %problem.slug,
                    tier = %problem.difficulty,
                    total_codegen_successes = s + 1,
                    "Opus: PASS (LOCAL CODEGEN — Gemini weaning!)"
                );
            } else {
                tracing::info!(slug = %problem.slug, tier = %problem.difficulty, "Opus: PASS");
            }
        } else {
            tracing::info!(
                slug = %problem.slug,
                tier = %problem.difficulty,
                error = %error_output.chars().take(100).collect::<String>(),
                "Opus: FAIL"
            );
        }

        // Save per-problem failure context for next attempt (persistent across sessions)
        if !success && !error_output.is_empty() {
            let key = format!("problem_context_{}", problem.slug);
            let failures = parse_test_failures(&error_output);
            let ctx = if failures.is_empty() {
                format!("Last error ({}): {}", chrono::Utc::now().format("%Y-%m-%d"),
                    error_output.chars().take(300).collect::<String>())
            } else {
                format!("Last attempt ({}) failed these tests:\n{}\nTry a different algorithm or approach.",
                    chrono::Utc::now().format("%Y-%m-%d"), failures)
            };
            let _ = db.set_state(&key, &ctx);
        } else if success {
            // Clear failure context on success
            let key = format!("problem_context_{}", problem.slug);
            let _ = db.set_state(&key, "");
        }

        // Brain training data
        brain_attempts.push(crate::brain::BenchmarkAttemptContext {
            difficulty: problem.difficulty.clone(),
            passed: success && retry_count == 0,
            retry_number: 0,
            had_peer_context: !peer_failures.is_empty(),
            had_peer_review: false,
            compiled: success || !error_output.contains("error[E"),
            elo_rating: current_elo,
            pass_at_1: current_pass_at_1,
            peer_count: 0,
            problem_slug: problem.slug.clone(),
        });

        record_run(
            db,
            problem,
            success,
            &last_solution,
            &error_output,
            elapsed_ms,
            &task_id,
        );

        // Clean shared target dir after EVERY problem to prevent /tmp from filling.
        // Deps recompile each time (~30s overhead) but that's better than OOM-crashing.
        let _ = tokio::fs::remove_dir_all(BENCHMARK_TARGET_DIR).await;
    }

    // Guard: if no problems were actually attempted (all API errors), skip scoring.
    if attempted == 0 {
        tracing::warn!(
            "Opus IQ: ALL problems skipped due to API errors — NOT updating score/ELO/IQ. \
             Fix the LLM API key/quota and scores will resume."
        );
        return Ok(0.0);
    }

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

    // Store Opus score (also as the primary benchmark score)
    update_opus_score(db, weighted_score, raw_rate, attempted, passed, now);
    update_score(db, weighted_score, raw_rate, attempted, passed, now);

    // Compute IQ
    let iq = crate::opus_bench::weighted_score_to_iq(weighted_score);

    let _ = tokio::fs::remove_dir_all(BENCHMARK_TARGET_DIR).await;

    // Codegen vs Gemini stats — THE metric for real learning
    let codegen_attempts: u64 = db.get_state("codegen_benchmark_attempts").ok().flatten()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    let codegen_successes: u64 = db.get_state("codegen_benchmark_successes").ok().flatten()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    let codegen_rate = if codegen_attempts > 0 {
        codegen_successes as f64 / codegen_attempts as f64 * 100.0
    } else { 0.0 };

    tracing::info!(
        weighted = format!("{:.1}%", weighted_score),
        raw = format!("{:.1}%", raw_rate),
        iq = format!("{:.0}", iq),
        passed = passed,
        attempted = attempted,
        codegen_attempts = codegen_attempts,
        codegen_successes = codegen_successes,
        codegen_rate = format!("{:.1}%", codegen_rate),
        "Opus IQ benchmark session complete"
    );

    // Store IQ and codegen rate for prompt injection
    let _ = db.set_state("opus_iq", &format!("{:.0}", iq));
    let _ = db.set_state("codegen_solve_rate", &format!("{:.1}", codegen_rate));

    // Update benchmark hints from failure analysis — improves future sessions
    update_benchmark_hints(db);

    // Train brain on self-play data
    crate::brain::train_on_benchmark_selfplay(db, &brain_attempts);

    Ok(weighted_score)
}

/// Update Opus benchmark score (separate key for Opus-specific tracking).
fn update_opus_score(
    db: &SoulDatabase,
    weighted_score: f64,
    raw_rate: f64,
    attempted: u32,
    passed: u32,
    measured_at: i64,
) {
    let mut score = db
        .get_state("opus_benchmark_score")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<BenchmarkScore>(&s).ok())
        .unwrap_or(BenchmarkScore {
            pass_at_1: 0.0,
            raw_pass_rate: 0.0,
            problems_attempted: 0,
            problems_passed: 0,
            measured_at: 0,
            history: Vec::new(),
        });

    if score.problems_attempted > 0 {
        score.history.push(HistoricalScore {
            pass_at_1: score.pass_at_1,
            problems_attempted: score.problems_attempted,
            measured_at: score.measured_at,
        });
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
        let _ = db.set_state("opus_benchmark_score", &json);
    }
}

/// Format Opus IQ benchmark for prompt injection.
pub fn opus_summary_for_prompt(db: &SoulDatabase) -> String {
    let score = match db
        .get_state("opus_benchmark_score")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<BenchmarkScore>(&s).ok())
    {
        Some(s) => s,
        None => return String::new(),
    };

    let iq = crate::opus_bench::weighted_score_to_iq(score.pass_at_1);

    let mut lines = vec![format!(
        "# Opus IQ Benchmark: {:.1}% weighted ({:.1}% raw, {}/{} problems) — IQ: {:.0}",
        score.pass_at_1, score.raw_pass_rate, score.problems_passed, score.problems_attempted, iq
    )];

    lines.push(
        "5 tiers: Generation(1x), Debugging(2x), Induction(3x), Reasoning(4x), Adversarial(5x). \
         Designed by Claude Opus 4.6. Higher tiers worth more."
            .into(),
    );

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

/// A failed attempt that can be shared between peers for collaborative solving.
/// The key insight: one agent's failure is another agent's learning signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFailure {
    pub task_id: String,
    pub entry_point: String,
    /// The solution that was attempted but failed.
    pub failed_solution: String,
    /// The error output from cargo test.
    pub error_output: String,
    /// Who attempted it.
    pub attempted_by: String,
}

/// Export failed attempts for peer sharing (collaborative solving).
/// Peers can use these as negative context to avoid the same mistakes.
pub fn export_failures(db: &SoulDatabase) -> Vec<SharedFailure> {
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| "self".into());

    // Get our failed attempts (only most recent per task_id)
    let runs = db.get_all_benchmark_runs().unwrap_or_default();
    let mut failures: std::collections::HashMap<String, SharedFailure> =
        std::collections::HashMap::new();

    for r in &runs {
        if !r.passed && !r.generated_solution.is_empty() && !r.error_output.is_empty() {
            // Keep the most recent failure per task_id
            let entry = failures
                .entry(r.task_id.clone())
                .or_insert_with(|| SharedFailure {
                    task_id: r.task_id.clone(),
                    entry_point: r.entry_point.clone(),
                    failed_solution: r.generated_solution.clone(),
                    error_output: r.error_output.clone(),
                    attempted_by: instance_id.clone(),
                });
            // Update if this is a more recent failure
            if r.created_at > 0 {
                entry.failed_solution = r.generated_solution.clone();
                entry.error_output = r.error_output.clone();
            }
        }
    }

    // Exclude task_ids we eventually solved (failure is no longer relevant)
    let solved: std::collections::HashSet<String> = runs
        .iter()
        .filter(|r| r.passed)
        .map(|r| r.task_id.clone())
        .collect();
    failures.retain(|task_id, _| !solved.contains(task_id));

    failures.into_values().collect()
}

/// Load peer failures from DB (imported via discover_peers).
pub fn load_peer_failures(db: &SoulDatabase) -> Vec<SharedFailure> {
    db.get_state("peer_failures")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Import peer failures for collaborative solving.
pub fn import_failures(db: &SoulDatabase, peer_failures: Vec<SharedFailure>) -> u32 {
    let mut existing: Vec<SharedFailure> = load_peer_failures(db);
    let existing_ids: std::collections::HashSet<String> = existing
        .iter()
        .map(|f| format!("{}:{}", f.task_id, f.attempted_by))
        .collect();

    let mut imported = 0u32;
    for failure in peer_failures {
        let key = format!("{}:{}", failure.task_id, failure.attempted_by);
        if !existing_ids.contains(&key) {
            existing.push(failure);
            imported += 1;
        }
    }

    // Cap at 200 to prevent unbounded growth
    if existing.len() > 200 {
        existing.drain(..existing.len() - 200);
    }

    if imported > 0 {
        if let Ok(json) = serde_json::to_string(&existing) {
            let _ = db.set_state("peer_failures", &json);
        }
    }

    imported
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

    // Load embedded problems for test validation
    let problems = crate::opus_bench::load_embedded_problems();
    let problem_map: std::collections::HashMap<String, &BenchmarkProblem> = problems
        .iter()
        .map(|p| (format!("opus/{}", p.slug), p))
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
/// Uses the total problem count from the embedded problem set.
pub fn collective_score(db: &SoulDatabase) -> (f64, u32, u32) {
    let all_solutions = export_solutions(db);
    let total_problems = crate::opus_bench::load_embedded_problems().len() as u32;

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

/// Parse cargo test output to extract specific failing test names and assertion messages.
/// Returns a focused summary like "- test foo FAILED: left=3, right=5"
fn parse_test_failures(output: &str) -> String {
    let mut failures = Vec::new();
    let mut current_test = String::new();

    for line in output.lines() {
        let trimmed = line.trim();
        // "test foo ... FAILED"
        if trimmed.starts_with("test ") && trimmed.ends_with("FAILED") {
            current_test = trimmed
                .strip_prefix("test ")
                .unwrap_or(trimmed)
                .strip_suffix(" ... FAILED")
                .unwrap_or(trimmed)
                .to_string();
        }
        // "left: `X`" or "right: `Y`" from assert_eq
        if trimmed.starts_with("left:") || trimmed.starts_with("right:") {
            let detail: String = trimmed.chars().take(100).collect();
            if !current_test.is_empty() {
                failures.push(format!("- test `{}` FAILED: {}", current_test, detail));
                current_test.clear();
            } else {
                failures.push(format!("- {}", detail));
            }
        }
        // "thread 'test_name' panicked at 'assertion failed"
        if trimmed.contains("panicked at") {
            let msg: String = trimmed.chars().take(150).collect();
            failures.push(format!("- {}", msg));
        }
    }

    // Also capture any remaining named test that didn't have assertion details
    if !current_test.is_empty() {
        failures.push(format!("- test `{}` FAILED", current_test));
    }

    failures.join("\n")
}

fn load_benchmark_hints(db: &SoulDatabase) -> String {
    // Check if we have cached hints
    if let Ok(Some(hints)) = db.get_state("benchmark_hints") {
        if !hints.is_empty() {
            return format!("\n## Lessons from Past Benchmark Failures\n{}\n", hints);
        }
    }
    String::new()
}

/// Analyze recent benchmark failures and extract common error patterns.
/// Called after each benchmark session to update the hint cache.
pub fn update_benchmark_hints(db: &SoulDatabase) {
    let runs = db.get_all_benchmark_runs().unwrap_or_default();
    let failures: Vec<&BenchmarkRun> = runs
        .iter()
        .filter(|r| !r.passed && !r.error_output.is_empty())
        .collect();

    if failures.len() < 3 {
        return; // Not enough data to extract patterns
    }

    let mut patterns: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();

    for f in &failures {
        let err = &f.error_output;
        // Categorize common Rust errors
        if err.contains("expected `&str`, found `String`")
            || err.contains("expected `String`, found `&str`")
        {
            *patterns.entry("String/&str type mismatch — check test expectations for owned vs borrowed strings").or_default() += 1;
        }
        if err.contains("not found in this scope") || err.contains("cannot find") {
            *patterns.entry("Missing imports or pub exports — ensure all types/functions used by tests are publicly accessible").or_default() += 1;
        }
        if err.contains("trait bound") && err.contains("not satisfied") {
            *patterns.entry("Missing trait implementation — check what traits the tests expect (Display, From, Iterator, etc.)").or_default() += 1;
        }
        if err.contains("mismatched types") {
            *patterns
                .entry("Type mismatch — carefully read the function signature the tests expect")
                .or_default() += 1;
        }
        if err.contains("overflow") || err.contains("attempt to") {
            *patterns
                .entry("Integer overflow/underflow — use checked arithmetic or handle edge cases")
                .or_default() += 1;
        }
        if err.contains("borrow") || err.contains("lifetime") {
            *patterns.entry("Borrow checker issue — prefer owned types (String, Vec) in return positions unless tests require references").or_default() += 1;
        }
        if err.contains("thread 'main' panicked") || err.contains("assertion") {
            *patterns.entry("Logic error — the code compiled but produced wrong output. Trace through the test cases manually").or_default() += 1;
        }
    }

    if patterns.is_empty() {
        return;
    }

    // Sort by frequency and format as hints
    let mut sorted: Vec<(&&str, &u32)> = patterns.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    let hints: Vec<String> = sorted
        .iter()
        .take(5) // Top 5 patterns
        .map(|(pattern, count)| format!("- ({} occurrences) {}", count, pattern))
        .collect();

    let hint_text = hints.join("\n");
    let _ = db.set_state("benchmark_hints", &hint_text);
    tracing::info!(
        patterns = hints.len(),
        failures = failures.len(),
        "Updated benchmark hints from failure analysis"
    );
}
