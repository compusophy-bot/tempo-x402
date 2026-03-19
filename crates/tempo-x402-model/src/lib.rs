#![allow(clippy::needless_range_loop, clippy::manual_range_contains)]
//! tempo-x402-model: Sequence model for autonomous plan generation.
//!
//! A from-scratch transformer that predicts optimal plan step sequences given
//! goal context. Trained on the colony's collective plan outcomes.
//!
//! ## Why Not Use an External ML Framework?
//!
//! - Must compile to a single binary for Railway deployment
//! - Must be serializable for federated weight sharing between agents
//! - Must train online (no batch jobs, no GPU)
//! - Must be small enough to run alongside the soul loop
//!
//! ## Architecture
//!
//! ```text
//! Goal keywords → Embedding → [Transformer Layer × 2] → Linear → Softmax → Next step
//!                               ↑ 4-head attention
//!                               ↑ 128-dim model
//!                               ↑ ~260K parameters
//! ```
//!
//! Input: sequence of plan step tokens (+ goal context tokens)
//! Output: probability distribution over next plan step
//!
//! Training: online SGD on successful plan sequences from the colony.
//! Inference: autoregressive generation — predict one step at a time.

pub mod inference;
pub mod trainer;
pub mod transformer;
pub mod vocab;

pub use inference::generate_plan;
pub use trainer::{train_batch, TrainingExample};
pub use transformer::PlanTransformer;
pub use vocab::Vocab;
