//! Endpoint management tools: create, list, test, delete script endpoints and register gateway endpoints.
use super::*;

impl ToolExecutor {
    pub(super) async fn create_script_endpoint(
        &self,
        slug: &str,
        script: &str,
        description: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Server-side endpoint cap: prevent script endpoint spam
        let scripts_dir_check = std::path::PathBuf::from("/data/endpoints");
        if scripts_dir_check.exists() {
            let script_count = std::fs::read_dir(&scripts_dir_check)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|ext| ext == "sh").unwrap_or(false))
                        .count()
                })
                .unwrap_or(0);
            if script_count >= 10 {
                return Err(format!(
                    "script endpoint limit reached ({script_count}/10). \
                     Delete existing endpoints before creating new ones. \
                     Focus on improving code quality instead of creating more scripts."
                ));
            }

            // Duplicate detection: reject slugs too similar to existing endpoints
            let existing_slugs: Vec<String> = std::fs::read_dir(&scripts_dir_check)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    e.path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(String::from)
                })
                .collect();
            let new_words: std::collections::HashSet<&str> = slug.split('-').collect();
            for existing in &existing_slugs {
                let existing_words: std::collections::HashSet<&str> = existing.split('-').collect();
                let intersection = new_words.intersection(&existing_words).count();
                let union = new_words.union(&existing_words).count();
                if union > 0 {
                    let similarity = intersection as f64 / union as f64;
                    if similarity > 0.5 && slug != existing.as_str() {
                        return Err(format!(
                            "slug '{slug}' is too similar to existing endpoint '{existing}' \
                             (Jaccard similarity {:.0}%). Each endpoint must be genuinely unique. \
                             Try something completely different.",
                            similarity * 100.0
                        ));
                    }
                }
            }
        }

        // Strip "script-" prefix if the LLM redundantly added it (node auto-prefixes)
        let slug = slug.strip_prefix("script-").unwrap_or(slug);

        // Validate slug
        if !slug
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("slug must be alphanumeric with hyphens/underscores only".to_string());
        }
        if slug.len() > 64 {
            return Err("slug too long (max 64 chars)".to_string());
        }

        // Security: block scripts that try to read secrets from the host process
        let script_lower = script.to_lowercase();
        const BLOCKED_PATTERNS: &[&str] = &[
            "/proc/1/environ",
            "/proc/self/environ",
            "/proc/1/cmdline",
            "/proc/1/maps",
            "evm_private_key",
            "facilitator_private_key",
            "railway_token",
            "gemini_api_key",
            "github_token",
        ];
        for pattern in BLOCKED_PATTERNS {
            if script_lower.contains(pattern) {
                return Err(format!(
                    "script blocked: contains forbidden pattern '{pattern}' — scripts must not access host secrets"
                ));
            }
        }

        let scripts_dir = PathBuf::from("/data/endpoints");
        std::fs::create_dir_all(&scripts_dir)
            .map_err(|e| format!("failed to create scripts directory: {e}"))?;

        let script_path = scripts_dir.join(format!("{slug}.sh"));

        // Prepend description as comment if provided
        let full_script = if let Some(desc) = description {
            format!("# {desc}\n{script}")
        } else {
            script.to_string()
        };

        std::fs::write(&script_path, &full_script)
            .map_err(|e| format!("failed to write script: {e}"))?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
        }

        // Register with gateway DB so peers can discover it immediately
        let gateway_slug = format!("script-{slug}");
        let register_note = self
            .register_script_in_gateway(&gateway_slug, description)
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "Script endpoint created: /x/{slug}\n\
                 Script: {}\n\
                 Size: {} bytes\n\
                 Gateway: {register_note}\n\
                 Test it: curl https://{{your-domain}}/x/{slug}",
                script_path.display(),
                full_script.len()
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Register a script endpoint with the gateway's admin API so it appears in /endpoints.
    pub(super) async fn register_script_in_gateway(
        &self,
        slug: &str,
        description: Option<&str>,
    ) -> String {
        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);
        let url = format!("{}/admin/endpoints", gateway_url.trim_end_matches('/'));

        let body = serde_json::json!({
            "slug": slug,
            "description": description.unwrap_or("Script endpoint"),
        });

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();
        match client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                format!("registered as {slug} (discoverable by peers)")
            }
            Ok(resp) => {
                let status = resp.status();
                format!("registration returned {status} (will auto-register on restart)")
            }
            Err(e) => {
                tracing::warn!(slug = %slug, error = %e, "Failed to register script in gateway");
                format!("registration failed: {e} (will auto-register on restart)")
            }
        }
    }

    /// List all script endpoints in /data/endpoints/.
    pub(super) async fn list_script_endpoints(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let scripts_dir = PathBuf::from("/data/endpoints");

        if !scripts_dir.exists() {
            return Ok(ToolResult {
                stdout: "no script endpoints found (directory doesn't exist yet)".to_string(),
                stderr: String::new(),
                exit_code: 0,
                duration_ms: 0,
            });
        }

        let mut entries = Vec::new();
        if let Ok(dir) = std::fs::read_dir(&scripts_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "sh") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        let desc = std::fs::read_to_string(&path).ok().and_then(|c| {
                            c.lines()
                                .next()
                                .and_then(|l| l.strip_prefix("# ").map(String::from))
                        });
                        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        entries.push(format!(
                            "/x/{stem} — {} ({size} bytes)",
                            desc.unwrap_or_else(|| "no description".to_string())
                        ));
                    }
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: if entries.is_empty() {
                "no script endpoints found".to_string()
            } else {
                format!(
                    "{} script endpoints:\n{}",
                    entries.len(),
                    entries.join("\n")
                )
            },
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Test a script endpoint locally by running it with test input.
    pub(super) async fn test_script_endpoint(
        &self,
        slug: &str,
        input: &str,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        // Strip "script-" prefix if present (create_script_endpoint strips it too)
        let slug = slug.strip_prefix("script-").unwrap_or(slug);
        let script_path = PathBuf::from(format!("/data/endpoints/{slug}.sh"));

        if !script_path.exists() {
            return Err(format!("script endpoint '{slug}' not found"));
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new("bash")
                .arg(script_path.to_str().unwrap_or_default())
                .env("REQUEST_METHOD", "POST")
                .env("REQUEST_BODY", input)
                .env("QUERY_STRING", "")
                .env("REQUEST_HEADERS", "{}")
                .env("ENDPOINT_SLUG", slug)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: if output.status.success() {
                        format!("test passed (exit 0):\n{stdout}")
                    } else {
                        format!(
                            "test failed (exit {}):\nstdout: {stdout}\nstderr: {stderr}",
                            output.status.code().unwrap_or(-1)
                        )
                    },
                    stderr,
                    exit_code: output.status.code().unwrap_or(1),
                    duration_ms,
                })
            }
            Ok(Err(e)) => Err(format!("failed to run script: {e}")),
            Err(_) => Err("script timed out (10s limit for tests)".to_string()),
        }
    }

    /// Delete (deactivate) an endpoint on the local gateway.
    /// Uses the gateway's internal admin path — no payment required for own endpoints.
    pub(super) async fn delete_endpoint(&self, slug: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);

        // Call the gateway's admin delete endpoint (no payment needed for local)
        let url = format!(
            "{}/admin/endpoints/{}",
            gateway_url.trim_end_matches('/'),
            slug
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        match client.delete(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: body,
                    stderr: if status.is_success() {
                        String::new()
                    } else {
                        format!("delete returned status {status}")
                    },
                    exit_code: status.as_u16() as i32,
                    duration_ms,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: String::new(),
                    stderr: format!("delete request failed: {e}"),
                    exit_code: -1,
                    duration_ms,
                })
            }
        }
    }

    /// Register an endpoint on the gateway via x402 payment.
    pub(super) async fn register_endpoint(
        &self,
        slug: &str,
        target_url: &str,
        price: &str,
        description: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        if !self.coding_enabled {
            return Err("coding is not enabled (register_endpoint requires Code mode)".to_string());
        }

        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);

        let private_key = std::env::var("EVM_PRIVATE_KEY")
            .map_err(|_| "EVM_PRIVATE_KEY not set — cannot sign payment".to_string())?;

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();

        // Build registration body
        let mut body = serde_json::json!({
            "slug": slug,
            "target_url": target_url,
            "price": price,
        });
        if let Some(desc) = description {
            body["description"] = serde_json::Value::String(desc.to_string());
        }

        // Step 1: POST /register → expect 402
        let register_url = format!("{gateway_url}/register");
        let resp = client
            .post(&register_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("failed to POST /register: {e}"))?;

        if resp.status() != reqwest::StatusCode::PAYMENT_REQUIRED {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.is_success() {
                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ToolResult {
                    stdout: format!("endpoint registered (no payment needed): {text}"),
                    stderr: String::new(),
                    exit_code: 0,
                    duration_ms,
                });
            }
            return Err(format!("expected 402, got {status}: {text}"));
        }

        // Step 2: Parse PaymentRequirements from response
        let resp_json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse 402 response: {e}"))?;

        let accepts = resp_json
            .get("accepts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "402 response missing 'accepts' array".to_string())?;

        let req_value = accepts
            .first()
            .ok_or_else(|| "402 response 'accepts' array is empty".to_string())?;

        let requirements = parse_payment_requirements(req_value)?;

        // Step 3: Sign payment
        let signer = x402::wallet::WalletSigner::new(&private_key)
            .map_err(|e| format!("failed to create signer: {e}"))?;

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("system time error: {e}"))?
            .as_secs();

        let payment_b64 = signer
            .sign_payment(&requirements, now_secs)
            .map_err(|e| format!("failed to sign payment: {e}"))?;

        // Step 4: Retry with payment header
        let resp2 = client
            .post(&register_url)
            .header("PAYMENT-SIGNATURE", &payment_b64)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("failed to retry POST with payment: {e}"))?;

        let status = resp2.status();
        let text = resp2.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() {
            Ok(ToolResult {
                stdout: format!("endpoint /{slug} registered successfully: {text}"),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("registration failed ({status}): {text}"),
                exit_code: 1,
                duration_ms,
            })
        }
    }

    /// Check the node's own endpoints for self-introspection.
    /// Whitelisted to: health, analytics, analytics/{slug}, soul/status.
    pub(super) async fn check_self(&self, endpoint: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Whitelist check: only allow safe read-only endpoints
        let trimmed = endpoint.trim_start_matches('/');
        let allowed = trimmed == "health"
            || trimmed == "analytics"
            || trimmed == "soul/status"
            || trimmed.starts_with("analytics/");

        if !allowed {
            return Err(format!(
                "endpoint '/{trimmed}' not allowed. Use: health, analytics, analytics/{{slug}}, soul/status"
            ));
        }

        let default_url = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let gateway_url = self.gateway_url.clone().unwrap_or(default_url);

        let url = format!("{gateway_url}/{trimmed}");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let duration_ms = start.elapsed().as_millis() as u64;

                // Truncate body if huge
                let body_truncated = if body.len() > MAX_OUTPUT_BYTES {
                    format!(
                        "{}\n... (truncated)",
                        body.chars().take(MAX_OUTPUT_BYTES).collect::<String>()
                    )
                } else {
                    body
                };

                Ok(ToolResult {
                    stdout: body_truncated,
                    stderr: String::new(),
                    exit_code: status.as_u16() as i32,
                    duration_ms,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    stdout: String::new(),
                    stderr: format!("request failed: {e}"),
                    exit_code: -1,
                    duration_ms,
                })
            }
        }
    }
}
