# tempo-x402

Pay-per-request APIs on the Tempo blockchain.

**[Live Demo](https://tempo-x402-demo.vercel.app)** · **[Documentation](https://tempo-x402-demo.vercel.app/docs)** · **[crates.io](https://crates.io/crates/tempo-x402)**

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
    │  + X-PAYMENT hdr │                     │                    │
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

**Client** requests a paid endpoint. **Server** returns 402 with price info. Client signs an EIP-712 payment authorization and retries. Server forwards to **Facilitator**, which verifies the signature and calls `transferFrom` on-chain to settle the payment. Server returns the content.

## Architecture

Three services, one chain:

| Component | What it does | Stack |
|-----------|--------------|-------|
| **Client** | Signs payments, makes requests | Rust (`tempo-x402` crate) or any language with EIP-712 |
| **Server** | Gates endpoints, returns 402, forwards payments | Rust, actix-web |
| **Facilitator** | Verifies signatures, settles on-chain | Rust, actix-web, alloy |
| **Chain** | Tempo Moderato (testnet), pathUSD token | EVM, TIP-20 |

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

## Network

- **Chain**: Tempo Moderato (testnet)
- **Chain ID**: 42431
- **Token**: pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals)
- **RPC**: https://rpc.moderato.tempo.xyz

## License

MIT
