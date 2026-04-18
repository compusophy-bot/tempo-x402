//! Results storage — JSON files for benchmark outputs.

use crate::runner::ProblemResult;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkOutput {
    pub model: String,
    pub timestamp: String,
    pub total_problems: usize,
    pub passed: usize,
    pub raw_pass_rate: f64,
    pub weighted_pass_rate: f64,
    pub results: Vec<ProblemResult>,
}

/// Save benchmark output to a JSON file.
pub fn save(output: &BenchmarkOutput, path: &str) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(output).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

/// Load benchmark output from a JSON file.
pub fn load(path: &str) -> Result<BenchmarkOutput, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("parse: {e}"))
}

/// Print a summary table of all results in a directory.
pub fn print_summary(dir: &str) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            println!("No results directory at {dir}");
            return;
        }
    };

    let mut outputs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(output) = load(&path.to_string_lossy()) {
                outputs.push(output);
            }
        }
    }

    if outputs.is_empty() {
        println!("No results found in {dir}");
        return;
    }

    println!("\n{:<30} {:>8} {:>8} {:>10} {:>10}", "Model", "Passed", "Total", "Raw %", "Weighted %");
    println!("{}", "-".repeat(70));
    for o in &outputs {
        println!(
            "{:<30} {:>8} {:>8} {:>9.1}% {:>9.1}%",
            o.model, o.passed, o.total_problems, o.raw_pass_rate, o.weighted_pass_rate
        );
    }

    // Per-tier breakdown
    println!("\nPer-tier breakdown:");
    for o in &outputs {
        let mut by_tier: std::collections::BTreeMap<String, (usize, usize)> =
            std::collections::BTreeMap::new();
        for r in &o.results {
            let entry = by_tier.entry(r.tier.clone()).or_insert((0, 0));
            entry.1 += 1;
            if r.passed {
                entry.0 += 1;
            }
        }
        println!("  {}:", o.model);
        for (tier, (pass, total)) in &by_tier {
            let pct = if *total > 0 {
                *pass as f64 / *total as f64 * 100.0
            } else {
                0.0
            };
            println!("    {tier}: {pass}/{total} ({pct:.0}%)");
        }
    }
    println!();
}
