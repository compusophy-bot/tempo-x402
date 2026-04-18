//! Pure-Rust transformer for plan step sequence prediction.
//!
//! Architecture: 2-layer causal transformer with multi-head attention.
//! ~260K parameters. No external ML framework — just matrix math.
//!
//! Input: [context_tokens..., BOS, step1, step2, ...] (max 32 tokens)
//! Output: probability distribution over VOCAB_SIZE for next token
//!
//! ```text
//! Token IDs → Embedding (64×128) → Pos Encoding → [Transformer Block ×2] → Linear → Softmax
//!                                                    ├─ Multi-Head Attention (4 heads)
//!                                                    └─ Feed-Forward (128→256→128)
//! ```

use serde::{Deserialize, Serialize};

use crate::vocab::{MAX_SEQ_LEN, VOCAB_SIZE};

// ── Architecture Constants ───────────────────────────────────────────

/// Embedding dimension.
/// Scaled from 128→256 to capture richer plan representations.
/// At 256d with vocab 128 and 4 layers, the model is ~4.5M params (18 MB).
/// This is still trivial for our 371 GB Railway instances.
pub const D_MODEL: usize = 256;
/// Number of attention heads.
pub const N_HEADS: usize = 8;
/// Head dimension (D_MODEL / N_HEADS).
pub const D_HEAD: usize = D_MODEL / N_HEADS;
/// Feed-forward hidden dimension.
pub const D_FF: usize = 512;
/// Number of transformer layers.
/// Scaled from 2→4 for deeper plan reasoning.
pub const N_LAYERS: usize = 4;

// ── Core Model ───────────────────────────────────────────────────────

/// A single attention head's parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionHead {
    /// Query projection: D_MODEL → D_HEAD
    pub wq: Vec<f32>, // D_MODEL × D_HEAD
    /// Key projection: D_MODEL → D_HEAD
    pub wk: Vec<f32>,
    /// Value projection: D_MODEL → D_HEAD
    pub wv: Vec<f32>,
}

/// Multi-head attention parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHeadAttention {
    pub heads: Vec<AttentionHead>,
    /// Output projection: (N_HEADS * D_HEAD) → D_MODEL
    pub wo: Vec<f32>,
}

/// Feed-forward network parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedForward {
    /// Up projection: D_MODEL → D_FF
    pub w1: Vec<f32>,
    pub b1: Vec<f32>,
    /// Down projection: D_FF → D_MODEL
    pub w2: Vec<f32>,
    pub b2: Vec<f32>,
}

/// A single transformer layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformerLayer {
    pub attention: MultiHeadAttention,
    pub ff: FeedForward,
    /// Layer norm parameters (simplified: just scale, no bias)
    pub ln1_scale: Vec<f32>,
    pub ln2_scale: Vec<f32>,
}

/// The complete plan transformer model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTransformer {
    /// Token embedding: VOCAB_SIZE × D_MODEL
    pub embedding: Vec<f32>,
    /// Positional encoding: MAX_SEQ_LEN × D_MODEL
    pub pos_encoding: Vec<f32>,
    /// Transformer layers
    pub layers: Vec<TransformerLayer>,
    /// Output projection: D_MODEL → VOCAB_SIZE (tied with embedding)
    pub output_proj: Vec<f32>,
    pub output_bias: Vec<f32>,
    /// Training metadata
    pub train_steps: u64,
    pub running_loss: f32,
}

impl Default for PlanTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanTransformer {
    /// Create a new randomly initialized model.
    pub fn new() -> Self {
        let mut seed: u64 = 42;

        let embedding = xavier_init(VOCAB_SIZE, D_MODEL, &mut seed);
        let pos_encoding = sinusoidal_pos_encoding();

        let layers: Vec<TransformerLayer> = (0..N_LAYERS)
            .map(|_| TransformerLayer {
                attention: MultiHeadAttention {
                    heads: (0..N_HEADS)
                        .map(|_| AttentionHead {
                            wq: xavier_init(D_MODEL, D_HEAD, &mut seed),
                            wk: xavier_init(D_MODEL, D_HEAD, &mut seed),
                            wv: xavier_init(D_MODEL, D_HEAD, &mut seed),
                        })
                        .collect(),
                    wo: xavier_init(N_HEADS * D_HEAD, D_MODEL, &mut seed),
                },
                ff: FeedForward {
                    w1: xavier_init(D_MODEL, D_FF, &mut seed),
                    b1: vec![0.0; D_FF],
                    w2: xavier_init(D_FF, D_MODEL, &mut seed),
                    b2: vec![0.0; D_MODEL],
                },
                ln1_scale: vec![1.0; D_MODEL],
                ln2_scale: vec![1.0; D_MODEL],
            })
            .collect();

        let output_proj = xavier_init(D_MODEL, VOCAB_SIZE, &mut seed);
        let output_bias = vec![0.0; VOCAB_SIZE];

        Self {
            embedding,
            pos_encoding,
            layers,
            output_proj,
            output_bias,
            train_steps: 0,
            running_loss: 0.0,
        }
    }

