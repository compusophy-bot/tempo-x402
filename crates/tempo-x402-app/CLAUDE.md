# tempo-x402-app

Leptos WASM SPA. Not published. Each gateway/node instance serves its own bundled WASM frontend.

**Single-page cockpit** — Bloomberg terminal x spaceship bridge. No router, no page navigation. All intelligence data visible simultaneously. Three wallet modes (MetaMask, demo key, encrypted embedded wallet).

Crate type: `["cdylib", "rlib"]` — compiles to WASM, no binary.

## Depends On

- `x402` (with `wasm` + `demo` features — wallet signing, key gen, constants)

## Architecture

Single `CockpitPage` component (`components/cockpit.rs`) replaces the old 4-page router (home/dashboard/studio/timeline). Layout:

- **Top bar**: brand + version + address + balance + uptime + wallet buttons
- **Left column**: Psi(t) + F(t) + regime + 5-component fitness bars + active goals
- **Center column**: 9-system cognitive grid + benchmark + active plan + recent thoughts
- **Right column**: processes (soul status/tools/coding) + cartridges + colony peers
- **Bottom panel**: tabbed CHAT | LOGS with plan approval bar
- **Status bar**: CPU/MEM/DISK + cycle count + version

Data sources: `/instance/info`, `/soul/status`, `/soul/system`, `/c`

## Non-Obvious Patterns

- Wallet encryption: AES-GCM via browser WebCrypto API (`wallet_crypto.rs`), not external crypto libs
- MetaMask signing: `eth_signTypedData_v4` via WASM bindings to `window.ethereum`
- Legacy unencrypted keys in localStorage (`0x...` hex) auto-detected alongside new encrypted format
- All deps must be WASM-compatible — no tokio, no std::fs, no native crypto
- Old components (dashboard.rs, home.rs, soul_panel.rs, etc.) still compiled but unused — kept for reference

## If You're Changing...

- **Cockpit layout/panels**: `components/cockpit.rs` — the entire UI
- **Design system/styling**: `style.css` — monospace, green-on-black theme
- **Wallet buttons**: `components/wallet_panel.rs` — reused in cockpit top bar
- **Payment signing**: `api.rs` — `sign_for_wallet()` dispatches by WalletMode
- **MetaMask integration**: `wallet.rs` — WASM FFI to `window.ethereum`
- **Adding dependencies**: Must be WASM-compatible
- **Chat**: integrated into cockpit bottom panel (was separate `ChatWidget` FAB)
