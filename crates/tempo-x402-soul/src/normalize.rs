//! Plan normalization and sanitization logic.
//!
//! Pure functions for converting messy LLM JSON output into valid `PlanStep` sequences,
//! fixing common path errors, and removing broken steps.

use crate::error::SoulError;
use crate::plan::PlanStep;

/// Fix common file path mistakes that LLMs produce.
pub fn fix_common_path_errors(path: &str) -> String {
    let mut p = path.to_string();

    // LLM writes "crates/tempo-x402/src/thinking.rs" but thinking.rs is in tempo-x402-soul
    let soul_files = [
        "thinking.rs",
        "plan.rs",
        "prompts.rs",
        "chat.rs",
        "memory.rs",
        "git.rs",
        "coding.rs",
        "mode.rs",
        "neuroplastic.rs",
        "persistent_memory.rs",
        "world_model.rs",
        "observer.rs",
    ];
    for &f in &soul_files {
        let wrong = format!("crates/tempo-x402/src/{f}");
        if p == wrong || p.ends_with(&format!("/{wrong}")) {
            p = format!("crates/tempo-x402-soul/src/{f}");
            break;
        }
    }

    // Strip leading /data/workspace/ prefix (agent's absolute path)
    if let Some(stripped) = p.strip_prefix("/data/workspace/") {
        p = stripped.to_string();
    }

    p
}

