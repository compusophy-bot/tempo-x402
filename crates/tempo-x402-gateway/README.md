# tempo-x402-gateway

A relay/proxy service that adds x402 payment rails to any HTTP API. Users pay a platform fee to register endpoints, callers pay endpoint owners to access proxied content.

## Quick Start

```bash
# Set required environment variables
export EVM_ADDRESS=0x...  # Platform fee recipient

# Run the gateway
cargo run
```

## Endpoints

| Method | Path | Payment | Description |
|--------|------|---------|-------------|
| POST | `/register` | Platform fee | Register a new endpoint |
| GET | `/endpoints` | Free | List all endpoints |
| GET | `/endpoints/:slug` | Free | Get endpoint details |
| PATCH | `/endpoints/:slug` | Platform fee | Update endpoint (owner only) |
| DELETE | `/endpoints/:slug` | Platform fee | Deactivate endpoint (owner only) |
| ANY | `/g/:slug/*` | Endpoint price | Proxy to target API |
| GET | `/health` | Free | Health check |
| GET | `/metrics` | Free | Prometheus metrics |

## Registration

```bash
# First request returns 402 with payment requirements
curl -X POST http://localhost:4023/register \
  -H "Content-Type: application/json" \
  -d '{"slug": "my-api", "target_url": "https://api.example.com", "price": "$0.05"}'

# Sign the payment and retry with X-PAYMENT header
curl -X POST http://localhost:4023/register \
  -H "Content-Type: application/json" \
  -H "X-PAYMENT: <base64-encoded-payment>" \
  -d '{"slug": "my-api", "target_url": "https://api.example.com", "price": "$0.05"}'
```

## Proxy Usage

```bash
# Request to /g/my-api/users/123 proxies to https://api.example.com/users/123
curl http://localhost:4023/g/my-api/users/123 \
  -H "X-PAYMENT: <base64-encoded-payment>"
```

The target API receives additional headers:
- `X-X402-Verified: true`
- `X-X402-Payer: 0x...`
- `X-X402-Amount: 50000`
- `X-X402-TxHash: 0x...`

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `EVM_ADDRESS` | Yes | - | Platform fee recipient |
| `FACILITATOR_URL` | No | Production | Facilitator endpoint |
| `FACILITATOR_SHARED_SECRET` | No | - | HMAC auth secret |
| `DB_PATH` | No | `./gateway.db` | SQLite database path |
| `PORT` | No | `4023` | Server port |
| `PLATFORM_FEE` | No | `$0.01` | Registration fee |
| `ALLOWED_ORIGINS` | No | localhost | CORS origins |
| `RATE_LIMIT_RPM` | No | `60` | Rate limit per IP |

## How It Works

1. **Register**: Pay platform fee to register your API endpoint
2. **Set price**: Choose how much callers pay per request
3. **Earn**: Receive payments directly when your endpoint is called

The gateway never holds funds - payments go directly from caller to endpoint owner via the Tempo blockchain.

## License

MIT
