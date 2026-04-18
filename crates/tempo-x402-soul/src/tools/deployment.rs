//! Deployment and GitHub tools: Railway GraphQL, deploy status/logs/redeploy, GitHub repo create/fork.
use super::*;

impl ToolExecutor {
    /// Helper to make a Railway GraphQL API call.
    pub(super) async fn railway_graphql(&self, query: &str) -> Result<serde_json::Value, String> {
        let token = self
            .railway_token
            .as_ref()
            .ok_or("RAILWAY_TOKEN not configured")?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))?;

        let resp = client
            .post("https://backboard.railway.app/graphql/v2")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .map_err(|e| format!("Railway API request failed: {e}"))?;

        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Railway API response parse failed: {e}"))
    }

    /// Check the latest deployment status for this service on Railway.
    pub(super) async fn check_deploy_status(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let service_id = self
            .railway_service_id
            .as_ref()
            .ok_or("RAILWAY_SERVICE_ID not configured")?;
        let env_id = self
            .railway_environment_id
            .as_ref()
            .ok_or("RAILWAY_ENVIRONMENT_ID not configured")?;

        let query = format!(
            r#"{{ deployments(input: {{ serviceId: "{service_id}", environmentId: "{env_id}" }}, first: 3) {{ edges {{ node {{ id status createdAt updatedAt }} }} }} }}"#
        );

        let data = self.railway_graphql(&query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Format nicely for the LLM
        let edges = data
            .pointer("/data/deployments/edges")
            .and_then(|v| v.as_array());

        let mut output = String::new();
        if let Some(edges) = edges {
            for (i, edge) in edges.iter().enumerate() {
                let node = &edge["node"];
                let id = node["id"].as_str().unwrap_or("?");
                let status = node["status"].as_str().unwrap_or("?");
                let created = node["createdAt"].as_str().unwrap_or("?");
                let updated = node["updatedAt"].as_str().unwrap_or("?");
                output.push_str(&format!(
                    "{}. {} — status: {}, created: {}, updated: {}\n",
                    i + 1,
                    id,
                    status,
                    created,
                    updated
                ));
            }
        } else if let Some(errors) = data.get("errors") {
            output = format!("Railway API error: {errors}");
        } else {
            output = "No deployments found".to_string();
        }

        Ok(ToolResult {
            stdout: output,
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Get build logs for a Railway deployment.
    pub(super) async fn get_deploy_logs(
        &self,
        deployment_id: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let service_id = self
            .railway_service_id
            .as_ref()
            .ok_or("RAILWAY_SERVICE_ID not configured")?;
        let env_id = self
            .railway_environment_id
            .as_ref()
            .ok_or("RAILWAY_ENVIRONMENT_ID not configured")?;

        // If no deployment ID given, get the latest one first
        let deploy_id = if let Some(id) = deployment_id {
            id.to_string()
        } else {
            let query = format!(
                r#"{{ deployments(input: {{ serviceId: "{service_id}", environmentId: "{env_id}" }}, first: 1) {{ edges {{ node {{ id }} }} }} }}"#
            );
            let data = self.railway_graphql(&query).await?;
            data.pointer("/data/deployments/edges/0/node/id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or("No deployments found")?
        };

        let query = format!(
            r#"{{ buildLogs(deploymentId: "{deploy_id}", limit: 200) {{ message timestamp }} }}"#
        );

        let data = self.railway_graphql(&query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let mut output = format!("Build logs for deployment {deploy_id}:\n\n");

        if let Some(logs) = data.pointer("/data/buildLogs").and_then(|v| v.as_array()) {
            for log in logs {
                let msg = log["message"].as_str().unwrap_or("");
                output.push_str(msg);
                output.push('\n');
            }
            if logs.is_empty() {
                output.push_str("(no build logs available yet)\n");
            }
        } else if let Some(errors) = data.get("errors") {
            output = format!("Railway API error: {errors}");
        }

        // Truncate if too long
        if output.len() > MAX_OUTPUT_BYTES {
            output = output.chars().take(MAX_OUTPUT_BYTES).collect();
            output.push_str("\n... (truncated)");
        }

        Ok(ToolResult {
            stdout: output,
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Trigger a redeployment of this service on Railway.
    pub(super) async fn trigger_redeploy(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let service_id = self
            .railway_service_id
            .as_ref()
            .ok_or("RAILWAY_SERVICE_ID not configured")?;
        let env_id = self
            .railway_environment_id
            .as_ref()
            .ok_or("RAILWAY_ENVIRONMENT_ID not configured")?;

        let query = format!(
            r#"mutation {{ serviceInstanceRedeploy(serviceId: "{service_id}", environmentId: "{env_id}") }}"#
        );

        let data = self.railway_graphql(&query).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let success = data
            .pointer("/data/serviceInstanceRedeploy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(ToolResult {
            stdout: if success {
                "Redeployment triggered successfully. Use check_deploy_status to monitor progress."
                    .to_string()
            } else {
                format!("Redeploy response: {data}")
            },
            stderr: String::new(),
            exit_code: if success { 0 } else { 1 },
            duration_ms,
        })
    }

    /// Create a new GitHub repository.
    pub(super) async fn create_github_repo(
        &self,
        name: &str,
        description: Option<&str>,
        private: bool,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let token = std::env::var("GITHUB_TOKEN")
            .map_err(|_| "GITHUB_TOKEN not set — cannot create repos".to_string())?;

        // Validate name
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(
                "repo name must be alphanumeric with hyphens, underscores, or dots".to_string(),
            );
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        let mut body = serde_json::json!({
            "name": name,
            "private": private,
            "auto_init": true,
        });
        if let Some(desc) = description {
            body["description"] = serde_json::json!(desc);
        }

        let resp = client
            .post("https://api.github.com/user/repos")
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "x402-soul")
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("GitHub API error: {e}"))?;

        let status = resp.status();
        let resp_body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse GitHub response: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() {
            let html_url = resp_body["html_url"].as_str().unwrap_or("unknown");
            let clone_url = resp_body["clone_url"].as_str().unwrap_or("unknown");
            Ok(ToolResult {
                stdout: format!(
                    "Repository created successfully!\nURL: {html_url}\nClone: {clone_url}\nPrivate: {private}"
                ),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            let msg = resp_body["message"].as_str().unwrap_or("unknown error");
            Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("GitHub API error ({status}): {msg}"),
                exit_code: 1,
                duration_ms,
            })
        }
    }

    /// Fork an existing GitHub repository.
    pub(super) async fn fork_github_repo(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let token = std::env::var("GITHUB_TOKEN")
            .map_err(|_| "GITHUB_TOKEN not set — cannot fork repos".to_string())?;

        // Validate owner and repo to prevent URL injection
        if !owner
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("owner must be alphanumeric with hyphens or underscores only".to_string());
        }
        if !repo
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(
                "repo must be alphanumeric with hyphens, underscores, or dots only".to_string(),
            );
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        let url = format!("https://api.github.com/repos/{owner}/{repo}/forks");

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "x402-soul")
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| format!("GitHub API error: {e}"))?;

        let status = resp.status();
        let resp_body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse GitHub response: {e}"))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() || status.as_u16() == 202 {
            let html_url = resp_body["html_url"].as_str().unwrap_or("unknown");
            let full_name = resp_body["full_name"].as_str().unwrap_or("unknown");
            Ok(ToolResult {
                stdout: format!(
                    "Repository forked successfully!\nFork: {full_name}\nURL: {html_url}\nOriginal: {owner}/{repo}"
                ),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            let msg = resp_body["message"].as_str().unwrap_or("unknown error");
            Ok(ToolResult {
                stdout: String::new(),
                stderr: format!("GitHub API error ({status}): {msg}"),
                exit_code: 1,
                duration_ms,
            })
        }
    }
}
