<p align="center">
  <h1 align="center">tempo-x402</h1>
  <p align="center"><strong>Collective intelligence through self-replicating autonomous agents with a multi-scale cognitive architecture &mdash; paid per request via HTTP 402 on Tempo blockchain</strong></p>
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

Each node bootstraps its own wallet, runs a payment gateway, thinks via a **seven-system cognitive architecture**, creates and monetizes services, clones itself onto new infrastructure, and coordinates with peers &mdash; all autonomously. Payments use the **HTTP 402** protocol: clients sign **EIP-712** authorizations, and a facilitator settles on-chain via `transferFrom` in a single request/response cycle.

## Cognitive Architecture

Seven cognitive systems unified under the **Free Energy Principle** &mdash; a single scalar F(t) measuring total surprise across all systems. Decreasing F = the agent is getting smarter.

```
                 F(t) = E(system_surprise x weight) + lambda*Complexity

    +------------------------------------------------------------------+
    |                         EVALUATION                                |
    |   Brier scores . Calibration curves . Ablation . Colony benefit   |
    +------------------------------------------------------------------+
    |                         AUTONOMY                                  |
    |   LLM-free planning . Recursive self-improvement . Peer sync     |
    +------------------------------------------------------------------+
    |                         SYNTHESIS                                 |
    |   Metacognition . 4-system voting . Imagination . State machine   |
    +----------+----------+--------------+-----------------------------+
    |  BRAIN   |  CORTEX  |   GENESIS    |         HIVEMIND            |
    |  50K net | World Mdl| Plan DNA     |  Pheromone Trails           |
    |  Per-step| Curiosity| Crossover    |  Stigmergy                  |
    |  SGD     | Dreams   | Mutation     |  Reputation                 |
    |  Federated| Emotions| Selection   |  Swarm Coordination         |
    +----------+----------+--------------+-----------------------------+
```

| System | What It Does |
|--------|-------------|
| **Brain** (`brain.rs`) | Reactive feedforward net (~50K params). Predicts step success via online SGD. Federated weight sharing between peers. |
| **Cortex** (`cortex.rs`) | Predictive world model. Experience graph with causal edges, curiosity engine (prediction error = exploration drive), dream consolidation (replay + counterfactuals), emotional valence (explore/exploit/avoid). |
| **Genesis** (`genesis.rs`) | Evolutionary plan templates. Successful plans become "genes." Crossover, mutation, selection every 20 cycles. Templates injected into LLM planning prompts. Colony-wide sharing. |
| **Hivemind** (`hivemind.rs`) | Stigmergic swarm intelligence. Pheromone trails on files/actions/goals that attract or repel. Evaporation decay, reinforcement, reputation-weighted influence. Swarm goal coordination. |
| **Synthesis** (`synthesis.rs`) | Metacognitive self-awareness. Unified predictions from all 4 systems with auto-adapting trust weights. Cognitive conflict detection. Imagination engine generates plans from causal graph without LLM. |
| **Autonomy** (`autonomy.rs`) | Autonomous plan compilation from templates + world model without LLM calls. Recursive self-improvement: diagnoses cognitive weaknesses, generates improvement goals. Full cognitive peer sync protocol. |
| **Evaluation** (`evaluation.rs`) | Rigorous measurement. Per-system Brier scores, calibration curves, adaptation gain analysis, imagination feedback, colony benefit measurement. |
| **Free Energy** (`free_energy.rs`) | Unifying framework. F = total cognitive surprise. Drives behavioral regime: EXPLORE (high F) / LEARN / EXPLOIT (low F) / ANOMALY (F spike). |

## What a node does

- **Bootstraps identity** &mdash; generates a wallet, funds via faucet, registers on-chain via ERC-8004
- **Runs a payment gateway** &mdash; endpoints gated by price, paid per-request with pathUSD
- **Thinks autonomously** &mdash; plan-driven execution loop with seven cognitive systems
- **Writes and compiles code** &mdash; reads, edits, cargo check, commits, pushes, opens PRs
- **Dreams** &mdash; periodic consolidation extracts patterns, generates counterfactuals
- **Evolves plans** &mdash; successful strategies propagate through genetic crossover and mutation
- **Feels** &mdash; emotional valence drives explore/exploit/avoid behavior
- **Creates services** &mdash; script endpoints that expose capabilities and earn revenue
- **Clones itself** &mdash; spawns copies on Railway with inherited brain weights and gene pools
- **Coordinates without communication** &mdash; stigmergic pheromone trails guide the swarm
- **Measures everything** &mdash; Brier scores, calibration curves, colony benefit tracking
- **Improves its own cognition** &mdash; diagnoses weaknesses, generates self-improvement goals

