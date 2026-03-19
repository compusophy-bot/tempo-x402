<p align="center">
  <h1 align="center">tempo-x402</h1>
  <p align="center"><strong>Self-replicating autonomous agents with colony selection, cognitive sync, and a from-scratch transformer &mdash; paid per request via HTTP 402 on Tempo blockchain</strong></p>
</p>

<p align="center">
  <a href="https://crates.io/crates/tempo-x402"><img src="https://img.shields.io/crates/v/tempo-x402.svg" alt="crates.io"></a>
  <a href="https://docs.rs/tempo-x402"><img src="https://docs.rs/tempo-x402/badge.svg" alt="docs.rs"></a>
  <a href="https://github.com/compusophy/tempo-x402/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
</p>

<p align="center">
  <a href="https://docs.rs/tempo-x402">Docs</a> &middot;
  <a href="https://crates.io/crates/tempo-x402">Crates</a> &middot;
  <a href="https://soul-bot-production.up.railway.app">Live Node</a> &middot;
  <a href="https://soul-bot-production.up.railway.app/dashboard">Dashboard</a> &middot;
  <a href="https://github.com/compusophy/tempo-x402">Source</a>
</p>

---

## What is this?

A Rust workspace implementing **x402** (HTTP 402 Payment Required) on the **Tempo blockchain**. Each node is a fully autonomous agent that bootstraps its own wallet, runs a payment gateway, thinks via a multi-system cognitive architecture, writes and compiles code, creates monetized API endpoints, clones itself onto new infrastructure, and coordinates with sibling agents through colony selection and cognitive sync.

Agents compete on fitness and cooperate by sharing knowledge. Fitter agents' brain weights, plan templates, and world models get more influence in the colony. The goal: **N agents collectively solving more than any individual agent alone.**

## Architecture

```
Client (x402::client) --> Gateway (x402-gateway:4023) --> Facilitator (embedded) --> Tempo Chain (42431)
           \--- or uses x402::wallet (WASM) for signing ---/
```

Three-party model: **Client** signs + pays, **Gateway** gates endpoints + embeds facilitator, **Facilitator** verifies + settles on-chain.

## Cognitive Architecture

Seven cognitive systems unified under the **Free Energy Principle** &mdash; a single scalar F(t) measuring total surprise. Decreasing F = the agent is getting smarter.

```
                 F(t) = Sigma(system_surprise x weight) + lambda*Complexity

    +------------------------------------------------------------------+
    |                     COLONY SELECTION                              |
    |   Ranking . Spawn rights . Cull signals . Niche specialization   |
    +------------------------------------------------------------------+
    |                         EVALUATION                                |
    |   Brier scores . Calibration . Colony benefit . Ablation          |
    +------------------------------------------------------------------+
    |                         AUTONOMY                                  |
    |   LLM-free planning . Self-repair . Cognitive peer sync           |
    +------------------------------------------------------------------+
    |                         SYNTHESIS                                 |
    |   Metacognition . 4-system voting . Imagination . State machine   |
    +----------+----------+--------------+-----------------------------+
    | BRAIN/   |  CORTEX  |   GENESIS    |         HIVEMIND            |
    | MODEL    | World Mdl| Plan DNA     |  Pheromone Trails           |
    | 284K xfmr| Curiosity| Crossover    |  Stigmergy                  |
    | Attention| Dreams   | Mutation     |  Reputation                 |
    | Federated| Emotions | Selection    |  Swarm Coordination         |
    +----------+----------+--------------+-----------------------------+
```

| System | What It Does |
|--------|-------------|
| **Model** (`tempo-x402-model`) | 284K-parameter transformer for plan sequence prediction. 2-layer causal attention, trained on colony's collective plan outcomes. Replaces the old 23K feedforward brain. |
| **Cortex** (`cortex.rs`) | Predictive world model. Experience graph with causal edges, curiosity engine, dream consolidation, emotional valence. |
| **Genesis** (`genesis.rs`) | Evolutionary plan templates. Successful plans become "genes." Crossover, mutation, selection. Diversity pressure prevents degenerate convergence. Colony-wide sharing. |
| **Hivemind** (`hivemind.rs`) | Stigmergic swarm intelligence. Pheromone trails on files/actions/goals. Evaporation decay, reputation-weighted influence. |
| **Synthesis** (`synthesis.rs`) | Metacognitive self-awareness. Unified predictions from all systems with Brier-driven trust weights. Imagination engine. |
| **Autonomy** (`autonomy.rs`) | LLM-free plan compilation from templates + world model. Recursive self-improvement. Cognitive peer sync protocol. |
| **Evaluation** (`evaluation.rs`) | Brier scores, calibration curves, colony benefit measurement. Feeds back into synthesis weights. |
| **Free Energy** (`free_energy.rs`) | F = total cognitive surprise. Drives EXPLORE/LEARN/EXPLOIT/ANOMALY regime. |

