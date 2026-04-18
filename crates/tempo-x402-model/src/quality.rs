//! Code Quality Model — predicts whether a code change improves the codebase.
//!
//! A 3-layer feedforward network that scores diffs. Trained on benchmark deltas:
//! "did this commit improve the agent's IQ?" The model learns which diff patterns
//! correlate with benchmark improvement and which correlate with regression.
//!
//! Architecture: 32 → 256 → 256 → 1 (~500K params, 2 MB RAM)
//! Input: diff features from `diff_features.rs`
//! Output: predicted quality score (-1.0 to +1.0)
//! Training: online SGD from (diff_features, benchmark_delta) pairs

use serde::{Deserialize, Serialize};

use crate::diff_features::DIFF_FEATURE_DIM;

/// Hidden layer size — scaled to match plan transformer capacity.
/// Code quality evaluation is at least as hard as plan sequence prediction.
/// 32 → 1024 → 1024 → 1 = ~1.1M params (comparable to brain's 1.2M).
const HIDDEN_SIZE: usize = 1024;
const LEARNING_RATE: f32 = 0.0005; // Lower for larger model stability
const WEIGHT_DECAY: f32 = 0.0001;

/// The code quality prediction model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeQualityModel {
    /// Layer 1: DIFF_FEATURE_DIM → HIDDEN_SIZE
    pub w1: Vec<f32>,
    pub b1: Vec<f32>,
    /// Layer 2: HIDDEN_SIZE → HIDDEN_SIZE
    pub w2: Vec<f32>,
    pub b2: Vec<f32>,
    /// Layer 3 (output): HIDDEN_SIZE → 1
    pub w3: Vec<f32>,
    pub b3: f32,
    /// Training metadata
    pub train_steps: u64,
    pub running_loss: f32,
}

/// Prediction from the quality model.
#[derive(Debug, Clone)]
pub struct QualityPrediction {
    /// Predicted quality score: -1.0 (regression) to +1.0 (improvement)
    pub score: f32,
    /// Confidence: how many training examples has the model seen
    pub confidence: f32,
}

/// Training example: (features, actual_delta).
#[derive(Debug, Clone)]
pub struct QualityExample {
    pub features: Vec<f32>,
    /// Actual benchmark delta after this commit (-1.0 to +1.0, normalized)
    pub target: f32,
}

impl Default for CodeQualityModel {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeQualityModel {
    /// Create a new randomly initialized model.
    pub fn new() -> Self {
        let mut seed: u64 = 1337;

        Self {
            w1: xavier_init(DIFF_FEATURE_DIM, HIDDEN_SIZE, &mut seed),
            b1: vec![0.0; HIDDEN_SIZE],
            w2: xavier_init(HIDDEN_SIZE, HIDDEN_SIZE, &mut seed),
            b2: vec![0.0; HIDDEN_SIZE],
            w3: xavier_init(HIDDEN_SIZE, 1, &mut seed),
            b3: 0.0,
            train_steps: 0,
            running_loss: 0.0,
        }
    }

    /// Total parameter count.
    pub fn param_count(&self) -> usize {
        self.w1.len() + self.b1.len() + self.w2.len() + self.b2.len() + self.w3.len() + 1
    }

    /// Forward pass: features → quality score.
    pub fn predict(&self, features: &[f32]) -> QualityPrediction {
        assert!(
            features.len() >= DIFF_FEATURE_DIM,
            "features must be at least {} elements",
            DIFF_FEATURE_DIM
        );

        // Layer 1: input → hidden (ReLU)
        let mut h1 = vec![0.0f32; HIDDEN_SIZE];
        for j in 0..HIDDEN_SIZE {
            let mut sum = self.b1[j];
            for i in 0..DIFF_FEATURE_DIM {
                sum += features[i] * self.w1[i * HIDDEN_SIZE + j];
            }
            h1[j] = sum.max(0.0); // ReLU
        }

        // Layer 2: hidden → hidden (ReLU)
        let mut h2 = vec![0.0f32; HIDDEN_SIZE];
        for j in 0..HIDDEN_SIZE {
            let mut sum = self.b2[j];
            for i in 0..HIDDEN_SIZE {
                sum += h1[i] * self.w2[i * HIDDEN_SIZE + j];
            }
            h2[j] = sum.max(0.0); // ReLU
        }

        // Layer 3: hidden → output (tanh for -1 to +1 range)
        let mut output = self.b3;
        for i in 0..HIDDEN_SIZE {
            output += h2[i] * self.w3[i];
        }
        let score = output.tanh();

        // Confidence based on training steps (saturates at 1.0 after 100 examples)
        let confidence = (self.train_steps as f32 / 100.0).min(1.0);

        QualityPrediction { score, confidence }
    }

