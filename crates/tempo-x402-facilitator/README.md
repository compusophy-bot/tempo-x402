# tempo-x402-facilitator

Payment verification and settlement service for the x402 protocol.

## Installation

```toml
[dependencies]
tempo-x402-facilitator = "1"
```

## Features

- **Signature Verification**: EIP-712 payment authorization validation
- **On-chain Settlement**: Atomic `transferFrom` execution
- **Nonce Store**: Replay protection (SQLite for persistence)
- **HMAC Auth**: Optional server authentication
- **Prometheus Metrics**: Track settlements and latency

## Quick Start

```bash
# Required
export FACILITATOR_PRIVATE_KEY=0x...  # Wallet with token approval

# Optional
export FACILITATOR_SHARED_SECRET=your-secret
export PORT=4022

cargo run
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check (includes block number) |
| GET | `/metrics` | Prometheus metrics |
| GET | `/supported` | Supported schemes and networks |
| POST | `/verify` | Verify payment (no settlement) |
| POST | `/verify-and-settle` | Atomic verify + settle |

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `FACILITATOR_PRIVATE_KEY` | Yes | - | Settlement wallet key |
| `FACILITATOR_SHARED_SECRET` | No | - | HMAC auth secret |
| `FACILITATOR_PORT` or `PORT` | No | 4022 | Server port |
| `RPC_URL` | No | Tempo RPC | Chain RPC URL |
| `NONCE_DB_PATH` | No | `./x402-nonces.db` | SQLite nonce store |
| `ALLOWED_ORIGINS` | No | localhost | CORS origins |
| `RATE_LIMIT_RPM` | No | 120 | Rate limit per IP |
| `WEBHOOK_URLS` | No | - | Comma-separated webhook URLs |

## Deployed

https://x402-facilitator-production-ec87.up.railway.app

## License

MIT
