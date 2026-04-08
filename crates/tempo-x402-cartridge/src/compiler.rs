//! Cartridge compiler — wraps `cargo build` for wasip1 (backend/interactive) and
//! wasm32-unknown-unknown + wasm-bindgen (frontend Leptos apps).

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

    // Ensure wasm32-unknown-unknown target is installed (might not be at runtime)
    let target_check = tokio::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .await;
    let has_wasip1 = target_check
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("wasm32-unknown-unknown"))
        .unwrap_or(false);
    if !has_wasip1 {
        tracing::info!("Installing wasm32-unknown-unknown target for cartridge compilation");
        let _ = tokio::process::Command::new("rustup")
            .args(["target", "add", "wasm32-unknown-unknown"])
            .output()
            .await;
    }

    // Use /tmp for build target to avoid bloating persistent volume
    let target_dir = format!(
        "/tmp/cartridge-build-{}",
        source_dir.file_name().unwrap_or_default().to_string_lossy()
    );

    // Build with wasm32-unknown-unknown target
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(COMPILE_TIMEOUT_SECS),
        tokio::process::Command::new("cargo")
            .args([
                "build",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "--manifest-path",
            ])
            .arg(cargo_toml.to_string_lossy().as_ref())
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("RUSTUP_HOME", "/usr/local/rustup")
            .env("CARGO_HOME", "/usr/local/cargo")
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

    // Find the compiled .wasm binary in the target directory
    let release_dir_path = format!("{}/wasm32-unknown-unknown/release", target_dir);
    let pattern = format!("{}/*.wasm", release_dir_path);

    let release_dir = std::path::PathBuf::from(&release_dir_path);
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

    // Clean up build directory to save disk space
    let _ = tokio::fs::remove_dir_all(&target_dir).await;

    wasm_path.ok_or_else(|| {
        CartridgeError::CompilationFailed(format!("no .wasm binary found in {}", pattern))
    })
}

/// Generate the default Cargo.toml for a new cartridge.
/// NOTE: No dependencies needed — the host ABI uses raw extern "C" FFI.
pub fn default_cargo_toml(slug: &str) -> String {
    format!(
        r#"[package]
name = "{slug}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

# NO DEPENDENCIES NEEDED — the x402 host ABI uses extern "C" functions.
# Do NOT add x402_sdk or any external crate — cartridges are self-contained.
[dependencies]

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

#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }

// Import host functions from the x402 namespace.
// The #[link] attribute ensures WASM imports come from "x402" module, not "env".
#[link(wasm_import_module = "x402")]
extern "C" {
    fn response(status: i32, body_ptr: *const u8, body_len: i32, ct_ptr: *const u8, ct_len: i32);
    fn log(level: i32, msg_ptr: *const u8, msg_len: i32);
    fn kv_get(key_ptr: *const u8, key_len: i32) -> i64;
    fn kv_set(key_ptr: *const u8, key_len: i32, val_ptr: *const u8, val_len: i32) -> i32;
    fn payment_info() -> i64;
}

/// Helper: send a response back to the host.
fn respond(status: i32, body: &str, content_type: &str) {
    unsafe {
        response(
            status,
            body.as_ptr(),
            body.len() as i32,
            content_type.as_ptr(),
            content_type.len() as i32,
        );
    }
}

/// Helper: log a message to the host.
fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

/// Entry point: handle an HTTP request.
///
/// `request_ptr` points to a JSON string in memory:
/// {"method": "GET", "path": "/", "body": "", "headers": {}}
#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "__SLUG__ cartridge invoked");

    // Read the request JSON from memory
    let request_bytes = unsafe {
        core::slice::from_raw_parts(request_ptr, request_len as usize)
    };
    let _request = core::str::from_utf8(request_bytes).unwrap_or("{}");

    // Cartridges can return HTML pages, JSON APIs, or any content type.
    // For apps with a UI: build an HTML string with inline CSS/JS.
    // For APIs: return JSON.

    let body = "<!DOCTYPE html>\
<html><head><meta charset=\"utf-8\"><title>__SLUG__</title>\
<style>body{background:#0a0a0a;color:#e0e0e0;font-family:monospace;display:flex;justify-content:center;align-items:center;height:100vh;margin:0}</style>\
</head><body><h1>__SLUG__</h1><p>WASM cartridge running.</p></body></html>";
    respond(200, body, "text/html");
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

/// Generate the lib.rs template for an INTERACTIVE cartridge.
///
/// Interactive cartridges render to a framebuffer at 60fps.
/// The host reads pixel data from WASM memory and blits to canvas.
/// No browser APIs needed — pure Rust computation.
pub fn default_interactive_lib_rs(slug: &str) -> String {
    let template = r##"//! __SLUG__ — interactive x402 WASM cartridge
//!
//! This cartridge renders to a framebuffer. The host calls x402_tick()
//! every frame, reads pixels via x402_get_framebuffer(), and blits to canvas.
//! Arrow keys are forwarded via x402_key_down/x402_key_up.

#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }

const WIDTH: usize = 320;
const HEIGHT: usize = 240;
const FB_SIZE: usize = WIDTH * HEIGHT * 4; // RGBA

static mut FB: [u8; FB_SIZE] = [0u8; FB_SIZE];
static mut W: usize = WIDTH;
static mut H: usize = HEIGHT;

// Game state
static mut PX: i32 = 160;
static mut PY: i32 = 120;
static mut VX: i32 = 2;
static mut VY: i32 = 1;
static mut KEY_LEFT: bool = false;
static mut KEY_RIGHT: bool = false;
static mut KEY_UP: bool = false;
static mut KEY_DOWN: bool = false;

/// Set a pixel in the framebuffer.
fn set_pixel(x: usize, y: usize, r: u8, g: u8, b: u8) {
    unsafe {
        if x < W && y < H {
            let i = (y * W + x) * 4;
            FB[i] = r;
            FB[i + 1] = g;
            FB[i + 2] = b;
            FB[i + 3] = 255;
        }
    }
}

/// Clear the framebuffer.
fn clear(r: u8, g: u8, b: u8) {
    unsafe {
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) * 4;
                FB[i] = r;
                FB[i + 1] = g;
                FB[i + 2] = b;
                FB[i + 3] = 255;
            }
        }
    }
}

