//! CartridgeEngine — WASM module loading, caching, and execution.
//!
//! Pre-compiles .wasm files at load time and caches them.
//! Each request creates a fresh Store with its own KV state and limits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use dashmap::DashMap;
use wasmtime::{Engine, Linker, Module, Store};

use crate::abi;
use crate::error::CartridgeError;
use crate::manifest::{CartridgeRequest, CartridgeResult, PaymentContext};

/// Per-request state passed into WASM host functions.
pub struct CartridgeState {
    /// Cartridge-scoped key-value store (in-memory per request; persisted externally).
    pub kv_store: HashMap<String, String>,
    /// Payment context for the current request.
    pub payment: Option<PaymentContext>,
    /// Response set by the cartridge via x402_response.
    pub response_status: u16,
    pub response_body: String,
    pub response_content_type: String,
}

impl Default for CartridgeState {
    fn default() -> Self {
        Self {
            kv_store: HashMap::new(),
            payment: None,
            response_status: 200,
            response_body: String::new(),
            response_content_type: "application/json".to_string(),
        }
    }
}

/// Maximum WASM memory per cartridge (64MB).
const MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;

/// Maximum fuel (instruction count) per invocation.
const MAX_FUEL: u64 = 100_000_000;

/// The cartridge runtime engine.
pub struct CartridgeEngine {
    engine: Engine,
    /// Pre-compiled module cache: slug → Module.
    modules: DashMap<String, Module>,
    /// Base directory for cartridge storage.
    pub cartridge_dir: PathBuf,
}

impl CartridgeEngine {
    /// Create a new cartridge engine.
    pub fn new(cartridge_dir: impl Into<PathBuf>) -> Result<Self, CartridgeError> {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        config.wasm_memory64(false);

        let engine = Engine::new(&config)
            .map_err(|e| CartridgeError::ModuleLoadFailed(format!("engine init: {e}")))?;

        Ok(Self {
            engine,
            modules: DashMap::new(),
            cartridge_dir: cartridge_dir.into(),
        })
    }

    /// Load and pre-compile a WASM module from a file path.
    pub fn load_module(&self, slug: &str, wasm_path: &Path) -> Result<(), CartridgeError> {
        let wasm_bytes = std::fs::read(wasm_path)?;
        let module = Module::new(&self.engine, &wasm_bytes)
            .map_err(|e| CartridgeError::ModuleLoadFailed(format!("{slug}: {e}")))?;
        self.modules.insert(slug.to_string(), module);
        tracing::info!(slug, path = %wasm_path.display(), "Cartridge module loaded");
        Ok(())
    }

    /// Unload a cached module.
    pub fn unload_module(&self, slug: &str) {
        self.modules.remove(slug);
    }

    /// Atomic hot-swap: unload old module and load new one.
    /// If loading fails, the old module is already gone (no rollback).
    pub fn replace_module(&self, slug: &str, wasm_path: &Path) -> Result<(), CartridgeError> {
        self.unload_module(slug);
        self.load_module(slug, wasm_path)
    }

    /// Unload all cached modules.
    pub fn unload_all(&self) {
        self.modules.clear();
    }

    /// List loaded module slugs.
    pub fn loaded_slugs(&self) -> Vec<String> {
        self.modules.iter().map(|e| e.key().clone()).collect()
    }

    /// Execute a cartridge with a request. Returns the response and the modified KV store.
    pub fn execute(
        &self,
        slug: &str,
        request: &CartridgeRequest,
        kv_preload: HashMap<String, String>,
        timeout_secs: u64,
    ) -> Result<(CartridgeResult, HashMap<String, String>), CartridgeError> {
        let module = self
            .modules
            .get(slug)
            .ok_or_else(|| CartridgeError::NotFound(slug.to_string()))?;

        let start = Instant::now();

        // Create per-request store with limits
        let state = CartridgeState {
            kv_store: kv_preload,
            payment: request.payment.clone(),
            ..Default::default()
        };

        let mut store = Store::new(&self.engine, state);
        store
            .set_fuel(MAX_FUEL)
            .map_err(|e| CartridgeError::ExecutionFailed(format!("fuel setup: {e}")))?;

        // Create linker and register host functions
        let mut linker = Linker::new(&self.engine);
        abi::register_host_functions(&mut linker)?;

        // Instantiate
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| CartridgeError::ExecutionFailed(format!("instantiate: {e}")))?;

