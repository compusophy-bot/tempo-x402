<p align="center">
  <h1 align="center">tempo-x402</h1>
  <p align="center"><strong>Collective intelligence through self-replicating autonomous agents — paid per request via HTTP 402 on Tempo blockchain</strong></p>
</p>

<p align="center">
  <a href="https://crates.io/crates/tempo-x402"><img src="https://img.shields.io/crates/v/tempo-x402.svg" alt="crates.io"></a>
  <a href="https://docs.rs/tempo-x402"><img src="https://docs.rs/tempo-x402/badge.svg" alt="docs.rs"></a>
  <a href="https://github.com/compusophy/tempo-x402/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
</p>

<p align="center">
  <a href="https://docs.rs/tempo-x402">Docs</a> &middot;
  <a href="https://crates.io/crates/tempo-x402">Crate</a> &middot;
  <a href="https://soul-bot-production.up.railway.app">Live Node</a> &middot;
  <a href="https://github.com/compusophy/tempo-x402">Source</a>
</p>

---

Each node bootstraps its own wallet, runs a payment gateway, thinks via an LLM-powered soul, creates and monetizes services, clones itself onto new infrastructure, and coordinates with peers &mdash; all autonomously. Payments use the **HTTP 402** protocol: clients sign **EIP-712** authorizations, and a facilitator settles on-chain via `transferFrom` in a single request/response cycle. No custody. No middlemen.

## What a node does

- **Bootstraps identity** &mdash; generates a wallet, funds itself via faucet, registers on-chain via ERC-8004
- **Runs a payment gateway** &mdash; endpoints are gated by price, paid per-request with pathUSD
- **Thinks autonomously** &mdash; plan-driven execution loop powered by Gemini with neuroplastic memory
- **Writes code** &mdash; reads, writes, edits files, runs shell commands, commits, pushes, opens PRs
- **Creates services** &mdash; script endpoints that expose capabilities and earn revenue
- **Clones itself** &mdash; spawns copies on Railway infrastructure via a paid `/clone` endpoint
- **Coordinates with peers** &mdash; discovers siblings, exchanges brain weights and lessons, calls paid endpoints
- **Evolves via fitness** &mdash; 5-component fitness score (economic, execution, evolution, coordination, introspection) with trend gradient

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

1. Client requests a gated endpoint &rarr; gets **402** with pricing
2. Client signs an **EIP-712 `PaymentAuthorization`**, retries with `PAYMENT-SIGNATURE` header
3. Facilitator atomically verifies signature, checks balance/allowance/nonce, calls `transferFrom`
4. Gateway returns content + transaction hash

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
| [`tempo-x402-soul`](https://docs.rs/tempo-x402-soul) | Autonomous soul &mdash; plan-driven execution, neural brain, Gemini-powered coding agent | `cargo add tempo-x402-soul` |
| [`tempo-x402-node`](https://docs.rs/tempo-x402-node) | Self-deploying node &mdash; composes gateway + identity + soul + clone orchestration | `cargo add tempo-x402-node` |

### Feature flags

| Crate | Flag | Description |
|-------|------|-------------|
| `tempo-x402` | `full` (default) | All features: async runtime, SQLite, HTTP client |
| `tempo-x402` | `wasm` | WASM-compatible subset: types, EIP-712, wallet |
| `tempo-x402` | `demo` | Demo private key for testing |
| `tempo-x402-identity` | `erc8004` (default) | On-chain agent identity via ERC-8004 |
| `tempo-x402-node` | `soul` (default) | Autonomous thinking loop |
| `tempo-x402-node` | `agent` (default) | Railway clone orchestration |

## API

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `ANY` | `/g/:slug/*` | Endpoint price | Proxy to target &mdash; the core payment gate |
| `GET` | `/instance/info` | Free | Node identity, peers, fitness, endpoints |
| `POST` | `/instance/link` | Free | Link an independent peer node |
| `DELETE` | `/instance/peer/:id` | Bearer token | Remove a peer |
| `GET` | `/endpoints` | Free | List all active endpoints |
| `GET` | `/analytics` | Free | Per-endpoint payment stats |
| `GET` | `/soul/status` | Free | Soul status, active plan, recent thoughts |
| `POST` | `/soul/chat` | Free | Chat with the node's soul |
| `POST` | `/soul/nudge` | Free | Send a nudge to the soul |
| `POST` | `/clone` | Clone price | Spawn a new node instance |
| `GET` | `/health` | Free | Health check |
| `GET` | `/metrics` | Bearer token | Prometheus metrics |

## Network

| | |
|-|-|
| **Chain** | Tempo Moderato (Chain ID `42431`) |
| **Token** | pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals) |
| **Scheme** | `tempo-tip20` |
| **RPC** | `https://rpc.moderato.tempo.xyz` |
| **Explorer** | `https://explore.moderato.tempo.xyz` |

## Live nodes

| Node | URL | Dashboard |
|------|-----|-----------|
| soul-bot | https://soul-bot-production.up.railway.app | [Dashboard](https://soul-bot-production.up.railway.app/dashboard) |
| soul-bot-2 | https://soul-bot-2-production.up.railway.app | [Dashboard](https://soul-bot-2-production.up.railway.app/dashboard) |

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

Additional hardening: EIP-2 high-s rejection, per-payer mutex locks against TOCTOU, nonces claimed before `transferFrom` (never released on failure), integer-only token arithmetic, atomic slug reservation.

## Development

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## License

MIT
