# tempo-x402-model

Four ML models for autonomous agent intelligence. Pure Rust, no external ML frameworks.

## Models

| Model | File | Params | Purpose |
|-------|------|--------|---------|
| Code Gen | `codegen.rs` | 55M | 10-layer decoder transformer, D=640, 10 heads, 8K BPE vocab. Trains on workspace + deps. |
| Plan Transformer | `transformer.rs` | 2.2M | Predict optimal plan step sequences |
| Code Quality | `quality.rs` | 1.1M | Evaluate whether code diffs improve the codebase |
| Diff Features | `diff_features.rs` | — | Extract 32-dim feature vectors from git diffs |

Total: 60M+ parameters across codegen + brain (in soul crate) + these models.

## Architecture Principle

Models live HERE, not in soul. Soul orchestrates — it calls into this crate for predictions and training. The soul crate has thin wrappers (`model.rs`) that load/save/train these models via `soul_state` DB keys.

## Depends On

None. Pure math — no runtime deps beyond serde.

## Module Overview

| Module | Purpose |
|--------|---------|
| `transformer.rs` | 4-layer causal transformer (D=256, 8 heads, vocab=128, seq=64) |
| `quality.rs` | 3-layer FFN (32→1024→1024→1) with tanh output for quality scoring |
| `diff_features.rs` | Extract features from `git diff --numstat` + unified diff |
| `trainer.rs` | Online SGD training for transformer |
| `inference.rs` | Beam search plan generation |
| `vocab.rs` | Token vocabulary (plan steps + cartridge + autophagy + context) |

## Scaling

Models should grow with data. CodeGen scaled to 55M params (D=640, 10 layers) to use available RAM (~220MB of 8GB). Trains on workspace source code, cargo registry deps, and benchmark solutions. File-based weight storage (codegen_model.bin) instead of sled for large models.

## If You're Changing...

- **Plan generation**: `transformer.rs` (architecture), `inference.rs` (beam search), `trainer.rs` (SGD)
- **Code quality evaluation**: `quality.rs` (model), `diff_features.rs` (features)
- **Vocabulary**: `vocab.rs` — adding new plan step types or context tokens
- **Integration**: used by `x402-soul` via `src/model.rs` wrapper
