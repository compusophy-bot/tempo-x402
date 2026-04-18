//! Local model backend — runs GGUF models via llama-cpp for inference.
//!
//! Supports any GGUF model: Qwen, DeepSeek, CodeGemma, StarCoder, etc.
//! Downloads from HuggingFace if not present locally.
//!
//! NOTE: Requires `llama-cpp` feature to be enabled. When disabled,
//! this module provides a stub that returns an error.

use crate::runner::CodeGenerator;
use x402_soul::benchmark::BenchmarkProblem;

/// Local GGUF model for code generation.
pub struct LocalModelGenerator {
    /// Human-readable name for results
    name: String,
    /// Path to GGUF model file
    model_path: String,
    /// Number of context tokens
    n_ctx: u32,
    /// Max tokens to generate
    max_tokens: u32,
    /// Temperature for sampling
    temperature: f32,
}

impl LocalModelGenerator {
    pub fn new(name: String, model_path: String) -> Self {
        Self {
            name,
            model_path,
            n_ctx: 4096,
            max_tokens: 2048,
            temperature: 0.2,
        }
    }

    pub fn with_ctx(mut self, n_ctx: u32) -> Self {
        self.n_ctx = n_ctx;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    fn build_prompt(problem: &BenchmarkProblem) -> String {
        // Chat-style prompt that works with instruction-tuned models
        format!(
            "You are an expert Rust programmer. Write a complete Rust library (src/lib.rs) that passes these tests.\n\n\
             ## Problem: {}\n\n\
             ## Instructions\n{}\n\n\
             ## Tests\n```rust\n{}\n```\n\n\
             ## Starter Code\n```rust\n{}\n```\n\n\
             Output ONLY the Rust code for src/lib.rs. No explanations.",
            problem.slug,
            problem.instructions,
            problem.test_code,
            problem.starter_code,
        )
    }

    /// Run inference using a subprocess call to llama-cli (llama.cpp).
    /// This avoids complex FFI bindings — just shell out to the binary.
    /// The user downloads llama.cpp and points to the binary.
    async fn generate_via_cli(&self, prompt: &str) -> Result<String, String> {
        // Look for llama-cli in PATH or LLAMA_CPP_PATH env var
        let llama_bin = std::env::var("LLAMA_CPP_PATH")
            .unwrap_or_else(|_| "llama-cli".to_string());

        let output = tokio::process::Command::new(&llama_bin)
            .args([
                "-m", &self.model_path,
                "-p", prompt,
                "-n", &self.max_tokens.to_string(),
                "--temp", &self.temperature.to_string(),
                "-c", &self.n_ctx.to_string(),
                "--no-display-prompt",
                "-e",  // escape sequences
            ])
            .output()
            .await
            .map_err(|e| format!(
                "llama-cli not found (set LLAMA_CPP_PATH or install llama.cpp): {e}"
            ))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("llama-cli failed: {stderr}"));
        }

        let text = String::from_utf8_lossy(&output.stdout).to_string();
        let code = strip_code_fences(&text);
        Ok(code.to_string())
    }
}

#[async_trait::async_trait]
impl CodeGenerator for LocalModelGenerator {
    async fn generate(&self, problem: &BenchmarkProblem) -> Result<String, String> {
        // Check model file exists
        if !std::path::Path::new(&self.model_path).exists() {
            return Err(format!(
                "Model file not found: {}. Download a GGUF model from HuggingFace:\n  \
                 huggingface-cli download Qwen/Qwen2.5-Coder-0.5B-Instruct-GGUF \
                 qwen2.5-coder-0.5b-instruct-q4_k_m.gguf --local-dir models/",
                self.model_path
            ));
        }

        let prompt = Self::build_prompt(problem);
        self.generate_via_cli(&prompt).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn strip_code_fences(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(after) = trimmed.strip_prefix("```rust") {
        if let Some(code) = after.strip_suffix("```") {
            return code.trim();
        }
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
