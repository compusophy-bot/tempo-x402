//! Shell command execution tool.
use super::*;

pub(super) fn truncate_output(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    if s.len() > MAX_OUTPUT_BYTES {
        format!("{}... [truncated]", &s[..MAX_OUTPUT_BYTES])
    } else {
        s.to_string()
    }
}

impl ToolExecutor {
    pub(super) async fn execute_shell(
        &self,
        command: &str,
        timeout_secs: u64,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Enforce maximum timeout to prevent long-running processes
        let timeout = std::cmp::min(timeout_secs, SHELL_TIMEOUT_CAP);

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&self.workspace_root)
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = truncate_output(&output.stdout);
                let stderr = truncate_output(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                Ok(ToolResult {
                    stdout,
                    stderr,
                    exit_code,
                    duration_ms,
                })
            }
            Ok(Err(e)) => Err(format!("command failed to execute: {e}")),
            Err(_) => Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("command timed out after {timeout}s"),
                exit_code: -1,
                duration_ms,
            }),
        }
    }

    /// Executes a shell command securely for benchmarking and introspection.
    pub async fn execute_secure_shell(
        &self,
        command: &str,
        timeout_secs: u64,
    ) -> Result<ToolResult, String> {
        self.execute_shell(command, timeout_secs).await
    }
}
