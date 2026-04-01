//! File operation tools: read, write, edit, list directory, search.
use super::*;

impl ToolExecutor {
    pub(super) async fn read_file(
        &self,
        path: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let resolved = self.resolve_path(path)?;

        // Check file size first
        let metadata = tokio::fs::metadata(&resolved)
            .await
            .map_err(|e| format!("cannot read file: {e}"))?;

        if !metadata.is_file() {
            return Err(format!("not a file: {path}"));
        }

        if metadata.len() > MAX_READ_BYTES as u64 && offset.is_none() && limit.is_none() {
            return Err(format!(
                "file too large ({} bytes, max {}). Use offset/limit to read portions.",
                metadata.len(),
                MAX_READ_BYTES
            ));
        }

        let content = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| format!("failed to read file: {e}"))?;

        let lines: Vec<&str> = content.lines().collect();
        let start_line = offset.unwrap_or(0);
        let end_line = limit
            .map(|l| (start_line + l).min(lines.len()))
            .unwrap_or(lines.len());

        // Return raw content WITHOUT line numbers.
        // Previously used `cat -n` style formatting ({:>6}\t{line}) but the LLM
        // copied line numbers into edit_file's old_string, causing every edit to fail.
        let mut output = String::new();
        for line in lines.iter().skip(start_line).take(end_line - start_line) {
            output.push_str(line);
            output.push('\n');
        }

        // Truncate if still too large (char-safe to avoid panicking on multi-byte boundaries)
        if output.len() > MAX_OUTPUT_BYTES {
            output = output.chars().take(MAX_OUTPUT_BYTES).collect();
            output.push_str("\n... (truncated)");
        }

        // Feed .rs files to codegen training (the codebase IS training data)
        if resolved.ends_with(".rs") && output.len() >= 100 {
            if let Some(db) = &self.db {
                crate::codegen::record_training_example(db, &output, &format!("read:{}", path));
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: output,
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Write (create or overwrite) a file. Guard-checked.
    pub(super) async fn write_file(&self, path: &str, content: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Guard check on the raw path (before resolving, to catch traversal)
        guard::validate_write_target(path).map_err(|e| e.to_string())?;

        let resolved = self.resolve_path(path)?;
        let rel = self.relative_path(&resolved);
        guard::validate_write_target(&rel).map_err(|e| e.to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create parent directory: {e}"))?;
        }

        tokio::fs::write(&resolved, content)
            .await
            .map_err(|e| format!("failed to write file: {e}"))?;

        // Distill: LLM-generated .rs code → codegen training (teacher→student)
        if resolved.to_string_lossy().ends_with(".rs") && content.len() >= 100 {
            if let Some(db) = &self.db {
                crate::codegen::record_training_example(db, content, &format!("gemini:{}", path));
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!("wrote {} bytes to {path}", content.len()),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Edit a file via search-and-replace. The old_string must appear exactly once. Guard-checked.
    pub(super) async fn edit_file(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Guard check
        guard::validate_write_target(path).map_err(|e| e.to_string())?;

        let resolved = self.resolve_path(path)?;
        let rel = self.relative_path(&resolved);
        guard::validate_write_target(&rel).map_err(|e| e.to_string())?;

        let content = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| format!("failed to read file: {e}"))?;

        let count = content.matches(old_string).count();
        if count == 0 {
            return Err("old_string not found in file".to_string());
        }
        if count > 1 {
            return Err(format!(
                "old_string found {count} times — must be unique. Provide more context."
            ));
        }

        let new_content = content.replacen(old_string, new_string, 1);
        tokio::fs::write(&resolved, &new_content)
            .await
            .map_err(|e| format!("failed to write file: {e}"))?;

        // Distill: LLM-generated edits to .rs files → codegen training
        if resolved.to_string_lossy().ends_with(".rs") && new_string.len() >= 50 {
            if let Some(db) = &self.db {
                crate::codegen::record_training_example(db, new_string, &format!("edit:{}", path));
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!("edited {path}: replaced 1 occurrence"),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// List directory entries with type indicators.
    pub(super) async fn list_directory(&self, path: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let resolved = self.resolve_path(path)?;

        let metadata = tokio::fs::metadata(&resolved)
            .await
            .map_err(|e| format!("cannot access path: {e}"))?;

        if !metadata.is_dir() {
            return Err(format!("not a directory: {path}"));
        }

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&resolved)
            .await
            .map_err(|e| format!("failed to read directory: {e}"))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("failed to read entry: {e}"))?
        {
            if entries.len() >= MAX_DIR_ENTRIES {
                entries.push("... (truncated, too many entries)".to_string());
                break;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry.file_type().await;
            let indicator = match ft {
                Ok(ft) if ft.is_dir() => "/",
                Ok(ft) if ft.is_symlink() => "@",
                _ => "",
            };
            entries.push(format!("{name}{indicator}"));
        }

        entries.sort();

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: entries.join("\n"),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Search for a literal string pattern across files. Uses grep via shell internally for
    /// performance (avoids reimplementing recursive file walking + binary detection).
    pub(super) async fn search_files(
        &self,
        pattern: &str,
        path: Option<&str>,
        glob: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let search_path = path.unwrap_or(".");

        // Build grep command with safe quoting
        let mut cmd = format!("grep -rn --max-count={} -l", MAX_SEARCH_MATCHES);

        if let Some(g) = glob {
            cmd.push_str(&format!(" --include='{}'", g.replace('\'', "'\\''")));
        }

        // Use fixed-string mode for literal search (no regex interpretation)
        cmd.push_str(&format!(
            " -F -- '{}' '{}'",
            pattern.replace('\'', "'\\''"),
            search_path.replace('\'', "'\\''")
        ));

        // Run via shell (in workspace root)
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(&cmd)
                .current_dir(&self.workspace_root)
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let files = truncate_output(&output.stdout);
                if files.is_empty() {
                    Ok(ToolResult {
                        stdout: "no matches found".to_string(),
                        stderr: String::new(),
                        exit_code: 0,
                        duration_ms,
                    })
                } else {
                    // Now get context lines for matched files (limited)
                    let file_list: Vec<&str> = files.lines().take(MAX_SEARCH_MATCHES).collect();
                    let file_args: String = file_list
                        .iter()
                        .map(|f| format!("'{}'", f.replace('\'', "'\\''").trim()))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let context_cmd = format!(
                        "grep -n -F -- '{}' {} | head -{}",
                        pattern.replace('\'', "'\\''"),
                        file_args,
                        MAX_SEARCH_MATCHES * 3
                    );

                    let ctx_result = tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        tokio::process::Command::new("bash")
                            .arg("-c")
                            .arg(&context_cmd)
                            .current_dir(&self.workspace_root)
                            .output(),
                    )
                    .await;

                    let output_text = match ctx_result {
                        Ok(Ok(out)) => truncate_output(&out.stdout),
                        _ => files,
                    };

                    Ok(ToolResult {
                        stdout: output_text,
                        stderr: String::new(),
                        exit_code: 0,
                        duration_ms,
                    })
                }
            }
            Ok(Err(e)) => Err(format!("search failed: {e}")),
            Err(_) => Ok(ToolResult {
                stdout: String::new(),
                stderr: "search timed out after 30s".to_string(),
                exit_code: -1,
                duration_ms,
            }),
        }
    }
}
