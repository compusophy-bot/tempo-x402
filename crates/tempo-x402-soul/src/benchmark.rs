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
    /// The actual Cargo.toml from Exercism (has correct dependencies).
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
        "hard" => 3.0,
        "medium" => 2.0,
        "tier1" => 1.0,
        "tier2" => 2.0,
        "tier3" => 3.0,
        "tier4" => 4.0,
        "tier5" => 5.0,
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
        .redirect(reqwest::redirect::Policy::limited(5))
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
        .redirect(reqwest::redirect::Policy::limited(5))
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

    // Fetch actual Cargo.toml (has correct dependencies)
    let cargo_toml_url = format!("{EXERCISM_BASE}/{slug}/Cargo.toml");
    let cargo_toml = client
        .get(&cargo_toml_url)
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
    let cargo_toml = match cargo_toml {
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
        cargo_toml,
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
    sample_problems_smart(problems, n, &std::collections::HashSet::new())
}

/// Smart sampling: prioritize unsolved problems, mix in some solved ones for regression testing.
/// `solved_slugs` is the set of problems already solved by the collective.
pub fn sample_problems_smart(
    problems: &[ExercismProblem],
    n: usize,
    solved_slugs: &std::collections::HashSet<String>,
) -> Vec<ExercismProblem> {
    use std::collections::HashSet;

    if problems.len() <= n {
        return problems.to_vec();
    }

    // Split into unsolved and solved
    let unsolved: Vec<&ExercismProblem> = problems
        .iter()
        .filter(|p| !solved_slugs.contains(&p.slug))
        .collect();
    let solved: Vec<&ExercismProblem> = problems
        .iter()
        .filter(|p| solved_slugs.contains(&p.slug))
        .collect();

    // Tier-weighted sampling: harder problems sampled more often.
    // This pushes agents toward tier 3+ (induction, reasoning, adversarial)
    // instead of grinding easy tier 1-2 problems.
    //
    // Weight by difficulty: tier1=1, tier2=2, tier3=4, tier4=6, tier5=8, tier6=10
    // Higher tiers get sampled proportionally more, accelerating learning on hard problems.
    let tier_weight = |difficulty: &str| -> usize {
        match difficulty {
            "tier1" => 1,
            "tier2" => 2,
            "tier3" => 4,
            "tier4" => 6,
            "tier5" => 8,
            "tier6" => 10,
            _ => 1,
        }
    };

    // 70% unsolved (push the frontier), 30% solved (regression testing)
    let unsolved_target = (n * 7 / 10).min(unsolved.len()).max(1);
    let solved_target = n.saturating_sub(unsolved_target).min(solved.len());
    let remaining = n.saturating_sub(unsolved_target + solved_target);

    let seed = chrono::Utc::now().timestamp() as usize;
    let mut selected = Vec::new();

    // Build weighted index for unsolved problems
    let mut weighted_unsolved: Vec<(usize, usize)> = Vec::new(); // (original_idx, cumulative_weight)
    let mut cum_weight = 0usize;
    for (idx, p) in unsolved.iter().enumerate() {
        cum_weight += tier_weight(&p.difficulty);
        weighted_unsolved.push((idx, cum_weight));
    }

    // Weighted random sampling from unsolved
    let mut picked: HashSet<usize> = HashSet::new();
    let mut rng = seed;
    let max_attempts = unsolved_target * 10;
    let mut attempts = 0;
    while picked.len() < unsolved_target && !weighted_unsolved.is_empty() && attempts < max_attempts
    {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let target = rng % cum_weight.max(1);
        // Binary search for the weighted index
        let idx = weighted_unsolved
            .iter()
            .position(|(_, w)| *w > target)
            .unwrap_or(0);
        let orig_idx = weighted_unsolved[idx].0;
        if picked.insert(orig_idx) {
            selected.push(unsolved[orig_idx].clone());
        }
        attempts += 1;
    }

    // Sample from solved (regression — uniform, no tier weighting)
    let mut indices: HashSet<usize> = HashSet::new();
    rng = seed.wrapping_add(12345);
    while indices.len() < (solved_target + remaining) && !solved.is_empty() {
        rng = (rng.wrapping_mul(6364136223846793005).wrapping_add(1)) % solved.len();
        indices.insert(rng);
    }
    for idx in &indices {
        selected.push(solved[*idx].clone());
    }

    selected
}

