//! Cartridge tools — create, compile, test, and list WASM cartridges.

use super::*;

impl ToolExecutor {
    /// Create a new cartridge project with source code.
    pub(super) async fn create_cartridge(
        &self,
        slug: &str,
        source_code: Option<&str>,
        description: Option<&str>,
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

        // Write Cargo.toml
        let cargo_toml = x402_cartridge::compiler::default_cargo_toml(slug);
        std::fs::write(format!("{src_dir}/Cargo.toml"), &cargo_toml)
            .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;

        // Write lib.rs (user-provided or default template)
        let lib_rs = source_code
            .map(String::from)
            .unwrap_or_else(|| x402_cartridge::compiler::default_lib_rs(slug));
        std::fs::write(format!("{src_dir}/src/lib.rs"), &lib_rs)
            .map_err(|e| format!("failed to write lib.rs: {e}"))?;

        let desc = description.unwrap_or("WASM cartridge");

        Ok(ToolResult {
            stdout: format!(
                "Cartridge '{slug}' created at {src_dir}\n\
                 Description: {desc}\n\
                 Source: {src_dir}/src/lib.rs\n\
                 IMPORTANT: Do NOT add any dependencies to Cargo.toml. No x402_sdk, no external crates.\n\
                 The host ABI uses #[link(wasm_import_module = \"x402\")] extern \"C\" functions.\n\
                 See the template in lib.rs — it has everything you need.\n\
                 Next: call compile_cartridge to build the WASM binary."
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

        let start = std::time::Instant::now();
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

                Ok(ToolResult {
                    stdout: format!(
                        "Cartridge '{slug}' compiled successfully!\n\
                         WASM: {}\n\
                         Size: {} bytes\n\
                         Hash: {hash}\n\
                         Build time: {duration_ms}ms\n\
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
            Ok(result) => Ok(ToolResult {
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
