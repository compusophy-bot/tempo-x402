//! Claude API backend for benchmarking.

use crate::runner::CodeGenerator;
use x402_soul::benchmark::BenchmarkProblem;

pub struct ClaudeGenerator {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl ClaudeGenerator {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    fn build_prompt(problem: &BenchmarkProblem) -> String {
        format!(
            "You are an expert Rust programmer. Solve this problem by writing a complete Rust \
             library (src/lib.rs) that passes the provided tests.\n\n\
             ## Problem: {}\n\n\
             ## Instructions\n{}\n\n\
             ## Test Code (must pass)\n```rust\n{}\n```\n\n\
             ## Starter Code\n```rust\n{}\n```\n\n\
             {}\
             IMPORTANT: Output ONLY the complete src/lib.rs code. No explanations, no markdown \
             fences, no commentary. Just the Rust code that will be written to src/lib.rs.",
            problem.slug,
            problem.instructions,
            problem.test_code,
            problem.starter_code,
            if !problem.cargo_toml.is_empty() {
                format!(
                    "## Available Dependencies (Cargo.toml)\n```toml\n{}\n```\n\n",
                    problem.cargo_toml
                )
            } else {
                String::new()
            }
        )
    }
}

#[async_trait::async_trait]
impl CodeGenerator for ClaudeGenerator {
    async fn generate(&self, problem: &BenchmarkProblem) -> Result<String, String> {
        let prompt = Self::build_prompt(problem);

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API error {status}: {text}"));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;

        // Extract text from Claude's response
        let text = json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| "no text in response".to_string())?;

        // Strip markdown fences if present
        let code = strip_code_fences(text);
        Ok(code.to_string())
    }

    fn name(&self) -> &str {
        &self.model
    }
}

/// Strip ```rust ... ``` fences from LLM output.
fn strip_code_fences(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(after) = trimmed.strip_prefix("```rust") {
        if let Some(code) = after.strip_suffix("```") {
            return code.trim();
        }
        // No closing fence — return everything after opening
        return after.trim();
    }
    if let Some(after) = trimmed.strip_prefix("```") {
        if let Some(code) = after.strip_suffix("```") {
            return code.trim();
        }
        return after.trim();
    }
    trimmed
}
