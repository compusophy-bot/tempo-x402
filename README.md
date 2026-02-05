# tempo-x402

[![crates.io](https://img.shields.io/crates/v/tempo-x402.svg)](https://crates.io/crates/tempo-x402)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Pay-per-request APIs on the [Tempo](https://tempo.xyz) blockchain. Clients sign EIP-712 payment authorizations, servers gate content behind HTTP 402, facilitators settle payments on-chain.

**[Live Demo](https://tempo-x402-demo.vercel.app)** — try it now, no setup required

## The Flow

```
┌────────┐         ┌────────┐         ┌─────────────┐         ┌───────┐
│ Client │         │ Server │         │ Facilitator │         │ Chain │
└───┬────┘         └───┬────┘         └──────┬──────┘         └───┬───┘
    │                  │                     │                    │
    │  GET /resource   │                     │                    │
    │─────────────────>│                     │                    │
    │                  │                     │                    │
    │  402 + payment   │                     │                    │
    │  requirements    │                     │                    │
    │<─────────────────│                     │                    │
    │                  │                     │                    │
    │  [sign EIP-712]  │                     │                    │
    │                  │                     │                    │
    │  GET /resource   │                     │                    │
    │  + X-PAYMENT hdr │                     │                    │
    │─────────────────>│                     │                    │
    │                  │                     │                    │
    │                  │  POST /verify-and-  │                    │
    │                  │  settle             │                    │
    │                  │────────────────────>│                    │
    │                  │                     │                    │
    │                  │                     │  transferFrom()    │
    │                  │                     │───────────────────>│
    │                  │                     │                    │
    │                  │                     │  tx hash           │
    │                  │                     │<───────────────────│
    │                  │                     │                    │
    │                  │  settlement result  │                    │
    │                  │<────────────────────│                    │
    │                  │                     │                    │
    │  200 + content   │                     │                    │
    │  + tx hash       │                     │                    │
    │<─────────────────│                     │                    │
    │                  │                     │                    │
```

1. Client requests a protected endpoint
2. Server returns **402** with price, token address, and recipient
3. Client signs an EIP-712 `PaymentAuthorization` and retries with `X-PAYMENT` header
4. Server forwards to facilitator for verification and on-chain settlement
5. Facilitator calls `transferFrom` to move tokens from client to server
6. Server returns the content plus the settlement transaction hash

## Crates

| Crate | Purpose |
|-------|---------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core library — types, EIP-712, TIP-20, HTTP client |
| [`tempo-x402-server`](https://crates.io/crates/tempo-x402-server) | Resource server with payment middleware |
| [`tempo-x402-facilitator`](https://crates.io/crates/tempo-x402-facilitator) | Payment verification and on-chain settlement |

## Quick Start

```bash
# Add to your Cargo.toml
cargo add tempo-x402
```

```rust
use alloy::signers::local::PrivateKeySigner;
use x402::{TempoSchemeClient, X402Client};

#[tokio::main]
async fn main() {
    let signer: PrivateKeySigner = "0xYOUR_KEY".parse().unwrap();
    let client = X402Client::new(TempoSchemeClient::new(signer));

    let (response, settlement) = client
        .fetch("https://api.example.com/paid-endpoint", reqwest::Method::GET)
        .await
        .unwrap();

    println!("Data: {}", response.text().await.unwrap());
    if let Some(s) = settlement {
        println!("Paid: {}", s.transaction);
    }
}
```

## Network

| | |
|---|---|
| Chain | Tempo Moderato (testnet) |
| Chain ID | 42431 |
| Token | pathUSD — `0x20c0000000000000000000000000000000000000` (6 decimals) |
| RPC | https://rpc.moderato.tempo.xyz |
| Explorer | https://explore.moderato.tempo.xyz |

Fund your wallet:
```bash
cast rpc tempo_fundAddress 0xYOUR_ADDRESS --rpc-url https://rpc.moderato.tempo.xyz
```

## Deployed Services

| Service | URL |
|---------|-----|
| Demo | https://tempo-x402-demo.vercel.app |
| Server | https://x402-server-production.up.railway.app |
| Facilitator | https://x402-facilitator-production-ec87.up.railway.app |

## Documentation

- [llms.txt](./llms.txt) — complete API reference for LLM integrations
- [Demo source](https://github.com/compusophy/tempo-x402-demo) — Next.js frontend example

## License

MIT
