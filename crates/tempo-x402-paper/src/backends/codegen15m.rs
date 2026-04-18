//! Ablation backend: the from-scratch 15M parameter encoder-decoder model.
//!
//! Wraps the existing `x402_model::codegen::CodeGenModel` for benchmarking.
//! This serves as the ablation study — same training data, same verification,
//! but trained from random initialization instead of a pretrained base.

use crate::runner::CodeGenerator;
use x402_soul::benchmark::BenchmarkProblem;

pub struct CodeGen15MGenerator {
    name: String,
    model: x402_model::codegen::CodeGenModel,
    tokenizer: x402_model::bpe::BpeTokenizer,
}

impl CodeGen15MGenerator {
    /// Load from a JSON weights file, or create a fresh model.
    pub fn new(weights_path: Option<&str>) -> Self {
        let model = if let Some(path) = weights_path {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|json| x402_model::codegen::CodeGenModel::from_json(&json))
                .unwrap_or_else(|| {
                    tracing::warn!("Failed to load weights from {path}, using fresh model");
                    x402_model::codegen::CodeGenModel::new()
                })
        } else {
            x402_model::codegen::CodeGenModel::new()
        };

        let name = format!(
            "codegen-15m-steps-{}",
            model.train_steps
        );

        Self {
            name,
            model,
            tokenizer: x402_model::bpe::BpeTokenizer::new(8192),
        }
    }

    /// Load tokenizer from a JSON file (previously trained BPE).
    pub fn with_tokenizer(mut self, tokenizer_json: &str) -> Self {
        if let Ok(json) = std::fs::read_to_string(tokenizer_json) {
            if let Some(tok) = x402_model::bpe::BpeTokenizer::from_json(&json) {
                self.tokenizer = tok;
            }
        }
        self
    }

    /// Train the model on a set of training examples (from self-play data).
    pub fn train_on_examples(&mut self, examples: &[crate::selfplay::TrainingExample]) {
        for ex in examples {
            let mut target_tokens = vec![x402_model::bpe::BOS_TOKEN];
            target_tokens.extend(self.tokenizer.encode(&ex.output));
            target_tokens.push(x402_model::bpe::EOS_TOKEN);
            if target_tokens.len() > x402_model::codegen::SMALL_MAX_SEQ {
                target_tokens.truncate(x402_model::codegen::SMALL_MAX_SEQ);
            }

            let mut context_tokens = vec![x402_model::bpe::BOS_TOKEN];
            context_tokens.extend(self.tokenizer.encode(&ex.instruction));
            context_tokens.push(x402_model::bpe::EOS_TOKEN);
            if context_tokens.len() > x402_model::codegen::SMALL_MAX_SEQ {
                context_tokens.truncate(x402_model::codegen::SMALL_MAX_SEQ);
            }

            if target_tokens.len() >= 3 && context_tokens.len() >= 3 {
                let lr = 0.001;
                self.model.train_enc_dec(&context_tokens, &target_tokens, lr);
            }
        }
    }
}

#[async_trait::async_trait]
impl CodeGenerator for CodeGen15MGenerator {
    async fn generate(&self, problem: &BenchmarkProblem) -> Result<String, String> {
        if self.tokenizer.merges.is_empty() {
            return Err("BPE tokenizer not trained (0 merges)".to_string());
        }
        if self.model.train_steps < 10 {
            return Err(format!(
                "Model needs >=10 training steps (has {})",
                self.model.train_steps
            ));
        }

        // Build context from problem
        let context = format!(
            "{}\n\nTests:\n{}\n\nStarter:\n{}",
            problem.instructions, problem.test_code, problem.starter_code
        );

        // Encode context
        let mut context_tokens = vec![x402_model::bpe::BOS_TOKEN];
        context_tokens.extend(self.tokenizer.encode(&context));
        context_tokens.push(x402_model::bpe::EOS_TOKEN);
        if context_tokens.len() > self.model.max_seq {
            context_tokens.truncate(self.model.max_seq);
        }

        let encoder_output = self.model.encode(&context_tokens);
        let enc_len = context_tokens.len().min(self.model.max_seq);

        // Decode
        let mut tokens = vec![x402_model::bpe::BOS_TOKEN];
        let max_tokens = 512;

        for _ in 0..max_tokens {
            if tokens.len() >= self.model.max_seq {
                break;
            }
            let logits = self.model.decode(&tokens, &encoder_output, enc_len);

            // Temperature sampling (0.8)
            let temperature: f32 = 0.8;
            let scaled: Vec<f32> = logits.iter().map(|l| l / temperature).collect();
            let max_logit = scaled.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = scaled.iter().map(|l| (l - max_logit).exp()).collect();
            let sum: f32 = exps.iter().sum();
            let probs: Vec<f32> = exps.iter().map(|e| e / sum).collect();

            let seed = (tokens.len() as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64);
            let r = ((seed >> 16) as f32) / (u32::MAX as f32);
            let mut cumulative = 0.0f32;
            let mut chosen = 0u32;
            for (i, p) in probs.iter().enumerate() {
                cumulative += p;
                if cumulative >= r {
                    chosen = i as u32;
                    break;
                }
            }

            if chosen == x402_model::bpe::EOS_TOKEN {
                break;
            }
            tokens.push(chosen);
        }

        if tokens.len() <= 1 {
            return Err("Model generated no tokens".to_string());
        }

        let generated = self.tokenizer.decode(&tokens[1..]);
        if generated.trim().is_empty() {
            return Err("Model generated empty output".to_string());
        }

        Ok(generated)
    }

    fn name(&self) -> &str {
        &self.name
    }
}
