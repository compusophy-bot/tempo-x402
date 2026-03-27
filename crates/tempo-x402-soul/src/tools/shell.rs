//! Shell command execution tool.
use super::*;

impl ToolExecutor {
    pub(super) async fn execute_shell(
        &self,
        command: &str,
        timeout_secs: u64,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
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
                stderr: format!("command timed out after {timeout_secs}s"),
                exit_code: -1,
                duration_ms,
            }),
        }
    }
}
