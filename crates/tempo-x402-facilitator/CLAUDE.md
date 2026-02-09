# tempo-x402-facilitator

Library name: `x402_facilitator`. Verifies EIP-712 payment signatures and settles on-chain via `transferFrom`.

HMAC-authenticated. Persistent SQLite nonce store (mandatory — no in-memory fallback). Webhooks with SSRF protection.

Binary: `x402-facilitator` on port 4022.

## Depends On

- `x402` (TempoSchemeFacilitator, types, nonce store, HMAC, security)

## Non-Obvious Patterns

- HMAC is **mandatory** — startup fails without `FACILITATOR_SHARED_SECRET`
- SQLite nonce store is **mandatory** — no in-memory fallback (replay risk on restart)
- Settlement logic lives in `x402` core (`scheme_facilitator.rs`), not in this crate
- Webhook security: HTTPS-only, private IP blocking, DNS rebinding prevention, no redirects, semaphore (50 concurrent)
- Separate `METRICS_TOKEN` from `FACILITATOR_SHARED_SECRET` for defense-in-depth
- `AppState` is re-exported and used by `x402-gateway` for embedded facilitator mode

## If You're Changing...

- **Settlement logic**: Change `x402` core crate (`scheme_facilitator.rs`), not here
- **Webhook behavior**: `webhook.rs` — validation at startup, fire-and-forget with retries
- **Adding endpoints**: `routes.rs` + register in `main.rs`
- **AppState fields**: Gateway crate imports this as `FacilitatorState` — check compatibility
