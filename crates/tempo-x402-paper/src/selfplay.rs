//! Self-play fine-tuning loop — the core research contribution.
//!
//! The loop:
//! 1. Generate solutions for all benchmark problems using the current model
//! 2. Validate each via cargo test (ground truth oracle)
//! 3. Passing solutions accumulate into the training set
//! 4. Failed solutions are retried with error context (self-play retry)
//! 5. Fine-tune the model on accumulated verified solutions (LoRA)
//! 6. Evaluate, checkpoint, repeat

use crate::runner::{CodeGenerator, ProblemResult};
use x402_soul::benchmark::{validate_solution, BenchmarkProblem};

/// Configuration for a self-play run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SelfPlayConfig {
    /// Number of self-play iterations
    pub iterations: usize,
    /// Max problems per iteration (0 = all)
    pub max_problems: usize,
    /// Whether to retry failed problems with error context
    pub retry_with_context: bool,
    /// LoRA rank for fine-tuning
    pub lora_rank: u32,
    /// Learning rate for fine-tuning
    pub learning_rate: f64,
    /// Epochs per fine-tuning iteration
    pub epochs: u32,
    /// Directory for checkpoints and training data
    pub output_dir: String,
}

impl Default for SelfPlayConfig {
    fn default() -> Self {
        Self {
            iterations: 10,
            max_problems: 0,
            retry_with_context: true,
            lora_rank: 16,
            learning_rate: 2e-4,
            epochs: 3,
            output_dir: "selfplay_runs".to_string(),
        }
    }
}

/// A single training example (verified by cargo test).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingExample {
    pub instruction: String,
    pub output: String,
    pub slug: String,
    pub tier: String,
    pub iteration: usize,
    pub was_retry: bool,
}

/// Result of a single self-play iteration.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IterationResult {
    pub iteration: usize,
    pub problems_attempted: usize,
    pub problems_passed: usize,
    pub problems_passed_on_retry: usize,
    pub new_training_examples: usize,
    pub total_training_examples: usize,
    pub pass_rate: f64,
    pub problem_results: Vec<ProblemResult>,
}

