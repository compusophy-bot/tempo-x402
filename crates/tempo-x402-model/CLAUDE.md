# tempo-x402-model

Sequence model for autonomous plan generation. Trained on collective colony experience.

## Architecture

2-layer transformer with causal attention. Predicts optimal plan step sequences
given goal context. Each agent has its own copy with weights that diverge through
local training and converge through colony sync (federated averaging).

## Depends On

None. Pure math — no external ML frameworks, no runtime deps beyond serde.

## If You're Changing...

- **Vocabulary**: `vocab.rs` — adding new plan step types
- **Architecture**: `transformer.rs` — layer count, dim, heads
- **Training**: `trainer.rs` — learning rate, batch size, loss function
- **Inference**: `inference.rs` — beam search, temperature
- **Integration**: used by `x402-soul` as alternative to LLM for plan generation
