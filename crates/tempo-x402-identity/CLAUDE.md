# tempo-x402-identity

Library crate. Identity management for x402 node instances: off-chain bootstrap + on-chain ERC-8004 identity.

Off-chain (`lib.rs`): generates/loads wallet keypairs, persists to disk, requests faucet funds, registers with parent node.
On-chain (`erc8004` feature): sol! contract bindings, NFT identity minting, reputation, validation, peer discovery, self-deployment.

## Depends On

- `x402` (wallet for key generation, HMAC for deriving facilitator shared secret)

## Non-Obvious Patterns

- `bootstrap()` injects env vars (`EVM_ADDRESS`, `FACILITATOR_PRIVATE_KEY`, `FACILITATOR_SHARED_SECRET`) only if not already set — respects explicit config
- `FACILITATOR_SHARED_SECRET` is deterministic HMAC of private key — safe for same-process use
- Private key: `#[serde(skip_serializing)]` — never in JSON output. File permissions 0o600 on Unix.
- Parent URL validated as HTTPS-only. Continues without parent if validation fails (graceful degradation).
- Faucet/registration: retry with exponential backoff, non-blocking

## If You're Changing...

- **Identity file format**: `PersistedIdentity` struct — changing fields needs migration logic
- **Env var injection**: Only in `bootstrap()` — grep for `env::set_var`
- **Used by**: `x402-node` calls `bootstrap()` at startup
