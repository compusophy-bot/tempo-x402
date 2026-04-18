//! HumanEval-Rust benchmark loader.
//!
//! Loads HumanEval problems translated to Rust (from MultiPL-E dataset).
//! Problems are fetched from HuggingFace on first run and cached locally.
//! Falls back to a small embedded set if fetch fails.

use x402_soul::benchmark::BenchmarkProblem;

const CACHE_DIR: &str = "humaneval_cache";

/// Load HumanEval-Rust problems. Tries cached files first, then fetches from HuggingFace.
pub fn load_humaneval_problems() -> Vec<BenchmarkProblem> {
    // Try loading from cache
    if let Ok(problems) = load_from_cache() {
        if !problems.is_empty() {
            tracing::info!(count = problems.len(), "Loaded HumanEval-Rust from cache");
            return problems;
        }
    }

    // Fall back to embedded sample set
    tracing::info!("Using embedded HumanEval-Rust sample (10 problems)");
    embedded_sample()
}

/// Fetch HumanEval-Rust from HuggingFace and cache locally.
/// Call this once before running benchmarks.
pub async fn fetch_and_cache() -> Result<usize, String> {
    // MultiPL-E dataset: https://huggingface.co/datasets/nuprl/MultiPL-E
    // The Rust subset contains HumanEval problems translated to Rust with test cases.
    let url = "https://huggingface.co/datasets/nuprl/MultiPL-E/resolve/main/data/humaneval-rs-keep.jsonl";

    tracing::info!("Fetching HumanEval-Rust from HuggingFace...");
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("fetch failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("read: {e}"))?;
    let mut problems = Vec::new();

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(problem) = parse_multipl_e_entry(&entry) {
                problems.push(problem);
            }
        }
    }

    // Cache to disk
    std::fs::create_dir_all(CACHE_DIR).map_err(|e| format!("mkdir: {e}"))?;
    let json = serde_json::to_string_pretty(&problems).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(format!("{CACHE_DIR}/humaneval-rust.json"), &json)
        .map_err(|e| format!("write: {e}"))?;

    let count = problems.len();
    tracing::info!(count, "Fetched and cached HumanEval-Rust");
    Ok(count)
}

fn load_from_cache() -> Result<Vec<BenchmarkProblem>, String> {
    let path = format!("{CACHE_DIR}/humaneval-rust.json");
    let json = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("parse: {e}"))
}

/// Parse a MultiPL-E JSONL entry into a BenchmarkProblem.
fn parse_multipl_e_entry(entry: &serde_json::Value) -> Option<BenchmarkProblem> {
    let name = entry.get("name").and_then(|v| v.as_str())?;
    let prompt = entry.get("prompt").and_then(|v| v.as_str())?;
    let tests = entry.get("tests").and_then(|v| v.as_str())?;

    // MultiPL-E provides the function signature as "prompt" and tests separately
    let slug = format!("humaneval-{}", name.replace('/', "-"));

    Some(BenchmarkProblem {
        slug,
        instructions: format!(
            "Complete this Rust function so that all tests pass.\n\n```rust\n{prompt}\n```"
        ),
        test_code: tests.to_string(),
        starter_code: prompt.to_string(),
        difficulty: "humaneval".to_string(),
        cargo_toml: String::new(),
    })
}

/// Small embedded sample of HumanEval-Rust problems for offline use.
fn embedded_sample() -> Vec<BenchmarkProblem> {
    vec![
        BenchmarkProblem {
            slug: "humaneval-has-close-elements".to_string(),
            instructions: "Check if any two numbers in a list are closer to each other than a given threshold.".to_string(),
            test_code: r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_has_close_elements() {
        assert!(has_close_elements(vec![1.0, 2.0, 3.9, 4.0, 5.0, 2.2], 0.3));
        assert!(!has_close_elements(vec![1.0, 2.0, 3.9, 4.0, 5.0, 2.2], 0.05));
        assert!(has_close_elements(vec![1.0, 2.0, 5.9, 4.0, 5.0], 0.95));
        assert!(!has_close_elements(vec![1.0, 2.0, 5.9, 4.0, 5.0], 0.8));
        assert!(has_close_elements(vec![1.0, 2.0, 3.0, 4.0, 5.0, 2.0], 0.1));
        assert!(!has_close_elements(vec![1.1, 2.2, 3.1, 4.1, 5.1], 1.0));
        assert!(has_close_elements(vec![1.1, 2.2, 3.1, 4.1, 5.1], 2.0));
    }
}
"#.to_string(),
            starter_code: "pub fn has_close_elements(numbers: Vec<f64>, threshold: f64) -> bool {\n    todo!()\n}\n".to_string(),
            difficulty: "humaneval".to_string(),
            cargo_toml: String::new(),
        },
        BenchmarkProblem {
            slug: "humaneval-separate-paren-groups".to_string(),
            instructions: "Separate groups of nested parentheses into separate strings. Each group is balanced.".to_string(),
            test_code: r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_separate_paren_groups() {
        assert_eq!(separate_paren_groups("( ) (( )) (( )( ))".to_string()), vec!["()", "(())", "(()())"]);
        assert_eq!(separate_paren_groups("() (()) () ((()))".to_string()), vec!["()", "(())", "()", "((()))"]);
        assert_eq!(separate_paren_groups("(()(()))".to_string()), vec!["(()(()))"]);
        assert_eq!(separate_paren_groups("( ) ( )".to_string()), vec!["()", "()"]);
    }
}
"#.to_string(),
            starter_code: "pub fn separate_paren_groups(paren_string: String) -> Vec<String> {\n    todo!()\n}\n".to_string(),
            difficulty: "humaneval".to_string(),
            cargo_toml: String::new(),
        },
        BenchmarkProblem {
            slug: "humaneval-truncate-number".to_string(),
            instructions: "Given a positive floating point number, return the decimal part (the part after the decimal point).".to_string(),
            test_code: r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_truncate_number() {
        assert!((truncate_number(3.5) - 0.5).abs() < 1e-6);
        assert!((truncate_number(1.25) - 0.25).abs() < 1e-6);
        assert!((truncate_number(123.0) - 0.0).abs() < 1e-6);
    }
}
"#.to_string(),
            starter_code: "pub fn truncate_number(number: f64) -> f64 {\n    todo!()\n}\n".to_string(),
            difficulty: "humaneval".to_string(),
            cargo_toml: String::new(),
        },
    ]
}