/// Draw a filled rectangle.
fn fill_rect(x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= 0 && py >= 0 {
                set_pixel(px as usize, py as usize, r, g, b);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn x402_init(w: i32, h: i32) {
    unsafe {
        W = w as usize;
        H = h as usize;
    }
}

#[no_mangle]
pub extern "C" fn x402_tick() {
    unsafe {
        // Handle input
        if KEY_LEFT { VX = -2; }
        if KEY_RIGHT { VX = 2; }
        if KEY_UP { VY = -2; }
        if KEY_DOWN { VY = 2; }

        // Move
        PX += VX;
        PY += VY;

        // Bounce off walls
        if PX <= 0 || PX >= (W as i32 - 20) { VX = -VX; PX += VX; }
        if PY <= 0 || PY >= (H as i32 - 20) { VY = -VY; PY += VY; }

        // Clear to dark background
        clear(10, 10, 20);

        // Draw the square
        fill_rect(PX, PY, 20, 20, 0, 200, 100);

        // Draw border
        for x in 0..W { set_pixel(x, 0, 40, 40, 60); set_pixel(x, H - 1, 40, 40, 60); }
        for y in 0..H { set_pixel(0, y, 40, 40, 60); set_pixel(W - 1, y, 40, 40, 60); }
    }
}

#[no_mangle]
pub extern "C" fn x402_key_down(code: i32) {
    unsafe {
        match code {
            37 => KEY_LEFT = true,   // Left arrow
            38 => KEY_UP = true,     // Up arrow
            39 => KEY_RIGHT = true,  // Right arrow
            40 => KEY_DOWN = true,   // Down arrow
            _ => {}
        }
    }
}

#[no_mangle]
pub extern "C" fn x402_key_up(code: i32) {
    unsafe {
        match code {
            37 => KEY_LEFT = false,
            38 => KEY_UP = false,
            39 => KEY_RIGHT = false,
            40 => KEY_DOWN = false,
            _ => {}
        }
    }
}

#[no_mangle]
pub extern "C" fn x402_get_framebuffer() -> *const u8 {
    unsafe { FB.as_ptr() }
}

#[no_mangle]
pub extern "C" fn x402_get_width() -> i32 {
    unsafe { W as i32 }
}

#[no_mangle]
pub extern "C" fn x402_get_height() -> i32 {
    unsafe { H as i32 }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    let layout = core::alloc::Layout::from_size_align(size as usize, 1).unwrap();
    unsafe { std::alloc::alloc(layout) }
}
"##;
    template.replace("__SLUG__", slug)
}

// ════════════════════════════════════════════════════════════════════
// FRONTEND CARTRIDGES — Leptos apps compiled to wasm32-unknown-unknown
// ════════════════════════════════════════════════════════════════════

/// Compile a frontend cartridge (Leptos app → wasm-bindgen).
///
/// 1. `cargo build --target wasm32-unknown-unknown --release`
/// 2. `wasm-bindgen --target web --out-dir {output_dir}/pkg {wasm_file}`
///
/// Returns the path to the output `pkg/` directory containing the JS glue + WASM.
pub async fn compile_frontend_cartridge(
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

    tokio::fs::create_dir_all(output_dir).await?;

    // Ensure wasm32-unknown-unknown target
    let target_check = tokio::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .await;
    let has_target = target_check
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("wasm32-unknown-unknown"))
        .unwrap_or(false);
    if !has_target {
        tracing::info!("Installing wasm32-unknown-unknown target for frontend cartridge");
        let _ = tokio::process::Command::new("rustup")
            .args(["target", "add", "wasm32-unknown-unknown"])
            .output()
            .await;
    }

    let target_dir = format!(
        "/tmp/cartridge-frontend-build-{}",
        source_dir.file_name().unwrap_or_default().to_string_lossy()
    );

    // Step 1: cargo build
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(COMPILE_TIMEOUT_SECS),
        tokio::process::Command::new("cargo")
            .args([
                "build",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "--manifest-path",
            ])
            .arg(cargo_toml.to_string_lossy().as_ref())
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("RUSTUP_HOME", "/usr/local/rustup")
            .env("CARGO_HOME", "/usr/local/cargo")
            .output(),
    )
    .await
    .map_err(|_| {
        CartridgeError::CompilationFailed(format!(
            "frontend compilation timed out after {COMPILE_TIMEOUT_SECS}s"
        ))
    })?
    .map_err(|e| CartridgeError::CompilationFailed(format!("cargo failed to start: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let truncated = if stderr.len() > 4096 {
            format!("{}...(truncated)", &stderr[..4096])
        } else {
            stderr.to_string()
        };
        return Err(CartridgeError::CompilationFailed(truncated));
    }

    // Find the .wasm binary
    let release_dir = format!("{}/wasm32-unknown-unknown/release", target_dir);
    let mut wasm_file = None;
    if let Ok(mut entries) = tokio::fs::read_dir(&release_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().map(|e| e == "wasm").unwrap_or(false)
                && !path.to_string_lossy().contains(".d")
            {
                wasm_file = Some(path);
                break;
            }
        }
    }

    let wasm_path = wasm_file.ok_or_else(|| {
        CartridgeError::CompilationFailed(format!(
            "no .wasm binary found in {}/wasm32-unknown-unknown/release",
            target_dir
        ))
    })?;

    // Step 2: wasm-bindgen to generate JS glue + optimized WASM
    let pkg_dir = output_dir.join("pkg");
    tokio::fs::create_dir_all(&pkg_dir).await?;

    let bindgen_output = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::process::Command::new("wasm-bindgen")
            .args([
                "--target",
                "web",
                "--out-dir",
            ])
            .arg(pkg_dir.to_string_lossy().as_ref())
            .arg(wasm_path.to_string_lossy().as_ref())
            .output(),
    )
    .await
    .map_err(|_| CartridgeError::CompilationFailed("wasm-bindgen timed out".to_string()))?
    .map_err(|e| {
        CartridgeError::CompilationFailed(format!("wasm-bindgen failed to start: {e}"))
    })?;

    if !bindgen_output.status.success() {
        let stderr = String::from_utf8_lossy(&bindgen_output.stderr);
        return Err(CartridgeError::CompilationFailed(format!(
            "wasm-bindgen failed: {stderr}"
        )));
    }

    // Clean up cargo build directory
    let _ = tokio::fs::remove_dir_all(&target_dir).await;

    Ok(pkg_dir)
}

