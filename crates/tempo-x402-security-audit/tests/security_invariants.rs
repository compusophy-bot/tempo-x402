//! Security invariant tests for the tempo-x402 payment gateway.
//!
//! These tests verify that critical security properties hold across the codebase.
//! They run on every `cargo test --workspace` invocation, ensuring that future
//! changes don't accidentally regress security posture.

use regex::Regex;
use std::path::Path;
use walkdir::WalkDir;

/// Read all .rs source files from production crates (excluding tests, examples, and this crate).
fn production_source_files() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let crates_dir = root.join("crates");
    let mut files = Vec::new();

    for entry in WalkDir::new(&crates_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // Only .rs files
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }

        // Skip this crate itself
        if path
            .to_str()
            .map(|s| s.contains("security-audit"))
            .unwrap_or(false)
        {
            continue;
        }

        // Skip test directories and test files
        let path_str = path.to_str().unwrap_or("");
        if path_str.contains("tests/") || path_str.contains("\\tests\\") {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            files.push((path_str.to_string(), content));
        }
    }

    files
}

/// Filter to only lines outside of `#[cfg(test)]` modules and `mod tests` blocks.
/// This is a heuristic: it removes everything after `#[cfg(test)]` in a file.
fn production_lines(content: &str) -> String {
    let mut result = Vec::new();
    let mut in_test_module = false;

    for line in content.lines() {
        if line.contains("#[cfg(test)]") || line.trim().starts_with("mod tests") {
            in_test_module = true;
        }
        if !in_test_module {
            result.push(line);
        }
    }

    result.join("\n")
}

#[test]
fn no_hardcoded_private_keys_in_production_code() {
    let hex_64_re = Regex::new(r"0x[a-fA-F0-9]{64}").unwrap();
    let files = production_source_files();

    let allowed_patterns = [
        "DEMO_PRIVATE_KEY",
        "SECP256K1_N_DIV_2",
        "#[cfg(feature = \"demo\")]",
        "#[deprecated",
    ];

    for (path, content) in &files {
        let prod_content = production_lines(content);

        for mat in hex_64_re.find_iter(&prod_content) {
            let line_num = prod_content[..mat.start()].lines().count() + 1;
            let line = prod_content.lines().nth(line_num - 1).unwrap_or("");

            // Check if this line or nearby context contains an allowed pattern
            let context_start = mat.start().saturating_sub(200);
            let context_end = (mat.end() + 200).min(prod_content.len());
            let context = &prod_content[context_start..context_end];

            let is_allowed = allowed_patterns.iter().any(|p| context.contains(p));

            assert!(
                is_allowed,
                "Potential hardcoded secret found at {}:{}: {}",
                path,
                line_num,
                line.trim()
            );
        }
    }
}

#[test]
fn hmac_no_early_returns_before_mac_computation() {
    let files = production_source_files();

    for (path, content) in &files {
        if !path.contains("hmac.rs") {
            continue;
        }

        // Find the verify_hmac function
        if let Some(func_start) = content.find("fn verify_hmac") {
            let func_body = &content[func_start..];
            // Find the end of the function (next `fn ` at the same or lower indentation)
            let func_end = func_body[1..]
                .find("\nfn ")
                .or_else(|| func_body[1..].find("\npub fn "))
                .unwrap_or(func_body.len() - 1)
                + 1;
            let func_text = &func_body[..func_end];

            // Check that hex decode failure doesn't cause early return before MAC verification
            // The pattern we're guarding against is: if decode fails, return false immediately
            // (which leaks timing information about the signature format)
            assert!(
                !func_text.contains("return false") || func_text.contains("verify_slice"),
                "HMAC verify function at {} may have timing-leaking early returns. \
                 Ensure hex decode failures use unwrap_or_default() and always proceed to MAC comparison.",
                path
            );
        }
    }
}

#[test]
fn http_clients_disable_redirects() {
    let files = production_source_files();
    let builder_re = Regex::new(r"reqwest::Client::builder\(\)").unwrap();
    let redirect_re = Regex::new(r"redirect\s*\(\s*.*Policy::none\(\)").unwrap();

    for (path, content) in &files {
        let prod_content = production_lines(content);

        // Find all reqwest Client builder invocations
        for mat in builder_re.find_iter(&prod_content) {
            // Look in the next ~500 chars for .build()
            let search_end = (mat.end() + 500).min(prod_content.len());
            let builder_chain = &prod_content[mat.start()..search_end];

            // Check that redirect policy is set to none
            assert!(
                redirect_re.is_match(builder_chain),
                "reqwest::Client::builder() at {} does not set redirect(Policy::none()). \
                 All HTTP clients must disable redirects to prevent SSRF via redirect.",
                path
            );
        }
    }
}

