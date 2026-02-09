# tempo-x402-node

Binary crate. Self-deploying x402 node: composes gateway + identity bootstrap + clone orchestration.

Runs a gateway, auto-generates its own wallet identity, and can clone itself onto Railway infrastructure. Extends gateway's SQLite DB with a `children` table.

Binary: `x402-node` on port 4023.

## Depends On

- `x402` (core)
- `x402-gateway` (AppState, Database, middleware, config, routes)
- `x402-identity` (bootstrap, faucet, registration)
- `x402-agent` (CloneOrchestrator)
- `x402-facilitator` (embedded facilitator state, routes)

## Non-Obvious Patterns

- Startup: identity bootstrap → gateway config → embedded facilitator → agent → server (order matters — bootstrap injects env vars used by later steps)
- `create_child_if_under_limit()` in `db.rs` is atomic (`BEGIN IMMEDIATE`) — don't replace with separate check+insert
- Input validation: UUID format for instance IDs, `0x` + 40 hex for addresses, HTTPS-only for URLs
- Extends gateway DB via `execute_schema()` — doesn't create a separate database
- Background tasks (faucet, parent registration) are best-effort, non-fatal

## If You're Changing...

- **Clone logic**: Endpoint in `routes/clone.rs`, orchestration in `x402-agent` crate
- **Identity bootstrap**: `x402-identity` crate — node just calls `bootstrap()`
- **Database schema**: `db.rs` — uses gateway's `execute_schema()` pattern
- **Startup order**: `main.rs` — bootstrap must run before gateway config reads env vars