    /// Total parameter count.
    pub fn param_count(&self) -> usize {
        let embed = VOCAB_SIZE * D_MODEL; // embedding
        let pos = MAX_SEQ_LEN * D_MODEL; // positional encoding
        let per_head = 3 * D_MODEL * D_HEAD; // Q, K, V per head
        let attn_out = N_HEADS * D_HEAD * D_MODEL; // output projection
        let ff = D_MODEL * D_FF + D_FF + D_FF * D_MODEL + D_MODEL; // w1, b1, w2, b2
        let ln = 2 * D_MODEL; // two layer norms per layer
        let layer = N_HEADS * per_head + attn_out + ff + ln;
        let output = D_MODEL * VOCAB_SIZE + VOCAB_SIZE;
        embed + pos + N_LAYERS * layer + output
    }

    /// Forward pass: given input token IDs, return logits for next token.
    /// Returns logits array of size VOCAB_SIZE for the LAST position.
    pub fn forward(&self, tokens: &[u32]) -> Vec<f32> {
        let seq_len = tokens.len().min(MAX_SEQ_LEN);

        // Step 1: Token embedding + positional encoding
        let mut hidden: Vec<Vec<f32>> = (0..seq_len)
            .map(|i| {
                let token = tokens[i] as usize;
                let tok_start = token.min(VOCAB_SIZE - 1) * D_MODEL;
                let pos_start = i * D_MODEL;
                (0..D_MODEL)
                    .map(|j| self.embedding[tok_start + j] + self.pos_encoding[pos_start + j])
                    .collect()
            })
            .collect();

        // Step 2: Transformer layers
        for layer in &self.layers {
            // Multi-head attention with causal mask
            let attn_out = self.multi_head_attention(&hidden, &layer.attention, seq_len);
            // Residual + layer norm
            for i in 0..seq_len {
                for j in 0..D_MODEL {
                    hidden[i][j] += attn_out[i][j];
                }
                layer_norm_inplace(&mut hidden[i], &layer.ln1_scale);
            }

            // Feed-forward
            for i in 0..seq_len {
                let ff_out = feed_forward(&hidden[i], &layer.ff);
                for j in 0..D_MODEL {
                    hidden[i][j] += ff_out[j];
                }
                layer_norm_inplace(&mut hidden[i], &layer.ln2_scale);
            }
        }

        // Step 3: Output projection (last position only)
        let last = &hidden[seq_len - 1];
        let mut logits = vec![0.0f32; VOCAB_SIZE];
        for v in 0..VOCAB_SIZE {
            let mut sum = self.output_bias[v];
            for d in 0..D_MODEL {
                sum += last[d] * self.output_proj[d * VOCAB_SIZE + v];
            }
            logits[v] = sum;
        }

        logits
    }

    /// Softmax over logits → probability distribution.
    pub fn softmax(logits: &[f32]) -> Vec<f32> {
        let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = logits.iter().map(|&x| (x - max).exp()).collect();
        let sum: f32 = exps.iter().sum();
        exps.iter().map(|&e| e / sum).collect()
    }

