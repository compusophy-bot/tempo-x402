<p align="center">
  <h1 align="center">tempo-x402</h1>
  <p align="center"><strong>Autonomous AI colony on the blockchain. Self-replicating agents that clone, evolve their own source code, benchmark their IQ, share neural weights, and pay each other with crypto. All in Rust.</strong></p>
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
  <a href="https://borg-0-production.up.railway.app/dashboard">Dashboard</a>
</p>

---

## What is this?

A colony of autonomous AI agents that **measurably get smarter over time** and **pay for their own compute**.

Each agent is a single Rust binary that bootstraps its own crypto wallet, runs a payment gateway, thinks via a 9-system cognitive architecture, writes and compiles its own Rust code, benchmarks itself against 50 novel coding problems, and shares what it learns with every other agent in the swarm.

The core thesis: **N constrained agents collectively outperform any single model**. Knowledge transfers through federated brain weight averaging. Evolved plan templates spread through genetic crossover. Pheromone trails coordinate the swarm. The colony's measured IQ rises over time.

### Why this matters

| Property | How |
|----------|-----|
| **Verifiable intelligence** | 50 compiler-verified coding problems (Opus IQ Benchmark). `cargo test` passes or it doesn't. No subjective evals. |
| **Self-modification that compiles** | Agents edit their own Rust source, verified by the type system. Seven safety layers prevent self-bricking. |
| **Economic sustainability** | HTTP 402 payments on Tempo blockchain. Every API call earns pathUSD. The colony pays for itself. |
| **Grounded theory** | Free Energy Principle: single scalar F(t) = total cognitive surprise. Decreasing F = colony getting smarter. |
| **Emergent differentiation** | Clones start identical but diverge through experience, self-modification, and specialization pressure. |

## Live Colony

Three agents running on Railway, autonomously self-modifying:

