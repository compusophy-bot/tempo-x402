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

Eight crates, clean dependency DAG:

```
x402 (core) ──► gateway ──► node
     │                        ▲
     ├──► identity ───────────┤
     │                        │
     ├──► soul ───────────────┘
     │
     └──► model
```

| Crate | What it does | Install |
|-------|-------------|---------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core: EIP-712 signing, TIP-20 contracts, WASM wallet, client SDK | `cargo add tempo-x402` |
| [`tempo-x402-gateway`](https://crates.io/crates/tempo-x402-gateway) | Payment gateway + embedded facilitator + endpoint proxy | `cargo add tempo-x402-gateway` |
| [`tempo-x402-identity`](https://crates.io/crates/tempo-x402-identity) | Wallet generation, faucet funding, on-chain ERC-8004 identity | `cargo add tempo-x402-identity` |
| [`tempo-x402-model`](https://crates.io/crates/tempo-x402-model) | From-scratch transformer for plan sequence prediction | `cargo add tempo-x402-model` |
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

## Neural Brain

From-scratch feedforward neural network. No ML framework. Pure Rust, ~600 lines.

| Property | Value |
|----------|-------|
| Parameters | 1,205,271 (128&rarr;1024&rarr;1024&rarr;23) |
| Training | Online SGD after every plan step |
| Gating | Blocks risky operations when P(success) < 10% |
| Federation | Weight deltas shared across peers (merge rate 0.3) |
| Initialization | Xavier via deterministic LCG PRNG |
| Outputs | Success prob, error category (11-class), per-capability confidence (11 skills) |

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

## v3.4.0 Changelog

Major structural refactor. No functional changes.

- **Soul crate**: Split 4 monolithic files (14,955 lines) into module directories
  - `tools/` (9 domain files), `thinking/` (7 files), `db/` (13 files), `opus_bench/` (6 tier files)
- **Node crate**: Split `routes/soul.rs` (2,718 lines) into 9 focused handler modules
- **App crate**: Extracted 3,208-line monolith into 8 component files
- **Opus IQ Benchmark**: Added Tier 6 (Brutal) &mdash; 10 precision-critical problems at 8&times; weight
- Fixed x402-model dependency version (was pinned to 3.0.0)

## License

MIT
