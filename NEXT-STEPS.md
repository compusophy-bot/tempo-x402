# Next Steps

Living document. Revisit each session. Strike items when done, add new ones as they emerge.

## Completed (v0.1.x - v0.3.x)

- [x] **End-to-end test on deployed infra** — run `x402-client` against the Railway server+facilitator with real testnet funds, confirm a full payment round-trip (402 -> sign -> pay -> 200 + tx hash)
- [x] **Demo project** — https://tempo-x402-demo.vercel.app ([repo](https://github.com/compusophy/tempo-x402-demo))
- [x] **Documentation** — crate-level rustdoc + llms.txt
- [x] **CI hardening** — release workflow has test gate (test job must pass before build)
- [x] **Docs CI** — version consistency validation on push/release (never stale again)
- [x] **Gateway** — API relay/proxy that adds x402 payment rails to any HTTP API
- [x] **x402 v2 compliance** — updated headers to match Coinbase spec (`PAYMENT-SIGNATURE`, `PAYMENT-RESPONSE`)
- [x] **OpenAPI specs** — OpenAPI 3.1 specs for facilitator, server, and gateway in `openapi/` directory
- [x] **GitHub auto-deploy** — all 3 Railway services deploy from GitHub on push to main

## Now (v0.4.x)

- [ ] **Multi-endpoint pricing** — server currently hardcodes one price for `/blockNumber`; support per-route pricing config (e.g. TOML config or builder API)
- [ ] **Gateway analytics** — endpoint call counts, revenue tracking per endpoint owner
- [ ] **ERC-8004 agent** — create separate repo for ERC-8004 Trustless Agent that uses the gateway infrastructure

## Soon

- [ ] **Subscription / batch payments** — allow N requests per payment instead of strict pay-per-request
- [ ] **Receipt endpoint** — server or facilitator exposes `/receipt/{txHash}` for clients to verify settlement after the fact
- [ ] **Retry / timeout resilience** — client should handle facilitator timeouts gracefully (retry with same nonce? or new nonce?)
- [ ] **Observability dashboard** — Prometheus metrics are wired up but no Grafana dashboard; consider a default dashboard config

## Later

- [ ] **Multi-chain** — ChainConfig is already parameterized; add a second chain (mainnet, another testnet) and test
- [ ] **Alternative token support** — support arbitrary ERC-20/TIP-20 tokens beyond pathUSD
- [ ] **WebSocket support** — streaming endpoints behind payment (pay once, stream N events)
- [ ] **Decentralized facilitator** — remove single-facilitator trust assumption; multi-sig or on-chain facilitator registry
- [ ] **x402-deploy** — pay to spawn dedicated server/facilitator instances
- [ ] **x402-compute** — serverless functions behind paywalls

## Questions to Resolve

- Should the facilitator be a public service anyone can use, or always self-hosted per resource server?
- What's the right nonce expiry window? Currently time-bounded by `validBefore` — is that sufficient for all use cases?
- Should the server support multiple facilitators (redundancy / load balancing)?
- How should pricing be denominated? Fixed USD amounts vs dynamic pricing?
