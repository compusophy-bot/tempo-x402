//! Cartridge error types.

/// Errors from cartridge operations.
#[derive(Debug, thiserror::Error)]
pub enum CartridgeError {
    #[error("compilation failed: {0}")]
    CompilationFailed(String),

    #[error("module load failed: {0}")]
    ModuleLoadFailed(String),

    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    #[error("execution timed out after {0}s")]
    Timeout(u64),

    #[error("cartridge not found: {0}")]
    NotFound(String),

    #[error("ABI error: {0}")]
    Abi(String),

    #[error("host function error: {0}")]
    HostFunction(String),

    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