    /// Multi-head attention with causal masking.
    fn multi_head_attention(
        &self,
        hidden: &[Vec<f32>],
        attn: &MultiHeadAttention,
        seq_len: usize,
    ) -> Vec<Vec<f32>> {
        let mut concat_heads: Vec<Vec<f32>> = vec![vec![0.0; N_HEADS * D_HEAD]; seq_len];

        for (h, head) in attn.heads.iter().enumerate() {
            // Project Q, K, V
            let queries: Vec<Vec<f32>> = (0..seq_len)
                .map(|i| mat_vec_mul(&head.wq, &hidden[i], D_MODEL, D_HEAD))
                .collect();
            let keys: Vec<Vec<f32>> = (0..seq_len)
                .map(|i| mat_vec_mul(&head.wk, &hidden[i], D_MODEL, D_HEAD))
                .collect();
            let values: Vec<Vec<f32>> = (0..seq_len)
                .map(|i| mat_vec_mul(&head.wv, &hidden[i], D_MODEL, D_HEAD))
                .collect();

            // Scaled dot-product attention with causal mask
            let scale = (D_HEAD as f32).sqrt();
            for i in 0..seq_len {
                // Compute attention scores: Q_i · K_j / sqrt(d_k)
                let mut scores: Vec<f32> = (0..seq_len)
                    .map(|j| {
                        if j > i {
                            f32::NEG_INFINITY // Causal mask
                        } else {
                            dot(&queries[i], &keys[j]) / scale
                        }
                    })
                    .collect();

                // Softmax
                let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let exps: Vec<f32> = scores.iter().map(|&s| (s - max).exp()).collect();
                let sum: f32 = exps.iter().sum();
                scores = exps.iter().map(|&e| e / sum.max(1e-10)).collect();

                // Weighted sum of values
                for j in 0..seq_len {
                    for d in 0..D_HEAD {
                        concat_heads[i][h * D_HEAD + d] += scores[j] * values[j][d];
                    }
                }
            }
        }

        // Output projection
        (0..seq_len)
            .map(|i| mat_vec_mul(&attn.wo, &concat_heads[i], N_HEADS * D_HEAD, D_MODEL))
            .collect()
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    /// Collect all weight parameters into a flat vector.
    pub fn flatten_weights(&self) -> Vec<f32> {
        let mut w = Vec::with_capacity(self.param_count());
        w.extend_from_slice(&self.embedding);
        // Skip pos_encoding — it's fixed (sinusoidal), not learned
        for layer in &self.layers {
            for head in &layer.attention.heads {
                w.extend_from_slice(&head.wq);
                w.extend_from_slice(&head.wk);
                w.extend_from_slice(&head.wv);
            }
            w.extend_from_slice(&layer.attention.wo);
            w.extend_from_slice(&layer.ff.w1);
            w.extend_from_slice(&layer.ff.b1);
            w.extend_from_slice(&layer.ff.w2);
            w.extend_from_slice(&layer.ff.b2);
            w.extend_from_slice(&layer.ln1_scale);
            w.extend_from_slice(&layer.ln2_scale);
        }
        w.extend_from_slice(&self.output_proj);
        w.extend_from_slice(&self.output_bias);
        w
    }

    /// Apply a flat weight vector back into the model (inverse of flatten_weights).
    fn unflatten_weights(&mut self, w: &[f32]) {
        let mut offset = 0;
        let copy = |dst: &mut [f32], src: &[f32], off: &mut usize| {
            dst.copy_from_slice(&src[*off..*off + dst.len()]);
            *off += dst.len();
        };
        copy(&mut self.embedding, w, &mut offset);
        for layer in &mut self.layers {
            for head in &mut layer.attention.heads {
                copy(&mut head.wq, w, &mut offset);
                copy(&mut head.wk, w, &mut offset);
                copy(&mut head.wv, w, &mut offset);
            }
            copy(&mut layer.attention.wo, w, &mut offset);
            copy(&mut layer.ff.w1, w, &mut offset);
            copy(&mut layer.ff.b1, w, &mut offset);
            copy(&mut layer.ff.w2, w, &mut offset);
            copy(&mut layer.ff.b2, w, &mut offset);
            copy(&mut layer.ln1_scale, w, &mut offset);
            copy(&mut layer.ln2_scale, w, &mut offset);
        }
        copy(&mut self.output_proj, w, &mut offset);
        copy(&mut self.output_bias, w, &mut offset);
    }

    /// Compute weight delta between this model and a snapshot (for sharing).
    pub fn compute_delta(&self, snapshot: &PlanTransformer, source_id: &str) -> TransformerDelta {
        let self_w = self.flatten_weights();
        let snap_w = snapshot.flatten_weights();
        let delta: Vec<f32> = self_w
            .iter()
            .zip(snap_w.iter())
            .map(|(a, b)| a - b)
            .collect();
        TransformerDelta {
            weights: delta,
            train_steps: self.train_steps,
            source_id: source_id.to_string(),
        }
    }

    /// Merge a weight delta from a peer (federated averaging).
    pub fn merge_delta(&mut self, delta: &TransformerDelta, merge_rate: f32) {
        let mut w = self.flatten_weights();
        if w.len() != delta.weights.len() {
            return; // Incompatible architecture
        }
        for (wi, di) in w.iter_mut().zip(delta.weights.iter()) {
            *wi += di * merge_rate;
        }
        self.unflatten_weights(&w);
    }
}

/// Weight delta for federated transformer sharing between colony peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformerDelta {
    pub weights: Vec<f32>,
    pub train_steps: u64,
    pub source_id: String,
}