        // Call x402_init if exported
        if let Ok(init_fn) = instance.get_typed_func::<(), i32>(&mut store, "x402_init") {
            let result = init_fn
                .call(&mut store, ())
                .map_err(|e| CartridgeError::ExecutionFailed(format!("x402_init: {e}")))?;
            if result != 0 {
                return Err(CartridgeError::ExecutionFailed(format!(
                    "x402_init returned {result}"
                )));
            }
        }

        // Prepare request data in guest memory
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| CartridgeError::Abi("no memory export".to_string()))?;

        // Check memory size limit
        if memory.data_size(&store) > MAX_MEMORY_BYTES {
            return Err(CartridgeError::ResourceLimit(
                "memory exceeds 64MB".to_string(),
            ));
        }

        // Serialize request as JSON and write to guest memory
        let request_json = serde_json::to_string(request)?;
        let request_bytes = request_json.as_bytes();

        // Try to get the guest allocator
        let alloc_fn = instance
            .get_typed_func::<i32, i32>(&mut store, "x402_alloc")
            .ok();

        let req_ptr = if let Some(ref alloc) = alloc_fn {
            let ptr = alloc
                .call(&mut store, request_bytes.len() as i32)
                .map_err(|e| CartridgeError::Abi(format!("alloc: {e}")))?;
            ptr as usize
        } else {
            // Write at beginning of memory (simple cartridges)
            0
        };

        // Write request JSON to guest memory
        let mem_data = memory.data_mut(&mut store);
        if req_ptr + request_bytes.len() > mem_data.len() {
            return Err(CartridgeError::ResourceLimit(
                "request too large for guest memory".to_string(),
            ));
        }
        mem_data[req_ptr..req_ptr + request_bytes.len()].copy_from_slice(request_bytes);

        // Call x402_handle(request_ptr: i32, request_len: i32)
        let handle_fn = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "x402_handle")
            .map_err(|e| CartridgeError::Abi(format!("no x402_handle export: {e}")))?;

        // Execute with timeout
        let result = std::thread::scope(|s| {
            let handle = s
                .spawn(|| handle_fn.call(&mut store, (req_ptr as i32, request_bytes.len() as i32)));

            // Wait with timeout
            let deadline = std::time::Duration::from_secs(timeout_secs);
            let start_wait = Instant::now();
            loop {
                if handle.is_finished() {
                    return handle.join().unwrap();
                }
                if start_wait.elapsed() > deadline {
                    return Err(wasmtime::Error::msg("execution timed out"));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        match result {
            Ok(()) => {}
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("timed out") {
                    return Err(CartridgeError::Timeout(timeout_secs));
                }
                if msg.contains("fuel") {
                    return Err(CartridgeError::ResourceLimit(
                        "CPU fuel exhausted".to_string(),
                    ));
                }
                return Err(CartridgeError::ExecutionFailed(msg));
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // Read response and KV from store state
        let state = store.data();
        let result = CartridgeResult {
            status: state.response_status,
            body: state.response_body.clone(),
            content_type: state.response_content_type.clone(),
            duration_ms,
        };
        let kv_out = state.kv_store.clone();
        Ok((result, kv_out))
    }

    /// Compute SHA-256 hash of a WASM binary file.
    pub fn hash_wasm(path: &Path) -> Result<String, CartridgeError> {
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(path)?;
        let hash = Sha256::digest(&bytes);
        Ok(format!("{:x}", hash))
    }
}
