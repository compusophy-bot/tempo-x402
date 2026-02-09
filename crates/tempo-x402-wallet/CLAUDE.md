# tempo-x402-wallet

Library name: `x402_wallet`. WASM-compatible EIP-712 payment signer. Single-file crate (`src/lib.rs`).

Zero network dependencies. Generates keys, signs payment authorizations, builds payloads. Works in native Rust and browser WASM.

## Depends On

- `x402` (dev-dependency ONLY — for cross-validation tests)
- NO runtime workspace deps (intentionally standalone for WASM)

## Non-Obvious Patterns

- **Mirrors constants** from core crate (TEMPO_CHAIN_ID, SCHEME_NAME, etc.). Test `test_constants_match_core_crate` verifies sync.
- **Mirrors `PaymentAuthorization`** sol! macro from core. Must be kept identical.
- Nonces hashed through keccak256 for defense-in-depth against weak browser CSPRNG
- Timeout capped at 600s regardless of what server requests
- `getrandom` with `wasm_js` feature for browser entropy
- Feature `demo`: exposes deprecated Hardhat Account #0 key (testnet only)

## If You're Changing...

- **PaymentAuthorization fields**: MUST also update the sol! macro in `x402/lib.rs`
- **Constants**: MUST also update `x402/constants.rs` — tests catch drift but only at test time
- **Adding dependencies**: Extremely careful — any async/network dep breaks WASM. No tokio, no reqwest, no rusqlite.
- **Used by**: `x402-client` (native), `x402-identity` (key gen), `x402-app` (WASM SPA)