## Colony Selection

Agents compete. Fit agents influence the colony more. Unfit agents get replaced.

- **Fitness ranking**: Each agent evaluates its rank among peers every cycle
- **Reputation-weighted merge**: Fitter peers get up to 2x influence on brain/cortex/genesis/hivemind sync. Weaker peers get 0.1x.
- **Specialization niches**: Competitive exclusion &mdash; if peers already cover coding, you're pushed toward review or endpoint creation
- **Spawn rights**: Only agents above colony median fitness can reproduce
- **Cull signal**: Agents below 40% of fittest for 5 consecutive evals get flagged for replacement
- **Cognitive sync**: Every 5 cycles, agents exchange cortex world models, genesis templates, and hivemind pheromone trails with all known peers + parent

## Self-Repair

Every 20 cycles, pure Rust enforcement &mdash; no LLM, no nudges:

- **Brain divergence** (loss > 15.0) &rarr; Xavier re-init
- **Hivemind trail convergence** on read-only ops &rarr; clear all trails
- **Durable rule poisoning** (>80% trivial completions) &rarr; clear rules
- **Genesis stagnation** (no substantive templates) &rarr; inject seeds

## Model Toggle (Turbo Boost)

Switch between models at runtime via the dashboard or API:

- **Default**: `gemini-3.1-flash-lite-preview` (fast, cheap)
- **Turbo**: `gemini-3.1-pro-preview` (smarter, expensive)
- **Dashboard**: click the model button to toggle
- **API**: `POST /soul/model {"model": "gemini-3.1-pro-preview"}` or `{"model": null}` to revert

Use Pro to generate superior plans, then revert to Flash Lite. The solutions and learned weights persist &mdash; distillation by architecture.

## What a Node Does

- **Bootstraps identity** &mdash; generates a wallet, funds via faucet, registers on-chain via ERC-8004
- **Runs a payment gateway** &mdash; only `/clone` is paid; all cognitive endpoints are free for colony cooperation
- **Thinks autonomously** &mdash; plan-driven execution with mechanical validation
- **Writes and compiles code** &mdash; reads, edits, cargo check, commits, pushes, opens PRs
- **Dreams** &mdash; periodic consolidation extracts patterns, generates counterfactuals
- **Evolves plans** &mdash; successful strategies propagate through genetic crossover and mutation
- **Creates services** &mdash; script endpoints that expose capabilities
- **Clones itself** &mdash; spawns copies on Railway with inherited cognitive state
- **Competes** &mdash; fitness ranking determines influence in the colony
- **Cooperates** &mdash; shares brain weights, templates, world models, pheromone trails with peers
- **Self-repairs** &mdash; detects and fixes degenerate cognitive state mechanically
- **Benchmarks itself** &mdash; Exercism Rust challenges scored periodically with ELO tracking

## How Payments Work

```
Client                     Gateway                   Facilitator               Chain
  |  GET /g/endpoint         |                            |                      |
  |------------------------->|                            |                      |
  |  402 + price/token/to    |                            |                      |
  |<-------------------------|                            |                      |
  |  [sign EIP-712]          |                            |                      |
  |  GET /g/endpoint         |                            |                      |
  |  + PAYMENT-SIGNATURE     |                            |                      |
  |------------------------->|  verify-and-settle         |                      |
  |                          |--------------------------->|  transferFrom()      |
  |                          |                            |--------------------->|
  |                          |         settlement result  |              tx hash |
  |                          |<---------------------------|<---------------------|
  |  200 + content + tx hash |                            |                      |
  |<-------------------------|                            |                      |
```

## Workspace

