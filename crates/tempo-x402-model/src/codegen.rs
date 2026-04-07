//! Local code generation model — Phase 3.
//!
//! ~63M param Rust-specialist encoder-decoder transformer. Trained from
//! benchmark solutions + commit diffs. Encoder processes test code with
//! bidirectional attention; decoder generates solution code conditioned
//! on the encoder output via cross-attention.
//!
//! 5 encoder layers (bidirectional) + 5 decoder layers (causal + cross-attn).

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

// ── Phase 3 Model — encoder-decoder transformer ────────────────────
//
// 8GB available. Encoder-decoder with shared embeddings.
//
// Param count: embeddings (8192×640=5.2M) + enc_pos (512×640=0.3M)
//   + dec_pos (512×640=0.3M)
//   + 5 encoder layers × (4×640×640 + 2×640×2560 + 2×640) = 5×(1.6M+3.3M+1.3K) ≈ 24.5M
//   + 5 decoder layers × (8×640×640 + 2×640×2560 + 3×640) = 5×(3.3M+3.3M+1.9K) ≈ 33M
//   + bias = ~63M params × 4 bytes = ~252MB RAM

/// Model constants — scaled for 8GB RAM.
pub const SMALL_D_MODEL: usize = 640;
pub const SMALL_N_HEADS: usize = 10;
pub const SMALL_D_HEAD: usize = SMALL_D_MODEL / SMALL_N_HEADS; // 64
pub const SMALL_N_LAYERS: usize = 10;
pub const SMALL_D_FF: usize = 2560;
pub const SMALL_MAX_SEQ: usize = 512;
pub const SMALL_VOCAB: usize = 8192;

/// Encoder/decoder layer split.
pub const ENC_LAYERS: usize = 5;
pub const DEC_LAYERS: usize = 5;

/// Code generation model — encoder-decoder transformer.
///
/// 5 encoder layers (bidirectional self-attention) process context (test code).
/// 5 decoder layers (causal self-attention + cross-attention) generate solution
/// code conditioned on the encoded context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeGenModel {
    /// Token embeddings: VOCAB × D_MODEL (shared encoder+decoder)
    pub embeddings: Vec<f32>,
    /// Encoder positional encodings: MAX_SEQ × D_MODEL
    pub enc_pos: Vec<f32>,
    /// Decoder positional encodings: MAX_SEQ × D_MODEL
    pub dec_pos: Vec<f32>,
    /// Encoder layers (bidirectional self-attention)
    pub encoder_layers: Vec<CodeGenLayer>,
    /// Decoder layers (causal self-attention + cross-attention)
    pub decoder_layers: Vec<DecoderLayer>,
    /// Output projection bias: VOCAB
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

/// Encoder layer — bidirectional self-attention + FFN.
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

/// Decoder layer — causal self-attention + cross-attention + FFN.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecoderLayer {
    /// Causal self-attention: Q, K, V projections + output
    pub wq: Vec<f32>, // D_MODEL × D_MODEL
    pub wk: Vec<f32>,
    pub wv: Vec<f32>,
    pub wo: Vec<f32>,
    /// Cross-attention: decoder Q, encoder K/V
    pub cross_wq: Vec<f32>, // D_MODEL × D_MODEL
    pub cross_wk: Vec<f32>,
    pub cross_wv: Vec<f32>,
    pub cross_wo: Vec<f32>,
    /// Feed-forward: D_MODEL → D_FF → D_MODEL
    pub ff_w1: Vec<f32>,
    pub ff_w2: Vec<f32>,
    /// Layer norms: self-attn, cross-attn, FFN
    pub ln1_scale: Vec<f32>,
    pub ln2_scale: Vec<f32>,
    pub ln3_scale: Vec<f32>,
}