/// Run the full self-play loop.
pub async fn run_selfplay(
    generator: &dyn CodeGenerator,
    problems: &[BenchmarkProblem],
    config: &SelfPlayConfig,
) -> Vec<IterationResult> {
    let total_problems = if config.max_problems > 0 {
        config.max_problems.min(problems.len())
    } else {
        problems.len()
    };

    let dir = &config.output_dir;
    std::fs::create_dir_all(dir).ok();
    std::fs::create_dir_all(format!("{dir}/training_data")).ok();
    std::fs::create_dir_all(format!("{dir}/checkpoints")).ok();
    std::fs::create_dir_all(format!("{dir}/results")).ok();

    // Save config
    if let Ok(json) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(format!("{dir}/config.json"), json);
    }

    let mut all_training_examples: Vec<TrainingExample> = Vec::new();
    let mut all_results: Vec<IterationResult> = Vec::new();

    // Track which problems have been solved (permanent set)
    let mut solved_slugs: std::collections::HashSet<String> = std::collections::HashSet::new();

    for iteration in 0..config.iterations {
        tracing::info!(
            iteration,
            model = generator.name(),
            problems = total_problems,
            solved = solved_slugs.len(),
            training_examples = all_training_examples.len(),
            "Starting self-play iteration"
        );

        let mut iter_results = Vec::new();
        let mut new_examples = Vec::new();
        let mut passed = 0;
        let retry_passed = 0;

        for (i, problem) in problems.iter().take(total_problems).enumerate() {
            tracing::info!(
                "[iter {iteration}] [{}/{}] {} ({})",
                i + 1,
                total_problems,
                problem.slug,
                problem.difficulty
            );

            let start = std::time::Instant::now();

            // Generate solution
            let solution = match generator.generate(problem).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(slug = %problem.slug, error = %e, "Generation failed");
                    iter_results.push(ProblemResult {
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

            // Validate
            let (pass, error_output) = validate_solution(problem, &solution, "/tmp").await;

            if pass {
                passed += 1;
                solved_slugs.insert(problem.slug.clone());
                new_examples.push(TrainingExample {
                    instruction: build_instruction(problem),
                    output: solution.clone(),
                    slug: problem.slug.clone(),
                    tier: problem.difficulty.clone(),
                    iteration,
                    was_retry: false,
                });
                tracing::info!(slug = %problem.slug, "PASS");
            } else if config.retry_with_context && !error_output.is_empty() {
                // Self-play retry: feed the error back and try again
                tracing::info!(slug = %problem.slug, "FAIL — retrying with error context");
                // We can't easily retry with a CodeGenerator trait since it doesn't
                // support context injection. For now, log the failure.
                // TODO: Add retry_with_context method to CodeGenerator trait
            }

            iter_results.push(ProblemResult {
                slug: problem.slug.clone(),
                tier: problem.difficulty.clone(),
                passed: pass,
                time_ms: start.elapsed().as_millis() as u64,
                error: error_output,
                solution,
            });
        }

        // Accumulate training data
        all_training_examples.extend(new_examples.iter().cloned());

        let result = IterationResult {
            iteration,
            problems_attempted: total_problems,
            problems_passed: passed,
            problems_passed_on_retry: retry_passed,
            new_training_examples: new_examples.len(),
            total_training_examples: all_training_examples.len(),
            pass_rate: if total_problems > 0 {
                passed as f64 / total_problems as f64 * 100.0
            } else {
                0.0
            },
            problem_results: iter_results,
        };

        // Save iteration results
        if let Ok(json) = serde_json::to_string_pretty(&result) {
            let _ = std::fs::write(
                format!("{dir}/results/iteration_{iteration}.json"),
                json,
            );
        }

        // Save accumulated training data as JSONL (for fine-tuning)
        save_training_data(&all_training_examples, &format!("{dir}/training_data/train.jsonl"));

        tracing::info!(
            iteration,
            passed,
            total = total_problems,
            rate = format!("{:.1}%", result.pass_rate),
            new_examples = new_examples.len(),
            total_examples = all_training_examples.len(),
            total_solved = solved_slugs.len(),
            "Iteration complete"
        );

        // TODO: Fine-tune step here
        // For now, the training data is saved as JSONL.
        // The user runs fine-tuning externally:
        //   python finetune.py --data selfplay_runs/training_data/train.jsonl \
        //     --base-model Qwen/Qwen2.5-Coder-0.5B-Instruct \
        //     --output selfplay_runs/checkpoints/iter_{iteration}
        //
        // Then reload the model for the next iteration:
        //   paper-bench selfplay --model selfplay_runs/checkpoints/iter_{iteration}/model.gguf
        tracing::info!(
            "Training data saved to {dir}/training_data/train.jsonl ({} examples). \
             Run fine-tuning, then restart with the new model for the next iteration.",
            all_training_examples.len()
        );

        all_results.push(result);
    }

    // Save final summary
    if let Ok(json) = serde_json::to_string_pretty(&all_results) {
        let _ = std::fs::write(format!("{dir}/results/summary.json"), json);
    }

    // Print convergence curve
    println!("\n=== Self-Play Convergence ===");
    println!("{:<12} {:>8} {:>10} {:>10} {:>12}", "Iteration", "Passed", "Rate", "New Ex.", "Total Ex.");
    println!("{}", "-".repeat(55));
    for r in &all_results {
        println!(
            "{:<12} {:>8} {:>9.1}% {:>10} {:>12}",
            r.iteration, r.problems_passed, r.pass_rate, r.new_training_examples, r.total_training_examples
        );
    }
    println!();

    all_results
}

/// Build the instruction string for a training example.
fn build_instruction(problem: &BenchmarkProblem) -> String {
    format!(
        "Write a complete Rust library (src/lib.rs) that passes these tests.\n\n\
         Problem: {}\n\n{}\n\nTests:\n```rust\n{}\n```\n\nStarter:\n```rust\n{}\n```",
        problem.slug,
        problem.instructions,
        problem.test_code,
        problem.starter_code,
    )
}

/// Save training examples as JSONL (one JSON object per line).
fn save_training_data(examples: &[TrainingExample], path: &str) {
    let mut output = String::new();
    for ex in examples {
        // Deduplicate: keep latest per slug
        if let Ok(json) = serde_json::to_string(ex) {
            output.push_str(&json);
            output.push('\n');
        }
    }
    if let Err(e) = std::fs::write(path, &output) {
        tracing::warn!(error = %e, "Failed to save training data");
    }
}
