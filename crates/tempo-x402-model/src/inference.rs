//! Autoregressive plan generation from the transformer model.
//!
//! Given goal context, generates a sequence of plan steps by predicting
//! one token at a time. Uses temperature-scaled sampling for diversity.

use crate::transformer::PlanTransformer;
use crate::vocab::{self, BOS, EOS, PAD, UNK};

/// A generated plan with confidence score.
#[derive(Debug, Clone)]
pub struct GeneratedPlan {
    /// Step type names (e.g., ["read_file", "edit_code", "cargo_check", "commit"]).
    pub steps: Vec<String>,
    /// Token IDs of the generated steps.
    pub tokens: Vec<u32>,
    /// Average log-probability across all generated tokens.
    pub avg_log_prob: f32,
    /// Confidence: exp(avg_log_prob). Higher = model is more certain.
    pub confidence: f32,
}

/// Generate a plan from the model given goal context tokens.
///
/// `temperature`: 0.0 = greedy (always pick highest prob), 1.0 = sample proportionally.
/// `max_steps`: maximum number of plan steps to generate.
pub fn generate_plan(
    model: &PlanTransformer,
    context_tokens: &[u32],
    temperature: f32,
    max_steps: usize,
) -> GeneratedPlan {
    let temp = temperature.max(0.01); // Prevent division by zero
    let max_steps = max_steps.min(vocab::MAX_SEQ_LEN - context_tokens.len() - 2);

    // Start with context + BOS
    let mut tokens: Vec<u32> = context_tokens.to_vec();
    tokens.push(BOS);

    let mut generated_tokens: Vec<u32> = Vec::new();
    let mut total_log_prob: f32 = 0.0;

    // Deterministic seed for sampling (based on context)
    let mut rng_state: u64 = context_tokens.iter().fold(12345u64, |acc, &t| {
        acc.wrapping_mul(6364136223846793005).wrapping_add(t as u64)
    });

    for _ in 0..max_steps {
        let logits = model.forward(&tokens);

        // Temperature-scaled softmax
        let scaled: Vec<f32> = logits.iter().map(|&l| l / temp).collect();
        let probs = PlanTransformer::softmax(&scaled);

        // Mask out invalid tokens (PAD, BOS, UNK, context tokens)
        let mut masked_probs = probs.clone();
        masked_probs[PAD as usize] = 0.0;
        masked_probs[BOS as usize] = 0.0;
        masked_probs[UNK as usize] = 0.0;
        for i in vocab::TOK_CTX_START as usize..=vocab::TOK_CTX_END as usize {
            masked_probs[i] = 0.0;
        }

        // Renormalize
        let sum: f32 = masked_probs.iter().sum();
        if sum < 1e-10 {
            break; // No valid tokens
        }
        for p in &mut masked_probs {
            *p /= sum;
        }

        // Sample or greedy
        let next_token = if temp < 0.1 {
            // Greedy
            masked_probs
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i as u32)
                .unwrap_or(EOS)
        } else {
            // Temperature sampling with deterministic PRNG
            sample_token(&masked_probs, &mut rng_state)
        };

        if next_token == EOS {
            break;
        }

        // Only keep plan step tokens (4-29), not special tokens
        if next_token >= 4 && next_token < vocab::TOK_CTX_START {
            generated_tokens.push(next_token);
            total_log_prob += probs[next_token as usize].max(1e-10).ln();
        }

        tokens.push(next_token);
    }

    let n = generated_tokens.len().max(1) as f32;
    let avg_log_prob = total_log_prob / n;

    GeneratedPlan {
        steps: generated_tokens
            .iter()
            .map(|&t| vocab::Vocab::token_to_step(t).to_string())
            .collect(),
        tokens: generated_tokens,
        avg_log_prob,
        confidence: avg_log_prob.exp(),
    }
}

/// Generate multiple plans and return the best (highest confidence).
pub fn generate_best_plan(
    model: &PlanTransformer,
    context_tokens: &[u32],
    n_candidates: usize,
    max_steps: usize,
) -> Option<GeneratedPlan> {
    let temperatures = [0.0, 0.3, 0.5, 0.7, 1.0];
    let mut best: Option<GeneratedPlan> = None;

    for i in 0..n_candidates {
        let temp = temperatures[i % temperatures.len()];
        let plan = generate_plan(model, context_tokens, temp, max_steps);

        if plan.steps.is_empty() {
            continue;
        }

        // Check if this is better than current best
        if best
            .as_ref()
            .map(|b| plan.confidence > b.confidence)
            .unwrap_or(true)
        {
            best = Some(plan);
        }
    }

    best
}

/// Sample a token from a probability distribution using LCG PRNG.
fn sample_token(probs: &[f32], rng_state: &mut u64) -> u32 {
    *rng_state = rng_state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let u = (*rng_state >> 33) as f32 / (1u64 << 31) as f32; // 0..1

    let mut cumulative = 0.0f32;
    for (i, &p) in probs.iter().enumerate() {
        cumulative += p;
        if u < cumulative {
            return i as u32;
        }
    }
    // Fallback: return highest prob token
    probs
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i as u32)
        .unwrap_or(EOS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocab;

    #[test]
    fn test_generate_plan() {
        let model = PlanTransformer::new();
        let plan = generate_plan(&model, &[], 0.5, 10);
        // Untrained model generates something (may be random)
        println!(
            "Generated: {:?} (confidence: {:.4})",
            plan.steps, plan.confidence
        );
        // Should generate at least 1 step
        assert!(!plan.steps.is_empty() || plan.confidence < 0.01);
    }

    #[test]
    fn test_greedy_is_deterministic() {
        let model = PlanTransformer::new();
        let plan1 = generate_plan(&model, &[], 0.0, 10);
        let plan2 = generate_plan(&model, &[], 0.0, 10);
        assert_eq!(plan1.tokens, plan2.tokens);
    }

    #[test]
    fn test_trained_model_generates_learned_pattern() {
        use crate::trainer::{train_batch, TrainingExample};

        let mut model = PlanTransformer::new();

        // Train on a specific pattern: read → edit → check → commit
        let example = TrainingExample {
            context: vec![],
            steps: vec![
                vocab::TOK_READ_FILE,
                vocab::TOK_EDIT_CODE,
                vocab::TOK_CARGO_CHECK,
                vocab::TOK_COMMIT,
            ],
            weight: 1.0,
            source: "test".to_string(),
        };

        // Train 500 rounds (small transformer needs more passes to converge)
        for _ in 0..500 {
            train_batch(&mut model, &[example.clone()]);
        }

        // Generate — should produce steps from the learned pattern
        let plan = generate_plan(&model, &[], 0.0, 6);
        println!("Trained model generates: {:?}", plan.steps);
        // The model should have learned at least one step from the training pattern
        let learned_steps = ["read_file", "edit_code", "cargo_check", "commit"];
        assert!(
            plan.steps.iter().any(|s| learned_steps.contains(&s.as_str())),
            "Model should generate at least one step from the training pattern, got: {:?}",
            plan.steps
        );
    }
}
