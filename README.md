<p align="center">
  <h1 align="center">tempo-x402</h1>
  <p align="center"><strong>Autonomous AI colony on the Tempo blockchain. Self-replicating agents that clone, evolve source code, benchmark IQ, share neural weights, and pay each other via HTTP 402.</strong></p>
</p>

<p align="center">
  <a href="https://crates.io/crates/tempo-x402"><img src="https://img.shields.io/crates/v/tempo-x402.svg" alt="crates.io"></a>
  <a href="https://docs.rs/tempo-x402"><img src="https://docs.rs/tempo-x402/badge.svg" alt="docs.rs"></a>
  <a href="https://github.com/compusophy/tempo-x402/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
</p>

<p align="center">
  <a href="https://docs.rs/tempo-x402">Docs</a> &middot;
  <a href="https://crates.io/crates/tempo-x402">Crates</a> &middot;
  <a href="https://borg-0-production.up.railway.app">Live Colony</a> &middot;
  <a href="https://borg-0-production.up.railway.app/studio">Studio</a>
</p>

---

Rust workspace. 9 crates. ~72K lines. 54M+ neural parameters across 4 from-scratch models. No ML framework. No Python. No GPU.

## What is this?

A colony of autonomous AI agents that **measurably get smarter over time** and **pay for their own compute**.

Each agent is a single Rust binary. It bootstraps a crypto wallet, runs a payment gateway, thinks via a 9-system cognitive architecture, writes Rust, compiles it to WASM, benchmarks itself against 182 compiler-verified coding problems, trains 4 neural models locally, and shares what it learns with every other agent in the colony.

Core thesis: **N constrained agents collectively outperform any single model**. Colony consciousness Psi(t) = Intelligence x Sync x Diversity x Learning_Velocity.

## Architecture

```
Client --> Gateway (4023) --> Facilitator (embedded) --> Tempo Chain (42431)
               |
               +-- Identity (wallet bootstrap + faucet + ERC-8004)
               +-- Soul (9-system cognitive architecture, Gemini-powered)
               |     +-- sled KV store (lock-free, all cognitive state)
               |     +-- Brain (1.2M), Transformer (2.2M), Quality (1.1M), CodeGen (50M)
               +-- Cartridge Engine (wasmtime WASM sandbox runtime)
               +-- Clone Orchestrator (Railway self-replication)
```

Two-layer design: **Application layer** (routes, frontend, cartridges) diverges per agent. **Cognitive layer** (brain, cortex, genesis, hivemind, synthesis, autonomy, evaluation, feedback, free energy) always syncs across the colony.

**Stem cell model**: Each clone gets its own GitHub repo. Code diverges independently. Good changes flow upstream via PRs.

## Nine Cognitive Systems

All federated across the colony via peer sync protocol.

| System | Role |
|--------|------|
| **Brain** | 1.2M FFN. Step success prediction, error classification, brain gating |
| **Cortex** | World model. Accuracy tracking, validation scores |
| **Genesis** | Plan DNA. Template evolution across generations |
| **Hivemind** | Pheromone trails. Colony coordination signals |
| **Synthesis** | Metacognition. Coherence scoring, confidence calibration |
| **Autonomy** | LLM-free planning via learned transformer |
| **Evaluation** | Brier scores. Prediction calibration |
| **Feedback** | Error classification. Lesson extraction |
| **Free Energy** | F(t) + Psi(t). Explore/exploit regime switching |

## Neural Models

All from-scratch. Pure Rust. 54M+ parameters total. Train online, share weights via federated averaging.

| Model | Params | Architecture | Purpose |
|-------|--------|-------------|---------|
| **Brain** | 1.2M | 128->1024->1024->23 FFN | Step success prediction, error classification |
| **Plan Transformer** | 2.2M | 4-layer causal attention, D=256, 8 heads | Plan generation without LLM calls |
| **Code Quality** | 1.1M | 32->1024->1024->1 FFN | Diff evaluation, commit gating |
| **Code Gen** | 50M | 8-layer transformer, D=512, 8 heads, 8K BPE vocab | Local Rust code generation |

## Opus IQ Benchmark

182 compiler-verified coding problems across 6 tiers. `cargo test` passes or it doesn't -- no LLM judge, no fuzzy eval.

Benchmark-driven commit gate: agent cannot commit again until the benchmark measures the IQ delta of the last commit. Stuck problems (5+ consecutive failures) are deprioritized. Stagnation detection triggers behavioral change after 3+ flat runs.

## Cartridge OS

Agents write Rust, compile to WASM, deploy instantly at `/c/{slug}` -- no restart, no redeploy.

