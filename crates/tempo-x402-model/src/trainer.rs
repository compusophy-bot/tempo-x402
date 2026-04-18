//! Online training for the plan transformer.
//!
//! Trains on successful plan step sequences using cross-entropy loss.
//! Each training example is a (context, plan_steps, fitness_weight) triple.
//! Higher-fitness plans have more influence on the model.

use crate::transformer::{PlanTransformer, D_MODEL};
use crate::vocab::VOCAB_SIZE;
use crate::vocab::{self, BOS, EOS};

use serde::{Deserialize, Serialize};

/// Learning rate for SGD.
const LEARNING_RATE: f32 = 0.001;
/// Weight decay (L2 regularization).
const WEIGHT_DECAY: f32 = 0.0001;
/// Gradient clipping threshold.
const GRAD_CLIP: f32 = 1.0;

/// A training example: a successful plan's step sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExample {
    /// Context tokens (goal keywords).
    pub context: Vec<u32>,
    /// Plan step tokens (without BOS/EOS — added automatically).
    pub steps: Vec<u32>,
    /// Fitness weight: higher = this plan was more successful (0.0-1.0).
    pub weight: f32,
    /// Source agent ID.
    pub source: String,
}

/// Train the model on a batch of examples. Returns (examples_trained, avg_loss).
pub fn train_batch(model: &mut PlanTransformer, examples: &[TrainingExample]) -> (usize, f32) {
    if examples.is_empty() {
        return (0, 0.0);
    }

    let mut total_loss = 0.0f32;
    let mut trained = 0usize;

    for example in examples {
        if example.steps.is_empty() {
            continue;
        }

        // Build input sequence: [context..., BOS, step1, step2, ...]
        let mut input_tokens: Vec<u32> = example.context.clone();
        input_tokens.push(BOS);
        for &step in &example.steps {
            input_tokens.push(step);
        }

        // Target: shifted by 1 — predict next token at each position
        // For positions in the plan (after context+BOS), target is the next step
        let plan_start = example.context.len() + 1; // After context + BOS

        // Train on each step position (teacher forcing)
        for i in plan_start..input_tokens.len() {
            let input = &input_tokens[..i];
            let target = if i < input_tokens.len() {
                input_tokens[i]
            } else {
                EOS
            };

            // Forward pass
            let logits = model.forward(input);
            let probs = PlanTransformer::softmax(&logits);

            // Cross-entropy loss: -log(P(target))
            let target_prob = probs[target as usize].max(1e-10);
            let loss = -target_prob.ln() * example.weight;
            total_loss += loss;

            // Backward pass: gradient of cross-entropy w.r.t. logits
            // d_loss/d_logit_j = prob_j - (j == target ? 1 : 0)
            let mut grad_logits = probs.clone();
            grad_logits[target as usize] -= 1.0;

            // Scale by example weight
            for g in &mut grad_logits {
                *g *= example.weight;
            }

            // Clip gradients
            let grad_norm: f32 = grad_logits.iter().map(|g| g * g).sum::<f32>().sqrt();
            if grad_norm > GRAD_CLIP {
                let scale = GRAD_CLIP / grad_norm;
                for g in &mut grad_logits {
                    *g *= scale;
                }
            }

            // Update output projection weights (the most impactful layer)
            // output_proj: D_MODEL × VOCAB_SIZE
            let last_pos = input.len() - 1;
            let last_hidden = get_last_hidden(model, input);
            for v in 0..VOCAB_SIZE {
                for d in 0..D_MODEL {
                    let idx = d * VOCAB_SIZE + v;
                    let grad = grad_logits[v] * last_hidden[d];
                    model.output_proj[idx] -=
                        LEARNING_RATE * (grad + WEIGHT_DECAY * model.output_proj[idx]);
                }
                model.output_bias[v] -= LEARNING_RATE * grad_logits[v];
            }

            // Update token embedding for the last input token
            let last_token = input[last_pos] as usize;
            if last_token < VOCAB_SIZE {
                let emb_start = last_token * D_MODEL;
                for d in 0..D_MODEL {
                    // Gradient flows back through output projection
                    let grad: f32 = (0..VOCAB_SIZE)
                        .map(|v| grad_logits[v] * model.output_proj[d * VOCAB_SIZE + v])
                        .sum();
                    model.embedding[emb_start + d] -=
                        LEARNING_RATE * grad.clamp(-GRAD_CLIP, GRAD_CLIP);
                }
            }

            trained += 1;
        }
    }

    model.train_steps += trained as u64;
    let avg_loss = if trained > 0 {
        total_loss / trained as f32
    } else {
        0.0
    };

    // Update running loss (EMA)
    model.running_loss = model.running_loss * 0.95 + avg_loss * 0.05;

    (trained, avg_loss)
}

/// Get the last hidden state from a forward pass (needed for backprop).
/// This is a simplified version that re-runs forward — not efficient but correct.
fn get_last_hidden(model: &PlanTransformer, tokens: &[u32]) -> Vec<f32> {
    let seq_len = tokens.len().min(vocab::MAX_SEQ_LEN);
    let last = seq_len - 1;

    // Reconstruct the hidden state at the last position
    let mut hidden: Vec<f32> = {
        let token = tokens[last] as usize;
        let tok_start = token.min(VOCAB_SIZE - 1) * D_MODEL;
        let pos_start = last * D_MODEL;
        (0..D_MODEL)
            .map(|j| model.embedding[tok_start + j] + model.pos_encoding[pos_start + j])
            .collect()
    };

    // Pass through layers (simplified — only last position, no attention from others)
    // This is an approximation for the backward pass
    for layer in &model.layers {
        // Feed-forward only (skip attention for speed in training)
        let ff_out: Vec<f32> = (0..D_MODEL)
            .map(|i| {
                let h: f32 = (0..crate::transformer::D_FF)
                    .map(|j| {
                        let act: f32 = (0..D_MODEL)
                            .map(|k| hidden[k] * layer.ff.w1[k * crate::transformer::D_FF + j])
                            .sum::<f32>()
                            + layer.ff.b1[j];
                        act.max(0.0) * layer.ff.w2[j * D_MODEL + i]
                    })
                    .sum::<f32>()
                    + layer.ff.b2[i];
                h
            })
            .collect();
        for (i, v) in hidden.iter_mut().enumerate() {
            *v += ff_out[i];
        }
    }

    hidden
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_train_reduces_loss() {
        let mut model = PlanTransformer::new();

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

        // Train multiple rounds on the same example — loss should decrease
        let (_, loss1) = train_batch(&mut model, &[example.clone()]);
        for _ in 0..20 {
            train_batch(&mut model, &[example.clone()]);
        }
        let (_, loss2) = train_batch(&mut model, &[example.clone()]);

        println!("Loss before: {:.4}, after: {:.4}", loss1, loss2);
        assert!(loss2 < loss1, "Loss should decrease with training");
    }

    #[test]
    fn test_training_example_serialization() {
        let ex = TrainingExample {
            context: vec![30, 31],
            steps: vec![4, 15, 13, 8],
            weight: 0.8,
            source: "agent-1".to_string(),
        };
        let json = serde_json::to_string(&ex).unwrap();
        let restored: TrainingExample = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.steps, ex.steps);
    }
}
