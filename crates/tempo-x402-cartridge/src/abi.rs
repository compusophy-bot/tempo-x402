//! Host function ABI — the contract between the node and WASM cartridges.
//!
//! Cartridges communicate with the host via JSON strings passed through
//! linear memory. This is deliberately simple so Flash Lite can generate
//! correct cartridge code.

use wasmtime::{Caller, Linker};

use crate::engine::CartridgeState;
use crate::error::CartridgeError;

/// Register all host functions on the linker.
pub fn register_host_functions(linker: &mut Linker<CartridgeState>) -> Result<(), CartridgeError> {
    // x402_log(level: i32, msg_ptr: i32, msg_len: i32)
    linker
        .func_wrap(
            "x402",
            "log",
            |mut caller: Caller<'_, CartridgeState>, level: i32, ptr: i32, len: i32| {
                let msg = read_string(&mut caller, ptr, len).unwrap_or_default();
                match level {
                    0 => tracing::debug!(cartridge = true, "{msg}"),
                    1 => tracing::info!(cartridge = true, "{msg}"),
                    2 => tracing::warn!(cartridge = true, "{msg}"),
                    _ => tracing::error!(cartridge = true, "{msg}"),
                }
            },
        )
        .map_err(|e| CartridgeError::Abi(format!("failed to register log: {e}")))?;

    // x402_kv_get(key_ptr: i32, key_len: i32) -> i64
    // Returns packed (ptr << 32 | len) or 0 if not found.
    linker
        .func_wrap(
            "x402",
            "kv_get",
            |mut caller: Caller<'_, CartridgeState>, key_ptr: i32, key_len: i32| -> i64 {
                let key = match read_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return 0,
                };
                let value = caller.data().kv_store.get(&key).cloned();
                match value {
                    Some(v) => write_bytes_to_guest(&mut caller, v.as_bytes()),
                    None => 0,
                }
            },
        )
        .map_err(|e| CartridgeError::Abi(format!("failed to register kv_get: {e}")))?;

    // x402_kv_set(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32) -> i32
    // Returns 0 on success, -1 on error.
    linker
        .func_wrap(
            "x402",
            "kv_set",
            |mut caller: Caller<'_, CartridgeState>,
             key_ptr: i32,
             key_len: i32,
             val_ptr: i32,
             val_len: i32|
             -> i32 {
                let key = match read_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return -1,
                };
                let val = match read_string(&mut caller, val_ptr, val_len) {
                    Some(v) => v,
                    None => return -1,
                };
                caller.data_mut().kv_store.insert(key, val);
                0
            },
        )
        .map_err(|e| CartridgeError::Abi(format!("failed to register kv_set: {e}")))?;

    // x402_payment_info() -> i64
    // Returns packed (ptr << 32 | len) with JSON payment context.
    linker
        .func_wrap(
            "x402",
            "payment_info",
            |mut caller: Caller<'_, CartridgeState>| -> i64 {
                let json = serde_json::to_string(&caller.data().payment)
                    .unwrap_or_else(|_| "null".to_string());
                write_bytes_to_guest(&mut caller, json.as_bytes())
            },
        )
        .map_err(|e| CartridgeError::Abi(format!("failed to register payment_info: {e}")))?;

    // x402_response(status: i32, body_ptr: i32, body_len: i32, ct_ptr: i32, ct_len: i32)
    // Cartridge calls this to set its response. Simpler than returning multiple values.
    linker
        .func_wrap(
            "x402",
            "response",
            |mut caller: Caller<'_, CartridgeState>,
             status: i32,
             body_ptr: i32,
             body_len: i32,
             ct_ptr: i32,
             ct_len: i32| {
                let body = read_string(&mut caller, body_ptr, body_len).unwrap_or_default();
                let content_type = read_string(&mut caller, ct_ptr, ct_len).unwrap_or_default();
                let state = caller.data_mut();
                state.response_status = status as u16;
                state.response_body = body;
                state.response_content_type = if content_type.is_empty() {
                    "application/json".to_string()
                } else {
                    content_type
                };
            },
        )
        .map_err(|e| CartridgeError::Abi(format!("failed to register response: {e}")))?;

    Ok(())
}

/// Read a UTF-8 string from guest linear memory at (ptr, len).
fn read_string(caller: &mut Caller<'_, CartridgeState>, ptr: i32, len: i32) -> Option<String> {
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data(caller);
    let start = ptr as usize;
    let end = start + len as usize;
    if end > data.len() {
        return None;
    }
    String::from_utf8(data[start..end].to_vec()).ok()
}

/// Write bytes into guest memory and return packed (ptr << 32 | len).
/// Allocates via the guest's `x402_alloc` export if available,
/// otherwise writes to a scratch area at the end of used memory.
fn write_bytes_to_guest(caller: &mut Caller<'_, CartridgeState>, bytes: &[u8]) -> i64 {
    let memory = match caller.get_export("memory") {
        Some(m) => match m.into_memory() {
            Some(m) => m,
            None => return 0,
        },
        None => return 0,
    };

    // Try guest allocator first
    let alloc = caller.get_export("x402_alloc");
    let ptr = if let Some(alloc_fn) = alloc.and_then(|e| e.into_func()) {
        let mut results = [wasmtime::Val::I32(0)];
        let _ = alloc_fn.call(
            &mut *caller,
            &[wasmtime::Val::I32(bytes.len() as i32)],
            &mut results,
        );
        results[0].unwrap_i32() as usize
    } else {
        // Fallback: use a scratch area. Not ideal but works for simple cartridges.
        let current_size = memory.data_size(&*caller);
        let needed = current_size + bytes.len();
        let pages_needed = ((needed + 65535) / 65536) - (current_size / 65536);
        if pages_needed > 0 {
            let _ = memory.grow(&mut *caller, pages_needed as u64);
        }
        current_size
    };

    let data = memory.data_mut(&mut *caller);
    if ptr + bytes.len() <= data.len() {
        data[ptr..ptr + bytes.len()].copy_from_slice(bytes);
        ((ptr as i64) << 32) | (bytes.len() as i64)
    } else {
        0
    }
}
