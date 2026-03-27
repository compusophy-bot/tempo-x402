//! Cartridge compiler — wraps `cargo build --target wasm32-wasip1`.

use std::path::{Path, PathBuf};

use crate::error::CartridgeError;

/// Maximum compilation time.
const COMPILE_TIMEOUT_SECS: u64 = 120;

/// Compile a cartridge from its source directory.
///
/// Source must have a valid Cargo.toml at `source_dir/Cargo.toml`.
/// Output `.wasm` binary goes to `output_dir/{name}.wasm`.
///
/// Returns the path to the compiled WASM binary.
pub async fn compile_cartridge(
    source_dir: &Path,
    output_dir: &Path,
) -> Result<PathBuf, CartridgeError> {
    let cargo_toml = source_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(CartridgeError::CompilationFailed(format!(
            "no Cargo.toml at {}",
            source_dir.display()
        )));
    }

    // Ensure output directory exists
    tokio::fs::create_dir_all(output_dir).await?;

    // Build with wasm32-wasip1 target
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(COMPILE_TIMEOUT_SECS),
        tokio::process::Command::new("cargo")
            .args([
                "build",
                "--target",
                "wasm32-wasip1",
                "--release",
                "--manifest-path",
            ])
            .arg(cargo_toml.to_string_lossy().as_ref())
            .env("CARGO_TARGET_DIR", output_dir.join("target"))
            .output(),
    )
    .await
    .map_err(|_| {
        CartridgeError::CompilationFailed(format!(
            "compilation timed out after {COMPILE_TIMEOUT_SECS}s"
        ))
    })?
    .map_err(|e| CartridgeError::CompilationFailed(format!("cargo failed to start: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Truncate long error output
        let truncated = if stderr.len() > 4096 {
            format!("{}...(truncated)", &stderr[..4096])
        } else {
            stderr.to_string()
        };
        return Err(CartridgeError::CompilationFailed(truncated));
    }

    // Find the compiled .wasm binary
    let wasm_glob = output_dir
        .join("target/wasm32-wasip1/release")
        .join("*.wasm");
    let pattern = wasm_glob.to_string_lossy();

    // Find .wasm files in the release directory
    let release_dir = output_dir.join("target/wasm32-wasip1/release");
    let mut wasm_path = None;
    if let Ok(mut entries) = tokio::fs::read_dir(&release_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().map(|e| e == "wasm").unwrap_or(false)
                && !path.to_string_lossy().contains(".d")
            {
                // Copy to output_dir/{name}.wasm
                let name = path.file_name().unwrap();
                let dest = output_dir.join(name);
                tokio::fs::copy(&path, &dest).await?;
                wasm_path = Some(dest);
                break;
            }
        }
    }

    wasm_path.ok_or_else(|| {
        CartridgeError::CompilationFailed(format!("no .wasm binary found in {}", pattern))
    })
}

/// Generate the default Cargo.toml for a new cartridge.
pub fn default_cargo_toml(slug: &str) -> String {
    format!(
        r#"[package]
name = "{slug}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
"#
    )
}

/// Generate the default lib.rs template for a new cartridge.
///
/// This is the simplest possible cartridge that compiles and returns a response.
/// The agent fills in the actual logic.
pub fn default_lib_rs(slug: &str) -> String {
    let template = r#"//! __SLUG__ — x402 WASM cartridge
//!
//! This cartridge handles HTTP requests via the x402 host ABI.
//! The host calls `x402_handle` with a JSON request.
//! Call `x402_response` to set the HTTP response.

// Import host functions
extern "C" {
    fn x402_response(status: i32, body_ptr: *const u8, body_len: i32, ct_ptr: *const u8, ct_len: i32);
    fn x402_log(level: i32, msg_ptr: *const u8, msg_len: i32);
    fn x402_kv_get(key_ptr: *const u8, key_len: i32) -> i64;
    fn x402_kv_set(key_ptr: *const u8, key_len: i32, val_ptr: *const u8, val_len: i32) -> i32;
    fn x402_payment_info() -> i64;
}

/// Helper: send a response back to the host.
fn respond(status: i32, body: &str, content_type: &str) {
    unsafe {
        x402_response(
            status,
            body.as_ptr(),
            body.len() as i32,
            content_type.as_ptr(),
            content_type.len() as i32,
        );
    }
}

/// Helper: log a message.
fn log(level: i32, msg: &str) {
    unsafe { x402_log(level, msg.as_ptr(), msg.len() as i32); }
}

/// Entry point: handle an HTTP request.
///
/// `request_ptr` points to a JSON string in memory:
/// {"method": "GET", "path": "/", "body": "", "headers": {}}
#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    log(1, "__SLUG__ cartridge invoked");

    // Read the request JSON from memory
    let request_bytes = unsafe {
        core::slice::from_raw_parts(request_ptr, request_len as usize)
    };
    let _request = core::str::from_utf8(request_bytes).unwrap_or("{}");

    // TODO: implement your cartridge logic here

    let body = "{\"message\": \"Hello from __SLUG__!\", \"status\": \"ok\"}";
    respond(200, body, "application/json");
}

/// Optional: allocator for host-to-guest memory transfers.
#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    let layout = core::alloc::Layout::from_size_align(size as usize, 1).unwrap();
    unsafe { std::alloc::alloc(layout) }
}
"#;
    template.replace("__SLUG__", slug)
}
