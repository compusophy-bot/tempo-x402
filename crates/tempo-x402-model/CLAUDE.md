# tempo-x402-model

ML models for autonomous agent intelligence. Pure Rust, no external ML frameworks.

## Models

| Model | File | Params | Purpose |
|-------|------|--------|---------|
| **Unified** | `unified.rs` | 16M | Shared encoder (3 layers D=384) + fast head (classify) + slow decoder (generate). ALL tasks. |
| Code Gen | `codegen.rs` | 15M | 3+3 encoder-decoder, D=384, 8K BPE. Test→code generation. (Being absorbed into unified.) |
| Plan Transformer | `transformer.rs` | 2.2M | Plan step sequences. (Being absorbed into unified.) |
| Code Quality | `quality.rs` | 1.1M | Diff quality evaluation. (Being absorbed into unified.) |
| BPE Tokenizer | `bpe.rs` | — | Byte-pair encoding, 8K vocab, shared by all models |
| Diff Features | `diff_features.rs` | — | Extract 32-dim feature vectors from git diffs |

The Unified Model is the target architecture: one encoder, all tasks, knowledge transfer.

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
