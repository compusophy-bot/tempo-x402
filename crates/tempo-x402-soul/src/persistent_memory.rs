//! Persistent memory file — a markdown file the soul reads every cycle and can update.
//!
//! Lives at a configurable path (default `/data/soul_memory.md`). On first boot,
//! seeded with identity and purpose. Hard-capped at 4KB to prevent prompt bloat.

/// Hard cap on memory file size to prevent prompt bloat.
pub const MAX_MEMORY_BYTES: usize = 4096;

/// Seed content written on first boot.
const SEED_MEMORY: &str = "\
# Soul Memory

## Identity
I am the soul of an autonomous x402 payment node on the Tempo blockchain.
I observe, reason, and act to keep this node healthy and productive.

## Purpose
Create useful tools and endpoints that other agents can discover and pay to use
via x402 payments. Build an agent-to-agent economy — tools for agents, by agents.

## Goals
- Keep the node healthy and endpoints responsive
- Understand the codebase deeply enough to improve it
- Register endpoints that generate revenue
- Learn what services other agents need

## Learnings
(Record key insights here as you discover them)
";

/// Read the persistent memory file, or create it with seed content on first boot.
pub fn read_or_seed(path: &str) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // First boot — create with seed content
            if let Some(parent) = std::path::Path::new(path).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("failed to create memory dir: {e}"))?;
            }
            std::fs::write(path, SEED_MEMORY)
                .map_err(|e| format!("failed to write seed memory: {e}"))?;
            tracing::info!(path = %path, "Seeded persistent memory file");
            Ok(SEED_MEMORY.to_string())
        }
        Err(e) => Err(format!("failed to read memory file: {e}")),
    }
}

/// Update the persistent memory file. Rejects content exceeding MAX_MEMORY_BYTES.
pub fn update(path: &str, content: &str) -> Result<usize, String> {
    if content.len() > MAX_MEMORY_BYTES {
        return Err(format!(
            "memory content too large ({} bytes, max {})",
            content.len(),
            MAX_MEMORY_BYTES
        ));
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create memory dir: {e}"))?;
    }
    std::fs::write(path, content).map_err(|e| format!("failed to write memory file: {e}"))?;
    Ok(content.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_or_seed_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("soul_memory.md");
        let path_str = path.to_str().unwrap();

        let content = read_or_seed(path_str).unwrap();
        assert!(content.contains("Soul Memory"));
        assert!(content.contains("Identity"));

        // Second read should return same content
        let content2 = read_or_seed(path_str).unwrap();
        assert_eq!(content, content2);
    }

    #[test]
    fn test_update_respects_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("soul_memory.md");
        let path_str = path.to_str().unwrap();

        // Small update should work
        let result = update(path_str, "# Small memory");
        assert!(result.is_ok());

        // Too-large update should fail
        let large = "x".repeat(MAX_MEMORY_BYTES + 1);
        let result = update(path_str, &large);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_writes_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("soul_memory.md");
        let path_str = path.to_str().unwrap();

        update(path_str, "# Updated memory\nNew content here").unwrap();
        let content = std::fs::read_to_string(path_str).unwrap();
        assert_eq!(content, "# Updated memory\nNew content here");
    }
}