#[test]
fn constant_time_uses_subtle_crate() {
    let files = production_source_files();

    for (path, content) in &files {
        let prod_content = production_lines(content);

        // Check for manual XOR-based constant-time comparison patterns
        // (the old pattern was: `a ^ b` in a loop for comparison)
        if prod_content.contains("fn constant_time_eq") {
            assert!(
                prod_content.contains("x402::security::constant_time_eq")
                    || prod_content.contains("subtle::")
                    || prod_content.contains("use subtle"),
                "File {} contains constant_time_eq that doesn't use the subtle crate. \
                 Use x402::security::constant_time_eq or subtle::ConstantTimeEq directly.",
                path
            );
        }
    }
}

#[test]
fn webhooks_require_https() {
    let files = production_source_files();

    for (path, content) in &files {
        if !path.contains("webhook.rs") {
            continue;
        }

        let prod_content = production_lines(content);

        // The validate_webhook_urls function must return an error (not just warn) for non-HTTPS
        if prod_content.contains("fn validate_webhook_urls") {
            assert!(
                prod_content.contains("return Err(") || prod_content.contains("Err(format!"),
                "webhook validation at {} must return Err for non-HTTPS URLs, not just log a warning.",
                path
            );
        }
    }
}

#[test]
fn error_responses_do_not_leak_internals() {
    let files = production_source_files();

    // Patterns that should NOT appear in user-facing error messages
    let dangerous_patterns = [
        "RPC unreachable",
        "database error:",
        "stack trace",
        "RUST_BACKTRACE",
    ];

    for (path, content) in &files {
        // Only check files that construct HTTP error responses
        if !content.contains("HttpResponse::") {
            continue;
        }

        let prod_content = production_lines(content);

        for pattern in &dangerous_patterns {
            // Check if the pattern appears in json!() response bodies
            if prod_content.contains(pattern) {
                // Check if it's inside a json!() or .json() response body
                for (i, line) in prod_content.lines().enumerate() {
                    if line.contains(pattern) && line.contains("json") {
                        panic!(
                            "Potentially sensitive error detail '{}' found in HTTP response at {}:{}. \
                             Internal details should be logged server-side only.",
                            pattern,
                            path,
                            i + 1
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn nonce_store_sqlite_preferred_in_production() {
    let files = production_source_files();

    for (path, content) in &files {
        // Check facilitator main.rs for nonce store initialization
        if !path.contains("facilitator") || !path.contains("main.rs") {
            continue;
        }

        let prod_content = production_lines(content);

        // The facilitator should create a SqliteNonceStore, not just InMemoryNonceStore
        if prod_content.contains("InMemoryNonceStore") && !prod_content.contains("SqliteNonceStore")
        {
            panic!(
                "Facilitator at {} appears to use only InMemoryNonceStore. \
                 Production deployments must use SqliteNonceStore for nonce persistence across restarts.",
                path
            );
        }
    }
}

#[test]
fn no_sql_string_formatting_in_node_db() {
    let files = production_source_files();

    // Patterns that indicate SQL injection via string formatting
    let dangerous_sql_patterns = [
        "format!(\"INSERT",
        "format!(\"UPDATE",
        "format!(\"DELETE",
        "format!(\"SELECT",
        "format!(\"DROP",
        // Escaped-quote approach used in the old code
        ".replace('\\'', \"''\")",
    ];

    for (path, content) in &files {
        // Only check node DB files
        if !path.contains("x402-node") || !path.contains("db.rs") {
            continue;
        }

        let prod_content = production_lines(content);

        for pattern in &dangerous_sql_patterns {
            assert!(
                !prod_content.contains(pattern),
                "SQL string formatting pattern '{}' found in {}. \
                 All SQL DML must use parameterized queries (rusqlite params![]).",
                pattern,
                path
            );
        }
    }
}

#[test]
fn node_registration_validates_input() {
    let files = production_source_files();

    for (path, content) in &files {
        if !path.contains("x402-node") || !path.contains("instance.rs") {
            continue;
        }

        let prod_content = production_lines(content);

        // Must validate instance_id format
        assert!(
            prod_content.contains("is_valid_uuid"),
            "Instance registration at {} must validate instance_id as UUID format.",
            path
        );

        // Must validate address format
        assert!(
            prod_content.contains("is_valid_evm_address"),
            "Instance registration at {} must validate address as EVM address format.",
            path
        );

        // Must validate URL scheme
        assert!(
            prod_content.contains("is_valid_https_url") || prod_content.contains("https://"),
            "Instance registration at {} must validate URL uses HTTPS.",
            path
        );
    }
}

#[test]
fn identity_private_key_not_logged() {
    let files = production_source_files();

    for (path, content) in &files {
        if !path.contains("x402-identity") {
            continue;
        }

        let prod_content = production_lines(content);

        // Check that tracing/logging macros don't reference private_key directly
        for (i, line) in prod_content.lines().enumerate() {
            let trimmed = line.trim();
            if (trimmed.contains("tracing::")
                || trimmed.contains("tracing::info!")
                || trimmed.contains("tracing::debug!")
                || trimmed.contains("tracing::warn!"))
                && trimmed.contains("private_key")
                && !trimmed.contains("FACILITATOR_PRIVATE_KEY")
                && !trimmed.contains("\"Injected FACILITATOR_PRIVATE_KEY\"")
            {
                panic!(
                    "Private key may be logged at {}:{}. Never log private key material.\n  {}",
                    path,
                    i + 1,
                    trimmed
                );
            }
        }
    }
}

#[test]
fn parent_url_requires_https() {
    let files = production_source_files();

    for (path, content) in &files {
        if !path.contains("x402-identity") || !path.contains("lib.rs") {
            continue;
        }

        let prod_content = production_lines(content);

        // The bootstrap function must validate PARENT_URL uses HTTPS
        if prod_content.contains("PARENT_URL") {
            assert!(
                prod_content.contains("starts_with(\"https://\")")
                    || prod_content.contains("starts_with(\"https:\")"),
                "Identity bootstrap at {} must validate PARENT_URL uses HTTPS to prevent SSRF.",
                path
            );
        }
    }
}

#[test]
fn clone_route_uses_atomic_limit_check() {
    let files = production_source_files();

    for (path, content) in &files {
        if !path.contains("x402-node") {
            continue;
        }

        // Check clone route uses atomic check-and-insert, not separate check-then-insert
        if path.contains("clone.rs") && content.contains("clone_instance") {
            let prod_content = production_lines(content);
            assert!(
                prod_content.contains("create_child_if_under_limit"),
                "Clone route at {} must use atomic create_child_if_under_limit to prevent \
                 TOCTOU race condition on children count.",
                path
            );
        }
    }
}

#[test]
fn clone_errors_do_not_leak_details() {
    let files = production_source_files();

    for (path, content) in &files {
        if !path.contains("x402-node") || !path.contains("clone.rs") {
            continue;
        }

        let prod_content = production_lines(content);

        // Ensure clone errors don't forward internal error messages to HTTP response
        // The pattern to check: map_err should use a generic message, not format!("{e}")
        for (i, line) in prod_content.lines().enumerate() {
            if line.contains("GatewayError::Internal(format!(\"clone failed: {e}\"))") {
                panic!(
                    "Clone error at {}:{} leaks internal error details to HTTP response. \
                     Use a generic error message and log the details server-side.",
                    path,
                    i + 1
                );
            }
        }
    }
}

#[test]
fn every_crate_has_claude_md() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let crates_dir = root.join("crates");
    let mut missing = Vec::new();

    for entry in std::fs::read_dir(&crates_dir).expect("read crates dir") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip hidden directories
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(true, |n| n.starts_with('.'))
        {
            continue;
        }
        if !path.join("CLAUDE.md").exists() {
            missing.push(path.display().to_string());
        }
    }

    assert!(
        missing.is_empty(),
        "Crates missing CLAUDE.md (add one when creating a new crate): {:?}",
        missing
    );
}

#[test]
fn hmac_secret_is_mandatory() {
    let files = production_source_files();

    for (path, content) in &files {
        // Check facilitator state for HMAC secret type
        if !path.contains("facilitator") || !path.contains("state.rs") {
            continue;
        }

        let prod_content = production_lines(content);

        // HMAC secret should not be Option<Vec<u8>> — it should be Vec<u8> (mandatory)
        assert!(
            !prod_content.contains("hmac_secret: Option<Vec<u8>>"),
            "HMAC secret at {} is still Optional. It must be mandatory (Vec<u8>, not Option<Vec<u8>>).",
            path
        );
    }
}
