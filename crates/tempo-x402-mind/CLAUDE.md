# tempo-x402-mind

Library crate. Cognitive architecture for autonomous nodes.

**Current (v1):** Dual-soul with forced lateralization (left=code, right=observe) + callosum integration bus. Works but imposes structure rather than letting it emerge.

**Direction (v2):** Single soul per node with emergent multi-modal thinking. Specialization emerges across nodes (different environments → different capabilities), not within one. The callosum becomes cross-node integration. See `lib.rs` module doc for the full vision.

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
