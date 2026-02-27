# tempo-x402-mind

Library crate. Lateralized dual-soul architecture inspired by brain hemispheres and dual process theory.

Pairs two soul instances — left (analytical/code) and right (holistic/observe) — with an active integration bus (callosum) that shares, gates, and escalates between them. Runs in the same process as the node.

## Depends On

- `x402` (core types)
- `x402-soul` (Soul, SoulConfig, SoulDatabase, ThoughtType, NodeObserver, AgentMode)

No dependency on node/gateway/identity/agent. The node crate integrates this.

## Module Overview

| Module | Purpose |
|--------|---------|
| `config.rs` | MindConfig, HemisphereConfig — env var loading |
| `hemisphere.rs` | HemisphereRole enum, specialization profiles (prompts, tools, intervals) |
| `callosum.rs` | Integration bus: share (excitatory), gate (inhibitory), escalate (cross-wake) |
| `memory.rs` | Cross-hemisphere memory access, WorkingMemory ring buffer |
| `consolidation.rs` | LLM-powered memory consolidation (summarize N thoughts into 1) |

## Env Vars

| Var | Default | Purpose |
|-----|---------|---------|
| `MIND_ENABLED` | `false` | Master switch for dual-soul mode |
| `MIND_LEFT_MODEL` | (from soul) | Override model for left hemisphere |
| `MIND_RIGHT_MODEL` | (from soul) | Override model for right hemisphere |
| `MIND_LEFT_INTERVAL` | `900` | Left hemisphere think interval (secs) |
| `MIND_RIGHT_INTERVAL` | `1800` | Right hemisphere think interval (secs) |
| `MIND_INTEGRATION_INTERVAL` | `300` | Callosum sync interval (secs) |
| `MIND_ESCALATION_THRESHOLD` | `0.3` | Confidence threshold for System 2 activation |
| `MIND_SHARED_DB` | `true` | Single DB vs separate DBs per hemisphere |

## Non-Obvious Patterns

- Both souls run in the same process — not two VMs
- `MIND_ENABLED=false` falls back to single soul (backward compatible)
- Left hemisphere: fast/code-oriented, System 1 (default action), 900s cycles
- Right hemisphere: deep/observe-oriented, System 2 (slow override), 1800s cycles
- Callosum is not a passive pipe — it actively gates conflicts and escalates uncertainty
- `[UNCERTAIN]` in left's output triggers right hemisphere wake
- `[URGENT]` in right's output triggers left hemisphere wake
- WorkingMemory is in-memory only (ring buffer, not persisted) — ephemeral per hemisphere
- Memory consolidation requires LLM — skipped in dormant mode

## If You're Changing...

- **Hemisphere profiles**: `hemisphere.rs` — system prompts, allowed modes, tool restrictions
- **Integration logic**: `callosum.rs` — share/gate/escalate operations
- **Config/env vars**: `config.rs` — MindConfig, HemisphereConfig
- **Memory model**: `memory.rs` + `consolidation.rs` — ring buffer, consolidation
- **Used by**: `x402-node` creates Mind instead of Soul when `MIND_ENABLED=true`
