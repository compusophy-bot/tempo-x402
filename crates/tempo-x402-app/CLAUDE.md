# tempo-x402-app

Leptos WASM SPA. Not published. Each gateway/node instance serves its own bundled WASM frontend.

Three pages with shared navigation: Colony (mandala) / Cockpit (dashboard) / Studio (app builder). Three wallet modes (MetaMask, demo key, encrypted embedded wallet).

Crate type: `["cdylib", "rlib"]` — compiles to WASM, no binary.

## Depends On

- `x402` (with `wasm` + `demo` features — wallet signing, key gen, constants)

## Architecture

Three routes via `leptos_router`, shared `NavBar` on all pages:

- `/` — **Mandala** (`components/mandala.rs`): Colony organism visualization, SSE events, IQ trajectory
- `/dashboard` — **CockpitPage** (`components/cockpit.rs`): Bloomberg terminal — 9-system grid, fitness, plans, chat, logs
- `/studio` — **StudioPage** (`studio.rs`): Three-panel app builder — cartridge sidebar, preview center, chat with tool execution visibility

App shell (`lib.rs`) wraps pages in `<div class="app-shell">` grid (24px nav + content).

Data sources: `/instance/info`, `/soul/status`, `/soul/system`, `/c`, `/soul/chat`

## Non-Obvious Patterns

- Wallet encryption: AES-GCM via browser WebCrypto API (`wallet_crypto.rs`), not external crypto libs
- MetaMask signing: `eth_signTypedData_v4` via WASM bindings to `window.ethereum`
- Legacy unencrypted keys in localStorage (`0x...` hex) auto-detected alongside new encrypted format
- All deps must be WASM-compatible — no tokio, no std::fs, no native crypto
- Studio chat shows tool executions (create_cartridge, compile_cartridge, etc.) as collapsible blocks
- Studio sidebar is a slide-out drawer on mobile (<768px)
- WASM-in-WASM: `cartridge_runner.rs` fetches and runs cartridge .wasm in browser via js_sys::WebAssembly

## If You're Changing...

- **Navigation**: `components/nav.rs` — shared bar, uses `use_location()` for active state
- **Cockpit layout/panels**: `components/cockpit.rs` — the dashboard UI
- **Studio/app builder**: `studio.rs` — three-panel layout with chat + tool visibility
- **Colony visualization**: `components/mandala.rs` — SSE streaming, SVG organism
- **Design system/styling**: `style.css` — monospace, green-on-black theme
- **Wallet buttons**: `components/wallet_panel.rs` — reused in cockpit top bar
- **Payment signing**: `api.rs` — `sign_for_wallet()` dispatches by WalletMode
- **Cartridge management**: `api.rs` — `delete_cartridge()`, `clear_all_cartridges()`
- **MetaMask integration**: `wallet.rs` — WASM FFI to `window.ethereum`
- **Adding dependencies**: Must be WASM-compatible