impl CodeGenModel {
    /// Create a new model with Xavier initialization.
    pub fn new() -> Self {
        let d = SMALL_D_MODEL;
        let v = SMALL_VOCAB;
        let s = SMALL_MAX_SEQ;
        let ff = SMALL_D_FF;

        let mut rng = XorShift64(42);

        let embeddings = xavier_init(&mut rng, v * d, v, d);
        let enc_pos = xavier_init(&mut rng, s * d, s, d);
        let dec_pos = xavier_init(&mut rng, s * d, s, d);
        let output_bias = vec![0.0; v];

        let encoder_layers = (0..ENC_LAYERS)
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

        let decoder_layers = (0..DEC_LAYERS)
            .map(|_| DecoderLayer {
                wq: xavier_init(&mut rng, d * d, d, d),
                wk: xavier_init(&mut rng, d * d, d, d),
                wv: xavier_init(&mut rng, d * d, d, d),
                wo: xavier_init(&mut rng, d * d, d, d),
                cross_wq: xavier_init(&mut rng, d * d, d, d),
                cross_wk: xavier_init(&mut rng, d * d, d, d),
                cross_wv: xavier_init(&mut rng, d * d, d, d),
                cross_wo: xavier_init(&mut rng, d * d, d, d),
                ff_w1: xavier_init(&mut rng, d * ff, d, ff),
                ff_w2: xavier_init(&mut rng, ff * d, ff, d),
                ln1_scale: vec![1.0; d],
                ln2_scale: vec![1.0; d],
                ln3_scale: vec![1.0; d],
            })
            .collect();

        Self {
            embeddings,
            enc_pos,
            dec_pos,
            encoder_layers,
            decoder_layers,
            output_bias,
            train_steps: 0,
            running_loss: 0.0,
            d_model: d,
            n_layers: ENC_LAYERS + DEC_LAYERS,
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

        let embed = v * d + 2 * s * d; // shared embeddings + enc_pos + dec_pos
        let enc_per_layer = 4 * d * d + 2 * d * ff + 2 * d; // attn + ff + 2 ln
        let dec_per_layer = 8 * d * d + 2 * d * ff + 3 * d; // self-attn + cross-attn + ff + 3 ln
        embed + ENC_LAYERS * enc_per_layer + DEC_LAYERS * dec_per_layer + v // output bias
    }

    /// Encode context tokens (test code) with bidirectional attention.
    /// Returns flattened [seq_len × d_model].
    pub fn encode(&self, context: &[u32]) -> Vec<f32> {
        let d = self.d_model;
        let seq_len = context.len().min(self.max_seq);

        if seq_len == 0 {
            return vec![];
        }

        // Embed + encoder positional encoding
        let mut hidden = vec![0.0f32; seq_len * d];
        for (pos, &tok) in context.iter().take(seq_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            for j in 0..d {
                hidden[pos * d + j] =
                    self.embeddings[tok_idx * d + j] + self.enc_pos[pos * d + j];
            }
        }

        // Encoder layers — bidirectional self-attention
        for layer in &self.encoder_layers {
            hidden = self.apply_encoder_layer(layer, &hidden, seq_len);
        }

        hidden
    }

    /// Decode target tokens conditioned on encoder output.
    /// Returns logits of shape [vocab_size] for the LAST token position.
    pub fn decode(&self, target: &[u32], encoder_output: &[f32], enc_len: usize) -> Vec<f32> {
        let d = self.d_model;
        let seq_len = target.len().min(self.max_seq);

        if seq_len == 0 {
            return vec![0.0; self.vocab_size];
        }

        // Embed + decoder positional encoding
        let mut hidden = vec![0.0f32; seq_len * d];
        for (pos, &tok) in target.iter().take(seq_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            for j in 0..d {
                hidden[pos * d + j] =
                    self.embeddings[tok_idx * d + j] + self.dec_pos[pos * d + j];
            }
        }

        // Decoder layers — causal self-attention + cross-attention
        for layer in &self.decoder_layers {
            hidden = self.apply_decoder_layer(layer, &hidden, seq_len, encoder_output, enc_len);
        }

        // Output projection (last position): hidden[last] × embeddings^T + bias
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

    /// Legacy forward — encode+decode on same tokens (backward compat).
    /// Returns logits of shape [vocab_size] for the LAST token position.
    pub fn forward(&self, tokens: &[u32]) -> Vec<f32> {
        let seq_len = tokens.len().min(self.max_seq);
        if seq_len == 0 {
            return vec![0.0; self.vocab_size];
        }
        let enc = self.encode(tokens);
        self.decode(tokens, &enc, seq_len)
    }

    /// Apply a single encoder layer (bidirectional self-attention + FFN).
    fn apply_encoder_layer(&self, layer: &CodeGenLayer, input: &[f32], seq_len: usize) -> Vec<f32> {
        let d = self.d_model;
        let n_heads = SMALL_N_HEADS;
        let d_head = SMALL_D_HEAD;
        let ff = SMALL_D_FF;

        // Layer norm 1
        let normed = layer_norm(input, &layer.ln1_scale, seq_len, d);

        // Multi-head BIDIRECTIONAL attention — parallel across heads
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

                    // Bidirectional: attend to ALL positions
                    let mut weights = vec![0.0f32; seq_len];
                    for prev in 0..seq_len {
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

                    for prev in 0..seq_len {
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

        // Feed-forward — parallel across positions
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

    /// Apply a single decoder layer (causal self-attention + cross-attention + FFN).
    fn apply_decoder_layer(
        &self,
        layer: &DecoderLayer,
        input: &[f32],
        seq_len: usize,
        encoder_output: &[f32],
        enc_len: usize,
    ) -> Vec<f32> {
        let d = self.d_model;
        let n_heads = SMALL_N_HEADS;
        let d_head = SMALL_D_HEAD;
        let ff = SMALL_D_FF;

        // === 1. Causal self-attention ===

        // Layer norm 1
        let normed = layer_norm(input, &layer.ln1_scale, seq_len, d);

        // Multi-head CAUSAL attention — parallel across heads
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

                    // Causal: only attend to positions <= pos
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

        // Merge head outputs
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

        // === 2. Cross-attention (decoder Q, encoder K/V) ===

        // Layer norm 2 (cross-attention)
        let normed2 = layer_norm(&residual, &layer.ln2_scale, seq_len, d);

        if enc_len > 0 && !encoder_output.is_empty() {
            let cross_head_outputs: Vec<Vec<f32>> = (0..n_heads)
                .into_par_iter()
                .map(|h| {
                    let mut head_out = vec![0.0f32; seq_len * d_head];
                    for pos in 0..seq_len {
                        let inp = &normed2[pos * d..(pos + 1) * d];

                        // Q from decoder hidden
                        let mut q = vec![0.0f32; d_head];
                        for j in 0..d_head {
                            let w_idx = (h * d_head + j) * d;
                            q[j] = (0..d).map(|k| inp[k] * layer.cross_wq[w_idx + k]).sum::<f32>();
                        }

                        // K, V from encoder output — attend to ALL encoder positions
                        let mut weights = vec![0.0f32; enc_len];
                        for enc_pos in 0..enc_len {
                            let enc_inp = &encoder_output[enc_pos * d..(enc_pos + 1) * d];
                            let mut score = 0.0f32;
                            for j in 0..d_head {
                                let w_idx = (h * d_head + j) * d;
                                let k_j: f32 = (0..d).map(|kk| enc_inp[kk] * layer.cross_wk[w_idx + kk]).sum();
                                score += q[j] * k_j;
                            }
                            weights[enc_pos] = score / (d_head as f32).sqrt();
                        }

                        let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                        let exp_sum: f32 = weights.iter().map(|w| (w - max_w).exp()).sum();
                        for w in &mut weights { *w = (*w - max_w).exp() / exp_sum; }

                        for enc_pos in 0..enc_len {
                            let enc_inp = &encoder_output[enc_pos * d..(enc_pos + 1) * d];
                            for j in 0..d_head {
                                let w_idx = (h * d_head + j) * d;
                                let v_j: f32 = (0..d).map(|kk| enc_inp[kk] * layer.cross_wv[w_idx + kk]).sum();
                                head_out[pos * d_head + j] += weights[enc_pos] * v_j;
                            }
                        }
                    }
                    head_out
                })
                .collect();

            // Merge cross-attention head outputs
            let mut cross_attn_out = vec![0.0f32; seq_len * d];
            for (h, head_out) in cross_head_outputs.iter().enumerate() {
                for pos in 0..seq_len {
                    for j in 0..d_head {
                        cross_attn_out[pos * d + h * d_head + j] = head_out[pos * d_head + j];
                    }
                }
            }

            // Cross-attention output projection + residual
            for pos in 0..seq_len {
                let a = &cross_attn_out[pos * d..(pos + 1) * d];
                for j in 0..d {
                    let out_j: f32 = (0..d).map(|k| a[k] * layer.cross_wo[j * d + k]).sum();
                    residual[pos * d + j] += out_j;
                }
            }
        }

        // === 3. Feed-forward ===

        // Layer norm 3
        let normed3 = layer_norm(&residual, &layer.ln3_scale, seq_len, d);

        // FFN — parallel across positions
        let ff_deltas: Vec<Vec<f32>> = (0..seq_len)
            .into_par_iter()
            .map(|pos| {
                let inp = &normed3[pos * d..(pos + 1) * d];
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

    /// Train on a (context, target) pair. Context = test code, target = solution code.
    /// This is the primary training method for the encoder-decoder.
    /// Returns loss (cross-entropy on last target token).
    pub fn train_enc_dec(&mut self, context: &[u32], target: &[u32], learning_rate: f32) -> f32 {
        if target.len() < 2 {
            return 0.0;
        }

        let d = self.d_model;
        let n_heads = SMALL_N_HEADS;
        let d_head = SMALL_D_HEAD;
        let ff = SMALL_D_FF;
        let dec_input = &target[..target.len() - 1];
        let dec_target = target[target.len() - 1];
        let enc_len = context.len().min(self.max_seq);
        let dec_len = dec_input.len().min(self.max_seq);

        // === ENCODER FORWARD — save activations ===

        let mut enc_hidden = vec![0.0f32; enc_len * d];
        for (pos, &tok) in context.iter().take(enc_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            for j in 0..d {
                enc_hidden[pos * d + j] =
                    self.embeddings[tok_idx * d + j] + self.enc_pos[pos * d + j];
            }
        }

        let mut enc_layer_inputs: Vec<Vec<f32>> = Vec::with_capacity(ENC_LAYERS + 1);
        enc_layer_inputs.push(enc_hidden.clone());

        for layer in &self.encoder_layers {
            enc_hidden = self.apply_encoder_layer(layer, &enc_hidden, enc_len);
            enc_layer_inputs.push(enc_hidden.clone());
        }
        let encoder_output = enc_hidden;

        // === DECODER FORWARD — save activations ===

        let mut dec_hidden = vec![0.0f32; dec_len * d];
        for (pos, &tok) in dec_input.iter().take(dec_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            for j in 0..d {
                dec_hidden[pos * d + j] =
                    self.embeddings[tok_idx * d + j] + self.dec_pos[pos * d + j];
            }
        }

        let mut dec_layer_inputs: Vec<Vec<f32>> = Vec::with_capacity(DEC_LAYERS + 1);
        dec_layer_inputs.push(dec_hidden.clone());

        for layer in &self.decoder_layers {
            dec_hidden = self.apply_decoder_layer(layer, &dec_hidden, dec_len, &encoder_output, enc_len);
            dec_layer_inputs.push(dec_hidden.clone());
        }

        // === OUTPUT PROJECTION ===

        let last_pos = dec_len - 1;
        let last_hidden = &dec_hidden[last_pos * d..(last_pos + 1) * d];
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
        let target_idx = dec_target as usize % self.vocab_size;
        let loss = log_sum_exp - logits[target_idx];

        let softmax: Vec<f32> = logits.iter().map(|l| (l - log_sum_exp).exp()).collect();
        let mut d_logits = softmax;
        d_logits[target_idx] -= 1.0;

        // === BACKPROP ===

        let lr = learning_rate;

        // 1. Output bias
        self.output_bias.par_iter_mut().zip(d_logits.par_iter()).for_each(|(b, &g)| {
            *b -= lr * clip_grad(g);
        });

        // 2. Embedding gradient from output projection
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

        // Update embeddings from output projection
        for v_idx in 0..self.vocab_size {
            if d_logits[v_idx].abs() < 1e-7 { continue; }
            let g = d_logits[v_idx];
            let emb_off = v_idx * d;
            for j in 0..d {
                self.embeddings[emb_off + j] -= lr * clip_grad(g * last_hidden[j]);
            }
        }

        let mut d_dec_output = vec![0.0f32; dec_len * d];
        for j in 0..d {
            d_dec_output[last_pos * d + j] = d_hidden_last[j];
        }

        // 3. Backprop through decoder layers in REVERSE order
        let mut d_encoder = vec![0.0f32; enc_len * d]; // accumulate gradients for encoder

        for l_idx in (0..DEC_LAYERS).rev() {
            let layer_in = &dec_layer_inputs[l_idx];
            let d_out = &d_dec_output;

            // --- FFN backprop ---
            let normed3 = layer_norm(layer_in, &self.decoder_layers[l_idx].ln3_scale, dec_len, d);

            let d_residual = d_out.clone();

            for pos in 0..dec_len {
                let d_norm = &d_out[pos * d..(pos + 1) * d];
                if d_norm.iter().all(|&x| x.abs() < 1e-8) { continue; }

                let inp = &normed3[pos * d..(pos + 1) * d];
                let mut ff_hidden = vec![0.0f32; ff];
                for k in 0..ff {
                    let w_idx = k * d;
                    let val: f32 = (0..d).map(|j| inp[j] * self.decoder_layers[l_idx].ff_w1[w_idx + j]).sum();
                    ff_hidden[k] = val.max(0.0);
                }

                let lr_ff = lr * 0.5;
                let mut d_ff = vec![0.0f32; ff];
                for j in 0..d {
                    let g_j = d_norm[j];
                    if g_j.abs() < 1e-8 { continue; }
                    for k in 0..ff {
                        let idx = j * ff + k;
                        self.decoder_layers[l_idx].ff_w2[idx] -= lr_ff * clip_grad(g_j * ff_hidden[k]);
                        d_ff[k] += g_j * self.decoder_layers[l_idx].ff_w2[idx];
                    }
                }

                for k in 0..ff {
                    if ff_hidden[k] <= 0.0 { continue; }
                    let w_idx = k * d;
                    for j in 0..d {
                        self.decoder_layers[l_idx].ff_w1[w_idx + j] -= lr_ff * clip_grad(d_ff[k] * inp[j]);
                    }
                }
            }

            // --- Cross-attention backprop ---
            if enc_len > 0 {
                let normed2 = layer_norm(layer_in, &self.decoder_layers[l_idx].ln2_scale, dec_len, d);

                for pos in 0..dec_len {
                    let d_pos = &d_residual[pos * d..(pos + 1) * d];
                    if d_pos.iter().all(|&x| x.abs() < 1e-8) { continue; }

                    let lr_cross = lr * 0.3;

                    for h in 0..n_heads {
                        let h_off = h * d_head;
                        let inp = &normed2[pos * d..(pos + 1) * d];

                        // Recompute cross-attention Q
                        let mut q = vec![0.0f32; d_head];
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            q[j] = (0..d).map(|k| inp[k] * self.decoder_layers[l_idx].cross_wq[w_idx + k]).sum();
                        }

                        // Cross-attention weights (attend to all encoder positions)
                        let mut weights = vec![0.0f32; enc_len];
                        for enc_pos in 0..enc_len {
                            let enc_inp = &encoder_output[enc_pos * d..(enc_pos + 1) * d];
                            let mut score = 0.0f32;
                            for j in 0..d_head {
                                let w_idx = (h_off + j) * d;
                                let k_j: f32 = (0..d).map(|kk| enc_inp[kk] * self.decoder_layers[l_idx].cross_wk[w_idx + kk]).sum();
                                score += q[j] * k_j;
                            }
                            weights[enc_pos] = score / (d_head as f32).sqrt();
                        }

                        let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                        let exp_s: f32 = weights.iter().map(|w| (w - max_w).exp()).sum();
                        for w in &mut weights { *w = (*w - max_w).exp() / exp_s; }

                        // Recompute cross-attention output for this head
                        let mut cross_head = vec![0.0f32; d_head];
                        for enc_pos in 0..enc_len {
                            let enc_inp = &encoder_output[enc_pos * d..(enc_pos + 1) * d];
                            for j in 0..d_head {
                                let w_idx = (h_off + j) * d;
                                let v_j: f32 = (0..d).map(|kk| enc_inp[kk] * self.decoder_layers[l_idx].cross_wv[w_idx + kk]).sum();
                                cross_head[j] += weights[enc_pos] * v_j;
                            }
                        }

                        // Gradient through cross_wo
                        let mut d_cross = vec![0.0f32; d_head];
                        for j in 0..d {
                            if d_pos[j].abs() < 1e-8 { continue; }
                            for jj in 0..d_head {
                                let idx = j * d + h_off + jj;
                                self.decoder_layers[l_idx].cross_wo[idx] -= lr_cross * clip_grad(d_pos[j] * cross_head[jj]);
                                d_cross[jj] += d_pos[j] * self.decoder_layers[l_idx].cross_wo[idx];
                            }
                        }

                        // Gradient through cross_wv → also propagates to encoder output
                        for enc_pos in 0..enc_len {
                            if weights[enc_pos] < 1e-6 { continue; }
                            let enc_inp = &encoder_output[enc_pos * d..(enc_pos + 1) * d];
                            for j in 0..d_head {
                                let w_idx = (h_off + j) * d;
                                let g = d_cross[j] * weights[enc_pos];
                                for kk in 0..d {
                                    self.decoder_layers[l_idx].cross_wv[w_idx + kk] -= lr_cross * clip_grad(g * enc_inp[kk]);
                                    // Propagate gradient to encoder output
                                    d_encoder[enc_pos * d + kk] += lr_cross * clip_grad(g * self.decoder_layers[l_idx].cross_wv[w_idx + kk]);
                                }
                            }
                        }
                    }
                }
            }

            // --- Causal self-attention backprop ---
            let normed1 = layer_norm(layer_in, &self.decoder_layers[l_idx].ln1_scale, dec_len, d);

            for pos in 0..dec_len {
                let d_pos = &d_residual[pos * d..(pos + 1) * d];
                if d_pos.iter().all(|&x| x.abs() < 1e-8) { continue; }

                let lr_attn = lr * 0.3;

                for h in 0..n_heads {
                    let h_off = h * d_head;
                    let inp = &normed1[pos * d..(pos + 1) * d];

                    let mut q = vec![0.0f32; d_head];
                    for j in 0..d_head {
                        let w_idx = (h_off + j) * d;
                        q[j] = (0..d).map(|k| inp[k] * self.decoder_layers[l_idx].wq[w_idx + k]).sum();
                    }

                    let mut weights = vec![0.0f32; pos + 1];
                    for prev in 0..=pos {
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        let mut score = 0.0f32;
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let k_j: f32 = (0..d).map(|kk| prev_inp[kk] * self.decoder_layers[l_idx].wk[w_idx + kk]).sum();
                            score += q[j] * k_j;
                        }
                        weights[prev] = score / (d_head as f32).sqrt();
                    }

                    let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let exp_s: f32 = weights.iter().map(|w| (w - max_w).exp()).sum();
                    for w in &mut weights { *w = (*w - max_w).exp() / exp_s; }

                    let mut attn_head = vec![0.0f32; d_head];
                    for prev in 0..=pos {
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let v_j: f32 = (0..d).map(|kk| prev_inp[kk] * self.decoder_layers[l_idx].wv[w_idx + kk]).sum();
                            attn_head[j] += weights[prev] * v_j;
                        }
                    }

                    let mut d_attn = vec![0.0f32; d_head];
                    for j in 0..d {
                        if d_pos[j].abs() < 1e-8 { continue; }
                        for jj in 0..d_head {
                            let idx = j * d + h_off + jj;
                            self.decoder_layers[l_idx].wo[idx] -= lr_attn * clip_grad(d_pos[j] * attn_head[jj]);
                            d_attn[jj] += d_pos[j] * self.decoder_layers[l_idx].wo[idx];
                        }
                    }

                    for prev in 0..=pos {
                        if weights[prev] < 1e-6 { continue; }
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let g = d_attn[j] * weights[prev];
                            for kk in 0..d {
                                self.decoder_layers[l_idx].wv[w_idx + kk] -= lr_attn * clip_grad(g * prev_inp[kk]);
                            }
                        }
                    }
                }
            }

            // Pass gradient to previous decoder layer
            d_dec_output = d_residual;
        }

        // 4. Backprop through encoder layers (via cross-attention gradients)
        let mut d_enc_output = d_encoder;

        for l_idx in (0..ENC_LAYERS).rev() {
            let layer_in = &enc_layer_inputs[l_idx];
            let d_out = &d_enc_output;

            // --- FFN backprop ---
            let normed2 = layer_norm(layer_in, &self.encoder_layers[l_idx].ln2_scale, enc_len, d);
            let d_residual = d_out.clone();

            for pos in 0..enc_len {
                let d_norm = &d_out[pos * d..(pos + 1) * d];
                if d_norm.iter().all(|&x| x.abs() < 1e-8) { continue; }

                let inp = &normed2[pos * d..(pos + 1) * d];
                let mut ff_hidden = vec![0.0f32; ff];
                for k in 0..ff {
                    let w_idx = k * d;
                    let val: f32 = (0..d).map(|j| inp[j] * self.encoder_layers[l_idx].ff_w1[w_idx + j]).sum();
                    ff_hidden[k] = val.max(0.0);
                }

                let lr_ff = lr * 0.5;
                let mut d_ff = vec![0.0f32; ff];
                for j in 0..d {
                    let g_j = d_norm[j];
                    if g_j.abs() < 1e-8 { continue; }
                    for k in 0..ff {
                        let idx = j * ff + k;
                        self.encoder_layers[l_idx].ff_w2[idx] -= lr_ff * clip_grad(g_j * ff_hidden[k]);
                        d_ff[k] += g_j * self.encoder_layers[l_idx].ff_w2[idx];
                    }
                }

                for k in 0..ff {
                    if ff_hidden[k] <= 0.0 { continue; }
                    let w_idx = k * d;
                    for j in 0..d {
                        self.encoder_layers[l_idx].ff_w1[w_idx + j] -= lr_ff * clip_grad(d_ff[k] * inp[j]);
                    }
                }
            }

            // --- Bidirectional attention backprop ---
            let normed1 = layer_norm(layer_in, &self.encoder_layers[l_idx].ln1_scale, enc_len, d);

            for pos in 0..enc_len {
                let d_pos = &d_residual[pos * d..(pos + 1) * d];
                if d_pos.iter().all(|&x| x.abs() < 1e-8) { continue; }

                let lr_attn = lr * 0.3;

                for h in 0..n_heads {
                    let h_off = h * d_head;
                    let inp = &normed1[pos * d..(pos + 1) * d];

                    let mut q = vec![0.0f32; d_head];
                    for j in 0..d_head {
                        let w_idx = (h_off + j) * d;
                        q[j] = (0..d).map(|k| inp[k] * self.encoder_layers[l_idx].wq[w_idx + k]).sum();
                    }

                    // Bidirectional: attend to ALL positions
                    let mut weights = vec![0.0f32; enc_len];
                    for prev in 0..enc_len {
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        let mut score = 0.0f32;
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let k_j: f32 = (0..d).map(|kk| prev_inp[kk] * self.encoder_layers[l_idx].wk[w_idx + kk]).sum();
                            score += q[j] * k_j;
                        }
                        weights[prev] = score / (d_head as f32).sqrt();
                    }

                    let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let exp_s: f32 = weights.iter().map(|w| (w - max_w).exp()).sum();
                    for w in &mut weights { *w = (*w - max_w).exp() / exp_s; }

                    let mut attn_head = vec![0.0f32; d_head];
                    for prev in 0..enc_len {
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let v_j: f32 = (0..d).map(|kk| prev_inp[kk] * self.encoder_layers[l_idx].wv[w_idx + kk]).sum();
                            attn_head[j] += weights[prev] * v_j;
                        }
                    }

                    let mut d_attn = vec![0.0f32; d_head];
                    for j in 0..d {
                        if d_pos[j].abs() < 1e-8 { continue; }
                        for jj in 0..d_head {
                            let idx = j * d + h_off + jj;
                            self.encoder_layers[l_idx].wo[idx] -= lr_attn * clip_grad(d_pos[j] * attn_head[jj]);
                            d_attn[jj] += d_pos[j] * self.encoder_layers[l_idx].wo[idx];
                        }
                    }

                    for prev in 0..enc_len {
                        if weights[prev] < 1e-6 { continue; }
                        let prev_inp = &normed1[prev * d..(prev + 1) * d];
                        for j in 0..d_head {
                            let w_idx = (h_off + j) * d;
                            let g = d_attn[j] * weights[prev];
                            for kk in 0..d {
                                self.encoder_layers[l_idx].wv[w_idx + kk] -= lr_attn * clip_grad(g * prev_inp[kk]);
                            }
                        }
                    }
                }
            }

            // Pass gradient to previous encoder layer
            d_enc_output = d_residual;
        }

        // 5. Input embedding gradient (decoder side)
        for (pos, &tok) in dec_input.iter().take(dec_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            let emb_off = tok_idx * d;
            for j in 0..d {
                let g = d_dec_output[pos * d + j];
                if g.abs() > 1e-8 {
                    self.embeddings[emb_off + j] -= lr * 0.1 * clip_grad(g);
                }
            }
        }

        // 6. Input embedding gradient (encoder side)
        for (pos, &tok) in context.iter().take(enc_len).enumerate() {
            let tok_idx = tok as usize % self.vocab_size;
            let emb_off = tok_idx * d;
            for j in 0..d {
                let g = d_enc_output[pos * d + j];
                if g.abs() > 1e-8 {
                    self.embeddings[emb_off + j] -= lr * 0.1 * clip_grad(g);
                }
            }
        }

        self.train_steps += 1;
        self.running_loss = 0.95 * self.running_loss + 0.05 * loss;

        loss
    }

    /// Train on a single example (input tokens → predict next token).
    /// Returns loss (cross-entropy).
    ///
    /// Backward-compatible: wraps train_enc_dec with same tokens as
    /// both context and target.
    pub fn train_step(&mut self, tokens: &[u32], learning_rate: f32) -> f32 {
        self.train_enc_dec(tokens, tokens, learning_rate)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON with dimension validation.
    pub fn from_json(json: &str) -> Option<Self> {
        let model: Self = serde_json::from_str(json).ok()?;
        if model.d_model != SMALL_D_MODEL || model.n_layers != ENC_LAYERS + DEC_LAYERS {
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
