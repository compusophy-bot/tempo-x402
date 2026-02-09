# tempo-x402 (core)

Library name: `x402`. Foundation crate — all other crates depend on this.

Core protocol types, EIP-712 signing, TIP-20 contract calls, nonce replay protection (InMemory + SQLite), and trait abstractions (SchemeClient, SchemeFacilitator, SchemeServer).

Binary: `x402-approve` — CLI for token approval to facilitator.

## Depends On

No workspace crates. External: alloy, serde, dashmap, rusqlite, hmac/sha2/subtle.

## Non-Obvious Patterns

- Integer-only arithmetic for prices — never f64 for token amounts (`checked_mul`, `saturating_sub`)
- Constant-time comparison for all secret comparisons (`security::constant_time_eq`, `hmac::verify_hmac`)
- Per-payer mutex locks in `scheme_facilitator.rs` prevent TOCTOU during settlement
- Nonce claimed BEFORE `transferFrom`, never released on failure (tx may still mine)
- EIP-2 high-s rejection in `eip712.rs` prevents signature malleability
- Generic error messages in HTTP responses — don't leak balances/allowances to clients

## If You're Changing...

- **EIP-712 struct fields**: Update `PaymentAuthorization` sol! macro in BOTH `lib.rs` AND `x402-wallet/src/lib.rs`
- **Chain constants**: Update `constants.rs` AND `x402-wallet/src/lib.rs` (wallet mirrors them; test verifies sync)
- **NonceStore trait**: Both InMemory and Sqlite impls must be updated. Security-audit checks Sqlite is used in prod.
- **Price parsing**: `scheme_server.rs` has edge-case tests — run them
- **HMAC or security module**: Security-audit crate tests for constant-time usage patterns
