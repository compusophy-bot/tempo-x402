//! Cartridge tools — create, compile, test, and list WASM cartridges.

use super::*;

/// Generate a cognitive cartridge template for the given system.
/// The cartridge receives JSON input via x402_handle and returns JSON predictions.
fn cognitive_cartridge_template(system: &str) -> String {
    let template = r##"#![no_std]

#[link(wasm_import_module = "x402")]
extern "C" {
    fn log(level: i32, msg_ptr: *const u8, msg_len: i32);
    fn response(status: i32, body_ptr: *const u8, body_len: i32, ct_ptr: *const u8, ct_len: i32);
}

static mut SCRATCH: [u8; 65536] = [0u8; 65536];

fn respond(status: i32, body: &str) {
    let ct = "application/json";
    unsafe {
        response(
            status,
            body.as_ptr(),
            body.len() as i32,
            ct.as_ptr(),
            ct.len() as i32,
        );
    }
}

fn log_info(msg: &str) {
    unsafe { log(1, msg.as_ptr(), msg.len() as i32); }
}

#[no_mangle]
pub extern "C" fn x402_handle(req_ptr: i32, req_len: i32) {
    log_info("cognitive-SYSTEM_NAME: handling request");

    // Default cognitive response — override with actual model logic
    let response_json = r#"{"success_prob":0.5,"likely_error":"unknown","error_confidence":0.5,"system":"SYSTEM_NAME","source":"cartridge"}"#;

    respond(200, response_json);
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
"##;
    template.replace("SYSTEM_NAME", system)
}

impl ToolExecutor {
    /// Create a new cartridge project with source code.
    pub(super) async fn create_cartridge(
        &self,
        slug: &str,
        source_code: Option<&str>,
        description: Option<&str>,
        interactive: bool,
        frontend: bool,
    ) -> Result<ToolResult, String> {
        // Validate slug
        if !slug
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("invalid slug: must be alphanumeric with hyphens/underscores".to_string());
        }

        let cartridge_dir = format!("/data/cartridges/{slug}");
        let src_dir = format!("{cartridge_dir}/src");
        let bin_dir = format!("{cartridge_dir}/bin");

        // Create directories
        std::fs::create_dir_all(format!("{src_dir}/src"))
            .map_err(|e| format!("failed to create source dir: {e}"))?;
        std::fs::create_dir_all(&bin_dir).map_err(|e| format!("failed to create bin dir: {e}"))?;

        // Write Cargo.toml + lib.rs based on cartridge type
        let (cargo_toml, lib_rs, cartridge_type) = if frontend {
            let lib = match source_code {
                Some(code) => {
                    // Transform user source to use init(selector) pattern instead of
                    // mount_to_body / wasm_bindgen(start). The Studio mounts cartridges
                    // into a specific DOM element, not <body>.
                    let mut transformed = code.to_string();
                    // Replace mount_to_body with mount_to using the selector
                    transformed = transformed.replace(
                        "mount_to_body(",
                        "{ let doc = web_sys::window().unwrap().document().unwrap(); let el = doc.query_selector(selector).unwrap().unwrap(); mount_to(el.unchecked_into(), ",
                    );
                    // Replace #[wasm_bindgen(start)] with regular init export
                    transformed = transformed.replace("#[wasm_bindgen(start)]", "#[wasm_bindgen]");
                    transformed = transformed.replace("#[wasm_bindgen::prelude::wasm_bindgen(start)]", "#[wasm_bindgen::prelude::wasm_bindgen]");
                    // Replace pub fn main() with pub fn init(selector: &str)
                    transformed = transformed.replace("pub fn main()", "pub fn init(selector: &str)");
                    transformed
                }
                None => x402_cartridge::compiler::frontend_lib_rs(slug),
            };
            (
                x402_cartridge::compiler::frontend_cargo_toml(slug),
                lib,
                "frontend (Leptos DOM app)",
            )
        } else if interactive {
            (
                x402_cartridge::compiler::default_cargo_toml(slug),
                x402_cartridge::compiler::default_interactive_lib_rs(slug),
                "interactive (60fps framebuffer)",
            )
        } else {
            (
                x402_cartridge::compiler::default_cargo_toml(slug),
                source_code
                    .map(String::from)
                    .unwrap_or_else(|| x402_cartridge::compiler::default_lib_rs(slug)),
                "backend (HTTP)",
            )
        };
        std::fs::write(format!("{src_dir}/Cargo.toml"), &cargo_toml)
            .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        std::fs::write(format!("{src_dir}/src/lib.rs"), &lib_rs)
            .map_err(|e| format!("failed to write lib.rs: {e}"))?;

        let desc = description.unwrap_or("WASM cartridge");

        Ok(ToolResult {
            stdout: format!(
                "Cartridge '{slug}' created at {src_dir} [{cartridge_type}]\n\
                 Description: {desc}\n\
                 Source: {src_dir}/src/lib.rs\n\
                 {interactive_note}\
                 Next: call compile_cartridge('{slug}') to build the WASM binary.",
                interactive_note = if frontend {
                    "This is a FRONTEND cartridge — a full Leptos app that mounts into the Studio.\n\
                     It compiles to wasm32-unknown-unknown via wasm-bindgen (not wasip1).\n\
                     IMPORTANT: Do NOT use mount_to_body() or #[wasm_bindgen(start)].\n\
                     Use init(selector) pattern: #[wasm_bindgen] pub fn init(selector: &str)\n\
                     with mount_to(el, App) to mount into the Studio's DOM element.\n\
                     You can use leptos view! macros, web-sys, and full DOM APIs.\n"
                } else if interactive {
                    "Template includes: x402_init, x402_tick, x402_key_down/up, x402_get_framebuffer,\n\
                     set_pixel, fill_rect, clear helpers. Edit the x402_tick() function to customize.\n\
                     Arrow keys: 37=Left, 38=Up, 39=Right, 40=Down, 32=Space.\n"
                } else {
                    "IMPORTANT: Do NOT add any dependencies to Cargo.toml.\n\
                     The host ABI uses #[link(wasm_import_module = \"x402\")] extern \"C\" functions.\n"
                }
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 0,
        })
    }

    /// Compile a cartridge from its source directory.
    pub(super) async fn compile_cartridge(&self, slug: &str) -> Result<ToolResult, String> {
        let src_dir = format!("/data/cartridges/{slug}/src");
        let bin_dir = format!("/data/cartridges/{slug}/bin");

        if !std::path::Path::new(&src_dir).join("Cargo.toml").exists() {
            return Err(format!(
                "no source found for cartridge '{slug}' — create it first"
            ));
        }

        let is_frontend =
            x402_cartridge::compiler::is_frontend_cartridge(std::path::Path::new(&src_dir));

        let start = std::time::Instant::now();
        if is_frontend {
            // Frontend cartridge: wasm32-unknown-unknown + wasm-bindgen
            match x402_cartridge::compiler::compile_frontend_cartridge(
                std::path::Path::new(&src_dir),
                std::path::Path::new(&bin_dir),
            )
            .await
            {
                Ok(pkg_dir) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    // Frontend cartridges are auto-registered by list_cartridges()
                    // when it scans /data/cartridges/ for dirs with bin/pkg/.
                    return Ok(ToolResult {
                        stdout: format!(
                            "Frontend cartridge '{slug}' compiled successfully!\n\
                             Package: {}\n\
                             Build time: {duration_ms}ms\n\
                             The cartridge is ready at /c/{slug} (frontend type — mounts into Studio DOM).",
                            pkg_dir.display()
                        ),
                        stderr: String::new(),
                        exit_code: 0,
                        duration_ms,
                    });
                }
                Err(e) => {
                    return Ok(ToolResult {
                        stdout: String::new(),
                        stderr: format!("Frontend compilation failed:\n{e}"),
                        exit_code: 1,
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                }
            }
        }

        // Backend/interactive cartridge: wasm32-wasip1
        match x402_cartridge::compiler::compile_cartridge(
            std::path::Path::new(&src_dir),
            std::path::Path::new(&bin_dir),
        )
        .await
        {
            Ok(wasm_path) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let hash = x402_cartridge::CartridgeEngine::hash_wasm(&wasm_path)
                    .unwrap_or_else(|_| "unknown".to_string());
                let size = std::fs::metadata(&wasm_path).map(|m| m.len()).unwrap_or(0);

                // Load into the shared engine so /c/{slug} works immediately
                // and the list endpoint can auto-register it in the DB.
                let mut load_status = String::new();
                if let Some(ref engine) = self.cartridge_engine {
                    match engine.replace_module(slug, &wasm_path) {
                        Ok(()) => load_status = "Loaded into runtime (hot-reloaded).".to_string(),
                        Err(e) => {
                            load_status = format!("Warning: failed to load into runtime: {e}")
                        }
                    }
                }

                Ok(ToolResult {
                    stdout: format!(
                        "Cartridge '{slug}' compiled successfully!\n\
                         WASM: {}\n\
                         Size: {} bytes\n\
                         Hash: {hash}\n\
                         Build time: {duration_ms}ms\n\
                         {load_status}\n\
                         The cartridge is ready to serve at /c/{slug}",
                        wasm_path.display(),
                        size
                    ),
                    stderr: String::new(),
                    exit_code: 0,
                    duration_ms,
                })
            }
            Err(e) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("Compilation failed:\n{e}"),
                exit_code: 1,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    /// Test a cartridge by executing it with sample input.
    pub(super) async fn test_cartridge(
        &self,
        slug: &str,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<ToolResult, String> {
        let bin_dir = format!("/data/cartridges/{slug}/bin");

        // Find the .wasm file
        let wasm_path = std::fs::read_dir(&bin_dir)
            .map_err(|e| format!("no bin dir: {e}"))?
            .filter_map(|e| e.ok())
            .find(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "wasm")
                    .unwrap_or(false)
            })
            .map(|e| e.path())
            .ok_or_else(|| format!("no .wasm binary found for '{slug}' — compile it first"))?;

        // Create a temporary engine and load the module
        let engine = x402_cartridge::CartridgeEngine::new("/data/cartridges")
            .map_err(|e| format!("engine init failed: {e}"))?;

        engine
            .load_module(slug, &wasm_path)
            .map_err(|e| format!("module load failed: {e}"))?;

        let request = x402_cartridge::CartridgeRequest {
            method: method.to_string(),
            path: path.to_string(),
            body: body.to_string(),
            headers: std::collections::HashMap::new(),
            payment: None,
        };

        let start = std::time::Instant::now();
        match engine.execute(slug, &request, Default::default(), 10) {
            Ok((result, _kv)) => Ok(ToolResult {
                stdout: format!(
                    "Status: {}\nContent-Type: {}\nDuration: {}ms\n\nBody:\n{}",
                    result.status, result.content_type, result.duration_ms, result.body
                ),
                stderr: String::new(),
                exit_code: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("Execution failed: {e}"),
                exit_code: 1,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    /// Create a cognitive cartridge — a hot-swappable brain module.
    pub(super) async fn create_cognitive_cartridge(
        &self,
        system: &str,
        description: Option<&str>,
    ) -> Result<ToolResult, String> {
        let valid_systems = [
            "brain",
            "cortex",
            "genesis",
            "hivemind",
            "synthesis",
            "unified",
        ];
        if !valid_systems.contains(&system) {
            return Err(format!(
                "invalid cognitive system '{system}'. Valid: {}",
                valid_systems.join(", ")
            ));
        }

        let slug = format!("cognitive-{system}");
        let cartridge_dir = format!("/data/cartridges/{slug}");
        let src_dir = format!("{cartridge_dir}/src");
        let bin_dir = format!("{cartridge_dir}/bin");

        std::fs::create_dir_all(format!("{src_dir}/src"))
            .map_err(|e| format!("failed to create source dir: {e}"))?;
        std::fs::create_dir_all(&bin_dir).map_err(|e| format!("failed to create bin dir: {e}"))?;

        let cargo_toml = x402_cartridge::compiler::default_cargo_toml(&slug);
        let lib_rs = cognitive_cartridge_template(system);

        std::fs::write(format!("{src_dir}/Cargo.toml"), &cargo_toml)
            .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        std::fs::write(format!("{src_dir}/src/lib.rs"), &lib_rs)
            .map_err(|e| format!("failed to write lib.rs: {e}"))?;

        let desc = description.unwrap_or("Cognitive cartridge");

        Ok(ToolResult {
            stdout: format!(
                "Cognitive cartridge '{slug}' created [{system} system]\n\
                 Description: {desc}\n\
                 Source: {src_dir}/src/lib.rs\n\
                 IMPORTANT: Do NOT add dependencies. Uses x402 host ABI.\n\
                 The cartridge receives JSON requests and returns JSON predictions.\n\
                 Next: compile_cartridge('{slug}') to build and hot-swap into the cognitive orchestrator."
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 0,
        })
    }

    /// List all cartridges on disk.
    pub(super) async fn list_cartridges(&self) -> Result<ToolResult, String> {
        let cartridge_dir = "/data/cartridges";
        let mut entries = Vec::new();

        if let Ok(dir) = std::fs::read_dir(cartridge_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let slug = path.file_name().unwrap().to_string_lossy().to_string();
                    let has_source = path.join("src/Cargo.toml").exists();
                    let has_binary = path
                        .join("bin")
                        .read_dir()
                        .ok()
                        .and_then(|mut d| {
                            d.find(|e| {
                                e.as_ref()
                                    .ok()
                                    .map(|e| {
                                        e.path()
                                            .extension()
                                            .map(|ext| ext == "wasm")
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false)
                            })
                        })
                        .is_some();
                    entries.push(format!(
                        "- {slug} [source:{}, binary:{}]",
                        if has_source { "yes" } else { "no" },
                        if has_binary { "yes" } else { "no" }
                    ));
                }
            }
        }

        if entries.is_empty() {
            Ok(ToolResult {
                stdout: "No cartridges found. Use create_cartridge to create one.".to_string(),
                stderr: String::new(),
                exit_code: 0,
                duration_ms: 0,
            })
        } else {
            Ok(ToolResult {
                stdout: format!("Cartridges ({}):\n{}", entries.len(), entries.join("\n")),
                stderr: String::new(),
                exit_code: 0,
                duration_ms: 0,
            })
        }
    }
}
