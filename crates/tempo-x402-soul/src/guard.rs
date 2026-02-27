//! Protected file guard — hardcoded safety layer preventing self-bricking.
//!
//! These paths are hardcoded (not env-var based) so the soul cannot bypass
//! protection by modifying environment variables via shell.

/// Protected path prefixes. Any file starting with one of these cannot be written.
const PROTECTED_PREFIXES: &[&str] = &[
    "crates/tempo-x402-soul/src/tools.rs",
    "crates/tempo-x402-soul/src/llm.rs",
    "crates/tempo-x402-soul/src/db.rs",
    "crates/tempo-x402-soul/src/error.rs",
    "crates/tempo-x402-soul/src/guard.rs",
    "crates/tempo-x402-soul/src/config.rs",
    "crates/tempo-x402-soul/src/tool_registry.rs",
    "crates/tempo-x402-identity/",
    ".github/",
];

/// Patterns that are protected regardless of location.
const PROTECTED_FILENAMES: &[&str] = &["Cargo.toml", "Cargo.lock"];

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
        assert!(is_protected("crates/tempo-x402-soul/src/tools.rs"));
        assert!(is_protected("crates/tempo-x402-soul/src/llm.rs"));
        assert!(is_protected("crates/tempo-x402-soul/src/db.rs"));
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
    fn protects_cargo_files() {
        assert!(is_protected("Cargo.toml"));
        assert!(is_protected("Cargo.lock"));
        assert!(is_protected("crates/tempo-x402-server/Cargo.toml"));
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
    fn allows_normal_files() {
        assert!(!is_protected("crates/tempo-x402-server/src/main.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/thinking.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/chat.rs"));
        assert!(!is_protected("crates/tempo-x402-soul/src/memory.rs"));
        assert!(!is_protected("README.md"));
    }

    #[test]
    fn normalizes_paths() {
        assert!(is_protected("./crates/tempo-x402-soul/src/tools.rs"));
        assert!(is_protected("/crates/tempo-x402-soul/src/tools.rs"));
    }

    #[test]
    fn validate_returns_error_for_protected() {
        assert!(validate_write_target("crates/tempo-x402-soul/src/tools.rs").is_err());
    }

    #[test]
    fn validate_returns_ok_for_allowed() {
        assert!(validate_write_target("crates/tempo-x402-server/src/main.rs").is_ok());
    }
}
