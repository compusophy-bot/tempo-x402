# Architecture Refactor Plan

## Vision

Fractal deployment architecture for AI agents + ERC-8004, using x402 as the payment method. The whole stack is the product - deployable as one unit.

## New Monorepo Structure

```
crates/
├── tempo-x402/              # Core SDK (types, signing, verification)
├── tempo-x402-client/       # Client SDK (X402Client, auto-402 handling)
├── tempo-x402-server/       # Server SDK (middleware framework, NO demo endpoints)
├── tempo-x402-facilitator/  # Facilitator service binary
├── tempo-x402-gateway/      # Gateway service binary
└── tempo-x402-app/          # Rust WASM web app (kitchen sink)
```

## Crate Purposes

| Crate | Type | Purpose |
|-------|------|---------|
| `tempo-x402` | Library | Core types, EIP-712, crypto - the foundation |
| `tempo-x402-client` | Library | SDK for making paid requests |
| `tempo-x402-server` | Library | SDK for building paid APIs (middleware) |
| `tempo-x402-facilitator` | Binary | Deployable payment settlement service |
| `tempo-x402-gateway` | Binary | Deployable API routing service |
| `tempo-x402-app` | WASM Binary | Kitchen sink web app showing everything |

## Action Items

- [x] 1. Extract `tempo-x402-client` from `tempo-x402` (the X402Client stuff) — v0.4.0
- [x] 2. Strip demo endpoints from `tempo-x402-server` — v0.4.0 (server is now SDK-only)
- [ ] 3. Create `tempo-x402-app` - Rust WASM kitchen sink (future work)
- [ ] 4. Deprecate external demo repo (tempo-x402-demo) (future work)
- [ ] 5. Add Railway config for deploying entire stack (future work)

### v0.4.0 Breaking Changes

- `tempo-x402` no longer exports `TempoSchemeClient`, `X402Client`, or `encode_payment`
- These are now in `tempo-x402-client` crate
- Migrate: `use x402_client::{TempoSchemeClient, X402Client, encode_payment}`
- `tempo-x402-server` no longer has `/blockNumber` or `/api/demo` endpoints (use gateway)

## The App (Kitchen Sink) Features

- Compile to WASM, run in browser
- Use `tempo-x402-client` to make paid requests
- Show wallet connection, signing, payments
- Display tx history, revenue earned
- Register endpoints on gateway
- Call those endpoints through gateway
- Full analytics dashboard

## Deploy Command

Someone forks the repo and runs:
```bash
railway up
```

Gets:
- Facilitator service
- Gateway service
- Web app (the kitchen sink UI)

All connected, all working.

## Future Extensions

- x402-deploy: Pay to spin up new services
- x402-compute: Serverless functions behind paywalls
- ERC-8004 integration: Trustless agent identity/reputation
