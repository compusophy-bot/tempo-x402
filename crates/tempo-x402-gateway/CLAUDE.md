# tempo-x402-gateway

Library name: `x402_gateway`. API proxy that adds x402 payment rails to any HTTP endpoint.

Register endpoints with a price, clients pay per-request via `/g/{slug}/{path}`. Includes SSRF protection, atomic slug registration, and optional embedded facilitator. Database is extensible — x402-node adds tables to it.

Binary: `x402-gateway` on port 4023.

## Depends On

- `x402` (core types, nonce store, HMAC, security, scheme server)
- `x402-facilitator` (AppState as FacilitatorState, routes, webhooks — for embedded mode)

## Non-Obvious Patterns

- Slug reservation is atomic: `reserve_slug()` with `BEGIN IMMEDIATE` BEFORE payment settlement, rollback on failure
- `Database` is extensible: `execute_schema()` + `with_connection()` let downstream crates (x402-node) add tables
- Proxy strips sensitive headers and has a response header allowlist (`proxy.rs`)
- SSRF protection: HTTPS-only targets, private IP blocking, DNS resolution check, no redirects, CRLF rejection
- Embedded facilitator (when `FACILITATOR_PRIVATE_KEY` set) runs in-process — no HTTP round-trip
- Soft deletes on endpoints (`active` boolean)
- Per-endpoint analytics: `endpoint_stats` table tracks request_count, payment_count, revenue_total per slug
- Per-endpoint Prometheus metrics: `ENDPOINT_PAYMENTS` and `ENDPOINT_REVENUE` (`IntCounterVec` with `slug` label)

## If You're Changing...

- **Proxy security**: `proxy.rs` (header stripping) + `validation.rs` (SSRF). Security-audit crate tests these.
- **Registration flow**: `routes/register.rs` — don't break the reserve→pay→activate sequence
- **Adding DB tables**: Use `execute_schema()` pattern (see x402-node `db.rs`)
- **Embedded facilitator**: Init in `main.rs`, mounted at `/facilitator/*`
- **SSRF patterns**: Security-audit checks `redirect(Policy::none())` on all HTTP clients
- **Analytics**: `routes/analytics.rs` serves `GET /analytics` and `GET /analytics/{slug}`. Stats recorded in `routes/gateway.rs` after successful proxy.
