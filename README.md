# tempo-x402

[![crates.io](https://img.shields.io/crates/v/tempo-x402.svg)](https://crates.io/crates/tempo-x402)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

x402 (HTTP 402 Payment Required) implementation for the [Tempo](https://tempo.xyz) blockchain. Pay-per-request API monetization using TIP-20 tokens (pathUSD) with EIP-712 signed payment authorizations.

## How it works

```
Client ──GET /resource──> Server ──402 + price──> Client
Client ──sign EIP-712───> Client
Client ──GET + X-PAYMENT──> Server ──verify+settle──> Facilitator ──transferFrom──> Tempo Chain
Server <──200 + data──────
```

1. Client requests a protected endpoint
2. Server responds **402** with payment requirements (price, token, recipient)
3. Client signs an EIP-712 `PaymentAuthorization` and retries with `X-PAYMENT` header
4. Server forwards to the facilitator, which atomically verifies the signature and executes `transferFrom` on-chain
5. Server returns the content + transaction hash

## Crates

| Crate | What it does |
|-------|-------------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core library: types, EIP-712, TIP-20, nonce store, HTTP client |
| [`tempo-x402-server`](https://crates.io/crates/tempo-x402-server) | Resource server + payment middleware (actix-web) |
| [`tempo-x402-facilitator`](https://crates.io/crates/tempo-x402-facilitator) | Payment verification + on-chain settlement server |

## Quick start

```bash
# 1. Clone and build
git clone https://github.com/compusophy/tempo-x402.git
cd tempo-x402
cargo build --workspace

# 2. Configure
cp .env.example .env
# Edit .env with your keys (see below)

# 3. Approve the facilitator to spend your tokens
cargo run --bin x402-approve

# 4. Start the facilitator (terminal 1)
cargo run --bin x402-facilitator

# 5. Start the server (terminal 2)
cargo run --bin x402-server

# 6. Make a paid request (terminal 3)
cargo run --bin x402-client
```

## Configuration

Copy `.env.example` to `.env` and fill in:

| Variable | Required | Description |
|----------|----------|-------------|
| `EVM_PRIVATE_KEY` | Yes | Client wallet private key (pays for requests) |
| `EVM_ADDRESS` | Yes | Server wallet address (receives payments) |
| `FACILITATOR_PRIVATE_KEY` | Yes | Facilitator wallet key (executes `transferFrom`) |
| `FACILITATOR_ADDRESS` | Yes | Facilitator address (for token approval) |

Fund wallets via the Tempo faucet:
```bash
cast rpc tempo_fundAddress 0xYOUR_ADDRESS --rpc-url https://rpc.moderato.tempo.xyz
```

Optional settings: `FACILITATOR_SHARED_SECRET`, `ALLOWED_ORIGINS`, `RATE_LIMIT_RPM` -- see `.env.example` for details.

## Network

- **Chain**: Tempo Moderato (testnet)
- **Chain ID**: 42431
- **Token**: pathUSD (`0x20c0000000000000000000000000000000000000`, 6 decimals)
- **RPC**: `https://rpc.moderato.tempo.xyz`
- **Explorer**: `https://explore.moderato.tempo.xyz`

## Security

- Nonce replay protection with automatic expiry
- HMAC authentication between server and facilitator
- Atomic verify-and-settle with per-payer mutex (no TOCTOU)
- Configurable CORS and rate limiting
- Request body size limits (64KB)
- Error sanitization (internal details logged, not exposed to clients)

## License

MIT