/// Sanitize plan steps to remove or fix obviously broken steps.
/// This runs at plan creation time so broken steps never count as execution failures.
pub fn sanitize_plan_steps(steps: Vec<PlanStep>) -> Vec<PlanStep> {
    let original_count = steps.len();
    let mut sanitized = Vec::with_capacity(original_count);

    for step in steps {
        match &step {
            // Remove shell commands that reference undefined shell variables
            // (LLM generates `echo "$soul_response"` thinking plan context is shell vars)
            PlanStep::RunShell { command, store_as } => {
                // Strip redundant tool-name prefix from command
                // LLM sometimes generates {"type":"run_shell","command":"run_shell curl ..."}
                let command =
                    if command.starts_with("run_shell ") || command.starts_with("execute_shell ") {
                        let stripped = command.split_once(' ').map(|x| x.1).unwrap_or(command);
                        tracing::debug!(
                            original = %command,
                            fixed = %stripped,
                            "Stripped tool-name prefix from RunShell command"
                        );
                        stripped
                    } else if command == "run_shell" || command == "execute_shell" {
                        // Bare tool name with no actual command — skip
                        tracing::debug!("Sanitized out bare run_shell/execute_shell command");
                        continue;
                    } else {
                        command.as_str()
                    };

                // Intercept tool names used as shell commands — LLM sometimes generates
                // {"type":"run_shell","command":"edit_code file.rs ..."} instead of using EditCode
                if command.starts_with("edit_code ") || command.starts_with("edit_file ") {
                    let rest = command.split_once(' ').map(|x| x.1).unwrap_or("");
                    tracing::info!(command = %command, "Converted RunShell(edit_code) → EditCode");
                    sanitized.push(PlanStep::EditCode {
                        file_path: rest.split_whitespace().next().unwrap_or("").to_string(),
                        description: rest.to_string(),
                        context_keys: vec![],
                    });
                    continue;
                }
                if command.starts_with("write_file ") || command.starts_with("generate_code ") {
                    let rest = command.split_once(' ').map(|x| x.1).unwrap_or("");
                    tracing::info!(command = %command, "Converted RunShell(write_file) → GenerateCode");
                    sanitized.push(PlanStep::GenerateCode {
                        file_path: rest.split_whitespace().next().unwrap_or("").to_string(),
                        description: rest.to_string(),
                        context_keys: vec![],
                    });
                    continue;
                }
                if command.starts_with("read_file ") || command.starts_with("cat ") {
                    let path = command.split_whitespace().nth(1).unwrap_or("").to_string();
                    if !path.is_empty() && !path.starts_with('-') {
                        tracing::info!(command = %command, "Converted RunShell(read_file) → ReadFile");
                        sanitized.push(PlanStep::ReadFile {
                            path,
                            store_as: store_as.clone(),
                        });
                        continue;
                    }
                }
                if command == "cargo check" || command.starts_with("cargo check ") {
                    tracing::info!(command = %command, "Converted RunShell(cargo check) → CargoCheck");
                    sanitized.push(PlanStep::CargoCheck {
                        store_as: store_as.clone(),
                    });
                    continue;
                }

                // Skip commands that are purely writing shell-variable placeholders to files
                // e.g., `echo "$response" > file.json` or `echo '$info_call_result' > file`
                let has_placeholder_var = command.contains("$soul_response")
                    || command.contains("$info_call_result")
                    || command.contains("$chat_response")
                    || command.contains("$soul_call_result")
                    || command.contains("${soul")
                    || command.contains("${info")
                    || command.contains("${chat");
                let is_just_echo_to_file = command.starts_with("echo ")
                    && (command.contains(" > ") || command.contains(" >> "))
                    && has_placeholder_var;

                if is_just_echo_to_file {
                    tracing::debug!(
                        command = %command,
                        "Sanitized out shell command with undefined variable placeholder"
                    );
                    continue;
                }

                // Skip commands that use unavailable tools
                if command.starts_with("jq ") || command.contains("| jq ") {
                    tracing::debug!(
                        command = %command,
                        "Sanitized out shell command using unavailable 'jq'"
                    );
                    continue;
                }

                // If command was modified (prefix stripped), push corrected step
                sanitized.push(PlanStep::RunShell {
                    command: command.to_string(),
                    store_as: store_as.clone(),
                });
            }
            // Remove ReadFile steps that reference non-existent plan context files
            // (LLM generates `read siblings.json` or `read discovered_peers.json`)
            PlanStep::ReadFile { path, store_as } => {
                // Detect shell syntax that was misclassified as ReadFile
                // (e.g. "read <<EOF > file.py" → path would be "<<EOF ...")
                if path.contains("<<") || path.contains(">>") || path.starts_with('<') {
                    tracing::debug!(
                        path = %path,
                        "Sanitized out ReadFile with shell heredoc/redirect syntax"
                    );
                    continue;
                }
                // Convert read_file on directories to list_dir
                if path == "."
                    || path == ".."
                    || path.ends_with('/')
                    || path == "crates"
                    || path == "src"
                {
                    tracing::debug!(
                        path = %path,
                        "Converted ReadFile on directory to ListDir"
                    );
                    sanitized.push(PlanStep::ListDir {
                        path: path.clone(),
                        store_as: store_as.clone(),
                    });
                    continue;
                }
                let bogus_files = [
                    "siblings.json",
                    "discovered_peers.json",
                    "filtered_peers.json",
                    "available_tools.txt",
                    "target_peer_info.json",
                    "verified_source_paths.txt",
                    "soul_call_result.json",
                    "last_soul_call.json",
                    "peer_info.json",
                    "peer_data.json",
                    "peer_status.json",
                    "info_call_result.json",
                    "soul_response.json",
                    "call_result.json",
                    "network_data.json",
                    "health_data.json",
                ];
                let filename = path.rsplit('/').next().unwrap_or(path);
                // Also catch any *_result.json or *_response.json patterns
                let is_bogus_pattern = filename.ends_with("_result.json")
                    || filename.ends_with("_response.json")
                    || filename.ends_with("_data.json")
                    || filename.ends_with("_output.json");
                if bogus_files.contains(&filename) || is_bogus_pattern {
                    tracing::debug!(
                        path = %path,
                        "Sanitized out ReadFile for non-existent plan artifact"
                    );
                    continue;
                }
                // Fix common path errors
                let fixed_path = fix_common_path_errors(path);
                if fixed_path != *path {
                    sanitized.push(PlanStep::ReadFile {
                        path: fixed_path,
                        store_as: store_as.clone(),
                    });
                } else {
                    sanitized.push(step);
                }
            }
            // Fix common path mistakes in EditCode/GenerateCode
            // LLM often writes "crates/tempo-x402/src/thinking.rs" (wrong crate)
            PlanStep::EditCode {
                file_path,
                description,
                context_keys,
            } => {
                let fixed_path = fix_common_path_errors(file_path);
                if crate::guard::is_protected(&fixed_path) {
                    tracing::debug!(
                        path = %fixed_path,
                        "Sanitized out EditCode for protected file"
                    );
                    continue;
                }
                if fixed_path != *file_path {
                    tracing::debug!(
                        original = %file_path,
                        fixed = %fixed_path,
                        "Fixed path in EditCode step"
                    );
                    sanitized.push(PlanStep::EditCode {
                        file_path: fixed_path,
                        description: description.clone(),
                        context_keys: context_keys.clone(),
                    });
                } else {
                    sanitized.push(step);
                }
            }
            PlanStep::GenerateCode {
                file_path,
                description,
                context_keys,
            } => {
                let fixed_path = fix_common_path_errors(file_path);
                if crate::guard::is_protected(&fixed_path) {
                    tracing::debug!(
                        path = %fixed_path,
                        "Sanitized out GenerateCode for protected file"
                    );
                    continue;
                }
                if fixed_path != *file_path {
                    sanitized.push(PlanStep::GenerateCode {
                        file_path: fixed_path,
                        description: description.clone(),
                        context_keys: context_keys.clone(),
                    });
                } else {
                    sanitized.push(step);
                }
            }
            // Remove CallPaidEndpoint with localhost URLs
            PlanStep::CallPaidEndpoint { url, .. } => {
                if url.contains("localhost") || url.contains("127.0.0.1") {
                    tracing::debug!(
                        url = %url,
                        "Sanitized out CallPaidEndpoint with localhost URL"
                    );
                    continue;
                }
                sanitized.push(step);
            }
            // Everything else passes through
            _ => sanitized.push(step),
        }
    }

    if sanitized.len() < original_count {
        tracing::info!(
            original = original_count,
            sanitized = sanitized.len(),
            removed = original_count - sanitized.len(),
            "Plan steps sanitized"
        );
    }

    sanitized
}

