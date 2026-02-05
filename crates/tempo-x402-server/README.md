# tempo-x402-server

Resource server with payment middleware for the x402 protocol.

## Installation

```toml
[dependencies]
tempo-x402-server = "0.2"
```

## Features

- **Payment Middleware**: Gate any endpoint behind HTTP 402
- **Configurable Routes**: Set prices per endpoint
- **Prometheus Metrics**: Track requests and payments
- **CORS Support**: Configurable allowed origins

## Quick Start

```bash
# Required
export EVM_ADDRESS=0x...  # Payment recipient

# Optional
export FACILITATOR_URL=https://x402-facilitator.example.com
export FACILITATOR_SHARED_SECRET=your-secret
export PORT=4021

cargo run
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/metrics` | Prometheus metrics |
| GET | `/blockNumber` | Example paid endpoint ($0.001) |

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `EVM_ADDRESS` | Yes | - | Payment recipient |
| `FACILITATOR_URL` | No | localhost:4022 | Facilitator endpoint |
| `FACILITATOR_SHARED_SECRET` | No | - | HMAC auth secret |
| `PORT` | No | 4021 | Server port |
| `RPC_URL` | No | Tempo RPC | Chain RPC URL |
| `ALLOWED_ORIGINS` | No | localhost | CORS origins |
| `RATE_LIMIT_RPM` | No | 60 | Rate limit per IP |

## Deployed

https://x402-server-production.up.railway.app

## License

MIT
