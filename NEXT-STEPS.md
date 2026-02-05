# Next Steps

Living document. Revisit each session. Strike items when done, add new ones as they emerge.

## Now (v0.1.x)

- [x] **End-to-end test on deployed infra** — run `x402-client` against the Railway server+facilitator with real testnet funds, confirm a full payment round-trip (402 -> sign -> pay -> 200 + tx hash) ✓
- [x] **Demo project** — https://tempo-x402-demo.vercel.app ([repo](https://github.com/compusophy/tempo-x402-demo)) ✓
- [x] **Documentation** — crate-level rustdoc + llms.txt ✓
- [ ] **CI hardening** — the release workflow builds binaries but doesn't run tests first; consider adding a test gate before the release job

## Soon (v0.2.0)

- [ ] **Multi-endpoint pricing** — server currently hardcodes one price for `/blockNumber`; support per-route pricing config (e.g. TOML config or builder API)
- [ ] **Subscription / batch payments** — allow N requests per payment instead of strict pay-per-request
- [ ] **Receipt endpoint** — server or facilitator exposes `/receipt/{txHash}` for clients to verify settlement after the fact
- [ ] **Retry / timeout resilience** — client should handle facilitator timeouts gracefully (retry with same nonce? or new nonce?)
- [ ] **Observability** — Prometheus metrics are wired up but no Grafana dashboard or alerting; consider a default dashboard config

## Later

- [ ] **Multi-chain** — ChainConfig is already parameterized; add a second chain (mainnet, another testnet) and test
- [ ] **Alternative token support** — support arbitrary ERC-20/TIP-20 tokens beyond pathUSD
- [ ] **SDK / client library** — a thin `tempo-x402-client` crate that wraps `reqwest` and handles the 402 dance transparently (the `http_client` module is close to this already)
- [ ] **WebSocket support** — streaming endpoints behind payment (pay once, stream N events)
- [ ] **Decentralized facilitator** — remove single-facilitator trust assumption; multi-sig or on-chain facilitator registry

## Questions to Resolve

- Should the facilitator be a public service anyone can use, or always self-hosted per resource server?
- What's the right nonce expiry window? Currently time-bounded by `validBefore` — is that sufficient for all use cases?
- Should the server support multiple facilitators (redundancy / load balancing)?
- How should pricing be denominated? Fixed USD amounts vs dynamic pricing?