/// Detect if a cartridge source is a frontend cartridge by checking Cargo.toml.
pub fn is_frontend_cartridge(source_dir: &Path) -> bool {
    let cargo_toml = source_dir.join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
        // If it depends on wasm-bindgen, it's a frontend cartridge
        content.contains("wasm-bindgen")
    } else {
        false
    }
}

/// Generate Cargo.toml for a frontend cartridge (Leptos app).
pub fn frontend_cargo_toml(slug: &str) -> String {
    format!(
        r#"[package]
name = "{slug}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
leptos = {{ version = "0.6", features = ["csr"] }}
wasm-bindgen = "=0.2.108"
web-sys = {{ version = "0.3", features = ["Document", "Element", "HtmlElement", "Window"] }}
console_error_panic_hook = "0.1"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
"#
    )
}

/// Generate lib.rs template for a frontend cartridge.
///
/// The cartridge exports `init(selector)` which mounts a Leptos component
/// into the given DOM element. The Studio calls this to load the app.
pub fn frontend_lib_rs(slug: &str) -> String {
    let template = r##"//! __SLUG__ — x402 frontend cartridge (Leptos app)
//!
//! This cartridge is a full Leptos app that mounts into the Studio.
//! The host calls `init("#mount-id")` to render it into a DOM element.

use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (count, set_count) = create_signal(0);

    view! {
        <div style="font-family: monospace; color: #e0e0e0; padding: 16px;">
            <h1 style="color: #00ff41;">"__SLUG__"</h1>
            <p>"A frontend cartridge running as a Leptos app."</p>
            <button
                style="background: #1a1a2e; color: #00ff41; border: 1px solid #00ff41; padding: 8px 16px; font-family: monospace; cursor: pointer;"
                on:click=move |_| set_count.update(|c| *c += 1)
            >
                "Clicked: " {count}
            </button>
        </div>
    }
}

/// Entry point — called by the Studio to mount this app into a DOM element.
#[wasm_bindgen]
pub fn init(selector: &str) {
    console_error_panic_hook::set_once();
    let document = web_sys::window().unwrap().document().unwrap();
    let el = document.query_selector(selector).unwrap().unwrap();
    mount_to(el.unchecked_into(), App);
}
"##;
    template.replace("__SLUG__", slug)
}
