# tempo-x402

Pay-per-request APIs on the Tempo blockchain. Security-hardened.

**[Live Demo](https://tempo-x402-app.vercel.app)** | **[Documentation](https://tempo-x402-app.vercel.app/docs)** | **[crates.io](https://crates.io/crates/tempo-x402)**

[![Deploy on Railway](https://railway.com/button.svg)](https://railway.com/template/tempo-x402?referralCode=tempo)

## How It Works

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
    │  + PAYMENT-      │                     │                    │
    │    SIGNATURE hdr │                     │                    │
    │─────────────────>│                     │                    │
    │                  │                     │                    │
    │                  │  verify + settle    │                    │
    │                  │────────────────────>│                    │
    │                  │                     │  transferFrom()    │
    │                  │                     │───────────────────>│
    │                  │                     │<───────────────────│
    │                  │<────────────────────│                    │
    │                  │                     │                    │
    │  200 + content   │                     │                    │
    │<─────────────────│                     │                    │
```

**Client** requests a paid endpoint. **Server** returns 402 with price info. Client signs an EIP-712 payment authorization and retries. Server forwards to **Facilitator**, which verifies the signature and calls `transferFrom` on-chain. Server returns the content.

## Architecture

| Component | What it does | Crate |
|-----------|--------------|-------|
| **Client** | Signs payments, makes requests | `tempo-x402` |
| **Server** | Gates endpoints, returns 402 | `tempo-x402-server` |
| **Facilitator** | Verifies signatures, settles on-chain | `tempo-x402-facilitator` |
| **Gateway** | Proxy any API with payment rails | `tempo-x402-gateway` |
| **Wallet** | WASM-compatible signing & key generation | `tempo-x402-wallet` |

The facilitator holds no funds — it just has approval to call `transferFrom` on behalf of clients who have pre-approved it.

## Install

```bash
cargo add tempo-x402
```

## Usage

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

    println!("{}", response.text().await.unwrap());
    if let Some(s) = settlement {
        println!("tx: {}", s.transaction);
    }
}
```

## Crates

| Crate | Purpose |
|-------|---------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core library — types, signing, HTTP client |
| [`tempo-x402-server`](https://crates.io/crates/tempo-x402-server) | Resource server with payment middleware |
| [`tempo-x402-facilitator`](https://crates.io/crates/tempo-x402-facilitator) | Payment verification and settlement |
| [`tempo-x402-gateway`](https://crates.io/crates/tempo-x402-gateway) | API relay/proxy with endpoint registration |
| [`tempo-x402-wallet`](https://crates.io/crates/tempo-x402-wallet) | WASM-compatible wallet — key generation, EIP-712 signing |

## Deployed Services

| Service | URL |
|---------|-----|
| Demo | https://tempo-x402-app.vercel.app |
| Server | https://x402-server-production.up.railway.app |
| Facilitator | https://x402-facilitator-production-ec87.up.railway.app |
| Gateway | https://x402-gateway-production-5018.up.railway.app |

## Network

- **Chain**: Tempo Moderato (testnet)
- **Chain ID**: 42431
- **Token**: pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals)
- **RPC**: https://rpc.moderato.tempo.xyz

## License

MIT
