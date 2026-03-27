//! Protected file guard — hardcoded safety layer preventing self-bricking.
//!
//! These paths are hardcoded (not env-var based) so the soul cannot bypass
//! protection by modifying environment variables via shell.

/// Protected path prefixes — ONLY files that would brick the agent or compromise infra.
/// Everything else should be editable for self-improvement.
const PROTECTED_PREFIXES: &[&str] = &[
    // Core safety: editing these could brick the agent
    "crates/tempo-x402-soul/src/guard.rs", // self-protection bypass
    "crates/tempo-x402-soul/src/db/mod.rs", // database schema corruption
    "crates/tempo-x402-soul/src/config.rs", // config corruption
    "crates/tempo-x402-soul/src/llm.rs",   // API client corruption
    "crates/tempo-x402-soul/src/tools/mod.rs", // tool executor dispatch corruption
    "crates/tempo-x402-soul/src/error.rs", // error type changes break everything
    // Infrastructure: these affect other systems, not just this agent
    "crates/tempo-x402-identity/",
    "crates/tempo-x402-node/src/main.rs",
    "crates/tempo-x402-gateway/src/",
    ".github/",
];

/// Patterns that are protected regardless of location.
/// Cargo.lock is NOT protected — it's auto-generated when code changes.
/// Protecting it blocks all commits after any code edit.
const PROTECTED_FILENAMES: &[&str] = &["Cargo.toml"];

/// Check if a path is protected from writes.
pub fn is_protected(path: &str) -> bool {
    let normalized = normalize_path(path);

    // Path traversal prevention
    if normalized.contains("..") {
        return true;
    }

    // Check exact/prefix matches
    for prefix in PROTECTED_PREFIXES {
        if normalized == *prefix || normalized.starts_with(prefix) {
            return true;
        }
    }

    // Check filename patterns (anywhere in the tree)
    for filename in PROTECTED_FILENAMES {
        if normalized.ends_with(filename) {
            return true;
        }
        // Also match paths like "some/dir/Cargo.toml"
        let with_slash = format!("/{filename}");
        if normalized.ends_with(&with_slash) || normalized == *filename {
            return true;
        }
    }

    false
}

/// Validate that a path is safe to write to. Returns Ok(()) or an error message.
pub fn validate_write_target(path: &str) -> Result<(), String> {
    if is_protected(path) {
        Err(format!(
            "PROTECTED: '{}' is a protected path and cannot be modified",
            path
        ))
    } else {
        Ok(())
    }
}

/// Normalize a path: strip leading `./` or `/`, convert backslashes to forward slashes.
fn normalize_path(path: &str) -> String {
    let s = path.replace('\\', "/");
    let s = s.strip_prefix("./").unwrap_or(&s);
    let s = s.strip_prefix('/').unwrap_or(s);
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_soul_core_files() {
        assert!(is_protected("crates/tempo-x402-soul/src/tools/mod.rs"));
        assert!(is_protected("crates/tempo-x402-soul/src/llm.rs"));
        assert!(is_protected("crates/tempo-x402-soul/src/db/mod.rs"));
        assert!(is_protected("crates/tempo-x402-soul/src/error.rs"));
        assert!(is_protected("crates/tempo-x402-soul/src/guard.rs"));
        assert!(is_protected("crates/tempo-x402-soul/src/config.rs"));
    }

    #[test]
    fn protects_identity_crate() {
        assert!(is_protected("crates/tempo-x402-identity/src/lib.rs"));
        assert!(is_protected("crates/tempo-x402-identity/Cargo.toml"));
    }

    #[test]
    fn protects_cargo_toml() {
        assert!(is_protected("Cargo.toml"));
        assert!(is_protected("crates/tempo-x402-server/Cargo.toml"));
        // Cargo.lock is NOT protected — it's auto-generated
        assert!(!is_protected("Cargo.lock"));
    }

    #[test]
    fn protects_github_dir() {
        assert!(is_protected(".github/workflows/ci.yml"));
    }

    #[test]
    fn blocks_path_traversal() {
        assert!(is_protected("../etc/passwd"));
        assert!(is_protected("crates/../../etc/shadow"));
    }

    #[test]
    fn protects_infra_files() {
        // Node main.rs is protected (startup/deployment)
        assert!(is_protected("crates/tempo-x402-node/src/main.rs"));
        // Gateway is protected (payment infrastructure)
        assert!(is_protected(
            "crates/tempo-x402-gateway/src/routes/register.rs"
        ));
        assert!(is_protected("crates/tempo-x402-gateway/src/proxy.rs"));
        // Node routes are NOT protected — agents can improve their own endpoints
        assert!(!is_protected("crates/tempo-x402-node/src/routes/soul.rs"));
        assert!(!is_protected("crates/tempo-x402-node/src/routes/clone.rs"));
    }

    #[test]
    fn allows_self_improvable_files() {
        // These were previously protected but agents need to edit them to improve
        assert!(!is_protected("crates/tempo-x402-soul/src/benchmark.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/brain.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/capability.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/feedback.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/elo.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/validation.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/thinking/mod.rs"));
        assert!(!is_protected(
            "crates/tempo-x402-soul/src/thinking/plan_cycle.rs"
        ));
        assert!(!is_protected("crates/tempo-x402-soul/src/prompts.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/chat.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/memory.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/temporal.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/cortex.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/genesis.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/hivemind.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/synthesis.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/tools/social.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/db/goals.rs"));
        assert!(!is_protected(
            "crates/tempo-x402-node/src/routes/soul/status.rs"
        ));
        assert!(!is_protected("README.md"));
    }

    #[test]
    fn normalizes_paths() {
        assert!(is_protected("./crates/tempo-x402-soul/src/tools/mod.rs"));
        assert!(is_protected("/crates/tempo-x402-soul/src/tools/mod.rs"));
    }

    #[test]
    fn validate_returns_error_for_protected() {
        assert!(validate_write_target("crates/tempo-x402-soul/src/tools/mod.rs").is_err());
    }

    #[test]
    fn validate_returns_ok_for_allowed() {
        assert!(validate_write_target("crates/tempo-x402-server/src/main.rs").is_ok());
    }
}
