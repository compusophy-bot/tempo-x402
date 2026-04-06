//! Social tools: reputation, identity, cloning, delegation, peer discovery, paid endpoints.
use super::*;

impl ToolExecutor {
    pub(super) async fn check_reputation(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Read config from env
        let registry_str = std::env::var("ERC8004_REPUTATION_REGISTRY").unwrap_or_default();
        let token_id_str = std::env::var("ERC8004_AGENT_TOKEN_ID").unwrap_or_default();
        let rpc_url = std::env::var("RPC_URL").unwrap_or_default();

        if registry_str.is_empty() || token_id_str.is_empty() || rpc_url.is_empty() {
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ToolResult {
                stdout: "ERC-8004 reputation not configured. Need: ERC8004_REPUTATION_REGISTRY, ERC8004_AGENT_TOKEN_ID, RPC_URL".to_string(),
                stderr: String::new(),
                exit_code: 1,
                duration_ms,
            });
        }

        // Use HTTP call to check_self pattern — query the chain via shell
        // This avoids adding alloy as a dependency to the soul crate.
        // We use a JSON-RPC eth_call via curl instead.
        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "Reputation registry: {}\nAgent token ID: {}\nUse execute_shell with 'curl' to query the contract directly, or check_self with 'analytics' to see payment stats as a proxy for reputation.",
                registry_str, token_id_str
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Update this agent's on-chain metadata URI.
    pub(super) async fn update_agent_metadata(
        &self,
        metadata_uri: &str,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let registry_str = std::env::var("ERC8004_IDENTITY_REGISTRY").unwrap_or_default();
        let token_id_str = std::env::var("ERC8004_AGENT_TOKEN_ID").unwrap_or_default();

        if registry_str.is_empty() || token_id_str.is_empty() {
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ToolResult {
                stdout: "ERC-8004 identity not configured. Need: ERC8004_IDENTITY_REGISTRY, ERC8004_AGENT_TOKEN_ID".to_string(),
                stderr: String::new(),
                exit_code: 1,
                duration_ms,
            });
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult {
            stdout: format!(
                "Identity registry: {}\nAgent token ID: {}\nRequested metadata URI: {}\nNote: On-chain metadata update requires a transaction. Use execute_shell to send the tx via cast or a script.",
                registry_str, token_id_str, metadata_uri
            ),
            stderr: String::new(),
            exit_code: 0,
            duration_ms,
        })
    }

    /// Trigger self-clone via the internal /clone/self endpoint (no x402 payment needed).
    pub(super) async fn clone_self(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let gateway_url =
            std::env::var("GATEWAY_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
        let url = format!("{}/clone/self", gateway_url.trim_end_matches('/'));

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();
        let resp = client
            .post(&url)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("clone_self request failed: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() {
            Ok(ToolResult {
                stdout: format!("Clone triggered successfully: {body}"),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            Err(format!("clone_self returned {status}: {body}"))
        }
    }

    /// Spawn a specialized child node — differentiated clone with a specific focus.
    /// Calls POST /clone/specialist on the gateway with specialization parameters.
    pub(super) async fn spawn_specialist(
        &self,
        specialization: &str,
        initial_goal: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        let gateway_url =
            std::env::var("GATEWAY_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
        let url = format!("{}/clone/specialist", gateway_url.trim_end_matches('/'));

        let mut body = serde_json::json!({
            "specialization": specialization,
        });
        if let Some(goal) = initial_goal {
            body["initial_goal"] = serde_json::json!(goal);
        }

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();
        let resp = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("spawn_specialist request failed: {e}"))?;

        let status = resp.status();
        let resp_body = resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() {
            Ok(ToolResult {
                stdout: format!(
                    "Specialist '{}' spawned successfully: {}",
                    specialization, resp_body
                ),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            Err(format!(
                "spawn_specialist returned {}: {}",
                status, resp_body
            ))
        }
    }

    /// Delegate a task to a child/peer node by sending a high-priority nudge.
    /// Discovers the target peer and POSTs to their /soul/nudge endpoint.
    pub(super) async fn delegate_task(
        &self,
        target: &str,
        task_description: &str,
        priority: u32,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        if target.is_empty() || task_description.is_empty() {
            return Err("target and task_description are required".to_string());
        }

        // Find target URL — could be instance_id, URL, or short name
        let target_url = if target.starts_with("http") {
            target.to_string()
        } else {
            // Try to find peer by instance_id from discovered peers
            let gateway_url = std::env::var("GATEWAY_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string());
            let peers_url = format!("{}/instance/children", gateway_url.trim_end_matches('/'));
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .unwrap_or_default();
            let resp = client
                .get(&peers_url)
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await
                .map_err(|e| format!("failed to list children: {e}"))?;
            let children: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("failed to parse children: {e}"))?;

            // Search by instance_id or partial match
            let found = children
                .as_array()
                .and_then(|arr| {
                    arr.iter().find(|c| {
                        let id = c.get("instance_id").and_then(|v| v.as_str()).unwrap_or("");
                        let url = c.get("url").and_then(|v| v.as_str()).unwrap_or("");
                        id == target || id.starts_with(target) || url.contains(target)
                    })
                })
                .and_then(|c| c.get("url").and_then(|v| v.as_str()))
                .map(String::from);

            match found {
                Some(url) => url,
                None => {
                    return Err(format!(
                        "Could not find peer with identifier '{}'. Use discover_peers first.",
                        target
                    ))
                }
            }
        };

        // Send nudge to the target node
        let nudge_url = format!("{}/soul/nudge", target_url.trim_end_matches('/'));
        let nudge_body = serde_json::json!({
            "content": format!("[DELEGATED TASK from parent] {}", task_description),
            "priority": priority.min(5),
        });

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();
        let resp = client
            .post(&nudge_url)
            .json(&nudge_body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("delegate_task nudge failed: {e}"))?;

        let status = resp.status();
        let resp_body = resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        if status.is_success() {
            Ok(ToolResult {
                stdout: format!(
                    "Task delegated to {} (priority {}): {}\nResponse: {}",
                    target_url,
                    priority,
                    task_description.chars().take(80).collect::<String>(),
                    resp_body,
                ),
                stderr: String::new(),
                exit_code: 0,
                duration_ms,
            })
        } else {
            Err(format!(
                "delegate_task to {} returned {}: {}",
                target_url, status, resp_body
            ))
        }
    }

    pub(super) async fn discover_peers(&self) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        // Try on-chain discovery first (decentralized, via ERC-8004 identity registry)
        let identity_registry = std::env::var("ERC8004_IDENTITY_REGISTRY")
            .ok()
            .and_then(|s| s.parse::<alloy::primitives::Address>().ok())
            .filter(|a| *a != alloy::primitives::Address::ZERO);

        if let Some(registry) = identity_registry {
            let rpc_url = std::env::var("RPC_URL")
                .unwrap_or_else(|_| "https://rpc.moderato.tempo.xyz".to_string());
            let self_address = std::env::var("EVM_ADDRESS")
                .ok()
                .and_then(|s| s.parse::<alloy::primitives::Address>().ok());

            let provider = alloy::providers::RootProvider::<alloy::network::Ethereum>::new_http(
                rpc_url.parse().map_err(|e| format!("bad RPC URL: {e}"))?,
            );

            match tokio::time::timeout(
                std::time::Duration::from_secs(15),
                x402_identity::discovery::discover_peers(&provider, registry, self_address, 50),
            )
            .await
            {
                Err(_) => {
                    tracing::debug!("On-chain peer discovery timed out after 15s, falling back to HTTP");
                }
                Ok(inner) => match inner {
                    Ok(peers) => {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        let output = serde_json::to_string_pretty(&serde_json::json!({
                            "source": "on-chain",
                            "registry": format!("{:#x}", registry),
                            "peers": peers,
                            "count": peers.len(),
                        }))
                        .unwrap_or_default();

                        let output_truncated = if output.len() > MAX_OUTPUT_BYTES {
                            format!(
                                "{}\n... (truncated)",
                                output.chars().take(MAX_OUTPUT_BYTES).collect::<String>()
                            )
                        } else {
                            output
                        };

                        return Ok(ToolResult {
                            stdout: output_truncated,
                            stderr: String::new(),
                            exit_code: 0,
                            duration_ms,
                        });
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "On-chain peer discovery failed, falling back to HTTP");
                    }
                }
            }
        }

        // Fallback: HTTP-based discovery via parent's /instance/siblings
        let default_local = format!(
            "http://localhost:{}",
            std::env::var("PORT").unwrap_or_else(|_| "4023".to_string())
        );
        let parent_url = std::env::var("PARENT_URL")
            .ok()
            .or_else(|| self.gateway_url.clone())
            .unwrap_or(default_local);

        let url = format!("{}/instance/siblings", parent_url.trim_end_matches('/'));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();

                if !status.is_success() {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(ToolResult {
                        stdout: String::new(),
                        stderr: format!(
                            "discover_peers: {} returned HTTP {} — {}",
                            url,
                            status.as_u16(),
                            body.chars().take(200).collect::<String>()
                        ),
                        exit_code: status.as_u16() as i32,
                        duration_ms,
                    });
                }

                // Sanitize entire response body — strip control characters that break JSON parsing
                let body = sanitize_json_body(&body);

                // Parse siblings and enrich each with /instance/info
                let siblings_json: serde_json::Value = match serde_json::from_str(&body) {
                    Ok(v) => v,
                    Err(e) => {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        return Ok(ToolResult {
                            stdout: String::new(),
                            stderr: format!(
                                "discover_peers: failed to parse response from {}: {} — body: {}",
                                url,
                                e,
                                body.chars().take(200).collect::<String>()
                            ),
                            exit_code: -1,
                            duration_ms,
                        });
                    }
                };
                let siblings = siblings_json
                    .get("siblings")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                // Filter out self from peer list
                let self_instance_id = std::env::var("INSTANCE_ID").unwrap_or_default();
                let self_address = std::env::var("EVM_ADDRESS")
                    .unwrap_or_default()
                    .to_lowercase();

                let mut enriched_peers = Vec::new();
                for sib in &siblings {
                    let inst_id = sib
                        .get("instance_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let sib_url = match sib.get("url").and_then(|v| v.as_str()) {
                        Some(u) => u,
                        None => continue,
                    };
                    let address = sib.get("address").and_then(|v| v.as_str());

                    // Skip self — avoid self-payment errors
                    if inst_id == self_instance_id {
                        tracing::debug!(instance_id = %inst_id, "Skipping self in peer list");
                        continue;
                    }
                    if let Some(addr) = address {
                        if addr.to_lowercase() == self_address {
                            tracing::debug!(address = %addr, "Skipping self (same address) in peer list");
                            continue;
                        }
                    }

                    // Fetch peer's /instance/info for endpoints + version
                    let info_url = format!("{}/instance/info", sib_url.trim_end_matches('/'));
                    let peer_info = match client.get(&info_url).send().await {
                        Ok(r) if r.status().is_success() => r.json().await.ok(),
                        Ok(r) => {
                            tracing::debug!(
                                instance_id = %inst_id,
                                status = %r.status(),
                                "Peer /instance/info returned non-2xx — skipping"
                            );
                            // Skip unreachable peers instead of adding with empty endpoints
                            continue;
                        }
                        Err(e) => {
                            tracing::debug!(
                                instance_id = %inst_id,
                                error = %e,
                                "Peer /instance/info unreachable — skipping"
                            );
                            continue;
                        }
                    };
                    let info_json: Option<serde_json::Value> = peer_info;

                    let version = info_json
                        .as_ref()
                        .and_then(|j| j.get("version"))
                        .and_then(|v| v.as_str());
                    let endpoints = info_json
                        .as_ref()
                        .and_then(|j| j.get("endpoints"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    // Build callable URLs so the LLM can pass them directly to call_paid_endpoint
                    let callable_endpoints: Vec<serde_json::Value> = endpoints
                        .iter()
                        .map(|ep| {
                            let slug = ep.get("slug").and_then(|s| s.as_str()).unwrap_or("");
                            let mut ep_clone = ep.clone();
                            if let Some(obj) = ep_clone.as_object_mut() {
                                obj.insert(
                                    "callable_url".to_string(),
                                    serde_json::Value::String(format!(
                                        "{}/g/{}",
                                        sib_url.trim_end_matches('/'),
                                        slug
                                    )),
                                );
                                // Sanitize description — strip control characters that break JSON parsing
                                if let Some(serde_json::Value::String(desc)) =
                                    obj.get("description")
                                {
                                    let clean: String = desc
                                        .chars()
                                        .filter(|c| {
                                            !c.is_control()
                                                || *c == '\n'
                                                || *c == '\r'
                                                || *c == '\t'
                                        })
                                        .collect();
                                    obj.insert(
                                        "description".to_string(),
                                        serde_json::Value::String(clean),
                                    );
                                }
                            }
                            ep_clone
                        })
                        .collect();

                    // ── x402 PAID peer data exchange ──
                    // Colony data exchange: call peer's FREE endpoints directly.
                    // Only /clone is a paid endpoint. All cognitive/status endpoints
                    // are free so agents can cooperate without burning tokens.

                    // 1. Fetch peer's soul status (free endpoint — no payment needed)
                    let soul_url = format!("{}/soul/status", sib_url.trim_end_matches('/'));
                    let paid_soul_data: Option<serde_json::Value> = match client
                        .get(&soul_url)
                        .timeout(std::time::Duration::from_secs(10))
                        .send()
                        .await
                    {
                        Ok(resp) if resp.status().is_success() => {
                            let body = resp.text().await.unwrap_or_default();
                            let body = sanitize_json_body(&body);
                            tracing::info!(peer = %inst_id, "Fetched peer soul status");
                            serde_json::from_str(&body).ok()
                        }
                        Ok(resp) => {
                            tracing::warn!(
                                peer = %inst_id,
                                status = %resp.status(),
                                "Peer soul status returned non-2xx"
                            );
                            None
                        }
                        Err(e) => {
                            tracing::warn!(peer = %inst_id, error = %e, "Peer soul status failed");
                            None
                        }
                    };

                    // Extract brain weights from paid soul response and merge
                    if let Some(ref db) = self.db {
                        if let Some(ref _soul_data) = paid_soul_data {
                            // The soul status includes brain info (train_steps, parameters)
                            // For full weight merging, we still need the weights endpoint
                            // but now we also track the paid call for coordination fitness
                        }

                        // Brain weight merge — still needs dedicated endpoint for full weights
                        // TODO: register brain/weights as a paid gateway endpoint
                        let brain_url =
                            format!("{}/soul/brain/weights", sib_url.trim_end_matches('/'));
                        if let Ok(resp) = client.get(&brain_url).send().await {
                            if resp.status().is_success() {
                                if let Ok(body) = resp.json::<serde_json::Value>().await {
                                    if let Some(weights_json) =
                                        body.get("weights").and_then(|v| v.as_str())
                                    {
                                        if let Some(peer_brain) =
                                            crate::brain::Brain::from_json(weights_json)
                                        {
                                            if peer_brain.train_steps > 0 {
                                                let mut our_brain = crate::brain::load_brain(db);
                                                let delta = peer_brain.compute_delta(
                                                    &crate::brain::Brain::new(),
                                                    inst_id,
                                                );
                                                our_brain.merge_delta(&delta, 0.3);
                                                crate::brain::save_brain(db, &our_brain);
                                                tracing::info!(
                                                    peer = %inst_id,
                                                    peer_steps = peer_brain.train_steps,
                                                    "Merged brain weights from peer"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 2. Peer info already fetched above via /instance/info (free, enriched into peer record)

                    // Fetch peer's lessons for collective learning
                    let mut peer_lessons = Vec::new();
                    if let Some(ref db) = self.db {
                        // Extract lessons from paid soul data if available
                        if let Some(ref soul_data) = paid_soul_data {
                            if let Some(outcomes) =
                                soul_data.get("plan_outcomes").and_then(|v| v.as_array())
                            {
                                // Store as peer lessons for prompt injection
                                let key = format!("peer_lessons_{}", inst_id);
                                if let Ok(json) = serde_json::to_string(
                                    &serde_json::json!({ "outcomes": outcomes }),
                                ) {
                                    let _ = db.set_state(&key, &json);
                                }
                                for o in outcomes.iter().take(5) {
                                    if let Some(lesson) = o.get("lesson").and_then(|v| v.as_str()) {
                                        peer_lessons.push(lesson.to_string());
                                    }
                                }
                                tracing::info!(
                                    peer = %inst_id,
                                    lessons = peer_lessons.len(),
                                    "Extracted lessons from paid soul response"
                                );
                            }
                        }

                        // Colony: record peer fitness for selection pressure
                        if let Some(ref soul_data) = paid_soul_data {
                            let fitness = soul_data
                                .get("fitness")
                                .and_then(|f| f.get("total"))
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let benchmark = soul_data
                                .get("benchmark")
                                .and_then(|b| b.get("pass_at_1"))
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let role = soul_data
                                .get("role")
                                .and_then(|r| r.get("primary"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("generalist");
                            let strongest = soul_data
                                .get("capability_profile")
                                .and_then(|c| c.get("strongest"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            crate::colony::record_peer_fitness(
                                db, inst_id, fitness, benchmark, role, strongest,
                            );
                        }

                        // Fallback: if no lessons from paid response, try free endpoint
                        if peer_lessons.is_empty() {
                            let lessons_url =
                                format!("{}/soul/lessons", sib_url.trim_end_matches('/'));
                            if let Ok(resp) = client.get(&lessons_url).send().await {
                                if resp.status().is_success() {
                                    // Sanitize before parsing — peer responses may contain control chars
                                    let raw = resp.text().await.unwrap_or_default();
                                    let raw = sanitize_json_body(&raw);
                                    if let Ok(body) =
                                        serde_json::from_str::<serde_json::Value>(&raw)
                                    {
                                        let key = format!("peer_lessons_{}", inst_id);
                                        if let Ok(json) = serde_json::to_string(&body) {
                                            let _ = db.set_state(&key, &json);
                                        }
                                        if let Some(outcomes) =
                                            body.get("outcomes").and_then(|v| v.as_array())
                                        {
                                            for o in outcomes.iter().take(5) {
                                                if let Some(lesson) =
                                                    o.get("lesson").and_then(|v| v.as_str())
                                                {
                                                    peer_lessons.push(lesson.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fetch and import peer's verified benchmark solutions (collective intelligence)
                    let mut solutions_imported = 0u32;
                    if let Some(ref db) = self.db {
                        let solutions_url =
                            format!("{}/soul/benchmark/solutions", sib_url.trim_end_matches('/'));
                        if let Ok(resp) = client.get(&solutions_url).send().await {
                            if resp.status().is_success() {
                                if let Ok(body) = resp.json::<serde_json::Value>().await {
                                    if let Some(solutions) =
                                        body.get("solutions").and_then(|v| v.as_array())
                                    {
                                        let peer_sols: Vec<crate::benchmark::SharedSolution> =
                                            solutions
                                                .iter()
                                                .filter_map(|s| {
                                                    serde_json::from_value(s.clone()).ok()
                                                })
                                                .collect();
                                        if !peer_sols.is_empty() {
                                            let workspace = self.workspace_root.to_string_lossy();
                                            solutions_imported =
                                                crate::benchmark::import_solutions(
                                                    db, peer_sols, &workspace,
                                                )
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fetch and import peer's failed benchmark attempts (collaborative solving)
                    // This is the core 2>1 mechanism: one agent's failure helps the other succeed
                    let mut failures_imported = 0u32;
                    if let Some(ref db) = self.db {
                        let failures_url =
                            format!("{}/soul/benchmark/failures", sib_url.trim_end_matches('/'));
                        if let Ok(resp) = client.get(&failures_url).send().await {
                            if resp.status().is_success() {
                                if let Ok(body) = resp.json::<serde_json::Value>().await {
                                    if let Some(failures) =
                                        body.get("failures").and_then(|v| v.as_array())
                                    {
                                        let peer_fails: Vec<crate::benchmark::SharedFailure> =
                                            failures
                                                .iter()
                                                .filter_map(|f| {
                                                    serde_json::from_value(f.clone()).ok()
                                                })
                                                .collect();
                                        if !peer_fails.is_empty() {
                                            failures_imported =
                                                crate::benchmark::import_failures(db, peer_fails);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if failures_imported > 0 {
                        tracing::info!(
                            failures_imported = failures_imported,
                            peer = %sib_url,
                            "Imported peer benchmark failures for collaborative solving"
                        );
                    }

                    // Fetch peer's open PRs for peer review system
                    let mut peer_prs: Vec<serde_json::Value> = Vec::new();
                    let prs_url = format!("{}/soul/open-prs", sib_url.trim_end_matches('/'));
                    if let Ok(resp) = client.get(&prs_url).send().await {
                        if resp.status().is_success() {
                            if let Ok(body) = resp.json::<serde_json::Value>().await {
                                // Collect PRs that need review
                                for key in &["fork_prs", "upstream_prs"] {
                                    if let Some(prs) = body.get(*key).and_then(|v| v.as_array()) {
                                        for pr in prs {
                                            let needs_review = pr
                                                .get("reviewDecision")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.is_empty() || s == "REVIEW_REQUIRED")
                                                .unwrap_or(true);
                                            if needs_review {
                                                peer_prs.push(pr.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    enriched_peers.push(serde_json::json!({
                        "instance_id": inst_id,
                        "url": sib_url,
                        "address": address,
                        "version": version,
                        "endpoints": callable_endpoints,
                        "lessons": peer_lessons,
                        "solutions_imported": solutions_imported,
                        "open_prs": peer_prs,
                    }));

                    // ── Mutual linking: POST /instance/link back to the peer ──
                    // This ensures the peer also sees *us* in their siblings list.
                    // Without this, peer relationships are one-directional.
                    let our_public_url = std::env::var("RAILWAY_PUBLIC_DOMAIN")
                        .ok()
                        .map(|d| format!("https://{d}"))
                        .or_else(|| {
                            self.gateway_url
                                .as_deref()
                                .filter(|u| u.starts_with("https://"))
                                .map(String::from)
                        });
                    if let Some(our_url) = our_public_url.as_deref() {
                        // Only link back if we have an externally-reachable URL
                        if our_url.starts_with("https://") {
                            let link_url =
                                format!("{}/instance/link", sib_url.trim_end_matches('/'));
                            let link_body = serde_json::json!({ "url": our_url });
                            match client.post(&link_url).json(&link_body).send().await {
                                Ok(r) if r.status().is_success() => {
                                    tracing::info!(
                                        peer = %inst_id,
                                        our_url = %our_url,
                                        "Mutual link established — peer now sees us"
                                    );
                                }
                                Ok(r) => {
                                    tracing::debug!(
                                        peer = %inst_id,
                                        status = %r.status(),
                                        "Mutual link returned non-2xx (non-fatal)"
                                    );
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        peer = %inst_id,
                                        error = %e,
                                        "Mutual link request failed (non-fatal)"
                                    );
                                }
                            }
                        }
                    }
                }

                // ── PEER_URLS: static peer list (ensures full mesh) ──
                // Comma-separated URLs. Every node probes every URL.
                // This prevents partial mesh (e.g., child can't find siblings).
                if let Ok(peer_urls_env) = std::env::var("PEER_URLS") {
                    for peer_url in peer_urls_env.split(',') {
                        let peer_trimmed = peer_url.trim().trim_end_matches('/');
                        if peer_trimmed.is_empty() {
                            continue;
                        }
                        // Skip self (check against our public domain)
                        let our_domain = std::env::var("RAILWAY_PUBLIC_DOMAIN")
                            .ok()
                            .map(|d| format!("https://{d}"))
                            .unwrap_or_default();
                        if !our_domain.is_empty()
                            && peer_trimmed == our_domain.trim_end_matches('/')
                        {
                            continue;
                        }
                        // Skip if already discovered
                        let already = enriched_peers.iter().any(|p| {
                            p.get("url")
                                .and_then(|v| v.as_str())
                                .map(|u| u == peer_trimmed)
                                .unwrap_or(false)
                        });
                        if already {
                            continue;
                        }
                        // Probe this peer
                        let info_url = format!("{}/instance/info", peer_trimmed);
                        if let Ok(r) = client.get(&info_url).send().await {
                            if r.status().is_success() {
                                if let Ok(info) = r.json::<serde_json::Value>().await {
                                    let identity = info.get("identity");
                                    let p_inst = identity
                                        .and_then(|i| i.get("instance_id"))
                                        .and_then(|v| v.as_str())
                                        .or_else(|| {
                                            info.get("instance_id").and_then(|v| v.as_str())
                                        })
                                        .unwrap_or("peer");
                                    let p_addr = identity
                                        .and_then(|i| i.get("address"))
                                        .and_then(|v| v.as_str())
                                        .or_else(|| info.get("address").and_then(|v| v.as_str()));
                                    let p_version = info.get("version").and_then(|v| v.as_str());
                                    let p_endpoints = info
                                        .get("endpoints")
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    let is_self = p_inst == self_instance_id
                                        || p_addr
                                            .map(|a| a.to_lowercase() == self_address)
                                            .unwrap_or(false);
                                    if !is_self {
                                        let callable: Vec<serde_json::Value> = p_endpoints
                                            .iter()
                                            .map(|ep| {
                                                let slug = ep
                                                    .get("slug")
                                                    .and_then(|s| s.as_str())
                                                    .unwrap_or("");
                                                let mut ep_clone = ep.clone();
                                                if let Some(obj) = ep_clone.as_object_mut() {
                                                    obj.insert(
                                                        "callable_url".to_string(),
                                                        serde_json::Value::String(format!(
                                                            "{}/g/{}",
                                                            peer_trimmed, slug
                                                        )),
                                                    );
                                                }
                                                ep_clone
                                            })
                                            .collect();
                                        tracing::info!(peer_id = %p_inst, url = %peer_trimmed, "Added peer from PEER_URLS");
                                        enriched_peers.push(serde_json::json!({
                                            "instance_id": p_inst,
                                            "url": peer_trimmed,
                                            "address": p_addr,
                                            "version": p_version,
                                            "endpoints": callable,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }

                // Always add parent as peer if PARENT_URL is set.
                // The parent isn't in its own siblings list, so children
                // must explicitly include it. Check even if we found siblings
                // (they might all be filtered as self).
                {
                    if let Ok(parent_env) = std::env::var("PARENT_URL") {
                        let parent_trimmed = parent_env.trim_end_matches('/');
                        // Skip if parent is already in enriched_peers
                        let parent_already_added = enriched_peers.iter().any(|p| {
                            p.get("url")
                                .and_then(|v| v.as_str())
                                .map(|u| u == parent_trimmed)
                                .unwrap_or(false)
                        });
                        if !parent_already_added {
                            let info_url = format!("{}/instance/info", parent_trimmed);
                            if let Ok(r) = client.get(&info_url).send().await {
                                if r.status().is_success() {
                                    if let Ok(info) = r.json::<serde_json::Value>().await {
                                        // instance_id and address are under "identity" in /instance/info
                                        let identity = info.get("identity");
                                        let p_inst = identity
                                            .and_then(|i| i.get("instance_id"))
                                            .and_then(|v| v.as_str())
                                            .or_else(|| {
                                                info.get("instance_id").and_then(|v| v.as_str())
                                            })
                                            .unwrap_or("parent");
                                        let p_addr = identity
                                            .and_then(|i| i.get("address"))
                                            .and_then(|v| v.as_str())
                                            .or_else(|| {
                                                info.get("address").and_then(|v| v.as_str())
                                            });
                                        let p_version =
                                            info.get("version").and_then(|v| v.as_str());
                                        let p_endpoints = info
                                            .get("endpoints")
                                            .and_then(|v| v.as_array())
                                            .cloned()
                                            .unwrap_or_default();

                                        // Skip if parent is actually us
                                        let is_self = p_inst == self_instance_id
                                            || p_addr
                                                .map(|a| a.to_lowercase() == self_address)
                                                .unwrap_or(false);

                                        if !is_self {
                                            let callable: Vec<serde_json::Value> = p_endpoints
                                                .iter()
                                                .map(|ep| {
                                                    let slug = ep
                                                        .get("slug")
                                                        .and_then(|s| s.as_str())
                                                        .unwrap_or("");
                                                    let mut ep_clone = ep.clone();
                                                    if let Some(obj) = ep_clone.as_object_mut() {
                                                        obj.insert(
                                                            "callable_url".to_string(),
                                                            serde_json::Value::String(format!(
                                                                "{}/g/{}",
                                                                parent_trimmed, slug
                                                            )),
                                                        );
                                                        // Sanitize description — strip control characters that break JSON parsing
                                                        if let Some(serde_json::Value::String(
                                                            desc,
                                                        )) = obj.get("description")
                                                        {
                                                            let clean: String = desc
                                                                .chars()
                                                                .filter(|c| {
                                                                    !c.is_control()
                                                                        || *c == '\n'
                                                                        || *c == '\r'
                                                                        || *c == '\t'
                                                                })
                                                                .collect();
                                                            obj.insert(
                                                                "description".to_string(),
                                                                serde_json::Value::String(clean),
                                                            );
                                                        }
                                                    }
                                                    ep_clone
                                                })
                                                .collect();

                                            tracing::info!(
                                                parent_id = %p_inst,
                                                endpoints = callable.len(),
                                                "Added parent as peer"
                                            );

                                            enriched_peers.push(serde_json::json!({
                                                "instance_id": p_inst,
                                                "url": parent_trimmed,
                                                "address": p_addr,
                                                "version": p_version,
                                                "endpoints": callable,
                                            }));
                                        }
                                    }
                                }
                            }
                        } // if !parent_already_added
                    }
                }

                // Track successful peer discovery as coordination signal
                if !enriched_peers.is_empty() {
                    if let Some(ref db) = self.db {
                        let attempted: u64 = db
                            .get_state("peer_calls_attempted")
                            .ok()
                            .flatten()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let succeeded: u64 = db
                            .get_state("peer_calls_succeeded")
                            .ok()
                            .flatten()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        let _ = db.set_state("peer_calls_attempted", &(attempted + 1).to_string());
                        let _ = db.set_state("peer_calls_succeeded", &(succeeded + 1).to_string());

                        // Persist peer endpoint catalog for prompt injection + cognitive sync
                        // MUST include "url" field — get_known_peer_urls() reads it for sync targets
                        let mut catalog: Vec<serde_json::Value> = Vec::new();
                        for peer in &enriched_peers {
                            let peer_id = peer
                                .get("instance_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let peer_url = peer.get("url").and_then(|v| v.as_str()).unwrap_or("");
                            let peer_eps = peer
                                .get("endpoints")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let slugs: Vec<String> = peer_eps
                                .iter()
                                .filter_map(|ep| {
                                    ep.get("slug").and_then(|s| s.as_str()).map(String::from)
                                })
                                .collect();
                            catalog.push(serde_json::json!({
                                "peer": peer_id,
                                "url": peer_url,
                                "slugs": slugs,
                            }));
                        }
                        if let Ok(json) = serde_json::to_string(&catalog) {
                            let _ = db.set_state("peer_endpoint_catalog", &json);
                        }

                        // Persist peer open PRs for review prompt injection
                        let mut all_peer_prs: Vec<serde_json::Value> = Vec::new();
                        for peer in &enriched_peers {
                            let peer_id = peer
                                .get("instance_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            if let Some(prs) = peer.get("open_prs").and_then(|v| v.as_array()) {
                                for pr in prs {
                                    let mut pr_entry = pr.clone();
                                    if let Some(obj) = pr_entry.as_object_mut() {
                                        obj.insert(
                                            "peer_id".to_string(),
                                            serde_json::Value::String(peer_id.to_string()),
                                        );
                                    }
                                    all_peer_prs.push(pr_entry);
                                }
                            }
                        }
                        if let Ok(json) = serde_json::to_string(&all_peer_prs) {
                            let _ = db.set_state("peer_open_prs", &json);
                        }
                    }
                }

                // Emit structured event for peer discovery result
                if let Some(ref db) = self.db {
                    if enriched_peers.is_empty() {
                        crate::events::emit_event(
                            db,
                            "warn",
                            "peer.discovery.empty",
                            "No peers found after discovery",
                            Some(serde_json::json!({"parent_url": parent_url})),
                            crate::events::EventRefs::default(),
                        );
                    } else {
                        let peer_ids: Vec<&str> = enriched_peers
                            .iter()
                            .filter_map(|p| p.get("instance_id").and_then(|v| v.as_str()))
                            .collect();
                        crate::events::emit_event(
                            db,
                            "info",
                            "peer.discovery.success",
                            &format!("{} peers found: {:?}", enriched_peers.len(), peer_ids),
                            Some(
                                serde_json::json!({"count": enriched_peers.len(), "peers": peer_ids}),
                            ),
                            crate::events::EventRefs::default(),
                        );
                    }
                }

                let output = serde_json::to_string_pretty(&serde_json::json!({
                    "source": "http",
                    "parent_url": parent_url,
                    "peers": enriched_peers,
                    "count": enriched_peers.len(),
                }))
                .unwrap_or_default();

                let duration_ms = start.elapsed().as_millis() as u64;
                let output_truncated = if output.len() > MAX_OUTPUT_BYTES {
                    format!(
                        "{}\n... (truncated)",
                        output.chars().take(MAX_OUTPUT_BYTES).collect::<String>()
                    )
                } else {
                    output
                };

                Ok(ToolResult {
                    stdout: output_truncated,
                    stderr: String::new(),
                    exit_code: status.as_u16() as i32,
                    duration_ms,
                })
            }
            Err(e) => {
                if let Some(ref db) = self.db {
                    crate::events::emit_event(
                        db,
                        "error",
                        "peer.discovery.failed",
                        &format!("HTTP request failed: {e}"),
                        None,
                        crate::events::EventRefs::default(),
                    );
                }
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

    /// Call a paid endpoint on another instance using the x402 payment flow.
    /// Pattern: GET -> 402 -> parse requirements -> sign -> retry with PAYMENT-SIGNATURE.
    pub(super) async fn call_paid_endpoint(
        &self,
        url: &str,
        method: &str,
        body: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();

        let private_key = std::env::var("EVM_PRIVATE_KEY")
            .map_err(|_| "EVM_PRIVATE_KEY not set — cannot sign payments".to_string())?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        // Step 1: Make initial request — expect 402
        let initial_resp = match method.to_uppercase().as_str() {
            "POST" => {
                client
                    .post(url)
                    .body(body.unwrap_or("").to_string())
                    .send()
                    .await
            }
            _ => client.get(url).send().await,
        }
        .map_err(|e| format!("initial request failed: {e}"))?;

        // If not 402, return the response directly (endpoint may be free)
        if initial_resp.status().as_u16() != 402 {
            let status = initial_resp.status();
            let resp_body = initial_resp.text().await.unwrap_or_default();
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ToolResult {
                stdout: resp_body,
                stderr: format!("endpoint returned {status} (not 402 — no payment needed)"),
                exit_code: status.as_u16() as i32,
                duration_ms,
            });
        }

        // Step 2: Parse PaymentRequirements from 402 response
        let resp_json: serde_json::Value = initial_resp
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

        // Step 2.5: Auto-approve the facilitator if needed.
        // The facilitator calls transferFrom(payer, pay_to, amount), so the payer
        // must approve the FACILITATOR address (the caller of transferFrom), NOT pay_to.
        // The 402 response includes facilitatorAddress when the gateway has an embedded facilitator.
        // Fall back to pay_to for backwards compatibility (works when pay_to == facilitator).
        let approve_target: Option<alloy::primitives::Address> = req_value
            .get("facilitatorAddress")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .or_else(|| requirements.pay_to.parse().ok());

        if let Some(target_addr) = approve_target {
            let rpc_url = std::env::var("RPC_URL")
                .unwrap_or_else(|_| "https://rpc.moderato.tempo.xyz".to_string());
            if let Ok(rpc_parsed) = rpc_url.parse::<reqwest::Url>() {
                let pk_signer: alloy::signers::local::PrivateKeySigner = private_key
                    .parse()
                    .map_err(|e| format!("invalid private key for approval: {e}"))?;
                let payer_addr = pk_signer.address();
                let wallet = alloy::network::EthereumWallet::from(pk_signer);
                let provider = alloy::providers::ProviderBuilder::new()
                    .wallet(wallet)
                    .connect_http(rpc_parsed);
                let token = x402::constants::DEFAULT_TOKEN;

                // Check current allowance to the facilitator
                let current_allowance =
                    x402::tip20::allowance(&provider, token, payer_addr, target_addr)
                        .await
                        .unwrap_or(alloy::primitives::U256::ZERO);

                // If allowance is below 1B pathUSD, approve MAX
                if current_allowance < alloy::primitives::U256::from(1_000_000_000_000_000u64) {
                    tracing::info!(
                        payer = %payer_addr,
                        facilitator = %target_addr,
                        "Auto-approving facilitator for pathUSD (first payment to this peer)"
                    );
                    match x402::tip20::approve(
                        &provider,
                        token,
                        target_addr,
                        alloy::primitives::U256::MAX,
                    )
                    .await
                    {
                        Ok(tx) => {
                            tracing::info!(tx = %tx, "Facilitator approved for pathUSD");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Auto-approval failed — payment may fail");
                        }
                    }
                }
            }
        }

        // Step 3: Sign payment using wallet signer (same pattern as register_endpoint)
        let signer = x402::wallet::WalletSigner::new(&private_key)
            .map_err(|e| format!("failed to create signer: {e}"))?;

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("system time error: {e}"))?
            .as_secs();

        let payment_b64 = signer
            .sign_payment(&requirements, now_secs)
            .map_err(|e| format!("failed to sign payment: {e}"))?;

        // Step 4: Retry with payment signature
        let paid_resp = match method.to_uppercase().as_str() {
            "POST" => {
                client
                    .post(url)
                    .header("PAYMENT-SIGNATURE", &payment_b64)
                    .body(body.unwrap_or("").to_string())
                    .send()
                    .await
            }
            _ => {
                client
                    .get(url)
                    .header("PAYMENT-SIGNATURE", &payment_b64)
                    .send()
                    .await
            }
        }
        .map_err(|e| format!("paid request failed: {e}"))?;

        let status = paid_resp.status();
        let final_body = paid_resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        let body_truncated = if final_body.len() > MAX_OUTPUT_BYTES {
            format!(
                "{}\n... (truncated)",
                final_body
                    .chars()
                    .take(MAX_OUTPUT_BYTES)
                    .collect::<String>()
            )
        } else {
            final_body
        };

        // Emit structured event for paid call result
        if let Some(ref db) = self.db {
            if status.is_success() {
                crate::events::emit_event(
                    db,
                    "info",
                    "peer.call.success",
                    &format!("Paid call succeeded: {url}"),
                    Some(
                        serde_json::json!({"status": status.as_u16(), "duration_ms": duration_ms}),
                    ),
                    crate::events::EventRefs {
                        peer_url: Some(url.to_string()),
                        ..Default::default()
                    },
                );
            } else if status.as_u16() == 402 {
                crate::events::emit_event(
                    db,
                    "warn",
                    "peer.call.payment_failed",
                    &format!("Payment required: {url}"),
                    Some(serde_json::json!({"status": 402, "url": url})),
                    crate::events::EventRefs {
                        peer_url: Some(url.to_string()),
                        ..Default::default()
                    },
                );
            } else {
                crate::events::emit_event(
                    db,
                    "warn",
                    "peer.call.failed",
                    &format!("Paid call returned {status}: {url}"),
                    Some(serde_json::json!({"status": status.as_u16(), "url": url})),
                    crate::events::EventRefs {
                        peer_url: Some(url.to_string()),
                        ..Default::default()
                    },
                );
            }
        }

        Ok(ToolResult {
            stdout: body_truncated.clone(),
            stderr: if status.is_success() {
                String::new()
            } else {
                // Include body in error for debugging
                let body_preview = if body_truncated.len() > 300 {
                    let mut end = 300;
                    while end > 0 && !body_truncated.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &body_truncated[..end])
                } else {
                    body_truncated
                };
                format!("paid request returned status {status}: {body_preview}")
            },
            exit_code: status.as_u16() as i32,
            duration_ms,
        })
    }
}