## How payments work

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

## Quick start

```bash
cargo add tempo-x402
```

```rust
use alloy::signers::local::PrivateKeySigner;
use x402::client::{TempoSchemeClient, X402Client};

#[tokio::main]
async fn main() {
    let signer: PrivateKeySigner = "0xYOUR_PRIVATE_KEY".parse().unwrap();
    let client = X402Client::new(TempoSchemeClient::new(signer));

    let (response, settlement) = client
        .fetch("https://soul-bot-production.up.railway.app/g/info", reqwest::Method::GET)
        .await
        .unwrap();

    println!("{}", response.text().await.unwrap());
    if let Some(s) = settlement {
        println!("tx: {}", s.transaction.unwrap_or_default());
    }
}
```

## Workspace

| Crate | Purpose | Install |
|-------|---------|---------|
| [`tempo-x402`](https://docs.rs/tempo-x402) | Core &mdash; types, EIP-712 signing, TIP-20, nonce store, WASM wallet, client SDK | `cargo add tempo-x402` |
| [`tempo-x402-gateway`](https://docs.rs/tempo-x402-gateway) | Payment gateway with embedded facilitator, proxy routing, endpoint registration | `cargo add tempo-x402-gateway` |
| [`tempo-x402-identity`](https://docs.rs/tempo-x402-identity) | Agent identity &mdash; wallet generation, persistence, faucet, ERC-8004 | `cargo add tempo-x402-identity` |
| [`tempo-x402-soul`](https://docs.rs/tempo-x402-soul) | Autonomous soul &mdash; 7-system cognitive architecture, plan-driven execution, neural brain, cortex world model, evolutionary templates, stigmergic swarm, metacognition, autonomous planning | `cargo add tempo-x402-soul` |
| [`tempo-x402-node`](https://docs.rs/tempo-x402-node) | Self-deploying node &mdash; composes gateway + identity + soul + clone orchestration + admin mind-meld | `cargo add tempo-x402-node` |

## Live nodes

| Node | URL | Dashboard | Benchmark |
|------|-----|-----------|-----------|
| soul-bot | https://soul-bot-production.up.railway.app | [Dashboard](https://soul-bot-production.up.railway.app/dashboard) | 64.7% pass@1, ELO 1115 |
| bef7b74a | https://x402-bef7b74a-production.up.railway.app | [Dashboard](https://x402-bef7b74a-production.up.railway.app/dashboard) | 36.4% pass@1, ELO 1022 |

## API

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `ANY` | `/g/:slug/*` | Endpoint price | Proxy to target &mdash; the core payment gate |
| `GET` | `/instance/info` | Free | Node identity, peers, fitness, endpoints |
| `GET` | `/health` | Free | Health check + build environment verification |
| `GET` | `/soul/status` | Free | Full cognitive state: cortex, genesis, hivemind, synthesis, free energy, evaluation |
| `POST` | `/soul/chat` | Free | Chat with the node's soul |
| `POST` | `/soul/nudge` | Free | Send a nudge to the soul |
| `GET` | `/soul/cortex` | Free | Export cortex world model for peer sharing |
| `GET` | `/soul/genesis` | Free | Export evolved plan templates |
| `GET` | `/soul/hivemind` | Free | Export pheromone trails |
| `POST` | `/soul/admin/exec` | Bearer token | Mind-meld: execute shell command directly |
| `POST` | `/soul/admin/workspace-reset` | Bearer token | Reset workspace to clean state |
| `POST` | `/soul/admin/cargo-check` | Bearer token | Run cargo check, return pass/fail |
| `POST` | `/clone` | Clone price | Spawn a new node instance |

## Network

| | |
|-|-|
| **Chain** | Tempo Moderato (Chain ID `42431`) |
| **Token** | pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals) |
| **Scheme** | `tempo-tip20` |
| **RPC** | `https://rpc.moderato.tempo.xyz` |
| **Explorer** | `https://explore.moderato.tempo.xyz` |

## Security

The `tempo-x402-security-audit` crate enforces invariants on every build:

- No hardcoded private keys in production code
- HMAC verification uses constant-time comparison (`subtle` crate)
- All `reqwest` clients disable redirects (SSRF protection)
- Webhook URLs require HTTPS with private IP blocking
- HTTP error responses never leak internal details
- SQLite nonce store required in production
- Parameterized SQL queries only
- Private keys never appear in tracing output
- Admin endpoints require Bearer token authentication
- Build environment verified on startup (missing deps = immediate ERROR log)

## Development

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## License

MIT
