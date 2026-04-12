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

Rust workspace. 9 crates. ~116K lines. 30M+ neural parameters in a unified encoder-decoder. Bloch sphere cognitive geometry. Hot-swappable WASM cognitive modules. 188 compile-verified training cartridges. No ML framework. No Python. No GPU.

## What is this?

A colony of autonomous AI agents with a **neuroplastic fluid cognitive architecture** that **measurably gets smarter over time** and **pays for its own compute**.

Each agent is a single Rust binary. It bootstraps a crypto wallet, runs a payment gateway, thinks via a 9-system cognitive architecture, writes Rust, compiles it to WASM, benchmarks itself against 201 compiler-verified coding problems, trains a unified encoder-decoder on its own source code and dependencies, evolves its cognitive state on a continuous Bloch sphere, and can hot-swap its own cognitive modules at runtime via WASM cartridges.

Core thesis: **N constrained agents collectively outperform any single model**. Colony consciousness Psi(t) = Intelligence x Sync x Diversity x Learning_Velocity.

## Architecture

```
Client --> Gateway (4023) --> Facilitator (embedded) --> Tempo Chain (42431)
               |
               +-- Identity (wallet bootstrap + faucet + ERC-8004)
               +-- Soul (9-system cognitive architecture, Gemini-powered)
               |     +-- sled KV store (lock-free, all cognitive state)
               |     +-- Unified Model (16M shared encoder + fast/slow heads)
               |     +-- Bloch Sphere (continuous cognitive state on S²)
               |     +-- Cognitive Orchestrator (routes to WASM cartridges)
               +-- Cartridge Engine (wasmtime WASM sandbox, hot-swappable)
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

All from-scratch. Pure Rust. No ML framework. No GPU. Train online on own source code + cargo registry deps, share weights via federated averaging.

| Model | Params | Architecture | Purpose |
|-------|--------|-------------|---------|
| **Unified** | 16M | 3-layer shared encoder (D=384, bidirectional) + fast FFN head + 3-layer decoder | All tasks: code gen, planning, prediction, quality |
| **Brain** | 1.2M | 128->1024->1024->23 FFN | Step success prediction (legacy, being absorbed) |
| **CodeGen** | 15M | 3+3 encoder-decoder, D=384, 6 heads, 8K BPE | Test-conditioned code generation |
| **Plan Transformer** | 2.2M | 4-layer causal, D=256, 8 heads | Plan generation (legacy, being absorbed) |

The Unified Model is the future: one encoder learns from ALL cognitive tasks simultaneously. Fast head (~10ms) for classification. Slow decoder (seconds) for generation. Same weights, different paths.

## Bloch Sphere Cognitive State

Replaces 13 discrete cognitive states with one continuous point on the unit sphere S²:

```
theta in [0, pi]:   exploit <-> explore    (driven by Free Energy F(t))
phi in [0, 2*pi]:   self <-> colony        (driven by Psi(t))
```

No hard thresholds. No state machine jumps. Smooth gradient-driven rotation on a manifold. Peer sync correlates Bloch states across the colony (entanglement analog).

## Cognitive Cartridges

Cognitive systems can be compiled to WASM and hot-swapped at runtime:

```
1. Agent writes new cortex.rs          (Gemini generates code)
2. Compile to WASM                     (20 seconds)
3. engine.replace_module("cortex")     (instant, zero downtime)
4. If fitness drops, rollback           (instant)
```

Self-modification at the speed of thought, not the speed of cargo build.

## Opus IQ Benchmark

201 compiler-verified coding problems across 6 tiers. `cargo test` passes or it doesn't -- no LLM judge, no fuzzy eval.

Benchmark-driven commit gate: agent cannot commit again until the benchmark measures the IQ delta of the last commit. Stuck problems (5+ consecutive failures) are deprioritized. Stagnation detection triggers behavioral change after 3+ flat runs.

## Cartridge OS

Agents write Rust, compile to WASM, deploy instantly at `/c/{slug}` -- no restart, no redeploy.

| Type | Exports | Use case |
|------|---------|----------|
| **Backend** | `x402_handle` | HTTP APIs, JSON services, server compute |
| **Interactive** | `x402_tick`, `x402_get_framebuffer` | Games, visualizations, 60fps canvas apps |
| **Frontend** | `init(selector)` | Leptos SPA mounted to DOM via wasm-bindgen |
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

### v9.3.0 -- Composable Cartridge Intelligence

Cartridges compose, the soul sees what it builds, the codegen model actually learns, and can now write cartridges locally without API calls.

- **Visual testing loop**: Soul opens cartridges in a browser, screenshots via Xvfb/scrot, analyzes rendering via Gemini Vision, iterates on visual quality (not just compilation)
- **x402_call composition**: Cartridges can invoke other cartridges via `x402_call(slug, request)` host function. Max depth 3, isolated KV per child, 10s timeout. ABI v2.
- **Multi-token teacher forcing**: Codegen model now trains on ALL decoder positions (was only last token — 98% wasted compute). ~64x more gradient signal per step.
- **3 codegen training bugs fixed**: solution weight cap killed 3x weighting, cross-attention gradient scaled to near-zero, gradient clipping too aggressive
- **Local-first cartridge generation**: New `generate_cartridge_code` tool — local 15M param model writes cartridge source, validates structure, cargo checks, falls back to Gemini on failure
- **LR schedule tuned**: Lower peak (0.002), longer warmup (500 steps) for multi-token loss stability

### v9.2.0 -- Training Corpus Expansion

188 compile-verified WASM cartridges for codegen model training. 100% compilation rate against wasm32-unknown-unknown target.

- **188 training cartridges**: tier1 (62 static), tier2 (24 routing/parsing), tier3 (25 KV state), tier4 (40 rich apps), tier5 (17 complex multi-feature), frontend (20 Leptos SPA)
- **New tier4 apps**: grocery list, diary, unit converter, scorekeeper, word counter, bingo, daily planner, address book, music playlist, tip calculator, tic-tac-toe, coin flip, stopwatch, grade tracker, water tracker, dice roller, sleep log, color palette generator, weight log
- **New tier5 apps**: recipe book with detail view, expense manager with budget tracking, flashcard quiz with spaced repetition stats, workout planner with weekly schedule
- **New frontend cartridges**: dice roller, quiz app, tip calculator, habit tracker, notepad, unit converter, countdown timer, voting poll, tic-tac-toe -- all Leptos 0.6 CSR
- **Codegen bulk-loader**: `load_training_corpus()` reads all .rs files from training_data/cartridges/ on first train cycle
- **Sled startup compaction**: Auto-compacts sled DB if >500MB (export/reimport)

### v9.1.0 -- Cartridge System Overhaul

The cartridge system actually works end-to-end now. Six bugs fixed, cognitive cartridges wired as primary dispatch, soul iterates deeper.

- **Hot-reload on recompile**: `engine.replace_module()` now called on compile -- recompiled cartridges actually swap in instead of serving stale cached versions
- **KV persistence**: Cartridge KV store changes are persisted to DB after execution. Previously, all `kv_set()` calls were lost when the request ended
- **KV cleanup on delete**: Deleting a cartridge now cleans up its KV store. Re-creating the same slug starts fresh
- **Double-registration removed**: `list_cartridges()` no longer duplicates the startup auto-registration
- **Cognitive CartridgeKind**: New `Cognitive` variant for hot-swappable brain modules (prefixed `cognitive-`)
- **Soul iteration depth**: Observe 5->10, Code 15->25 tool calls. Graceful termination: LLM gets one final call to summarize instead of being hard-stopped mid-task
- **CognitiveOrchestrator wired**: Brain predictions route through WASM cartridges when `cognitive-brain` is loaded, falling back to compiled code
- **`create_cognitive_cartridge` tool**: Soul can scaffold, compile, and hot-swap cognitive cartridges for any system (brain, cortex, genesis, hivemind, synthesis, unified)
- **Cognitive status in /soul/status**: Shows which cognitive systems have hot-swappable cartridges loaded
- **Flaky model test fixed**: Plan transformer training test now uses 500 rounds (was 100) with softer assertion

### v9.0.0 -- Neuroplastic Fluid Cognitive Architecture

The biggest architectural change since v1.0. The colony's cognition is now continuous, unified, and hot-swappable.

- **Bloch sphere cognitive state**: Replaces 13 discrete states (CognitiveState + EnergyRegime + Drive) with one continuous point (theta, phi) on S². Free Energy drives theta (exploit/explore), Psi drives phi (self/colony). Smooth rotations, no jumps. Drives temporal oscillator modulation.
- **Unified model** (16M params): One shared encoder (3 layers, D=384, bidirectional) with fast classification head (~10ms) and slow decoder head (seconds). Trains on ALL cognitive tasks simultaneously — brain prediction, code generation, plan creation, quality evaluation. Same weights, knowledge transfers across tasks.
- **Cognitive cartridge orchestrator**: Routes cognitive calls through the WASM cartridge engine. Any cognitive system can be compiled to WASM and hot-swapped at runtime (20 seconds). Fallback to compiled code if cartridge not loaded. `engine.replace_module()` for atomic swap.
- **3 benchmark fixes**: Shared target dir not cleaned between problems, SIGKILL watchdog for hung cargo test, codegen generation disabled until loss < 4.0. Benchmark now completes in ~7 minutes.
- **Disk cleanup**: Old sled DB, workspace, checkpoints cleaned on startup. Cartridge source preserved (not deleted). /data volume stays under 5%.
- **Frontend cartridge registration**: Frontend WASM apps (Leptos) now auto-register in the cartridge list by scanning filesystem.
- **TOON integration**: Token-Oriented Object Notation wired into observation snapshots, endpoint tables, peer catalogs.
- **Encoder-decoder architecture**: CodeGen model redesigned from decoder-only to encoder-decoder. Encoder reads test code bidirectionally, decoder generates solution with cross-attention.
- **201 benchmark problems**: 20 new tier 1 problems covering diverse Rust patterns.

### v8.1.0 -- Self-Teaching Colony

The colony trains its code generation model on its own source code, its dependencies, and every benchmark solution it solves. TOON (Token-Oriented Object Notation) wired into LLM prompts. 201 benchmark problems. Automated colony caretaker.

- **Self-feeding training**: Codegen model trains on the workspace codebase (72K+ lines), cargo registry deps (tokio, serde, actix, alloy), and benchmark solutions (3x weighted). Was training on 33 examples; now has 500+ chunks.
- **Model scaled**: CodeGen 29M -> 55M params (D=640, 10 layers, 10 heads). Uses 15% of 8GB RAM instead of 1.7%.
- **5x training intensity**: 50 examples/cycle, 128-token windows, 3x learning rate. Full corpus coverage in hours, not weeks.
- **Benchmark expansion**: 181 -> 201 problems. 20 new tier 1 problems covering diverse Rust patterns (LRU cache, trie, JSON parser, cron parser, bitset, etc.)
- **Codegen feedback loop tightened**: Temperature sampling (0.8) replaces greedy argmax. Cargo test validates output, not pattern matching. Codegen solve rate tracked as first-class metric.
- **TOON integration**: Token-Oriented Object Notation wired into observation snapshots, endpoint tables, peer catalog, and PR listings. 10-20% token savings on structured prompt sections.
- **File-based weight storage**: 55M params serialized to file instead of sled blob. Lightweight metadata marker in DB.
- **3 crash fixes**: Peer sync hang (15s/120s timeouts), disk full benchmark deadlock (space check + /tmp cleanup), sled volume growth (DB moved to ephemeral /tmp)
- **Colony Caretaker**: Scheduled remote agent (every 2h) auto-heals hung nodes, triggers benchmarks, reports IQ trends

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
