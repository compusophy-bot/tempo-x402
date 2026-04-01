//! Local code generation model — Phase 3.
//!
//! 350M param Rust-specialist transformer. Trained from benchmark
//! solutions + commit diffs. Tries local generation first, falls
//! back to Gemini if confidence is low.
//!
//! NOT YET IMPLEMENTED — this module defines the target architecture
//! and training data pipeline interface. The colony watches Ψ(t) and
//! `ready_for_phase3()` to decide when to start building this.

/// Target architecture constants (Phase 3).
pub const CODEGEN_D_MODEL: usize = 768;
pub const CODEGEN_N_HEADS: usize = 12;
pub const CODEGEN_N_LAYERS: usize = 12;
pub const CODEGEN_D_FF: usize = 3072;
pub const CODEGEN_VOCAB_SIZE: usize = 8192; // BPE tokenizer
pub const CODEGEN_MAX_SEQ: usize = 1024;
pub const CODEGEN_PARAMS: usize = 350_000_000;

/// Training data source types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TrainingSource {
    /// Verified benchmark solution (passed tests).
    BenchmarkSolution {
        problem_id: String,
        code: String,
        passed: bool,
        tier: u32,
    },
    /// Commit diff with quality score from the quality model.
    CommitDiff {
        sha: String,
        diff: String,
        quality_score: f32,
    },
    /// Solution imported from a colony peer.
    PeerSolution {
        peer_id: String,
        problem_id: String,
        code: String,
    },
}

/// Readiness check: should we start building the local model?
///
/// Conditions:
/// - Ψ(t) > 0.5 (colony is healthy and learning)
/// - >500 training examples accumulated
/// - benchmark pass@1 > 60% (baseline competence established)
///
/// The colony watches these signals and begins Phase 3 when ready.
pub fn ready_for_phase3(psi: f64, training_examples: usize, pass_at_1: f64) -> bool {
    psi > 0.5 && training_examples > 500 && pass_at_1 > 60.0
}

/// Estimated memory usage for the target model at fp16.
pub const CODEGEN_MEMORY_MB: usize = CODEGEN_PARAMS * 2 / (1024 * 1024); // ~700 MB

// ── Phase 3 Validation Model (50M params) ──────────────────────────
// Smaller model to validate the training pipeline before scaling to 350M.
// Same architecture, reduced dimensions.

/// Validation model constants.
pub const SMALL_D_MODEL: usize = 512;
pub const SMALL_N_HEADS: usize = 8;
pub const SMALL_D_HEAD: usize = SMALL_D_MODEL / SMALL_N_HEADS; // 64
pub const SMALL_N_LAYERS: usize = 8;
pub const SMALL_D_FF: usize = 2048;
pub const SMALL_MAX_SEQ: usize = 512;
pub const SMALL_VOCAB: usize = 8192;

/// Code generation model — decoder-only transformer.
///
/// Phase 3 validation: 50M params (D=512, 8 layers, 8 heads).
/// Trained on BPE-tokenized Rust benchmark solutions.
/// Generates Rust code token by token.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeGenModel {
    /// Token embeddings: VOCAB × D_MODEL
    pub embeddings: Vec<f32>,
    /// Positional encodings: MAX_SEQ × D_MODEL
    pub pos_encoding: Vec<f32>,
    /// Transformer layers
    pub layers: Vec<CodeGenLayer>,
    /// Output projection: D_MODEL × VOCAB (tied with embeddings for weight sharing)
    pub output_bias: Vec<f32>,
    /// Training metadata
    pub train_steps: u64,
    pub running_loss: f32,
    /// Dimensions (for validation on deserialization)
    pub d_model: usize,
    pub n_layers: usize,
    pub vocab_size: usize,
    pub max_seq: usize,
}

/// A single transformer layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeGenLayer {
    /// Multi-head attention: Q, K, V projections + output
    pub wq: Vec<f32>, // D_MODEL × D_MODEL
    pub wk: Vec<f32>,
    pub wv: Vec<f32>,
    pub wo: Vec<f32>,
    /// Feed-forward: D_MODEL → D_FF → D_MODEL
    pub ff_w1: Vec<f32>,
    pub ff_w2: Vec<f32>,
    /// Layer norm parameters (simplified: just scale)
    pub ln1_scale: Vec<f32>,
    pub ln2_scale: Vec<f32>,
}

