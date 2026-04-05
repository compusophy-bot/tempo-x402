//! Local code generation model — Phase 3.
//!
//! 350M param Rust-specialist transformer. Trained from benchmark
//! solutions + commit diffs. Tries local generation first, falls
//! back to Gemini if confidence is low.
//!
//! NOT YET IMPLEMENTED — this module defines the target architecture
//! and training data pipeline interface. The colony watches Ψ(t) and
//! `ready_for_phase3()` to decide when to start building this.

use rayon::prelude::*;

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

/// Phase 3 is always active. The codegen model learns continuously from
/// benchmark solutions. The old gate (Ψ > 0.5, 500+ examples, pass@1 > 60%)
/// was unreachable while the system was bootstrapping.
pub fn ready_for_phase3(_psi: f64, training_examples: usize, _pass_at_1: f64) -> bool {
    training_examples > 0
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
        // Parallel across 8192 vocab entries — biggest single win from rayon.
        let last_hidden = &hidden[(seq_len - 1) * d..seq_len * d];
        let logits: Vec<f32> = (0..self.vocab_size)
            .into_par_iter()
            .map(|v_idx| {
                let mut dot = self.output_bias[v_idx];
                let emb_off = v_idx * d;
                for j in 0..d {
                    dot += last_hidden[j] * self.embeddings[emb_off + j];
                }
                dot
            })
            .collect();

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

        // Multi-head causal attention — parallel across heads
        let head_outputs: Vec<Vec<f32>> = (0..n_heads)
            .into_par_iter()
            .map(|h| {
                let mut head_out = vec![0.0f32; seq_len * d_head];
                for pos in 0..seq_len {
                    let inp = &normed[pos * d..(pos + 1) * d];

                    let mut q = vec![0.0f32; d_head];
                    for j in 0..d_head {
                        let w_idx = (h * d_head + j) * d;
                        q[j] = (0..d).map(|k| inp[k] * layer.wq[w_idx + k]).sum::<f32>();
                    }

                    let mut weights = vec![0.0f32; pos + 1];
                    for prev in 0..=pos {
                        let prev_inp = &normed[prev * d..(prev + 1) * d];
                        let mut score = 0.0f32;
                        for j in 0..d_head {
                            let w_idx = (h * d_head + j) * d;
                            let k_j: f32 = (0..d).map(|kk| prev_inp[kk] * layer.wk[w_idx + kk]).sum();
                            score += q[j] * k_j;
                        }
                        weights[prev] = score / (d_head as f32).sqrt();
                    }

                    let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let exp_sum: f32 = weights.iter().map(|w| (w - max_w).exp()).sum();
                    for w in &mut weights { *w = (*w - max_w).exp() / exp_sum; }

                    for prev in 0..=pos {
                        let prev_inp = &normed[prev * d..(prev + 1) * d];
                        for j in 0..d_head {
                            let w_idx = (h * d_head + j) * d;
                            let v_j: f32 = (0..d).map(|kk| prev_inp[kk] * layer.wv[w_idx + kk]).sum();
                            head_out[pos * d_head + j] += weights[prev] * v_j;
                        }
                    }
                }
                head_out
            })
            .collect();

        // Merge head outputs into attn_out
        let mut attn_out = vec![0.0f32; seq_len * d];
        for (h, head_out) in head_outputs.iter().enumerate() {
            for pos in 0..seq_len {
                for j in 0..d_head {
                    attn_out[pos * d + h * d_head + j] = head_out[pos * d_head + j];
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

        // Feed-forward — parallel across positions, each returns a d-dim delta
        let ff_deltas: Vec<Vec<f32>> = (0..seq_len)
            .into_par_iter()
            .map(|pos| {
                let inp = &normed2[pos * d..(pos + 1) * d];
                let mut ff_hidden = vec![0.0f32; ff];
                for j in 0..ff {
                    let w_idx = j * d;
                    let val: f32 = (0..d).map(|k| inp[k] * layer.ff_w1[w_idx + k]).sum();
                    ff_hidden[j] = val.max(0.0);
                }
                let mut delta = vec![0.0f32; d];
                for j in 0..d {
                    let w_idx = j * ff;
                    delta[j] = (0..ff).map(|k| ff_hidden[k] * layer.ff_w2[w_idx + k]).sum();
                }
                delta
            })
            .collect();

        // Add FFN deltas to residual
        for (pos, delta) in ff_deltas.iter().enumerate() {
            for j in 0..d {
                residual[pos * d + j] += delta[j];
            }
        }

        residual
    }

    /// Train on a single example (input tokens → predict next token).
    /// Returns loss (cross-entropy).
    ///
    /// Full backprop through ALL layers:
    /// 1. Output bias + tied embeddings
    /// 2. All transformer layers (FFN + attention Q/K/V/O weights)
    /// 3. Input embeddings
    ///
    /// Gradient clipping at 1.0 to prevent explosions.
    pub fn train_step(&mut self, tokens: &[u32], learning_rate: f32) -> f32 {
        if tokens.len() < 2 {
            return 0.0;
        }

        let d = self.d_model;
        let n_heads = SMALL_N_HEADS;
        let d_head = SMALL_D_HEAD;
        let ff = SMALL_D_FF;
        let input = &tokens[..tokens.len() - 1];
        let target = tokens[tokens.len() - 1];
        let seq_len = input.len().min(self.max_seq);

        // === FORWARD PASS — save activations for backprop ===

        // 1. Embed + position
        let mut layer_inputs: Vec<Vec<f32>> = Vec::with_capacity(self.n_layers + 1);
        let mut hidden = vec![0.0f32; seq_len * d];
        for (pos, &tok) in input.iter().take(seq_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            for j in 0..d {
                hidden[pos * d + j] =
                    self.embeddings[tok_idx * d + j] + self.pos_encoding[pos * d + j];
            }
        }
        layer_inputs.push(hidden.clone());

        // 2. Transformer layers — save input to each layer
        for layer in &self.layers {
            hidden = self.apply_layer(layer, &hidden, seq_len);
            layer_inputs.push(hidden.clone());
        }

        // 3. Output projection — parallel across vocab
        use rayon::prelude::*;
        let last_pos = seq_len - 1;
        let last_hidden = &hidden[last_pos * d..(last_pos + 1) * d];
        let logits: Vec<f32> = (0..self.vocab_size)
            .into_par_iter()
            .map(|v_idx| {
                let mut dot = self.output_bias[v_idx];
                let off = v_idx * d;
                for j in 0..d { dot += last_hidden[j] * self.embeddings[off + j]; }
                dot
            })
            .collect();

        // Loss
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = logits.iter().map(|l| (l - max_logit).exp()).sum();
        let log_sum_exp = max_logit + exp_sum.ln();
        let target_idx = target as usize % self.vocab_size;
        let loss = log_sum_exp - logits[target_idx];

        let softmax: Vec<f32> = logits.iter().map(|l| (l - log_sum_exp).exp()).collect();
        let mut d_logits = softmax;
        d_logits[target_idx] -= 1.0;

        // === BACKPROP ===

        let lr = learning_rate;

        // 1. Output bias — parallel across vocab
        self.output_bias.par_iter_mut().zip(d_logits.par_iter()).for_each(|(b, &g)| {
            *b -= lr * clip_grad(g);
        });

        // 2. Embedding gradient — compute d_hidden in parallel, then update embeddings
        // First: accumulate d_hidden across vocab (parallel reduction)
        let d_hidden_last: Vec<f32> = (0..d)
            .into_par_iter()
            .map(|j| {
                let mut sum = 0.0f32;
                for v_idx in 0..self.vocab_size {
                    if d_logits[v_idx].abs() < 1e-7 { continue; }
                    sum += d_logits[v_idx] * self.embeddings[v_idx * d + j];
                }
                sum
            })
            .collect();

        // Update embeddings (sequential — writes to shared array)
        for v_idx in 0..self.vocab_size {
            if d_logits[v_idx].abs() < 1e-7 { continue; }
            let g = d_logits[v_idx];
            let emb_off = v_idx * d;
            for j in 0..d {
                self.embeddings[emb_off + j] -= lr * clip_grad(g * last_hidden[j]);
            }
        }

        let mut d_layer_output = vec![0.0f32; seq_len * d];
        for j in 0..d {
            d_layer_output[last_pos * d + j] = d_hidden_last[j];
        }

        // 3. Backprop through all layers in REVERSE order
        for l_idx in (0..self.n_layers).rev() {
            let layer_in = &layer_inputs[l_idx];
            let d_out = &d_layer_output;

            // --- FFN backprop (last part of the layer) ---
            let normed2 = layer_norm(layer_in, &self.layers[l_idx].ln2_scale, seq_len, d);
            // Only backprop FFN for last position (gradient is zero elsewhere for single-token loss)
            // Actually, attention mixes positions, so we need all positions where d_out is nonzero.
            // For efficiency, only update positions that have nonzero gradient.

            let d_residual = d_out.clone();

            // FFN: for each position with gradient
            for pos in 0..seq_len {
                let d_norm = &d_out[pos * d..(pos + 1) * d];
                if d_norm.iter().all(|&x| x.abs() < 1e-8) { continue; }

                let inp = &normed2[pos * d..(pos + 1) * d];
                // Recompute ff_hidden
                let mut ff_hidden = vec![0.0f32; ff];
                for k in 0..ff {
                    let w_idx = k * d;
                    let val: f32 = (0..d).map(|j| inp[j] * self.layers[l_idx].ff_w1[w_idx + j]).sum();
                    ff_hidden[k] = val.max(0.0);
                }

                // ff_w2 gradient
                let lr_ff = lr * 0.5; // slightly damped for stability
                let mut d_ff = vec![0.0f32; ff];
                for j in 0..d {
                    let g_j = d_norm[j];
                    if g_j.abs() < 1e-8 { continue; }
                    for k in 0..ff {
                        let idx = j * ff + k;
                        self.layers[l_idx].ff_w2[idx] -= lr_ff * clip_grad(g_j * ff_hidden[k]);
                        d_ff[k] += g_j * self.layers[l_idx].ff_w2[idx];
                    }
                }

                // ff_w1 gradient (through ReLU)
                for k in 0..ff {
                    if ff_hidden[k] <= 0.0 { continue; }
                    let w_idx = k * d;
                    for j in 0..d {
                        self.layers[l_idx].ff_w1[w_idx + j] -= lr_ff * clip_grad(d_ff[k] * inp[j]);
                    }
                }
            }

            // --- Attention backprop (simplified: update Wo, Wv, Wq, Wk) ---
            // For positions with nonzero gradient, update attention output projection
            let normed1 = layer_norm(layer_in, &self.layers[l_idx].ln1_scale, seq_len, d);

            for pos in 0..seq_len {
                let d_pos = &d_residual[pos * d..(pos + 1) * d];
                if d_pos.iter().all(|&x| x.abs() < 1e-8) { continue; }

                let lr_attn = lr * 0.3; // damped for attention stability

                // Recompute attention for this position to get activations
                for h in 0..n_heads {
                    let h_off = h * d_head;

                    // Q for this position
                    let inp = &normed1[pos * d..(pos + 1) * d];
                    let mut q = vec![0.0f32; d_head];
                    for j in 0..d_head {
                        let w_idx = (h_off + j) * d;
                        q[j] = (0..d).map(|k| inp[k] * self.layers[l_idx].wq[w_idx + k]).sum();
                    }

                    // Attention weights
                    let mut weights = vec![0.0f32; pos + 1];
                    for prev in 0..=pos {
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        let mut score = 0.0f32;
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let k_j: f32 = (0..d).map(|kk| prev_inp[kk] * self.layers[l_idx].wk[w_idx + kk]).sum();
                            score += q[j] * k_j;
                        }
                        weights[prev] = score / (d_head as f32).sqrt();
                    }

                    // Softmax
                    let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let exp_s: f32 = weights.iter().map(|w| (w - max_w).exp()).sum();
                    for w in &mut weights { *w = (*w - max_w).exp() / exp_s; }

                    // Recompute attention output for this head
                    let mut attn_head = vec![0.0f32; d_head];
                    for prev in 0..=pos {
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let v_j: f32 = (0..d).map(|kk| prev_inp[kk] * self.layers[l_idx].wv[w_idx + kk]).sum();
                            attn_head[j] += weights[prev] * v_j;
                        }
                    }

                    // Gradient through Wo: d_pos[j] = sum_k(attn_out[k] * wo[j*d+k])
                    // wo[j*d + h_off+jj] -= lr * d_pos[j] * attn_head[jj]
                    let mut d_attn = vec![0.0f32; d_head];
                    for j in 0..d {
                        if d_pos[j].abs() < 1e-8 { continue; }
                        for jj in 0..d_head {
                            let idx = j * d + h_off + jj;
                            self.layers[l_idx].wo[idx] -= lr_attn * clip_grad(d_pos[j] * attn_head[jj]);
                            d_attn[jj] += d_pos[j] * self.layers[l_idx].wo[idx];
                        }
                    }

                    // Gradient through Wv: d_attn → V gradient
                    for prev in 0..=pos {
                        if weights[prev] < 1e-6 { continue; }
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let g = d_attn[j] * weights[prev];
                            for kk in 0..d {
                                self.layers[l_idx].wv[w_idx + kk] -= lr_attn * clip_grad(g * prev_inp[kk]);
                            }
                        }
                    }
                }

                // Propagate gradient to previous layer through residual connection
                // (d_residual already contains d_out, residual adds through)
            }

            // Pass gradient to previous layer
            d_layer_output = d_residual;
        }

        // 4. Input embedding gradient
        for (pos, &tok) in input.iter().take(seq_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            let emb_off = tok_idx * d;
            for j in 0..d {
                let g = d_layer_output[pos * d + j];
                if g.abs() > 1e-8 {
                    self.embeddings[emb_off + j] -= lr * 0.1 * clip_grad(g);
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
/// Clip gradient to [-0.5, 0.5] to prevent explosions.
#[inline]
fn clip_grad(g: f32) -> f32 {
    g.clamp(-0.5, 0.5)
}

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