/// Generate a solution for an Exercism Rust problem using the LLM.
/// If `peer_failures` is provided, they're injected as negative context —
/// the LLM sees what was tried before and why it failed, making it more
/// likely to find a different, working approach. This is the core mechanism
/// for proving collective intelligence (2 agents > 1 agent).
pub async fn generate_solution(
    llm: &LlmClient,
    db: &SoulDatabase,
    problem: &ExercismProblem,
    peer_failures: &[SharedFailure],
) -> Result<String, String> {
    // Extract accumulated lessons from past benchmark runs
    let benchmark_hints = load_benchmark_hints(db);

    let base_system = format!(
        "You are a Rust coding expert solving Exercism exercises. \
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
         {}", benchmark_hints);

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
    let task_id = format!("exercism/{}", problem.slug);
    let relevant_failures: Vec<&SharedFailure> = peer_failures
        .iter()
        .filter(|f| f.task_id == task_id)
        .collect();
    if !relevant_failures.is_empty() {
        prompt.push_str(
            "## FAILED PREVIOUS ATTEMPTS (from peer agents — learn from these mistakes)\n\n",
        );
        for (i, failure) in relevant_failures.iter().enumerate().take(2) {
            let sol_preview: String = failure.failed_solution.chars().take(1500).collect();
            let err_preview: String = failure.error_output.chars().take(500).collect();
            prompt.push_str(&format!(
                "### Attempt {} (by peer {})\n```rust\n{}\n```\n**Error:**\n```\n{}\n```\n\n",
                i + 1,
                failure.attempted_by,
                sol_preview,
                err_preview,
            ));
        }
        prompt.push_str(
            "Your solution MUST avoid the same errors. Take a fundamentally different approach.\n\n",
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
        to an Exercism exercise. You are NOT the author — you are an independent reviewer. \
        Be adversarial: assume the code has bugs and look hard for them. \
        Check: type mismatches, off-by-one errors, missing edge cases, wrong algorithm, \
        incorrect trait implementations, panic-prone code, integer overflow.\n\n\
        Respond in this EXACT JSON format (no markdown fences):\n\
        {\"bugs_found\": [\"description of bug 1\", ...], \"suggested_fix\": \"complete corrected src/lib.rs code\", \"likely_passes\": false}\n\n\
        If you find NO bugs and the code looks correct, respond:\n\
        {\"bugs_found\": [], \"suggested_fix\": \"\", \"likely_passes\": true}";

    let mut prompt = format!("Review this Exercism Rust solution for: {}\n\n", req.slug);

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

/// Shared target directory for all benchmark compilations.
/// Deps compile once, then each exercise only recompiles its own lib + tests.
const BENCHMARK_TARGET_DIR: &str = "/tmp/exercism_bench_target";

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
        // Clean up any previous run of THIS exercise (not the shared target)
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
        tokio::fs::create_dir_all(format!("{test_dir}/src")).await?;
        tokio::fs::create_dir_all(format!("{test_dir}/tests")).await?;

        // Write Cargo.toml — use the actual one from Exercism (has correct deps),
        // fall back to minimal if not available
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

    // Try to load cached problems, fetch if not cached.
    // Invalidate cache if problems lack cargo_toml (old cache format).
    let problems = match load_cached_problems(db) {
        Some(p) if p.len() >= 20 && p.iter().any(|prob| !prob.cargo_toml.is_empty()) => p,
        _ => {
            let fetched = fetch_problems().await?;
            cache_problems(db, &fetched);
            fetched
        }
    };

    // Build set of already-solved problems (own + collective) to prioritize unsolved
    let solved_slugs: std::collections::HashSet<String> = db
        .get_all_benchmark_runs()
        .unwrap_or_default()
        .iter()
        .filter(|r| r.passed)
        .map(|r| r.entry_point.clone())
        .collect();
    let sample = sample_problems_smart(&problems, sample_size, &solved_slugs);
    tracing::info!(
        total_problems = problems.len(),
        solved = solved_slugs.len(),
        sampled = sample.len(),
        "Benchmark: smart sampling (80% unsolved, 20% regression)"
    );
    let mut total_weight = 0.0f64;
    let mut earned_weight = 0.0f64;
    let mut passed = 0u32;
    let mut attempted = 0u32;
    let mut rescued = 0u32; // solved BECAUSE of peer failure context
    let mut review_improved = 0u32; // solved BECAUSE peer review caught bugs
    let now = chrono::Utc::now().timestamp();

    // Collect self-play training data for the brain
    let mut brain_attempts: Vec<crate::brain::BenchmarkAttemptContext> = Vec::new();
    let current_elo = crate::elo::load_rating(db) as f32;
    let current_pass_at_1 = db
        .get_state("benchmark_score")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<BenchmarkScore>(&s).ok())
        .map(|s| s.pass_at_1 as f32)
        .unwrap_or(0.0);
    let current_peer_count = load_peer_failures(db).len() as u32; // rough peer signal

    // Load peer failures for collaborative solving
    let peer_failures = load_peer_failures(db);
    let peer_failure_task_ids: std::collections::HashSet<&str> =
        peer_failures.iter().map(|f| f.task_id.as_str()).collect();

    // Get peer URL for adversarial review (if any peer is live)
    let peer_url = get_peer_url(db);

    for problem in &sample {
        attempted += 1;
        let weight = difficulty_weight(&problem.difficulty);
        total_weight += weight;
        let task_id = format!("exercism/{}", problem.slug);
        let has_peer_context = peer_failure_task_ids.contains(task_id.as_str());

        // Load own past failures for this specific problem — avoid repeating mistakes
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

        // Combine own + peer failures for maximum context
        let mut all_failures: Vec<SharedFailure> = own_failures;
        all_failures.extend(
            peer_failures
                .iter()
                .filter(|f| f.task_id == task_id)
                .cloned(),
        );

        // Generate solution (with own + peer failure context + accumulated hints)
        let solution = match generate_solution(llm, db, problem, &all_failures).await {
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

        // === ADVERSARIAL VERIFICATION ===
        // Agent A generated the solution. Now agent B reviews it.
        // If the reviewer finds bugs and provides a fix, use the fix.
        // This breaks the correlation of errors — the reviewer uses a
        // fundamentally different prompt (critic, not creator).
        let mut used_review_fix = false;
        let final_solution = if let Some(ref peer) = peer_url {
            let review_req = ReviewRequest {
                slug: problem.slug.clone(),
                instructions: problem.instructions.chars().take(2000).collect(),
                test_code: problem.test_code.clone(),
                solution: solution.clone(),
            };

            match request_peer_review(peer, &review_req).await {
                Some(review) if !review.likely_passes && !review.suggested_fix.is_empty() => {
                    tracing::info!(
                        slug = %problem.slug,
                        bugs = review.bugs_found.len(),
                        "Benchmark: peer review found bugs, using suggested fix"
                    );
                    used_review_fix = true;
                    review.suggested_fix
                }
                Some(review) => {
                    tracing::debug!(
                        slug = %problem.slug,
                        likely_passes = review.likely_passes,
                        bugs = review.bugs_found.len(),
                        "Benchmark: peer review complete"
                    );
                    solution.clone()
                }
                None => {
                    tracing::debug!(
                        slug = %problem.slug,
                        "Benchmark: peer unreachable for review, using original solution"
                    );
                    solution.clone()
                }
            }
        } else {
            solution.clone()
        };

        // Validate via cargo test — with retry on failure (self-play)
        let start = std::time::Instant::now();
        let (mut success, mut error_output) =
            validate_solution(problem, &final_solution, workspace_root).await;

        // Self-play retry: if attempt fails, retry up to 3 times with accumulated error context.
        // Each retry sees ALL previous failures for this problem, building understanding.
        // This is how the swarm outperforms a single model call — iterative refinement.
        let max_retries = 3;
        let mut retry_count = 0;
        let mut last_solution = final_solution.clone();
        let mut retry_context = all_failures.clone();
        while !success && !error_output.is_empty() && retry_count < max_retries {
            retry_count += 1;
            tracing::info!(
                slug = %problem.slug,
                retry = retry_count,
                max_retries,
                "Benchmark: attempt failed, retrying with error context (self-play)"
            );
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
                    tracing::info!(
                        slug = %problem.slug,
                        retry = retry_count,
                        "Benchmark: PASS on retry {} (self-play worked!)",
                        retry_count
                    );
                    success = true;
                    error_output = String::new();
                } else {
                    error_output = retry_err;
                    last_solution = retry_solution;
                }
            } else {
                break; // LLM call failed, no point retrying
            }
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        if success {
            passed += 1;
            earned_weight += weight;
            if used_review_fix {
                review_improved += 1;
                tracing::info!(
                    slug = %problem.slug,
                    difficulty = %problem.difficulty,
                    "Benchmark: PASS (IMPROVED by peer review — 2>1 proof)"
                );
            } else if has_peer_context {
                rescued += 1;
                tracing::info!(
                    slug = %problem.slug,
                    difficulty = %problem.difficulty,
                    "Benchmark: PASS (RESCUED by peer failure context)"
                );
            } else {
                tracing::info!(
                    slug = %problem.slug,
                    difficulty = %problem.difficulty,
                    "Benchmark: PASS"
                );
            }
        } else {
            // If the review fix also failed, try the original (maybe reviewer made it worse)
            if used_review_fix {
                let (orig_success, orig_error) =
                    validate_solution(problem, &solution, workspace_root).await;
                if orig_success {
                    passed += 1;
                    earned_weight += weight;
                    tracing::info!(
                        slug = %problem.slug,
                        "Benchmark: PASS (original — reviewer's fix was WRONG)"
                    );
                    record_run(db, problem, true, &solution, "", elapsed_ms);
                    continue;
                }
                tracing::info!(
                    slug = %problem.slug,
                    difficulty = %problem.difficulty,
                    error = %orig_error.chars().take(100).collect::<String>(),
                    "Benchmark: FAIL (both original and review fix failed)"
                );
            } else {
                tracing::info!(
                    slug = %problem.slug,
                    difficulty = %problem.difficulty,
                    error = %error_output.chars().take(100).collect::<String>(),
                    has_peer_context = has_peer_context,
                    "Benchmark: FAIL"
                );
            }
        }

        // Record brain self-play training data for each attempt
        // First attempt
        brain_attempts.push(crate::brain::BenchmarkAttemptContext {
            difficulty: problem.difficulty.clone(),
            passed: success && retry_count == 0,
            retry_number: 0,
            had_peer_context: has_peer_context,
            had_peer_review: used_review_fix,
            compiled: success
                || !error_output.contains("Compiling")
                || error_output.contains("test"),
            elo_rating: current_elo,
            pass_at_1: current_pass_at_1,
            peer_count: current_peer_count,
        });
        // Record each retry as a separate training example
        for r in 0..retry_count {
            brain_attempts.push(crate::brain::BenchmarkAttemptContext {
                difficulty: problem.difficulty.clone(),
                passed: success && r + 1 == retry_count,
                retry_number: r + 1,
                had_peer_context: has_peer_context,
                had_peer_review: used_review_fix,
                compiled: true, // retries only happen after compilation
                elo_rating: current_elo,
                pass_at_1: current_pass_at_1,
                peer_count: current_peer_count,
            });
        }

        record_run(
            db,
            problem,
            success,
            &final_solution,
            &error_output,
            elapsed_ms,
        );
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

    // Clean up shared target dir to reclaim disk space
    let _ = tokio::fs::remove_dir_all(BENCHMARK_TARGET_DIR).await;

    tracing::info!(
        weighted = format!("{:.1}%", weighted_score),
        raw = format!("{:.1}%", raw_rate),
        passed = passed,
        attempted = attempted,
        rescued = rescued,
        review_improved = review_improved,
        peer_failures_available = peer_failures.len(),
        has_review_peer = peer_url.is_some(),
        "Exercism Rust benchmark session complete"
    );

    // Always record multi-agent contribution tracking (even if 0 — shows baseline)
    {
        let _ = db.set_state(
            "benchmark_multiagent",
            &serde_json::json!({
                "rescued": rescued,
                "review_improved": review_improved,
                "total_passed": passed,
                "total_attempted": attempted,
                "solo_passed": passed - rescued - review_improved,
                "multiagent_contribution_pct": if passed > 0 {
                    (rescued + review_improved) as f64 / passed as f64 * 100.0
                } else {
                    0.0
                },
                "has_review_peer": peer_url.is_some(),
                "peer_failures_available": peer_failures.len(),
                "measured_at": now,
            })
            .to_string(),
        );
    }

    // Update benchmark hints from failure analysis — improves future sessions
    update_benchmark_hints(db);

    // Train brain on self-play data — the AlphaZero loop
    // Each benchmark attempt becomes a training signal for the brain
    crate::brain::train_on_benchmark_selfplay(db, &brain_attempts);

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

    // Don't benchmark in first 5 cycles — minimal warmup
    if total_cycles < 5 {
        return false;
    }

    // Check cooldown — don't run more than once per 15 minutes
    // The benchmark IS the training loop. Run it frequently.
    let last_benchmark: i64 = db
        .get_state("last_benchmark_at")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let now = chrono::Utc::now().timestamp();
    if now - last_benchmark < 900 {
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

/// Run benchmark every 10 cycles — the benchmark IS the Rust training curriculum.
/// Every problem solved teaches Rust patterns. Every failure identifies weaknesses.
pub const DEFAULT_BENCHMARK_INTERVAL: u64 = 10;
/// Sample 15 problems per session (was 10) — broader coverage per run.
pub const DEFAULT_SAMPLE_SIZE: usize = 15;

/// Benchmark mode: Exercism (external, Rust exercises) or Opus (embedded, IQ-calibrated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkMode {
    Exercism,
    Opus,
}

impl BenchmarkMode {
    pub fn from_env() -> Self {
        let raw = std::env::var("SOUL_BENCHMARK_MODE").unwrap_or_default();
        let trimmed = raw.trim().to_lowercase();
        let mode = if trimmed == "opus" {
            BenchmarkMode::Opus
        } else {
            BenchmarkMode::Exercism
        };
        tracing::info!(raw = %raw, mode = ?mode, "Benchmark mode selected");
        mode
    }
}

/// Run a benchmark session using Opus IQ problems (embedded, no network).
/// Same pipeline as Exercism: solve via LLM, validate via cargo test, record, train brain.
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

    let problems = crate::opus_bench::load_embedded_problems();
    if problems.is_empty() {
        return Err("No Opus benchmark problems loaded".into());
    }

    // Build set of already-solved problems to prioritize unsolved
    let solved_slugs: std::collections::HashSet<String> = db
        .get_all_benchmark_runs()
        .unwrap_or_default()
        .iter()
        .filter(|r| r.passed)
        .map(|r| r.entry_point.clone())
        .collect();
    // Stratified sampling: guarantee at least 1 problem from EACH tier,
    // then fill remaining slots randomly. This ensures harder tiers are always tested.
    let sample = {
        let mut by_tier: std::collections::HashMap<String, Vec<&ExercismProblem>> =
            std::collections::HashMap::new();
        for p in &problems {
            by_tier.entry(p.difficulty.clone()).or_default().push(p);
        }

        let mut selected = Vec::new();
        let seed = chrono::Utc::now().timestamp() as usize;
        let mut rng = seed;

        // Phase 1: one from each tier (guaranteed representation)
        let mut tiers: Vec<String> = by_tier.keys().cloned().collect();
        tiers.sort(); // deterministic order
        for tier in &tiers {
            if let Some(tier_problems) = by_tier.get(tier) {
                if !tier_problems.is_empty() {
                    rng = (rng.wrapping_mul(6364136223846793005).wrapping_add(1))
                        % tier_problems.len();
                    selected.push(tier_problems[rng].clone());
                }
            }
        }

        // Phase 2: fill remaining slots, preferring unsolved + higher tiers
        let selected_slugs: std::collections::HashSet<String> =
            selected.iter().map(|p| p.slug.clone()).collect();
        let mut remaining: Vec<&ExercismProblem> = problems
            .iter()
            .filter(|p| !selected_slugs.contains(&p.slug) && !solved_slugs.contains(&p.slug))
            .collect();
        // Sort by tier descending (harder first)
        remaining.sort_by(|a, b| b.difficulty.cmp(&a.difficulty));

        let slots_left = sample_size.saturating_sub(selected.len());
        for p in remaining.iter().take(slots_left) {
            selected.push((*p).clone());
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
                tracing::warn!(slug = %problem.slug, tier = %problem.difficulty, error = %e, "Opus: gen failed");
                record_run(db, problem, false, "", &e, 0);
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

        if success {
            passed += 1;
            earned_weight += weight;
            tracing::info!(slug = %problem.slug, tier = %problem.difficulty, "Opus: PASS");
        } else {
            tracing::info!(
                slug = %problem.slug,
                tier = %problem.difficulty,
                error = %error_output.chars().take(100).collect::<String>(),
                "Opus: FAIL"
            );
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
        });

        record_run(
            db,
            problem,
            success,
            &last_solution,
            &error_output,
            elapsed_ms,
        );
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

    // Store Opus-specific score
    update_opus_score(db, weighted_score, raw_rate, attempted, passed, now);

    // Compute IQ
    let iq = crate::opus_bench::weighted_score_to_iq(weighted_score);

    let _ = tokio::fs::remove_dir_all(BENCHMARK_TARGET_DIR).await;

    tracing::info!(
        weighted = format!("{:.1}%", weighted_score),
        raw = format!("{:.1}%", raw_rate),
        iq = format!("{:.0}", iq),
        passed = passed,
        attempted = attempted,
        "Opus IQ benchmark session complete"
    );

    // Store IQ for prompt injection
    let _ = db.set_state("opus_iq", &format!("{:.0}", iq));

    // Train brain on self-play data
    crate::brain::train_on_benchmark_selfplay(db, &brain_attempts);

    Ok(weighted_score)
}

/// Update Opus benchmark score (separate from Exercism score).
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
        "5 tiers: Generation(1×), Debugging(2×), Induction(3×), Reasoning(4×), Adversarial(5×). \
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

/// Extract accumulated lessons from past benchmark failures.
/// Analyzes error patterns across all runs and generates hints for the LLM.
/// Stored in soul_state as "benchmark_hints" and updated after each session.
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