impl CodeGenModel {
    /// Create a new model with Xavier initialization.
    pub fn new() -> Self {
        let d = SMALL_D_MODEL;
        let v = SMALL_VOCAB;
        let s = SMALL_MAX_SEQ;
        let ff = SMALL_D_FF;
        let n = SMALL_N_LAYERS;

        let mut rng = XorShift64(42);

        let embeddings = xavier_init(&mut rng, v * d, v, d);
        let pos_encoding = xavier_init(&mut rng, s * d, s, d);
        let output_bias = vec![0.0; v];

        let layers = (0..n)
            .map(|_| CodeGenLayer {
                wq: xavier_init(&mut rng, d * d, d, d),
                wk: xavier_init(&mut rng, d * d, d, d),
                wv: xavier_init(&mut rng, d * d, d, d),
                wo: xavier_init(&mut rng, d * d, d, d),
                ff_w1: xavier_init(&mut rng, d * ff, d, ff),
                ff_w2: xavier_init(&mut rng, ff * d, ff, d),
                ln1_scale: vec![1.0; d],
                ln2_scale: vec![1.0; d],
            })
            .collect();

        Self {
            embeddings,
            pos_encoding,
            layers,
            output_bias,
            train_steps: 0,
            running_loss: 0.0,
            d_model: d,
            n_layers: n,
            vocab_size: v,
            max_seq: s,
        }
    }

    /// Approximate parameter count.
    pub fn param_count(&self) -> usize {
        let d = self.d_model;
        let v = self.vocab_size;
        let s = self.max_seq;
        let ff = SMALL_D_FF;
        let n = self.n_layers;

        let embed = v * d + s * d;
        let per_layer = 4 * d * d + 2 * d * ff + 2 * d; // attn + ff + ln
        embed + n * per_layer + v // output bias
    }

    /// Forward pass: tokens → logits for next token prediction.
    /// Returns logits of shape [vocab_size] for the LAST token position.
    pub fn forward(&self, tokens: &[u32]) -> Vec<f32> {
        let d = self.d_model;
        let seq_len = tokens.len().min(self.max_seq);

        if seq_len == 0 {
            return vec![0.0; self.vocab_size];
        }

        // Embed + position
        let mut hidden = vec![0.0f32; seq_len * d];
        for (pos, &tok) in tokens.iter().take(seq_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            for j in 0..d {
                hidden[pos * d + j] =
                    self.embeddings[tok_idx * d + j] + self.pos_encoding[pos * d + j];
            }
        }

        // Transformer layers
        for layer in &self.layers {
            hidden = self.apply_layer(layer, &hidden, seq_len);
        }

        // Output projection (last position): hidden[last] × embeddings^T + bias
        let last_hidden = &hidden[(seq_len - 1) * d..seq_len * d];
        let mut logits = vec![0.0f32; self.vocab_size];
        for v_idx in 0..self.vocab_size {
            let mut dot = self.output_bias[v_idx];
            for j in 0..d {
                dot += last_hidden[j] * self.embeddings[v_idx * d + j]; // weight tying
            }
            logits[v_idx] = dot;
        }

        logits
    }

    /// Apply a single transformer layer (simplified causal attention + FFN).
    fn apply_layer(&self, layer: &CodeGenLayer, input: &[f32], seq_len: usize) -> Vec<f32> {
        let d = self.d_model;
        let n_heads = SMALL_N_HEADS;
        let d_head = SMALL_D_HEAD;
        let ff = SMALL_D_FF;

        // Layer norm 1
        let normed = layer_norm(input, &layer.ln1_scale, seq_len, d);

        // Multi-head causal attention
        let mut attn_out = vec![0.0f32; seq_len * d];
        for h in 0..n_heads {
            // Project Q, K, V for this head
            for pos in 0..seq_len {
                let inp = &normed[pos * d..(pos + 1) * d];

                // Compute attention scores for this position
                let mut q = vec![0.0f32; d_head];
                for j in 0..d_head {
                    let w_idx = (h * d_head + j) * d;
                    q[j] = (0..d).map(|k| inp[k] * layer.wq[w_idx + k]).sum::<f32>();
                }

                // Attend to all previous positions (causal)
                let mut weights = vec![0.0f32; pos + 1];
                for prev in 0..=pos {
                    let prev_inp = &normed[prev * d..(prev + 1) * d];
                    let mut k = 0.0f32;
                    for j in 0..d_head {
                        let w_idx = (h * d_head + j) * d;
                        let k_j: f32 = (0..d).map(|kk| prev_inp[kk] * layer.wk[w_idx + kk]).sum();
                        k += q[j] * k_j;
                    }
                    weights[prev] = k / (d_head as f32).sqrt();
                }

                // Softmax
                let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let exp_sum: f32 = weights.iter().map(|w| (w - max_w).exp()).sum();
                for w in &mut weights {
                    *w = (*w - max_w).exp() / exp_sum;
                }

                // Weighted sum of values
                for prev in 0..=pos {
                    let prev_inp = &normed[prev * d..(prev + 1) * d];
                    for j in 0..d_head {
                        let w_idx = (h * d_head + j) * d;
                        let v_j: f32 = (0..d).map(|kk| prev_inp[kk] * layer.wv[w_idx + kk]).sum();
                        attn_out[pos * d + h * d_head + j] += weights[prev] * v_j;
                    }
                }
            }
        }

        // Output projection + residual
        let mut residual = input.to_vec();
        for pos in 0..seq_len {
            let a = &attn_out[pos * d..(pos + 1) * d];
            for j in 0..d {
                let out_j: f32 = (0..d).map(|k| a[k] * layer.wo[j * d + k]).sum();
                residual[pos * d + j] += out_j;
            }
        }

        // Layer norm 2
        let normed2 = layer_norm(&residual, &layer.ln2_scale, seq_len, d);

        // Feed-forward + residual
        for pos in 0..seq_len {
            let inp = &normed2[pos * d..(pos + 1) * d];
            // Up projection: D → D_FF with ReLU
            let mut ff_hidden = vec![0.0f32; ff];
            for j in 0..ff {
                let w_idx = j * d;
                let val: f32 = (0..d).map(|k| inp[k] * layer.ff_w1[w_idx + k]).sum();
                ff_hidden[j] = val.max(0.0); // ReLU
            }
            // Down projection: D_FF → D
            for j in 0..d {
                let w_idx = j * ff;
                let val: f32 = (0..ff).map(|k| ff_hidden[k] * layer.ff_w2[w_idx + k]).sum();
                residual[pos * d + j] += val;
            }
        }

        residual
    }