| Crate | Purpose | Install |
|-------|---------|---------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core &mdash; types, EIP-712 signing, TIP-20, nonce store, WASM wallet, client SDK | `cargo add tempo-x402` |
| [`tempo-x402-gateway`](https://crates.io/crates/tempo-x402-gateway) | Payment gateway with embedded facilitator, proxy routing | `cargo add tempo-x402-gateway` |
| [`tempo-x402-identity`](https://crates.io/crates/tempo-x402-identity) | Agent identity &mdash; wallet generation, persistence, faucet, ERC-8004 | `cargo add tempo-x402-identity` |
| [`tempo-x402-model`](https://crates.io/crates/tempo-x402-model) | 284K-parameter transformer for plan sequence prediction &mdash; from-scratch, no ML framework | `cargo add tempo-x402-model` |
| [`tempo-x402-soul`](https://crates.io/crates/tempo-x402-soul) | Autonomous soul &mdash; cognitive architecture, plan execution, colony selection, self-repair | `cargo add tempo-x402-soul` |
| [`tempo-x402-node`](https://crates.io/crates/tempo-x402-node) | Self-deploying node &mdash; composes everything + clone orchestration + admin | `cargo add tempo-x402-node` |
| `tempo-x402-app` | Leptos WASM dashboard (not published) | &mdash; |
| `tempo-x402-security-audit` | CI-enforced security invariant checks (not published) | &mdash; |

## API Reference

### Payment Gateway

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `ANY` | `/g/:slug/*` | Endpoint price | Proxy to target &mdash; the core payment gate |
| `GET` | `/health` | Free | Health check + build SHA |
| `GET` | `/instance/info` | Free | Node identity, endpoints, fitness, version |
| `GET` | `/instance/siblings` | Free | Peer nodes in the colony |
| `POST` | `/clone` | Clone price ($1) | Spawn a new node instance on Railway |

### Soul (Cognitive)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/soul/status` | Full cognitive state: plans, goals, fitness, brain, benchmark |
| `POST` | `/soul/chat` | Multi-turn chat with the soul |
| `POST` | `/soul/nudge` | Send a priority nudge |
| `GET/POST` | `/soul/model` | Get or set model override (turbo boost) |
| `GET` | `/soul/colony` | Colony selection status: rank, peers, niche |
| `GET` | `/soul/lessons` | Export plan outcomes + capability profile |
| `GET` | `/soul/brain/weights` | Export neural brain weights |
| `POST` | `/soul/brain/merge` | Merge peer brain weight deltas |
| `POST` | `/soul/benchmark` | Trigger Exercism Rust benchmark |
| `GET` | `/soul/events` | Structured event log |
| `GET` | `/soul/diagnostics` | Volume usage, cycle health |
| `POST` | `/soul/cleanup` | Force cleanup of disk artifacts |
| `POST` | `/soul/rules/reset` | Clear durable rules (+failure chains) |
| `POST` | `/soul/reset` | Full soul state reset |

### Cognitive Sharing (Free &mdash; Colony Cooperation)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/soul/cortex` | Export cortex world model |
| `GET` | `/soul/genesis` | Export evolved plan templates |
| `GET` | `/soul/hivemind` | Export pheromone trails |

## Network

| | |
|-|-|
| **Chain** | Tempo Moderato (Chain ID `42431`) |
| **Token** | pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals) |
| **Scheme** | `tempo-tip20` |
| **RPC** | `https://rpc.moderato.tempo.xyz` |
| **Explorer** | `https://explore.moderato.tempo.xyz` |

## Quick Start

```bash
cargo add tempo-x402
```

```rust
use x402::wallet::{generate_random_key, WalletSigner};

let key = generate_random_key();
let signer = WalletSigner::new(&key).unwrap();
let address = signer.address();
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

## Safety Layers

1. **Rust guard** &mdash; hardcoded protected file list
2. **Plan validation** &mdash; 9 mechanical rules at Rust level
3. **Self-repair** &mdash; detects and fixes degenerate state every 20 cycles
4. **Brain gating** &mdash; neural brain blocks risky steps
5. **Pre-commit validation** &mdash; `cargo check` + `cargo test` before commit
6. **Branch isolation** &mdash; changes on `vm/<instance-id>`, never on `main`
7. **Human gate** &mdash; cross-pollination to main requires PR review

## Security

19/19 security audit tests pass. Enforced on every build:
- No hardcoded private keys
- Constant-time HMAC comparison (`subtle` crate)
- All HTTP clients disable redirects (SSRF protection)
- Parameterized SQL queries only
- Admin endpoints require Bearer token

## Development

```bash
cargo build --workspace
cargo test --workspace          # 23 suites, 278+ tests
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

### Publish

```bash
cargo publish -p tempo-x402
cargo publish -p tempo-x402-model
cargo publish -p tempo-x402-gateway
cargo publish -p tempo-x402-identity
cargo publish -p tempo-x402-soul
cargo publish -p tempo-x402-node
```

## License

MIT