    /// Train on a single example using SGD with backprop.
    pub fn train(&mut self, example: &QualityExample) -> f32 {
        let features = &example.features;
        let target = example.target.clamp(-1.0, 1.0);

        // Forward pass (save activations for backprop)
        let mut h1 = vec![0.0f32; HIDDEN_SIZE];
        let mut h1_pre = vec![0.0f32; HIDDEN_SIZE]; // pre-ReLU
        for j in 0..HIDDEN_SIZE {
            let mut sum = self.b1[j];
            for i in 0..DIFF_FEATURE_DIM {
                sum += features[i] * self.w1[i * HIDDEN_SIZE + j];
            }
            h1_pre[j] = sum;
            h1[j] = sum.max(0.0);
        }

        let mut h2 = vec![0.0f32; HIDDEN_SIZE];
        let mut h2_pre = vec![0.0f32; HIDDEN_SIZE];
        for j in 0..HIDDEN_SIZE {
            let mut sum = self.b2[j];
            for i in 0..HIDDEN_SIZE {
                sum += h1[i] * self.w2[i * HIDDEN_SIZE + j];
            }
            h2_pre[j] = sum;
            h2[j] = sum.max(0.0);
        }

        let mut output = self.b3;
        for i in 0..HIDDEN_SIZE {
            output += h2[i] * self.w3[i];
        }
        let prediction = output.tanh();

        // Loss: MSE
        let error = prediction - target;
        let loss = error * error;

        // Backprop through tanh
        let d_tanh = 1.0 - prediction * prediction;
        let d_output = 2.0 * error * d_tanh;

        // Layer 3 gradients
        for i in 0..HIDDEN_SIZE {
            let grad = d_output * h2[i] + WEIGHT_DECAY * self.w3[i];
            self.w3[i] -= LEARNING_RATE * grad;
        }
        self.b3 -= LEARNING_RATE * d_output;

        // Layer 2 gradients
        let mut d_h2 = vec![0.0f32; HIDDEN_SIZE];
        for j in 0..HIDDEN_SIZE {
            d_h2[j] = d_output * self.w3[j];
            if h2_pre[j] <= 0.0 {
                d_h2[j] = 0.0; // ReLU derivative
            }
        }
        for i in 0..HIDDEN_SIZE {
            for j in 0..HIDDEN_SIZE {
                let grad = d_h2[j] * h1[i] + WEIGHT_DECAY * self.w2[i * HIDDEN_SIZE + j];
                self.w2[i * HIDDEN_SIZE + j] -= LEARNING_RATE * grad;
            }
        }
        for j in 0..HIDDEN_SIZE {
            self.b2[j] -= LEARNING_RATE * d_h2[j];
        }

        // Layer 1 gradients
        let mut d_h1 = vec![0.0f32; HIDDEN_SIZE];
        for i in 0..HIDDEN_SIZE {
            for j in 0..HIDDEN_SIZE {
                d_h1[i] += d_h2[j] * self.w2[i * HIDDEN_SIZE + j];
            }
            if h1_pre[i] <= 0.0 {
                d_h1[i] = 0.0; // ReLU derivative
            }
        }
        for i in 0..DIFF_FEATURE_DIM {
            for j in 0..HIDDEN_SIZE {
                let grad = d_h1[j] * features[i] + WEIGHT_DECAY * self.w1[i * HIDDEN_SIZE + j];
                self.w1[i * HIDDEN_SIZE + j] -= LEARNING_RATE * grad;
            }
        }
        for j in 0..HIDDEN_SIZE {
            self.b1[j] -= LEARNING_RATE * d_h1[j];
        }

        // Update stats
        self.train_steps += 1;
        self.running_loss = self.running_loss * 0.95 + loss * 0.05;

        loss
    }

    /// Train on a batch of examples.
    pub fn train_batch(&mut self, examples: &[QualityExample]) -> (usize, f32) {
        let mut total_loss = 0.0;
        for example in examples {
            total_loss += self.train(example);
        }
        let avg_loss = if examples.is_empty() {
            0.0
        } else {
            total_loss / examples.len() as f32
        };
        (examples.len(), avg_loss)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON, with dimension validation.
    pub fn from_json(json: &str) -> Option<Self> {
        let model: Self = serde_json::from_str(json).ok()?;
        // Validate dimensions
        if model.w1.len() != DIFF_FEATURE_DIM * HIDDEN_SIZE {
            return None; // Architecture mismatch
        }
        Some(model)
    }
}

/// Xavier initialization for weight matrices.
fn xavier_init(fan_in: usize, fan_out: usize, seed: &mut u64) -> Vec<f32> {
    let scale = (2.0 / (fan_in + fan_out) as f64).sqrt() as f32;
    (0..fan_in * fan_out)
        .map(|_| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = (*seed as f32) / (u64::MAX as f32);
            (u * 2.0 - 1.0) * scale
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_model() {
        let model = CodeQualityModel::new();
        let params = model.param_count();
        println!("Code quality model parameters: {}", params);
        // 32*1024 + 1024 + 1024*1024 + 1024 + 1024*1 + 1 ≈ 1.1M params
        assert!(params > 1_000_000, "Should have >1M params, got {}", params);
        assert!(params < 2_000_000, "Should have <2M params, got {}", params);
    }

    #[test]
    fn test_predict() {
        let model = CodeQualityModel::new();
        let features = vec![0.5; DIFF_FEATURE_DIM];
        let pred = model.predict(&features);
        assert!(pred.score >= -1.0 && pred.score <= 1.0);
        assert_eq!(pred.confidence, 0.0); // Untrained
    }

    #[test]
    fn test_train() {
        let mut model = CodeQualityModel::new();
        let example = QualityExample {
            features: vec![0.5; DIFF_FEATURE_DIM],
            target: 0.8, // This was a good commit
        };

        let _loss1 = model.train(&example);
        let _loss2 = model.train(&example);
        // Loss should generally decrease with repeated training on same example
        assert!(model.train_steps == 2);
        // After training, prediction should move toward target
        let pred = model.predict(&example.features);
        println!("After 2 steps: prediction={:.3}, target=0.8", pred.score);
    }

    #[test]
    fn test_serialization() {
        let model = CodeQualityModel::new();
        let json = model.to_json();
        let restored = CodeQualityModel::from_json(&json).unwrap();
        assert_eq!(model.param_count(), restored.param_count());
        assert_eq!(model.train_steps, restored.train_steps);
    }
}
