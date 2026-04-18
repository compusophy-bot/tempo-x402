//! Admin endpoints — exec, workspace reset, cargo check, file browser.

use super::*;

fn verify_admin(req: &actix_web::HttpRequest) -> bool {
    let token = std::env::var("SOUL_ADMIN_TOKEN")
        .or_else(|_| std::env::var("GEMINI_API_KEY").map(|k| k.chars().take(16).collect()))
        .unwrap_or_default();
    if token.is_empty() {
        return false;
    }
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v) == token)
        .unwrap_or(false)
}

#[derive(Deserialize)]
pub(super) struct ExecRequest {
    command: String,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}
fn default_timeout() -> u64 {
    30
}

/// POST /soul/admin/exec — execute a shell command directly on the agent.
/// Auth: Bearer token (SOUL_ADMIN_TOKEN or first 16 chars of GEMINI_API_KEY).
pub(super) async fn admin_exec(
    req: actix_web::HttpRequest,
    body: web::Json<ExecRequest>,
) -> HttpResponse {
    if !verify_admin(&req) {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "invalid admin token"}));
    }

    let timeout = std::time::Duration::from_secs(body.timeout_secs.min(120));
    match tokio::time::timeout(
        timeout,
        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&body.command)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            HttpResponse::Ok().json(serde_json::json!({
                "exit_code": output.status.code().unwrap_or(-1),
                "stdout": &stdout[..stdout.len().min(8000)],
                "stderr": &stderr[..stderr.len().min(4000)],
            }))
        }
        Ok(Err(e)) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
        Err(_) => {
            HttpResponse::GatewayTimeout().json(serde_json::json!({"error": "command timed out"}))
        }
    }
}

/// POST /soul/admin/workspace-reset — reset workspace to clean state.
pub(super) async fn admin_workspace_reset(req: actix_web::HttpRequest) -> HttpResponse {
    if !verify_admin(&req) {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "invalid admin token"}));
    }

    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let script = format!(
        "rm -rf {ws}/target /tmp/x402_cargo_target {ws}/.cargo 2>/dev/null; \
         echo \"Cleaned: $(du -sh {ws} 2>/dev/null | cut -f1) workspace, $(du -sh /data 2>/dev/null | cut -f1) total\"; \
         cd {ws} && \
         git stash 2>/dev/null; \
         git fetch origin main 2>&1 && \
         git reset --hard origin/main 2>&1 && \
         git clean -fd 2>&1 && \
         echo '=== WORKSPACE RESET OK ===' && \
         git log --oneline -3"
    );

    match tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&script)
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            HttpResponse::Ok().json(serde_json::json!({
                "success": output.status.success(),
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string(),
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
    }
}

/// POST /soul/admin/cargo-check — run cargo check and return results.
pub(super) async fn admin_cargo_check(req: actix_web::HttpRequest) -> HttpResponse {
    if !verify_admin(&req) {
        return HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "invalid admin token"}));
    }

    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let script = format!("cd {ws} && cargo check --workspace 2>&1 | tail -40");

    match tokio::time::timeout(
        std::time::Duration::from_secs(120),
        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let passed = output.status.success();
            HttpResponse::Ok().json(serde_json::json!({
                "passed": passed,
                "output": stdout.to_string(),
            }))
        }
        Ok(Err(e)) => {
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{e}")}))
        }
        Err(_) => HttpResponse::GatewayTimeout()
            .json(serde_json::json!({"error": "cargo check timed out (120s)"})),
    }
}

/// GET /soul/admin/ls?path=src — list files in workspace directory.
/// No admin auth — read-only, safe for the Studio IDE.
pub(super) async fn admin_ls(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let rel_path = query
        .get("path")
        .cloned()
        .unwrap_or_else(|| ".".to_string());

    // Sanitize path — no traversal
    let sanitized = rel_path.replace("..", "").replace("//", "/");
    let full_path = format!("{}/{}", ws, sanitized);

    match tokio::fs::read_dir(&full_path).await {
        Ok(mut entries) => {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                // Skip hidden files and target dirs
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                let meta = entry.metadata().await.ok();
                let entry_type = if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                    "directory"
                } else {
                    "file"
                };
                let size = meta.as_ref().map(|m| m.len());
                files.push(serde_json::json!({
                    "name": name,
                    "type": entry_type,
                    "size": size,
                }));
            }
            // Sort: directories first, then alphabetical
            files.sort_by(|a, b| {
                let a_dir = a.get("type").and_then(|v| v.as_str()) == Some("directory");
                let b_dir = b.get("type").and_then(|v| v.as_str()) == Some("directory");
                match (a_dir, b_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => {
                        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        a_name.cmp(b_name)
                    }
                }
            });
            HttpResponse::Ok().json(files)
        }
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Cannot read directory: {e}"),
            "path": full_path,
        })),
    }
}

/// GET /soul/admin/cat?path=src/lib.rs — read file content from workspace.
/// No admin auth — read-only, safe for the Studio IDE.
pub(super) async fn admin_cat(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let ws = std::env::var("SOUL_WORKSPACE_ROOT").unwrap_or_else(|_| "/data/workspace".to_string());
    let rel_path = match query.get("path") {
        Some(p) => p.clone(),
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": "path required"}))
        }
    };

    // Sanitize path — no traversal
    let sanitized = rel_path.replace("..", "").replace("//", "/");
    let full_path = format!("{}/{}", ws, sanitized);

    // Max 1MB file read
    match tokio::fs::read_to_string(&full_path).await {
        Ok(content) => {
            if content.len() > 1_048_576 {
                HttpResponse::Ok().body(content[..1_048_576].to_string())
            } else {
                HttpResponse::Ok().body(content)
            }
        }
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Cannot read file: {e}"),
            "path": full_path,
        })),
    }
}
