#![allow(clippy::needless_range_loop, clippy::manual_range_contains)]
//! tempo-x402-model: Three neural models for autonomous agent intelligence.
//!
//! All from-scratch. No ML framework. Pure Rust. 4.5M total parameters.
//!
//! ## Models
//!
//! - **Plan Transformer** (2.2M params): 4-layer causal attention, D=256, 8 heads.
//!   Predicts optimal plan step sequences. Generates plans WITHOUT LLM calls.
//! - **Code Quality Model** (1.1M params): 3-layer FFN, 32→1024→1024→1.
//!   Predicts whether a code diff improves the codebase. Trained on benchmark deltas.
//! - **Diff Features**: 32-dimensional feature extraction from git diffs.
//!   Detects LOC changes, Rust construct patterns, duplication, test coverage.
//!
//! ## Vocabulary
//!
//! 128-token vocab: plan step types + Rust construct tokens + context keywords.
//! The "Rust alphabet" — fn, struct, impl, match, Result, Option, async, trait, etc.
//!
//! ## Design
//!
//! - Serializable for federated weight sharing between colony peers
//! - Online SGD training (no batch jobs, no GPU)
//! - Dimension validation on deserialization (safe scaling)
//! - Xavier initialization via deterministic LCG PRNG

pub mod bpe;
pub mod codegen;
pub mod diff_features;
pub mod inference;
pub mod quality;
pub mod trainer;
pub mod transformer;
pub mod unified;
pub mod vocab;

pub use diff_features::DiffFeatures;
pub use inference::generate_plan;
pub use quality::{CodeQualityModel, QualityExample, QualityPrediction};
pub use trainer::{train_batch, TrainingExample};
pub use transformer::{PlanTransformer, TransformerDelta};
pub use vocab::Vocab;