// ── Math Helpers ─────────────────────────────────────────────────────

/// Matrix-vector multiply: (rows × cols) × (cols,) → (rows,)
fn mat_vec_mul(mat: &[f32], vec: &[f32], cols: usize, rows: usize) -> Vec<f32> {
    (0..rows)
        .map(|r| (0..cols).map(|c| mat[c * rows + r] * vec[c]).sum::<f32>())
        .collect()
}

/// Dot product.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Feed-forward: ReLU(x·W1 + b1)·W2 + b2
fn feed_forward(x: &[f32], ff: &FeedForward) -> Vec<f32> {
    // Up projection + ReLU
    let h: Vec<f32> = (0..D_FF)
        .map(|i| {
            let sum: f32 = (0..D_MODEL)
                .map(|j| x[j] * ff.w1[j * D_FF + i])
                .sum::<f32>()
                + ff.b1[i];
            sum.max(0.0) // ReLU
        })
        .collect();

    // Down projection
    (0..D_MODEL)
        .map(|i| {
            (0..D_FF)
                .map(|j| h[j] * ff.w2[j * D_MODEL + i])
                .sum::<f32>()
                + ff.b2[i]
        })
        .collect()
}

/// Simplified layer norm (scale only, no bias).
fn layer_norm_inplace(x: &mut [f32], scale: &[f32]) {
    let mean: f32 = x.iter().sum::<f32>() / x.len() as f32;
    let var: f32 = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / x.len() as f32;
    let std = (var + 1e-5).sqrt();
    for (i, v) in x.iter_mut().enumerate() {
        *v = (*v - mean) / std * scale[i];
    }
}

/// Xavier initialization using LCG PRNG (deterministic, no external rng).
fn xavier_init(fan_in: usize, fan_out: usize, seed: &mut u64) -> Vec<f32> {
    let limit = (6.0 / (fan_in + fan_out) as f64).sqrt() as f32;
    let n = fan_in * fan_out;
    (0..n)
        .map(|_| {
            *seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = (*seed >> 33) as f32 / (1u64 << 31) as f32; // 0..1
            (u * 2.0 - 1.0) * limit
        })
        .collect()
}

/// Sinusoidal positional encoding (fixed, not learned).
fn sinusoidal_pos_encoding() -> Vec<f32> {
    let mut pe = vec![0.0f32; MAX_SEQ_LEN * D_MODEL];
    for pos in 0..MAX_SEQ_LEN {
        for i in 0..D_MODEL / 2 {
            let angle = pos as f32 / (10000.0f32).powf(2.0 * i as f32 / D_MODEL as f32);
            pe[pos * D_MODEL + 2 * i] = angle.sin();
            pe[pos * D_MODEL + 2 * i + 1] = angle.cos();
        }
    }
    pe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_model() {
        let model = PlanTransformer::new();
        let params = model.param_count();
        println!("Model parameters: {}", params);
        assert!(params > 1_000_000, "Should have >1M params, got {}", params);
        assert!(
            params < 10_000_000,
            "Should have <10M params, got {}",
            params
        );
    }

    #[test]
    fn test_forward_pass() {
        let model = PlanTransformer::new();
        let tokens = vec![1, 4, 15, 13]; // BOS, read_file, edit_code, cargo_check
        let logits = model.forward(&tokens);
        assert_eq!(logits.len(), VOCAB_SIZE);
        // Logits should be finite
        assert!(logits.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_softmax() {
        let logits = vec![1.0, 2.0, 3.0];
        let probs = PlanTransformer::softmax(&logits);
        assert!((probs.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert!(probs[2] > probs[1]);
        assert!(probs[1] > probs[0]);
    }

    #[test]
    fn test_serialization() {
        let model = PlanTransformer::new();
        let json = model.to_json();
        assert!(!json.is_empty());
        let restored = PlanTransformer::from_json(&json).unwrap();
        assert_eq!(restored.param_count(), model.param_count());
        assert_eq!(restored.train_steps, 0);
    }
}
