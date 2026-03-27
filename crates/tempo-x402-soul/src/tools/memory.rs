//! Memory tools: persistent memory file updates.
use super::*;

impl ToolExecutor {
    pub(super) async fn update_memory(&self, content: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let bytes = persistent_memory::update(&self.memory_file_path, content)?;
        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "memory updated ({bytes} bytes written to {})",
                self.memory_file_path
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }
}
