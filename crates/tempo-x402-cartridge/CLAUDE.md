# tempo-x402-cartridge

Library crate. WASM cartridge runtime — sandboxed app execution with payment rails.

Cartridges are precompiled `.wasm` binaries that run inside a wasmtime sandbox. The host exposes a minimal ABI (HTTP client, KV store, logging, payment info). Each cartridge handles HTTP requests and returns responses. Instant deployment, no node restart.

## Depends On

- None (standalone runtime). Used by: `x402-node`.

## Module Overview

| Module | Purpose |
|--------|---------|
| `engine.rs` | CartridgeEngine: wasmtime Engine + Module cache + per-request Store |
| `abi.rs` | Host function registration (log, kv_get/set, payment_info, response) |
| `manifest.rs` | CartridgeManifest, CartridgeRequest, CartridgeResult types |
| `compiler.rs` | `cargo build --target wasm32-wasip1` wrapper + project templates |
| `error.rs` | CartridgeError enum |

## ABI Contract

Cartridges import from `"x402"` namespace: `log`, `kv_get`, `kv_set`, `payment_info`, `response`.
Cartridges export: `x402_handle(request_ptr, request_len)` and optionally `x402_alloc(size)`.
Communication: JSON strings over linear memory.
