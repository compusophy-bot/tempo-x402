//! Benchmark runner — runs a CodeGenerator against the Opus-201 benchmark.

use x402_soul::benchmark::{validate_solution, BenchmarkProblem};
use x402_soul::opus_bench;

/// Trait for any code generation backend (Claude, Gemini, Qwen, local model).
#[async_trait::async_trait]
pub trait CodeGenerator: Send + Sync {
    /// Generate a Rust solution for a benchmark problem.
    async fn generate(&self, problem: &BenchmarkProblem) -> Result<String, String>;
    /// Human-readable name for results (e.g. "claude-opus-4", "qwen-0.5b-base").
    fn name(&self) -> &str;
}

/// Result of a single problem attempt.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProblemResult {
    pub slug: String,
    pub tier: String,
    pub passed: bool,
    pub time_ms: u64,
    pub error: String,
    pub solution: String,
}

/// Run the Opus-201 benchmark with a given generator.
pub async fn run_benchmark(
    generator: &dyn CodeGenerator,
    limit: Option<usize>,
    output_path: &str,
) {
    let problems = opus_bench::load_embedded_problems();
    run_benchmark_on(generator, &problems, limit, output_path).await;
}

/// Run a benchmark on an arbitrary set of problems.
pub async fn run_benchmark_on(
    generator: &dyn CodeGenerator,
    problems: &[BenchmarkProblem],
    limit: Option<usize>,
    output_path: &str,
) {
    let total = match limit {
        Some(n) if n > 0 => n.min(problems.len()),
        _ => problems.len(),
    };

    tracing::info!(
        model = generator.name(),
        total_problems = total,
        "Starting Opus-201 benchmark"
    );

    let mut results = Vec::new();
    let mut passed = 0u32;
    let mut total_weight = 0.0f64;
    let mut earned_weight = 0.0f64;

    for (i, problem) in problems.iter().take(total).enumerate() {
        let weight = opus_bench::opus_difficulty_weight(&problem.difficulty);
        total_weight += weight;

        tracing::info!(
            "[{}/{}] {} ({})",
            i + 1,
            total,
            problem.slug,
            problem.difficulty
        );

        let start = std::time::Instant::now();
        let solution = match generator.generate(problem).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(slug = %problem.slug, error = %e, "Generation failed");
                results.push(ProblemResult {
                    slug: problem.slug.clone(),
                    tier: problem.difficulty.clone(),
                    passed: false,
                    time_ms: start.elapsed().as_millis() as u64,
                    error: e,
                    solution: String::new(),
                });
                continue;
            }
        };

        // Validate via cargo test
        let (pass, error_output) = validate_solution(problem, &solution, "/tmp").await;
        let time_ms = start.elapsed().as_millis() as u64;

        if pass {
            passed += 1;
            earned_weight += weight;
            tracing::info!(slug = %problem.slug, time_ms, "PASS");
        } else {
            tracing::info!(slug = %problem.slug, time_ms, "FAIL");
        }

        results.push(ProblemResult {
            slug: problem.slug.clone(),
            tier: problem.difficulty.clone(),
            passed: pass,
            time_ms,
            error: error_output,
            solution,
        });
    }

    let raw_pct = if total > 0 {
        passed as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let weighted_pct = if total_weight > 0.0 {
        earned_weight / total_weight * 100.0
    } else {
        0.0
    };

    tracing::info!(
        model = generator.name(),
        passed,
        total,
        raw = format!("{raw_pct:.1}%"),
        weighted = format!("{weighted_pct:.1}%"),
        "Benchmark complete"
    );

    // Save results
    let output = crate::results::BenchmarkOutput {
        model: generator.name().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        total_problems: total,
        passed: passed as usize,
        raw_pass_rate: raw_pct,
        weighted_pass_rate: weighted_pct,
        results,
    };

    if let Err(e) = crate::results::save(&output, output_path) {
        tracing::error!(error = %e, "Failed to save results");
    } else {
        tracing::info!(path = output_path, "Results saved");
    }
}
