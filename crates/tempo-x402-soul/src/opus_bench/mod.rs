//! # Opus IQ Benchmark
//!
//! 50 novel problems designed by Claude Opus 4.6 (March 2026).
//! Five difficulty tiers measuring distinct cognitive capabilities.
//! All problems verifiable via `cargo test` — the tests ARE the spec.
//!
//! ## Tiers
//!
//! | Tier | Capability | Weight | Flash Lite Expected |
//! |------|-----------|--------|-------------------|
//! | 1: Generation | Multi-constraint Rust coding | 1× | ~70% |
//! | 2: Debugging | Find + fix bugs from failing tests | 2× | ~40% |
//! | 3: Induction | Infer algorithm from I/O examples only | 3× | ~20% |
//! | 4: Reasoning | Logic puzzles + constraint satisfaction | 4× | ~10% |
//! | 5: Adversarial | Exploit known LLM failure modes | 5× | ~5% |
//! | 6: Brutal | Multi-step algorithms, precision-critical | 8× | ~0% |

mod tier1;
mod tier1_ext;
mod tier1_ext2;
mod tier2;
mod tier2_ext;
mod tier2_ext2;
mod tier3;
mod tier3_ext;
mod tier3_ext2;
mod tier4;
mod tier4_ext;
mod tier5;
mod tier6;

use crate::benchmark::ExercismProblem;

/// Load all embedded Opus IQ benchmark problems.
pub fn load_embedded_problems() -> Vec<ExercismProblem> {
    let mut problems = Vec::new();
    problems.extend(tier1::tier1_generation());
    problems.extend(tier1_ext::tier1_ext());
    problems.extend(tier1_ext2::tier1_ext2());
    problems.extend(tier2::tier2_debugging());
    problems.extend(tier2_ext::tier2_ext());
    problems.extend(tier2_ext2::tier2_ext2());
    problems.extend(tier3::tier3_induction());
    problems.extend(tier3_ext::tier3_ext());
    problems.extend(tier3_ext2::tier3_ext2());
    problems.extend(tier4::tier4_reasoning());
    problems.extend(tier4_ext::tier4_ext());
    problems.extend(tier5::tier5_adversarial());
    problems.extend(tier6::tier6_brutal());
    problems
}

/// Difficulty weight for Opus tiers (higher tiers worth more).
pub fn opus_difficulty_weight(difficulty: &str) -> f64 {
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

/// Map a weighted Opus score to an IQ-like rating.
/// Calibrated: 0% = 85, 50% = 115, 100% = 150.
pub fn weighted_score_to_iq(weighted_pct: f64) -> f64 {
    85.0 + (weighted_pct / 100.0) * 65.0
}

fn problem(
    slug: &str,
    difficulty: &str,
    instructions: &str,
    starter: &str,
    tests: &str,
) -> ExercismProblem {
    ExercismProblem {
        slug: slug.to_string(),
        instructions: instructions.to_string(),
        test_code: tests.to_string(),
        starter_code: starter.to_string(),
        difficulty: difficulty.to_string(),
        cargo_toml: String::new(), // std-only, no external deps
    }
}
