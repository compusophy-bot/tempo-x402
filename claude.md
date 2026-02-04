# x402 on Tempo - Project Context

## What This Is

A custom x402 (HTTP 402 Payment Required) implementation for the **Tempo blockchain** using TIP-20 tokens (pathUSD). This enables pay-per-request API monetization where clients sign EIP-712 payment authorizations and a facilitator settles them on-chain.

**This is a Rust implementation** organized as a Cargo workspace under `rust/`.

## Architecture

```
Client (x402-client) --> Resource Server (x402-server:4021) --> Facilitator (x402-facilitator:4022) --> Tempo Blockchain (42431)
```

### Three-party model:
1. **Client** - Signs EIP-712 PaymentAuthorization, pays for API access
2. **Resource Server** - Gates endpoints behind payment, returns 402 if unpaid
3. **Facilitator** - Verifies signatures, checks balances/allowances, executes `transferFrom`

## Workspace Layout

```
rust/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── x402-types/         # Core types, constants, traits, HMAC utilities
│   ├── x402-tempo/         # Tempo-specific EIP-712, TIP-20, scheme impls
│   ├── x402-facilitator/   # Facilitator HTTP server (actix-web)
│   ├── x402-server/        # Resource server + reusable payment middleware
│   ├── x402-client/        # CLI client + approve script
│   └── x402-frontend/      # WASM frontend (Trunk, excluded from workspace)
```

## Key Crates

| Crate | Purpose | Port |
|-------|---------|------|
| `x402-types` | Core types, payment structs, traits, HMAC auth | - |
| `x402-tempo` | EIP-712 signing/verification, TIP-20 calls, nonce tracking | - |
| `x402-server` | Resource server, payment middleware, serves frontend | 4021 |
| `x402-facilitator` | Payment verification + on-chain settlement | 4022 |
| `x402-client` | CLI client with wrapped fetch + approve script | CLI |
| `x402-frontend` | Interactive docs + live demo UI (WASM) | served |

## Blockchain Details

- **Chain**: Tempo Moderato (Chain ID `42431`)
- **Network ID (CAIP-2)**: `eip155:42431`
- **Token**: pathUSD at `0x20c0000000000000000000000000000000000000` (6 decimals)
- **Scheme name**: `tempo-tip20`
- **Explorer**: `https://explore.moderato.tempo.xyz`
- **RPC**: `https://rpc.moderato.tempo.xyz`

## Payment Flow

1. Client sends GET to protected endpoint
2. Server responds 402 with payment requirements (scheme, price, network, payTo, asset)
3. Client signs EIP-712 `PaymentAuthorization` message
4. Client retries with `X-PAYMENT` header containing signed payload
5. Server forwards to facilitator `/verify-and-settle` endpoint
6. Facilitator atomically: verifies signature, checks balance/allowance, nonce, then `transferFrom`
7. Server returns content + settlement transaction hash

## Security Features

- **Nonce replay protection**: DashMap tracks used nonces, background task purges expired entries
- **HMAC authentication**: Server signs requests to facilitator with shared secret (`FACILITATOR_SHARED_SECRET`)
- **CORS restriction**: Configurable via `ALLOWED_ORIGINS` env var (defaults to localhost)
- **Rate limiting**: Configurable via `RATE_LIMIT_RPM` env var (actix-governor)
- **Atomic settlement**: `/verify-and-settle` endpoint with per-payer mutex prevents TOCTOU
- **Request body limits**: 64KB max payload size
- **Error sanitization**: Internal errors logged, generic messages returned to clients

## EIP-712 Domain

```rust
eip712_domain! {
    name: "x402-tempo",
    version: "1",
    chain_id: 42431,
    verifying_contract: token_address,
}
```

## Environment Variables

| Var | Used By | Purpose |
|-----|---------|---------|
| `EVM_ADDRESS` | server | Payment recipient address |
| `FACILITATOR_URL` | server | Facilitator endpoint (default: localhost:4022) |
| `EVM_PRIVATE_KEY` | client, server (demo) | Client wallet private key |
| `RESOURCE_SERVER_URL` | client | Server endpoint (default: localhost:4021) |
| `ENDPOINT_PATH` | client | Protected endpoint path (default: /blockNumber) |
| `FACILITATOR_PRIVATE_KEY` | facilitator | Facilitator wallet key |
| `FACILITATOR_ADDRESS` | approve | Facilitator address for token approval |
| `FACILITATOR_SHARED_SECRET` | server, facilitator | HMAC shared secret for auth |
| `ALLOWED_ORIGINS` | server, facilitator | Comma-separated CORS origins |
| `RATE_LIMIT_RPM` | server, facilitator | Rate limit requests per minute |
| `APPROVE_AMOUNT` | approve | Token approval amount (default: U256::MAX) |
| `RPC_URL` | all | Tempo RPC endpoint |

## Building

```bash
cd rust
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## Security Notes

- Private keys in `.env` are **testnet only** — `.env` is gitignored
- EIP-712 signatures prevent replay via nonce + time window
- Facilitator checks on-chain state before settlement (no trust of client claims)
- `transferFrom` requires prior `approve` (run `cargo run --bin x402-approve` first)