/// Parse plan steps from LLM text output.
/// Extracts a JSON array of `PlanStep` from potentially messy LLM output,
/// applying normalization and truncating to `max_plan_steps`.
pub fn parse_plan_steps(text: &str, max_plan_steps: usize) -> Result<Vec<PlanStep>, SoulError> {
    let try_parse = |json_str: &str| -> Result<Vec<PlanStep>, serde_json::Error> {
        // Always normalize first — coerces wrong types (maps→strings) and fixes
        // missing "type" fields before attempting deserialization.
        let normalized = normalize_plan_json(json_str);
        match serde_json::from_str::<Vec<PlanStep>>(&normalized) {
            Ok(steps) => Ok(steps),
            Err(_) if normalized != json_str => {
                // Normalization changed something but still failed — try original
                // in case normalization mangled valid JSON
                serde_json::from_str::<Vec<PlanStep>>(json_str)
            }
            Err(e) => Err(e),
        }
    };

    // Try to find a JSON array in the response
    if let Some((json_str, _, _)) = extract_json_array(text) {
        match try_parse(&json_str) {
            Ok(mut steps) => {
                if steps.len() > max_plan_steps {
                    steps.truncate(max_plan_steps);
                }
                if steps.is_empty() {
                    return Err(SoulError::Config("LLM returned empty plan".to_string()));
                }
                return Ok(steps);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse plan steps JSON");
            }
        }
    }

    // Fallback: try parsing the entire text as JSON
    match try_parse(text.trim()) {
        Ok(mut steps) => {
            if steps.len() > max_plan_steps {
                steps.truncate(max_plan_steps);
            }
            if steps.is_empty() {
                Err(SoulError::Config("LLM returned empty plan".to_string()))
            } else {
                Ok(steps)
            }
        }
        Err(e) => Err(SoulError::Config(format!(
            "Cannot parse plan steps: {e}. Response: {}",
            &text[..text.len().min(200)]
        ))),
    }
}