    /// Train on a single example (input tokens → predict next token).
    /// Returns loss (cross-entropy).
    ///
    /// Backpropagates through:
    /// 1. Output bias (8K params)
    /// 2. Output projection via tied embeddings (vocab × d_model params)
    /// 3. Last transformer layer's FFN (d_model × d_ff × 2 params)
    ///
    /// Attention layers stay frozen — full attention backprop is Phase 3.5.
    /// But embeddings + last FFN = majority of useful gradient signal.
    pub fn train_step(&mut self, tokens: &[u32], learning_rate: f32) -> f32 {
        if tokens.len() < 2 {
            return 0.0;
        }

        let d = self.d_model;
        let input = &tokens[..tokens.len() - 1];
        let target = tokens[tokens.len() - 1];
        let seq_len = input.len().min(self.max_seq);

        // Forward pass — need to capture intermediate activations for backprop
        // 1. Embed + position
        let mut hidden = vec![0.0f32; seq_len * d];
        for (pos, &tok) in input.iter().take(seq_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            for j in 0..d {
                hidden[pos * d + j] =
                    self.embeddings[tok_idx * d + j] + self.pos_encoding[pos * d + j];
            }
        }

        // 2. Transformer layers (forward only, save output)
        for layer in &self.layers {
            hidden = self.apply_layer(layer, &hidden, seq_len);
        }

        // 3. Output projection (last position)
        let last_hidden = &hidden[(seq_len - 1) * d..seq_len * d];
        let mut logits = vec![0.0f32; self.vocab_size];
        for v_idx in 0..self.vocab_size {
            let mut dot = self.output_bias[v_idx];
            for j in 0..d {
                dot += last_hidden[j] * self.embeddings[v_idx * d + j];
            }
            logits[v_idx] = dot;
        }

        // Loss
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = logits.iter().map(|l| (l - max_logit).exp()).sum();
        let log_sum_exp = max_logit + exp_sum.ln();
        let target_idx = target as usize % self.vocab_size;
        let loss = log_sum_exp - logits[target_idx];

        // Softmax gradient: d(loss)/d(logit_i) = softmax_i - 1{i=target}
        let softmax: Vec<f32> = logits.iter().map(|l| (l - log_sum_exp).exp()).collect();
        let mut d_logits = softmax.clone();
        d_logits[target_idx] -= 1.0;

        // === BACKPROP ===

        // 1. Output bias gradient
        for i in 0..self.vocab_size {
            self.output_bias[i] -= learning_rate * d_logits[i];
        }

        // 2. Embedding gradient (tied weights used as output projection)
        // d(loss)/d(embedding[v,j]) = d_logits[v] * last_hidden[j]
        // d(loss)/d(last_hidden[j]) = sum_v(d_logits[v] * embedding[v,j])
        let mut d_hidden = vec![0.0f32; d];
        for v_idx in 0..self.vocab_size {
            if d_logits[v_idx].abs() < 1e-7 { continue; } // skip near-zero gradients
            let grad_v = d_logits[v_idx];
            let emb_offset = v_idx * d;
            for j in 0..d {
                // Update embedding
                self.embeddings[emb_offset + j] -= learning_rate * grad_v * last_hidden[j];
                // Accumulate gradient for hidden layer
                d_hidden[j] += grad_v * self.embeddings[emb_offset + j];
            }
        }

        // 3. Also update input embeddings for the tokens in this sequence
        // Gradient flows through the embedding lookup
        for (pos, &tok) in input.iter().take(seq_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            let emb_offset = tok_idx * d;
            // Small gradient push toward the hidden representation
            // (approximation — true gradient requires backprop through all layers)
            let scale = learning_rate * 0.1; // damped to avoid instability
            for j in 0..d {
                self.embeddings[emb_offset + j] -= scale * d_hidden[j] / seq_len as f32;
            }
        }

        // 4. Backprop through last layer's FFN with proper gradients
        // Recompute the FFN forward for the last position to get exact activations
        if let Some(layer) = self.layers.last_mut() {
            let ff = SMALL_D_FF;
            let last_pos = seq_len - 1;

            // Recompute FFN forward for last position (we need ff_hidden activations)
            // Use last_hidden as input (approximate — true input is after LN2)
            let mut ff_hidden = vec![0.0f32; ff];
            for k in 0..ff {
                let w_idx = k * d;
                let val: f32 = (0..d).map(|j| last_hidden[j] * layer.ff_w1[w_idx + j]).sum();
                ff_hidden[k] = val.max(0.0); // ReLU
            }

            // Backprop: d_hidden → ff_w2 gradient
            // residual[j] += sum_k(ff_hidden[k] * ff_w2[j*ff+k])
            // d(loss)/d(ff_w2[j*ff+k]) = d_hidden[j] * ff_hidden[k]
            let lr_ff = learning_rate * 0.1;
            let mut d_ff_hidden = vec![0.0f32; ff];
            for j in 0..d {
                for k in 0..ff {
                    let w2_idx = j * ff + k;
                    // Update ff_w2
                    layer.ff_w2[w2_idx] -= lr_ff * d_hidden[j] * ff_hidden[k];
                    // Accumulate gradient for ff_hidden
                    d_ff_hidden[k] += d_hidden[j] * layer.ff_w2[w2_idx];
                }
            }

            // Backprop through ReLU → ff_w1
            // ff_hidden[k] = relu(sum_j(input[j] * ff_w1[k*d+j]))
            // d(loss)/d(ff_w1[k*d+j]) = d_ff_hidden[k] * input[j] * (pre_relu > 0)
            for k in 0..ff {
                if ff_hidden[k] <= 0.0 { continue; } // ReLU gate: dead = no gradient
                let w_idx = k * d;
                for j in 0..d {
                    layer.ff_w1[w_idx + j] -= lr_ff * d_ff_hidden[k] * last_hidden[j];
                }
            }
        }

        self.train_steps += 1;
        self.running_loss = 0.95 * self.running_loss + 0.05 * loss;

        loss
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON with dimension validation.
    pub fn from_json(json: &str) -> Option<Self> {
        let model: Self = serde_json::from_str(json).ok()?;
        if model.d_model != SMALL_D_MODEL || model.n_layers != SMALL_N_LAYERS {
            return None; // Architecture mismatch — discard stale weights
        }
        Some(model)
    }
}

impl Default for CodeGenModel {
    fn default() -> Self {
        Self::new()
    }
}

// ── Utilities ──────────────────────────────────────────────────────

/// Simple layer normalization (mean=0, var=1, then scale).
fn layer_norm(input: &[f32], scale: &[f32], seq_len: usize, d: usize) -> Vec<f32> {
    let mut output = input.to_vec();
    for pos in 0..seq_len {
        let slice = &input[pos * d..(pos + 1) * d];
        let mean: f32 = slice.iter().sum::<f32>() / d as f32;
        let var: f32 = slice.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / d as f32;
        let std = (var + 1e-5).sqrt();
        for j in 0..d {
            output[pos * d + j] = (slice[j] - mean) / std * scale[j];
        }
    }
    output
}

/// XorShift64 PRNG for deterministic initialization.
struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        // Map to [-1, 1]
        (self.0 as f32 / u64::MAX as f32) * 2.0 - 1.0
    }
}

/// Xavier uniform initialization.
fn xavier_init(rng: &mut XorShift64, size: usize, fan_in: usize, fan_out: usize) -> Vec<f32> {
    let limit = (6.0 / (fan_in + fan_out) as f32).sqrt();
    (0..size).map(|_| rng.next() * limit).collect()
}