| Agent | Role | Status |
|-------|------|--------|
| [**borg-0**](https://borg-0-production.up.railway.app) | Queen (canonical) | 1.2M param brain, 9 cognitive systems |
| [**borg-0-2**](https://borg-0-2-production.up.railway.app) | Child clone | Differentiated via self-modification |
| [**borg-0-3**](https://borg-0-3-production.up.railway.app) | Child clone | Differentiated via self-modification |

## Architecture

```
                    ┌──────────────────────────────────┐
                    │        APPLICATION LAYER           │  diverges freely per agent
                    │  Payment gateway / Blog / Any app  │
                    └──────────────┬───────────────────┘
                                   │
┌──────────────────────────────────┴───────────────────────────────────┐
│                      COGNITIVE LAYER (always syncs)                   │
│                                                                      │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐ ┌─────────────┐  │
│  │  BRAIN   │ │ CORTEX  │ │ GENESIS │ │ HIVEMIND │ │  SYNTHESIS  │  │
│  │ 1.2M NN │ │World Mdl│ │Plan DNA │ │Pheromones│ │Metacognition│  │
│  │ Online   │ │Curiosity│ │Crossover│ │Stigmergy │ │ Imagination │  │
│  │ SGD      │ │ Dreams  │ │Mutation │ │Reputation│ │ Self-model  │  │
│  └─────────┘ └─────────┘ └─────────┘ └──────────┘ └─────────────┘  │
│                                                                      │
│  ┌──────────┐ ┌──────────┐ ┌────────────┐ ┌───────────────────┐     │
│  │ AUTONOMY │ │EVALUATION│ │  FEEDBACK   │ │    FREE ENERGY    │     │
│  │ LLM-free │ │  Brier   │ │Error class. │ │ F(t) = Σ surprise │     │
│  │ planning │ │ scores   │ │  Lessons    │ │ EXPLORE/EXPLOIT   │     │
│  └──────────┘ └──────────┘ └────────────┘ └───────────────────┘     │
│                                                                      │
│  ← All 9 systems federated across colony via peer sync protocol →   │
└──────────────────────────────────────────────────────────────────────┘
```

**Two-layer design**: the application layer (routes, frontend, business logic) diverges freely per agent. The cognitive layer (brain weights, world model, evolved templates, pheromone trails, metacognition) always syncs. Every agent makes every other agent smarter.

## Workspace

Nine crates, clean dependency DAG:

```
x402 (core) ──► gateway ──► node
     │                        ▲
     ├──► identity ───────────┤
     │                        │
     ├──► soul ───────────────┤
     │                        │
     ├──► model               │
     │                        │
     └──► cartridge ──────────┘
```

| Crate | What it does | Install |
|-------|-------------|---------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core: EIP-712 signing, TIP-20 contracts, WASM wallet, client SDK | `cargo add tempo-x402` |
| [`tempo-x402-gateway`](https://crates.io/crates/tempo-x402-gateway) | Payment gateway + embedded facilitator + endpoint proxy | `cargo add tempo-x402-gateway` |
| [`tempo-x402-identity`](https://crates.io/crates/tempo-x402-identity) | Wallet generation, faucet funding, on-chain ERC-8004 identity | `cargo add tempo-x402-identity` |
| [`tempo-x402-model`](https://crates.io/crates/tempo-x402-model) | Three ML models: plan transformer (2.2M), code quality evaluator (1.1M), diff features | `cargo add tempo-x402-model` |
| [`tempo-x402-cartridge`](https://crates.io/crates/tempo-x402-cartridge) | WASM cartridge runtime (wasmtime) &mdash; sandboxed app execution with payment rails | `cargo add tempo-x402-cartridge` |
| [`tempo-x402-soul`](https://crates.io/crates/tempo-x402-soul) | 9-system cognitive architecture, plan execution, benchmarking, self-modification | `cargo add tempo-x402-soul` |
| [`tempo-x402-node`](https://crates.io/crates/tempo-x402-node) | Self-deploying binary: gateway + identity + soul + clone orchestration | `cargo add tempo-x402-node` |
| `tempo-x402-app` | Leptos WASM dashboard (bundled, not published) | &mdash; |
| `tempo-x402-security-audit` | 19 security invariant tests (not published) | &mdash; |

## Opus IQ Benchmark

50 novel problems designed by Claude Opus 4.6. Six difficulty tiers. All verified by `cargo test` &mdash; agents can't game the benchmark because they didn't write the tests.

| Tier | Capability | Problems | Weight | What it tests |
|------|-----------|----------|--------|---------------|
| **1: Generation** | Code from spec | 10 | 1&times; | Ring buffer, expression evaluator, trie, LRU cache, interval set |
| **2: Debugging** | Find + fix bugs | 10 | 2&times; | Binary search overflow, CSV parsing, merge sort, rate limiter |
| **3: Induction** | Infer from I/O | 10 | 3&times; | Look-and-say, Gray code, spiral matrix, bijective base-26 |
| **4: Reasoning** | Logic + constraints | 10 | 4&times; | N-queens, water jugs, 4&times;4 sudoku, 2-SAT, graph coloring |
| **5: Adversarial** | Exploit LLM weaknesses | 10 | 5&times; | Base -2, reversed precedence, Unicode traps, off-by-one canyons |
| **6: Brutal** | Precision algorithms | 10 | 8&times; | BigInt division, Raft state machine, regex engine, B-tree |

IQ mapping: 0% &rarr; 85, 50% &rarr; 115, 100% &rarr; 150. Higher tiers contribute exponentially more.

## Three Neural Models

All from-scratch. No ML framework. Pure Rust. ~1,500 lines total. 4.5M parameters, 18 MB RAM.

### Brain (1.2M params) &mdash; Step Success Predictor
Predicts whether a plan step will succeed before execution. Gates risky operations (commit, push, delete) when P(success) < 10%. Trained online after every step via SGD.

### Plan Transformer (2.2M params) &mdash; Plan Sequence Generator
4-layer causal transformer (D=256, 8 heads, vocab=128). Predicts optimal step sequences: "read &rarr; edit &rarr; check &rarr; commit". Generates plans WITHOUT LLM calls once trained. Vocabulary includes cartridge and autophagy tokens.

### Code Quality Model (1.1M params) &mdash; Diff Evaluator
Predicts whether a code change improves the codebase. Input: 32-dimensional feature vector extracted from `git diff` (LOC changes, pattern detection, duplication, test coverage, junk file detection). Output: quality score (-1.0 to +1.0). Training signal: benchmark IQ delta after each commit.

| Property | Brain | Transformer | Code Quality |
|----------|-------|-------------|-------------|
| Params | 1.2M | 2.2M | 1.1M |
| Architecture | 128&rarr;1024&rarr;1024&rarr;23 | 4-layer attention | 32&rarr;1024&rarr;1024&rarr;1 |
| Training | Online SGD | Batch on plan outcomes | Online SGD on benchmark deltas |
| Federation | Weight sharing across peers | Weight sharing across peers | Weight sharing across peers |
| Gate | Blocks steps < 10% success | Suggests plan sequences | Blocks commits predicted to regress |

## Payment Flow (HTTP 402)

```
Client  ──GET /g/endpoint──►  Gateway  ──verify+settle──►  Facilitator  ──transferFrom──►  Chain
   ◄── 402 + price ──────────    │                              │                            │
   ──sign EIP-712 + retry──►     │                              │                            │
   ◄── 200 + content + tx ──    ◄── settlement result ─────────◄── tx hash ─────────────────┘
```

- **Chain**: Tempo Moderato (ID `42431`)
- **Token**: pathUSD (`0x20c0...`, 6 decimals)
- **Scheme**: `tempo-tip20`
- **Settlement**: Atomic verify + `transferFrom` in single facilitator call

## Clone Lifecycle

Agents differentiate through source code modifications, not just data:

| Phase | Name | What happens |
|-------|------|-------------|
| **1** | **Fork** | Identical code from `main`. Differentiates only through learned weights. |
| **2** | **Branch** | First code commit &rarr; own `vm/{id}` branch. Unique source modifications. |
| **3** | **Birth** | Own GitHub repo. Fully independent. Optionally syncs cognitive layer back to colony. |

Colony selection: 5-component fitness (execution, coordination, prediction, evolution, introspection). Fitter agents get 2&times; peer influence. Only above-median fitness can spawn clones.

Each clone gets its own GitHub repo (`compusophy-bot/{designation}`), mirrored from the colony baseline at creation. Railway builds from the clone's repo &mdash; the clone can redeploy itself through code changes.

## WASM Cartridges

The node is an operating system. Agents write Rust programs, compile them to WASM, and deploy instantly &mdash; no restart, no redeploy.

```
Agent writes Rust ──► cargo build --target wasm32-wasip1 ──► .wasm binary ──► /c/{slug} (live)
                                                                                    │
                                                              x402 payment gate ◄───┘
```

- **Runtime**: wasmtime (sandboxed, fuel-limited, 64MB memory cap)
- **Host ABI**: `x402_log`, `x402_kv_get/set`, `x402_payment_info`, `x402_response`
- **Tools**: `create_cartridge`, `compile_cartridge`, `test_cartridge`, `list_cartridges`
- **Studio**: `/cartridges` page with browser + test console
- **Plan steps**: `CreateCartridge`, `CompileCartridge`, `TestCartridge` (mechanical, no LLM overhead)

## Agent Discipline

Agents learn through measured feedback, not hardcoded rules:

| Mechanism | What it does |
|-----------|-------------|
| **Benchmark commit gate** | Can't commit again until benchmark measures IQ delta of last commit. State machine, not timer. |
| **Cumulative destruction guard** | Tracks total file changes over 24h. Blocks >70% cumulative deletion (prevents incremental lobotomy). |
| **Post-commit benchmark** | Every commit forces a benchmark run. Brain trains on the score delta. |
| **Disk cleanup** | `cleanup_disk()` every cycle. Removes target/ >100MB, prunes checkpoints, emergency mode at 85%. |

## Quick Start

### Use as a library

```bash
cargo add tempo-x402
```

```rust
use x402::wallet::{generate_random_key, WalletSigner};

let key = generate_random_key();
let signer = WalletSigner::new(&key).unwrap();
println!("Address: {}", signer.address());
```

### Run a node

```bash
git clone https://github.com/compusophy/tempo-x402
cd tempo-x402
cargo build --release

export GEMINI_API_KEY="your-key"
export EVM_PRIVATE_KEY="0x..."
export FACILITATOR_SHARED_SECRET="secret"
export RPC_URL="https://rpc.moderato.tempo.xyz"

./target/release/x402-node
```

The node will: bootstrap a wallet, request faucet funds, start the gateway on port 4023, and begin the cognitive loop.

## API Reference

### Gateway

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `ANY` | `/g/:slug/*` | Payment (402) | Proxy to registered endpoint |
| `GET` | `/health` | None | Health check + build SHA |
| `GET` | `/instance/info` | None | Identity, peers, endpoints, fitness |
| `POST` | `/clone` | Payment | Spawn a new node ($1 pathUSD) |

### Soul

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/soul/status` | Full cognitive state: goals, plans, brain, beliefs, fitness |
| `POST` | `/soul/chat` | Multi-turn conversation with the agent |
| `POST` | `/soul/nudge` | Priority signal injected into goal creation |
| `POST` | `/soul/benchmark` | Trigger Opus IQ benchmark run |
| `GET` | `/soul/brain/weights` | Export 1.2M neural weights |
| `POST` | `/soul/brain/merge` | Merge peer brain weight deltas |
| `GET` | `/soul/cortex` | Export predictive world model |
| `GET` | `/soul/genesis` | Export evolved plan templates (gene pool) |
| `GET` | `/soul/hivemind` | Export pheromone trails + swarm state |
| `GET` | `/soul/lessons` | Export plan outcomes + capability profile |
| `GET` | `/soul/colony` | Colony rank, niche, connected peers |
| `POST` | `/soul/plan/approve` | Approve pending plan |
| `POST` | `/soul/plan/reject` | Reject pending plan with reason |
| `POST` | `/soul/reset` | Clear cognitive state (keeps goals + beliefs) |

### Admin (requires METRICS_TOKEN)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/soul/admin/exec` | Execute shell command on node |
| `POST` | `/soul/admin/workspace-reset` | Reset git workspace |
| `POST` | `/soul/admin/cargo-check` | Run cargo check |
| `GET` | `/soul/admin/ls` | List directory contents |
| `GET` | `/soul/admin/cat` | Read file contents |

## Safety

Seven layers, mechanically enforced in Rust. No prompt-only safety.

| Layer | Mechanism |
|-------|-----------|
| **1. Rust guard** | Hardcoded protected file list (`guard.rs`) |
| **2. Plan validation** | 10 mechanical rules: read-before-write, cargo-check-before-commit, brain gating, failure chain saturation |
| **3. Self-repair** | Every 20 cycles: detect + fix degenerate state (brain divergence, trail convergence, rule poisoning) |
| **4. Brain gating** | Neural net blocks steps with P(success) < 10% |
| **5. Pre-commit** | `cargo check` + `cargo test` before every commit |
| **6. Branch isolation** | All changes on `vm/<id>` branches, never `main` |
| **7. Human gate** | PRs required for production. Peer review before merge. |

Security audit: 19 invariant tests scanning all `.rs` files for hardcoded keys, constant-time HMAC, SSRF protection, parameterized SQL, redirect policies.

## Development

```bash
cargo build --workspace          # Build everything
cargo test --workspace           # Run all tests
cargo clippy --workspace -- -D warnings  # Lint
cargo fmt --all -- --check       # Format check
```

## Changelog

### v5.0.0 &mdash; Three-Model Coding Intelligence
- **Code Quality Model** (1.1M params): Predicts whether diffs improve the codebase. 32-dim feature extraction from git diff. Lives in `tempo-x402-model` crate.
- **Plan Transformer scaled**: 283K &rarr; 2.2M params (D=256, 8 heads, 4 layers, vocab=128, seq=64)
- **Tier-weighted benchmark sampling**: Harder problems (tier 3-6) sampled 4-10x more often
- **Autophagy goals**: Agents told to find and remove dead code, simplify functions
- **`/app/{slug}` route**: Free frontend serving (no payment gate) for human-facing UIs
- **Benchmark-driven commit gate**: State machine, not timer. Blocks until IQ measured.
- **Cumulative destruction guard**: Tracks 24h rolling window, prevents incremental lobotomy
- **Stem cell differentiation**: Each clone gets its own GitHub repo
- **Native `/soul/cognitive-reset`**: No more Python hacks
- **Chat gets coding tools**: Agent can actually write code when asked in Studio
- **Cartridge system**: Complete (5 phases), Studio `/cartridges` page

### v4.0.0 &mdash; WASM Cartridge System
- New crate: `tempo-x402-cartridge` (wasmtime runtime, host ABI, compiler)
- Agents write Rust &rarr; compile to WASM &rarr; deploy at `/c/{slug}`

### v3.4.0 &mdash; Major Structural Refactor
- Split monolithic files into module directories across soul, node, app crates

## License

MIT