/// Normalize common LLM plan JSON mistakes into valid PlanStep format.
/// The LLM often outputs {"action": "ls", "name": "explore"} instead of
/// {"type": "run_shell", "command": "ls"}.
/// Also coerces non-string values to strings (LLM sometimes returns maps/arrays
/// where strings are expected).
pub fn normalize_plan_json(json_str: &str) -> String {
    let parsed: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return json_str.to_string(),
    };

    let normalized: Vec<serde_json::Value> = parsed
        .into_iter()
        .filter_map(|mut obj| {
            let map = obj.as_object_mut()?;

            // Coerce non-string field values to strings (except "type" and "context_keys").
            // LLMs sometimes return {"store_as": {"key": "val"}} instead of "val".
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in &keys {
                if key == "type" || key == "context_keys" {
                    continue;
                }
                let needs_coerce = match map.get(key) {
                    Some(serde_json::Value::Object(_)) => true,
                    Some(serde_json::Value::Array(_)) => true,
                    Some(serde_json::Value::Number(n)) => {
                        // Coerce numbers to strings
                        map.insert(key.clone(), serde_json::json!(n.to_string()));
                        false
                    }
                    Some(serde_json::Value::Bool(b)) => {
                        map.insert(key.clone(), serde_json::json!(b.to_string()));
                        false
                    }
                    _ => false,
                };
                if needs_coerce {
                    // Try to extract a string from the nested value
                    let coerced = match map.get(key) {
                        Some(serde_json::Value::Object(inner)) => {
                            // Take the first string value, or serialize the whole thing
                            inner
                                .values()
                                .find_map(|v| v.as_str().map(String::from))
                                .unwrap_or_else(|| {
                                    serde_json::to_string(map.get(key).unwrap()).unwrap_or_default()
                                })
                        }
                        Some(serde_json::Value::Array(arr)) => {
                            // For arrays of strings, join them; otherwise serialize
                            let strings: Vec<&str> =
                                arr.iter().filter_map(|v| v.as_str()).collect();
                            if strings.len() == arr.len() {
                                strings.join(", ")
                            } else {
                                serde_json::to_string(map.get(key).unwrap()).unwrap_or_default()
                            }
                        }
                        _ => continue,
                    };
                    map.insert(key.clone(), serde_json::json!(coerced));
                }
            }

            // Coerce context_keys: if it's not an array of strings, fix it
            if let Some(ck) = map.get("context_keys") {
                match ck {
                    serde_json::Value::String(s) => {
                        // Single string → wrap in array
                        map.insert("context_keys".to_string(), serde_json::json!([s.clone()]));
                    }
                    serde_json::Value::Object(inner) => {
                        // Map → extract string values as array
                        let vals: Vec<String> = inner
                            .values()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        map.insert("context_keys".to_string(), serde_json::json!(vals));
                    }
                    serde_json::Value::Array(arr) => {
                        // Array of non-strings → coerce each to string
                        let vals: Vec<String> = arr
                            .iter()
                            .map(|v| {
                                v.as_str()
                                    .map(String::from)
                                    .unwrap_or_else(|| v.to_string())
                            })
                            .collect();
                        map.insert("context_keys".to_string(), serde_json::json!(vals));
                    }
                    _ => {}
                }
            }

            // Remap common LLM type aliases to the canonical type names
            if let Some(type_val) = map.get("type").and_then(|v| v.as_str()).map(String::from) {
                let canonical = match type_val.as_str() {
                    "shell" | "execute_shell" | "exec" | "execute" => "run_shell",
                    "read" => "read_file",
                    "write" | "write_file" => "generate_code",
                    "edit" | "edit_file" | "modify" => "edit_code",
                    "search" | "grep" | "find_code" => "search_code",
                    "list" | "ls" => "list_dir",
                    "check" | "check_self_status" | "cargo_check" => "check_self",
                    "peer" | "call" => "call_peer",
                    "peers" | "find_peers" => "discover_peers",
                    "clone" => "clone_self",
                    _ => type_val.as_str(),
                };
                if canonical != type_val {
                    tracing::debug!(
                        original = %type_val,
                        canonical = %canonical,
                        "Remapped LLM type alias to canonical step type"
                    );
                    map.insert("type".to_string(), serde_json::json!(canonical));
                }
                return Some(obj);
            }

            // Infer type from other fields the LLM commonly uses
            let action = map
                .get("action")
                .or_else(|| map.get("command"))
                .or_else(|| map.get("cmd"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if let Some(action_str) = action {
                let mut step = serde_json::Map::new();
                let action_lower = action_str.to_lowercase();

                if action_lower.starts_with("ls")
                    || action_lower.starts_with("find ")
                    || action_lower.starts_with("tree")
                {
                    // Directory listing
                    if action_lower == "ls" || action_lower.starts_with("ls ") {
                        let path = action_str.strip_prefix("ls").unwrap_or(".").trim();
                        let path = if path.is_empty() || path == "-F" || path == "-la" {
                            "."
                        } else {
                            path.trim_start_matches("-F ")
                                .trim_start_matches("-la ")
                                .trim()
                        };
                        step.insert("type".to_string(), serde_json::json!("list_dir"));
                        step.insert("path".to_string(), serde_json::json!(path));
                    } else {
                        step.insert("type".to_string(), serde_json::json!("run_shell"));
                        step.insert("command".to_string(), serde_json::json!(action_str));
                    }
                } else if action_lower.starts_with("cat ") || action_lower.starts_with("read ") {
                    let path = action_str.split_once(' ').map(|x| x.1).unwrap_or("");
                    // Detect shell syntax: heredocs (<<), redirects (>), pipes (|)
                    if path.contains("<<") || path.contains('>') || path.contains('|') {
                        step.insert("type".to_string(), serde_json::json!("run_shell"));
                        step.insert("command".to_string(), serde_json::json!(action_str));
                    } else if path.ends_with('/') || (!path.contains('.') && !path.is_empty()) {
                        step.insert("type".to_string(), serde_json::json!("list_dir"));
                        step.insert("path".to_string(), serde_json::json!(path));
                    } else {
                        step.insert("type".to_string(), serde_json::json!("read_file"));
                        step.insert("path".to_string(), serde_json::json!(path));
                    }
                } else if action_lower.starts_with("grep ") || action_lower.starts_with("rg ") {
                    step.insert("type".to_string(), serde_json::json!("run_shell"));
                    step.insert("command".to_string(), serde_json::json!(action_str));
                } else if action_lower.starts_with("read_file")
                    || action_lower.starts_with("read file")
                {
                    // LLM used "read_file /path" as action — convert to read_file step
                    let path = action_str.split_once(' ').map(|x| x.1).unwrap_or(".");
                    step.insert("type".to_string(), serde_json::json!("read_file"));
                    step.insert("path".to_string(), serde_json::json!(path));
                } else if action_lower.starts_with("search_code")
                    || action_lower.starts_with("search code")
                    || action_lower.starts_with("search_files")
                {
                    // LLM used "search_code pattern" as action — convert to search_code step
                    let pattern = action_str.split_once(' ').map(|x| x.1).unwrap_or("*");
                    step.insert("type".to_string(), serde_json::json!("search_code"));
                    step.insert("pattern".to_string(), serde_json::json!(pattern));
                    step.insert("directory".to_string(), serde_json::json!("."));
                } else if action_lower.starts_with("list_dir")
                    || action_lower.starts_with("list dir")
                {
                    let path = action_str.split_once(' ').map(|x| x.1).unwrap_or(".");
                    step.insert("type".to_string(), serde_json::json!("list_dir"));
                    step.insert("path".to_string(), serde_json::json!(path));
                } else if action_lower.starts_with("call_peer")
                    || action_lower.starts_with("call peer")
                {
                    // LLM used "call_peer soul" as action — convert to call_peer step
                    let slug = action_str
                        .split_once(' ')
                        .map(|x| x.1.trim())
                        .unwrap_or("info");
                    step.insert("type".to_string(), serde_json::json!("call_peer"));
                    step.insert("slug".to_string(), serde_json::json!(slug));
                } else if action_lower.starts_with("discover_peers")
                    || action_lower.starts_with("discover peers")
                {
                    step.insert("type".to_string(), serde_json::json!("discover_peers"));
                } else if action_lower.starts_with("check_self")
                    || action_lower.starts_with("check self")
                {
                    let endpoint = action_str
                        .split_once(' ')
                        .map(|x| x.1.trim())
                        .unwrap_or("health");
                    step.insert("type".to_string(), serde_json::json!("check_self"));
                    step.insert("endpoint".to_string(), serde_json::json!(endpoint));
                } else if action_lower.starts_with("commit") {
                    let message = action_str
                        .split_once(' ')
                        .map(|x| x.1.trim())
                        .unwrap_or("auto-commit");
                    step.insert("type".to_string(), serde_json::json!("commit"));
                    step.insert("message".to_string(), serde_json::json!(message));
                } else if action_lower.starts_with("cargo_check")
                    || action_lower.starts_with("cargo check")
                {
                    step.insert("type".to_string(), serde_json::json!("cargo_check"));
                } else if action_lower.starts_with("clone_self")
                    || action_lower.starts_with("clone self")
                {
                    step.insert("type".to_string(), serde_json::json!("clone_self"));
                } else if action_lower.starts_with("spawn_specialist")
                    || action_lower.starts_with("spawn specialist")
                {
                    let spec = action_str
                        .split_once(' ')
                        .map(|x| x.1.trim())
                        .unwrap_or("generalist");
                    step.insert("type".to_string(), serde_json::json!("spawn_specialist"));
                    step.insert("specialization".to_string(), serde_json::json!(spec));
                } else if action_lower.starts_with("delegate_task")
                    || action_lower.starts_with("delegate task")
                {
                    let desc = action_str.split_once(' ').map(|x| x.1.trim()).unwrap_or("");
                    step.insert("type".to_string(), serde_json::json!("delegate_task"));
                    step.insert("task_description".to_string(), serde_json::json!(desc));
                    step.insert("target".to_string(), serde_json::json!(""));
                } else if action_lower.starts_with("run_shell")
                    || action_lower.starts_with("execute_shell")
                    || action_lower.starts_with("shell:")
                    || action_lower.starts_with("shell ")
                {
                    // LLM used "run_shell curl ...", "execute_shell ls", or "shell: cmd" as action —
                    // strip the tool name prefix and keep just the actual command
                    let actual_cmd = if action_lower.starts_with("shell:") {
                        action_str.strip_prefix("shell:").unwrap_or("").trim()
                    } else if action_lower.starts_with("shell ") {
                        action_str.strip_prefix("shell ").unwrap_or("").trim()
                    } else {
                        action_str.split_once(' ').map(|x| x.1.trim()).unwrap_or("")
                    };
                    if actual_cmd.is_empty() {
                        // Bare tool name with no actual command — skip entirely
                        return None;
                    }
                    step.insert("type".to_string(), serde_json::json!("run_shell"));
                    step.insert("command".to_string(), serde_json::json!(actual_cmd));
                } else if action_lower.starts_with("edit_code")
                    || action_lower.starts_with("edit_file")
                    || action_lower.starts_with("edit ")
                {
                    // LLM used "edit_code file.rs ..." as action — convert to EditCode step
                    let rest = action_str.split_once(' ').map(|x| x.1).unwrap_or("");
                    let file_path = rest.split_whitespace().next().unwrap_or("").to_string();
                    step.insert("type".to_string(), serde_json::json!("edit_code"));
                    step.insert("file_path".to_string(), serde_json::json!(file_path));
                    step.insert("description".to_string(), serde_json::json!(rest));
                } else if action_lower.starts_with("generate_code")
                    || action_lower.starts_with("write_file")
                    || action_lower.starts_with("write ")
                {
                    // LLM used "generate_code file.rs ..." as action — convert to GenerateCode step
                    let rest = action_str.split_once(' ').map(|x| x.1).unwrap_or("");
                    let file_path = rest.split_whitespace().next().unwrap_or("").to_string();
                    step.insert("type".to_string(), serde_json::json!("generate_code"));
                    step.insert("file_path".to_string(), serde_json::json!(file_path));
                    step.insert("description".to_string(), serde_json::json!(rest));
                } else if action_lower.starts_with("think") {
                    // LLM used "think: ..." or "think about ..." as action
                    let question = action_str
                        .strip_prefix("think:")
                        .or_else(|| action_str.strip_prefix("think "))
                        .map(|s| s.trim())
                        .unwrap_or(&action_str);
                    step.insert("type".to_string(), serde_json::json!("think"));
                    step.insert("question".to_string(), serde_json::json!(question));
                } else {
                    // Default: treat as shell command
                    step.insert("type".to_string(), serde_json::json!("run_shell"));
                    step.insert("command".to_string(), serde_json::json!(action_str));
                }

                // Carry over store_as if present
                if let Some(store) = map.get("store_as").or_else(|| map.get("name")) {
                    step.insert("store_as".to_string(), store.clone());
                }

                return Some(serde_json::Value::Object(step));
            }

            // Has "path" but no type — infer read_file or list_dir
            if let Some(path) = map.get("path").and_then(|v| v.as_str()) {
                let mut step = serde_json::Map::new();
                if path.ends_with('/')
                    || path == "."
                    || path == ".."
                    || (!path.contains('.') && !path.is_empty())
                {
                    step.insert("type".to_string(), serde_json::json!("list_dir"));
                } else {
                    step.insert("type".to_string(), serde_json::json!("read_file"));
                }
                step.insert("path".to_string(), serde_json::json!(path));
                if let Some(store) = map.get("store_as") {
                    step.insert("store_as".to_string(), store.clone());
                }
                return Some(serde_json::Value::Object(step));
            }

            // Has "question" but no type — probably a think
            if let Some(q) = map.get("question").and_then(|v| v.as_str()) {
                let mut step = serde_json::Map::new();
                step.insert("type".to_string(), serde_json::json!("think"));
                step.insert("question".to_string(), serde_json::json!(q));
                if let Some(store) = map.get("store_as") {
                    step.insert("store_as".to_string(), store.clone());
                }
                return Some(serde_json::Value::Object(step));
            }

            None // Unrecognizable step — skip it
        })
        .collect();

    serde_json::to_string(&normalized).unwrap_or_else(|_| json_str.to_string())
}

/// Extract a JSON array from text, returning (json_str, before, after).
/// Handles nested brackets and string escaping.
pub fn extract_json_array(text: &str) -> Option<(String, String, String)> {
    let start = text.find('[')?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &ch) in bytes[start..].iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b'[' if !in_string => depth += 1,
            b']' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    let end = start + i + 1;
                    let json_str = text[start..end].to_string();
                    let before = text[..start].to_string();
                    let after = text[end..].to_string();
                    return Some((json_str, before, after));
                }
            }
            _ => {}
        }
    }
    None
}
