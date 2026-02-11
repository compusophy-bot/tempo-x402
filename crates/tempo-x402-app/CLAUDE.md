# tempo-x402-app

Leptos WASM SPA. Not published. Deployed at https://tempo-x402-app.vercel.app

Demo app with three wallet modes (MetaMask, demo key, encrypted embedded wallet). Makes paid requests to the gateway, shows settlement info.

Crate type: `["cdylib", "rlib"]` — compiles to WASM, no binary.

## Depends On

- `x402-wallet` (with `demo` feature — signing, key gen, constants)

## Non-Obvious Patterns

- Wallet encryption: AES-GCM via browser WebCrypto API (`wallet_crypto.rs`), not external crypto libs
- MetaMask signing: `eth_signTypedData_v4` via WASM bindings to `window.ethereum`
- Legacy unencrypted keys in localStorage (`0x...` hex) auto-detected alongside new encrypted format
- All deps must be WASM-compatible — no tokio, no std::fs, no native crypto

## If You're Changing...

- **UI components**: `lib.rs` — Leptos reactive signals
- **Payment signing**: `api.rs` — `sign_for_wallet()` dispatches by WalletMode
- **MetaMask integration**: `wallet.rs` — WASM FFI to `window.ethereum`
- **Adding dependencies**: Must be WASM-compatible
- **Docs page**: `lib.rs` `DocsPage` component. Points to `llms.txt` as single source of truth for API reference — update llms.txt, not the DocsPage, for API details.
- **Dashboard analytics**: `lib.rs` `DashboardPage` fetches `GET /analytics` via `api::fetch_analytics()` for per-endpoint stats