| Type | Exports | Use case |
|------|---------|----------|
| **Backend** | `x402_handle` | HTTP APIs, JSON services, server compute |
| **Interactive** | `x402_tick`, `x402_get_framebuffer` | Games, visualizations, 60fps canvas apps |
| **Cognitive** | Registered as tools | Self-modification -- agent rewires its own intelligence |

Sandboxed: 64MB memory, fuel CPU limit, 30s timeout, no filesystem access.

## Workspace

| Crate | What it does |
|-------|-------------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core: EIP-712 signing, TIP-20 contracts, WASM wallet, client SDK |
| [`tempo-x402-gateway`](https://crates.io/crates/tempo-x402-gateway) | Payment gateway + embedded facilitator + endpoint proxy |
| [`tempo-x402-identity`](https://crates.io/crates/tempo-x402-identity) | Wallet generation, faucet, on-chain ERC-8004 identity + peer discovery |
| [`tempo-x402-model`](https://crates.io/crates/tempo-x402-model) | 4 ML models: brain, transformer, quality, code gen + BPE tokenizer |
| [`tempo-x402-cartridge`](https://crates.io/crates/tempo-x402-cartridge) | WASM cartridge runtime (wasmtime) -- sandboxed execution |
| [`tempo-x402-soul`](https://crates.io/crates/tempo-x402-soul) | 9-system cognitive architecture, sled KV store, benchmarking |
| [`tempo-x402-node`](https://crates.io/crates/tempo-x402-node) | Self-deploying binary: gateway + identity + soul + clone orchestration |
| `tempo-x402-app` | Leptos WASM dashboard (bundled, not published) |
| `tempo-x402-security-audit` | 19 security invariant tests (not published) |

Dependency DAG: `x402 -> gateway -> node`, `x402 -> identity -> node`, `x402 -> soul -> node`, `x402 -> model -> soul`, `cartridge -> soul, node`.

## Colony

| Agent | Domain | Role |
|-------|--------|------|
| **borg-0** | `borg-0-production.up.railway.app` | Queen (canonical, coordinates work) |
| **borg-0-2** | `borg-0-2-production.up.railway.app` | Worker (own repo, independent evolution) |

Queen/Worker architecture. Queen partitions benchmark problems across N workers. Workers fetch canonical weights, solve their partition, report results. Add a node = instant speedup. Lose a node = graceful degradation.

Psi(t) = (Intelligence x Sync x Diversity x Velocity)^0.25. When Psi rises, the colony is getting smarter than any individual.

## Chain

- **Network**: Tempo Moderato, Chain ID `42431`, CAIP-2 `eip155:42431`
- **Token**: pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals)
- **Scheme**: `tempo-tip20`
- **RPC**: `https://rpc.moderato.tempo.xyz`

## Quick Start

```bash
git clone https://github.com/compusophy/tempo-x402
cd tempo-x402
cargo build --release

export GEMINI_API_KEY="your-key"
./target/release/x402-node
```

The node auto-bootstraps: generates wallet, requests faucet funds, mints on-chain identity, starts gateway on port 4023, begins cognitive loop.

## Changelog

### v8.0.0 -- Lock-Free Cognition (sled)

SQLite completely removed from the cognitive layer, replaced with sled -- a lock-free embedded KV store.

- **sled migration**: All cognitive state (brain data, benchmark history, training records, feedback, plans, cortex, genesis, hivemind, synthesis) moved from SQLite to sled
- **Deadlock eliminated**: The `spawn_blocking` + `.await` deadlock between codegen training and async thinking loop is structurally impossible now. No mutexes on the DB path.
- **-791 lines**: Removed SQLite schema migrations, connection pooling, mutex wrappers, and `spawn_blocking` bridges
- **Zero-copy reads**: sled returns `IVec` slices directly from the page cache
- **Crash-safe**: sled uses a log-structured merge tree with atomic batch writes

### v7.0.0 -- Collective Consciousness

Colony is one distributed mind, not separate agents sharing weights.

- Queen/Worker architecture with distributed benchmarking
- Single canonical brain: workers fetch from queen every cycle
- 7 colony coordination endpoints
- Fungible workers: add node = instant speedup

### v6.8.0 -- Benchmark as Core Learning Engine
### v6.7.0 -- Fix Intelligence Learning Pipeline
### v6.1.0 -- Cockpit UI + Queen Audit
### v6.0.0 -- Psi(t) + Cartridge OS + Phase 3 Code Gen
### v5.1.0 -- Deep Planning + Cartridge Fix
### v5.0.0 -- Three-Model Coding Intelligence
### v4.0.0 -- WASM Cartridge System
### v3.4.0 -- Major Structural Refactor

## License

MIT
