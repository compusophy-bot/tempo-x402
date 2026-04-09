# tempo-x402-cartridge

Library crate. WASM cartridge runtime — sandboxed app execution with payment rails.

Cartridges are precompiled `.wasm` binaries that run inside a wasmtime sandbox. The host exposes a minimal ABI (HTTP client, KV store, logging, payment info). Each cartridge handles HTTP requests and returns responses. Instant deployment, no node restart.

## Depends On

- None (standalone runtime). Used by: `x402-soul`, `x402-node`.

## Module Overview

| Module | Purpose |
|--------|---------|
| `engine.rs` | CartridgeEngine: wasmtime Engine + Module cache + per-request Store. `execute()` returns `(CartridgeResult, HashMap<String, String>)` — both the response and modified KV |
| `abi.rs` | Host function registration (log, kv_get/set, payment_info, response) |
| `manifest.rs` | CartridgeManifest, CartridgeRequest, CartridgeResult, CartridgeKind (Backend/Interactive/Frontend/Cognitive) |
| `compiler.rs` | `cargo build --target wasm32-unknown-unknown` wrapper + project templates (backend, interactive, frontend) |
| `error.rs` | CartridgeError enum |

## CartridgeKind

Four types: **Backend** (server-side HTTP), **Interactive** (60fps framebuffer), **Frontend** (Leptos SPA via wasm-bindgen), **Cognitive** (hot-swappable brain modules, prefixed `cognitive-`).

## ABI Contract

Cartridges import from `"x402"` namespace: `log`, `kv_get`, `kv_set`, `payment_info`, `response`.
Cartridges export: `x402_handle(request_ptr, request_len)` and optionally `x402_alloc(size)`.
Communication: JSON strings over linear memory.

## Non-Obvious Patterns

- `execute()` returns modified KV alongside the result — caller is responsible for persisting changes
- `replace_module()` atomically hot-swaps a cached module (used on recompile)
- KV store is per-request in memory, loaded from DB before execution, saved after
- Cognitive cartridges use `cognitive-{system}` slug convention
