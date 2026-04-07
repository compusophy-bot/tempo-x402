//! Code generation model orchestration — Phase 3.
//!
//! Thin wrapper around the model crate's BPE tokenizer and code generation
//! transformer. Handles:
//! - Loading/saving BPE tokenizer from soul_state
//! - Periodic training on accumulated benchmark solutions + commit diffs
//! - Local-first code generation with Gemini fallback
//! - Full observability: every gate logs why it blocks

use crate::db::SoulDatabase;

/// Load the BPE tokenizer from soul_state.
pub fn load_tokenizer(db: &SoulDatabase) -> x402_model::bpe::BpeTokenizer {
    match db.get_state("codegen_bpe_tokenizer").ok().flatten() {
        Some(json) if !json.is_empty() => {
            x402_model::bpe::BpeTokenizer::from_json(&json)
                .unwrap_or_else(|| x402_model::bpe::BpeTokenizer::new(8192))
        }
        _ => x402_model::bpe::BpeTokenizer::new(8192),
    }
}

/// Save the BPE tokenizer to soul_state.
pub fn save_tokenizer(db: &SoulDatabase, tok: &x402_model::bpe::BpeTokenizer) {
    let json = tok.to_json();
    if let Err(e) = db.set_state("codegen_bpe_tokenizer", &json) {
        tracing::warn!(error = %e, "Failed to save BPE tokenizer");
    }
}

/// Scan a directory tree for .rs files and append to corpus.
/// Caps total at `max_bytes` to prevent OOM on huge dep trees.
fn walk_rs_files(dir: &std::path::Path, corpus: &mut String, depth: u32, max_bytes: usize) {
    if depth > 10 || corpus.len() >= max_bytes {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if corpus.len() >= max_bytes {
            break;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip build artifacts, git, hidden dirs, tests, benches, examples
        if name_str.starts_with('.')
            || name_str == "target"
            || name_str == "node_modules"
            || name_str == "tests"
            || name_str == "benches"
            || name_str == "examples"
        {
            continue;
        }

        if path.is_dir() {
            walk_rs_files(&path, corpus, depth + 1, max_bytes);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                // Skip very large files (generated code, test fixtures)
                if content.len() < 50_000 && content.len() > 50 {
                    corpus.push_str(&content);
                    corpus.push('\n');
                }
            }
        }
    }
}

/// Scan workspace + cargo registry for .rs files.
/// Sources (in priority order):
/// 1. Workspace (own source code — 72K+ lines)
/// 2. Cargo registry (dependency source — tokio, serde, actix, alloy, etc.)
///
/// The model learns Rust from its own codebase AND the best crates in the ecosystem.
fn collect_workspace_corpus(workspace_root: &str) -> String {
    // Cap at 2MB to keep training cycles fast and avoid OOM
    const MAX_CORPUS_BYTES: usize = 2 * 1024 * 1024;
    let mut corpus = String::new();

    // Source 1: own source code (highest priority — it's learning about itself)
    let root = std::path::Path::new(workspace_root);
    if root.exists() {
        walk_rs_files(root, &mut corpus, 0, MAX_CORPUS_BYTES);
    }
    let own_bytes = corpus.len();

    // Source 2: cargo registry — dependency source code
    // Check common cargo home locations
    let cargo_homes = [
        std::env::var("CARGO_HOME").unwrap_or_default(),
        "/root/.cargo".to_string(),
        "/usr/local/cargo".to_string(),
        std::env::var("HOME")
            .map(|h| format!("{h}/.cargo"))
            .unwrap_or_default(),
    ];

    let mut deps_scanned = 0u32;
    for cargo_home in &cargo_homes {
        if cargo_home.is_empty() {
            continue;
        }
        let registry_src = std::path::Path::new(cargo_home).join("registry/src");
        if !registry_src.exists() {
            continue;
        }
        // registry/src/ contains one dir per registry (e.g., index.crates.io-xxx)
        if let Ok(registries) = std::fs::read_dir(&registry_src) {
            for reg in registries.flatten() {
                if !reg.path().is_dir() {
                    continue;
                }
                // Each registry dir contains crate dirs (e.g., serde-1.0.228/)
                if let Ok(crates) = std::fs::read_dir(reg.path()) {
                    for krate in crates.flatten() {
                        if corpus.len() >= MAX_CORPUS_BYTES {
                            break;
                        }
                        if !krate.path().is_dir() {
                            continue;
                        }
                        let src_dir = krate.path().join("src");
                        if src_dir.exists() {
                            let before = corpus.len();
                            walk_rs_files(&src_dir, &mut corpus, 0, MAX_CORPUS_BYTES);
                            if corpus.len() > before {
                                deps_scanned += 1;
                            }
                        }
                    }
                }
            }
        }
        if deps_scanned > 0 {
            break; // Found a valid cargo home, stop looking
        }
    }

    let deps_bytes = corpus.len() - own_bytes;
    if corpus.len() > 1000 {
        tracing::info!(
            own_bytes = own_bytes,
            deps_bytes = deps_bytes,
            deps_crates = deps_scanned,
            total_bytes = corpus.len(),
            "codegen: corpus collected (workspace + dependencies)"
        );
    }

    corpus
}

/// Train the BPE tokenizer on ALL available Rust code:
/// 1. Benchmark solutions (verified, high quality)
/// 2. Workspace codebase (72K+ lines of real Rust)
pub fn train_tokenizer(db: &SoulDatabase) {
    let solutions: Vec<serde_json::Value> = db
        .get_state("codegen_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut corpus = String::new();

    // Source 1: benchmark solutions (highest quality — verified by cargo test)
    for sol in &solutions {
        if let Some(code) = sol.get("code").and_then(|v| v.as_str()) {
            corpus.push_str(code);
            corpus.push('\n');
        }
    }

    // Source 2: workspace codebase (massive, real-world Rust)
    let workspace_root = std::env::var("SOUL_WORKSPACE_ROOT")
        .unwrap_or_else(|_| "/tmp/workspace".to_string());
    let ws_corpus = collect_workspace_corpus(&workspace_root);
    let ws_bytes = ws_corpus.len();
    corpus.push_str(&ws_corpus);

    if corpus.len() < 100 {
        tracing::debug!(
            solutions = solutions.len(),
            corpus_bytes = corpus.len(),
            "codegen: BPE skip — corpus too small (<100 bytes)"
        );
        return;
    }

    let mut tok = load_tokenizer(db);
    let before_vocab = tok.current_vocab_size();
    tok.train(&corpus);
    let after_vocab = tok.current_vocab_size();
    let ratio = tok.compression_ratio(&corpus);

    save_tokenizer(db, &tok);

    tracing::info!(
        solutions = solutions.len(),
        workspace_bytes = ws_bytes,
        corpus_bytes = corpus.len(),
        vocab_before = before_vocab,
        vocab_after = after_vocab,
        compression_ratio = format!("{ratio:.2}"),
        "codegen: BPE tokenizer trained (solutions + workspace)"
    );
}

/// Load the code gen model from soul_state.
pub fn load_model(db: &SoulDatabase) -> x402_model::codegen::CodeGenModel {
    match db.get_state("codegen_model").ok().flatten() {
        Some(json) if json.len() > 100 => {
            x402_model::codegen::CodeGenModel::from_json(&json)
                .unwrap_or_default()
        }
        _ => x402_model::codegen::CodeGenModel::new(),
    }
}

/// Save the code gen model to soul_state.
pub fn save_model(db: &SoulDatabase, model: &x402_model::codegen::CodeGenModel) {
    let json = model.to_json();
    if let Err(e) = db.set_state("codegen_model", &json) {
        tracing::warn!(error = %e, "Failed to save codegen model");
    }
}

/// Train the code generation model on ALL available Rust code:
/// 1. Benchmark solutions (verified, high quality — weighted 3x)
/// 2. Workspace .rs files (massive corpus — real-world patterns)
pub fn train_model(db: &SoulDatabase) {
    let tok = load_tokenizer(db);
    if tok.merges.is_empty() {
        tracing::debug!("codegen: model skip — BPE not trained yet (0 merges)");
        return;
    }

    // Collect all training examples as (code, weight) pairs
    let mut examples: Vec<(String, u32)> = Vec::new();

    // Source 1: benchmark solutions (verified — train 3x more on these)
    let solutions: Vec<serde_json::Value> = db
        .get_state("codegen_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    for sol in &solutions {
        if sol.get("passed").and_then(|v| v.as_bool()).unwrap_or(true) {
            if let Some(code) = sol.get("code").and_then(|v| v.as_str()) {
                if code.len() >= 50 {
                    examples.push((code.to_string(), 3)); // 3x weight for verified code
                }
            }
        }
    }

    // Source 2: workspace .rs files (bulk training data)
    let workspace_root = std::env::var("SOUL_WORKSPACE_ROOT")
        .unwrap_or_else(|_| "/tmp/workspace".to_string());
    let ws_corpus = collect_workspace_corpus(&workspace_root);
    // Split workspace into function-sized chunks (~200 lines each)
    // so the model sees complete logical units, not arbitrary slices
    let mut chunk = String::new();
    for line in ws_corpus.lines() {
        chunk.push_str(line);
        chunk.push('\n');
        // Split on blank lines after `}` (heuristic for function boundaries)
        if line.trim() == "}" && chunk.len() > 200 {
            examples.push((std::mem::take(&mut chunk), 1));
        }
        // Cap chunk size
        if chunk.len() > 5000 {
            examples.push((std::mem::take(&mut chunk), 1));
        }
    }
    if chunk.len() > 100 {
        examples.push((chunk, 1));
    }

    let total_examples = examples.len();
    let solution_count = solutions.len();

    if total_examples < 2 {
        tracing::debug!(
            examples = total_examples,
            "codegen: model skip — need >=2 training examples"
        );
        return;
    }

    let mut model = load_model(db);
    let mut total_loss = 0.0f32;
    let mut trained = 0u32;

    // Train on 50 examples per cycle — the model has 29M params and was only
    // seeing 10 examples (150 gradient steps) per cycle. At that rate it would
    // take thousands of cycles to see the full corpus once. 50 examples × ~15
    // windows × weight = ~1000+ gradient steps per cycle. With cycles every
    // few minutes, the model will see the full corpus in hours, not weeks.
    let offset = (model.train_steps as usize) % total_examples.max(1);
    let batch_size = 50.min(total_examples);
    for (code, weight) in examples.iter().cycle().skip(offset).take(batch_size) {
        // Skip very short code
        if code.len() < 50 {
            continue;
        }

        // Tokenize with BPE
        let mut tokens = vec![x402_model::bpe::BOS_TOKEN];
        tokens.extend(tok.encode(code));
        tokens.push(x402_model::bpe::EOS_TOKEN);

        // Truncate to max seq length
        if tokens.len() > x402_model::codegen::SMALL_MAX_SEQ {
            tokens.truncate(x402_model::codegen::SMALL_MAX_SEQ);
        }

        if tokens.len() < 3 {
            continue;
        }

        // Repeat training on high-weight examples (verified solutions get 3x passes)
        for _ in 0..*weight {
            // Train on sliding windows (128 tokens, step 64)
            // Was 64/32 — too small to learn function-level patterns.
            // 128 tokens ≈ 5-10 lines of Rust, enough to see a complete function.
            let window_size = 128.min(tokens.len());
            for start in (0..tokens.len().saturating_sub(window_size)).step_by(64) {
                let end = (start + window_size).min(tokens.len());
                let window = &tokens[start..end];
                let step = model.train_steps as f32;
                // Higher LR to converge faster. Was 0.0001-0.001, now 0.0005-0.003.
                // The model is at loss 9.2 (random). It needs aggressive updates
                // to break out of the noise floor, not gentle nudges.
                let lr = if step < 200.0 {
                    0.0005 + (0.003 - 0.0005) * (step / 200.0)
                } else {
                    let decay = ((step - 200.0) * std::f32::consts::PI / 10000.0).cos();
                    0.0005 + (0.003 - 0.0005) * 0.5 * (1.0 + decay)
                };
                let loss = model.train_step(window, lr);
                total_loss += loss;
                trained += 1;
            }
        }
    }

    if trained > 0 {
        save_model(db, &model);
        tracing::info!(
            trained,
            loss = format!("{:.4}", total_loss / trained as f32),
            running_loss = format!("{:.4}", model.running_loss),
            steps = model.train_steps,
            params = model.param_count(),
            solutions = solution_count,
            workspace_chunks = total_examples - solution_count,
            total_examples = total_examples,
            "codegen: model trained on solutions + workspace"
        );

        // Track loss history for learning acceleration metric (α)
        let now = chrono::Utc::now().timestamp();
        let mut loss_hist: Vec<(i64, f64)> = db
            .get_state("codegen_loss_history")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        loss_hist.push((now, model.running_loss as f64));
        if loss_hist.len() > 100 {
            loss_hist.drain(..loss_hist.len() - 100);
        }
        if let Ok(json) = serde_json::to_string(&loss_hist) {
            let _ = db.set_state("codegen_loss_history", &json);
        }
    }
}

/// Generate code given a prompt. Returns None if model not ready.
/// Minimum 10 training steps (was 100) — let it try earlier.
pub fn generate(db: &SoulDatabase, prompt: &str, max_tokens: usize) -> Option<String> {
    let tok = load_tokenizer(db);
    if tok.merges.is_empty() {
        tracing::debug!("codegen: generate skip — BPE not trained");
        return None;
    }

    let model = load_model(db);
    if model.train_steps < 10 {
        tracing::debug!(
            steps = model.train_steps,
            "codegen: generate skip — need >=10 training steps (have {})",
            model.train_steps
        );
        return None;
    }

    // Tokenize prompt
    let mut tokens = vec![x402_model::bpe::BOS_TOKEN];
    tokens.extend(tok.encode(prompt));

    // Generate token by token (greedy)
    for _ in 0..max_tokens {
        if tokens.len() >= model.max_seq {
            break;
        }

        let logits = model.forward(&tokens);

        // Temperature sampling — explore diverse outputs instead of repeating
        // the same greedy argmax every time. Temperature 0.8 balances quality
        // and diversity. Without this, codegen produces identical output forever.
        let temperature: f32 = 0.8;
        let next_token = {
            // Apply temperature
            let scaled: Vec<f32> = logits.iter().map(|l| l / temperature).collect();
            // Softmax
            let max_logit = scaled.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = scaled.iter().map(|l| (l - max_logit).exp()).collect();
            let sum: f32 = exps.iter().sum();
            let probs: Vec<f32> = exps.iter().map(|e| e / sum).collect();
            // Sample from distribution using a simple LCG PRNG seeded from token position
            let seed = (tokens.len() as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64);
            let r = ((seed >> 16) as f32) / (u32::MAX as f32);
            let mut cumulative = 0.0f32;
            let mut chosen = 0u32;
            for (i, p) in probs.iter().enumerate() {
                cumulative += p;
                if cumulative >= r {
                    chosen = i as u32;
                    break;
                }
            }
            chosen
        };

        if next_token == x402_model::bpe::EOS_TOKEN {
            break;
        }

        tokens.push(next_token);
    }

    // Decode (skip BOS + prompt tokens)
    let prompt_len = 1 + tok.encode(prompt).len();
    if tokens.len() <= prompt_len {
        tracing::debug!("codegen: generate produced no tokens");
        return None;
    }

    let generated = tok.decode(&tokens[prompt_len..]);
    if generated.trim().is_empty() {
        tracing::debug!("codegen: generate produced empty output");
        return None;
    }

    tracing::info!(
        prompt_tokens = prompt_len,
        generated_tokens = tokens.len() - prompt_len,
        "codegen: local model generated code"
    );
    Some(generated)
}

/// Record a successful code diff as training data for the codegen model.
/// Called after successful commits — supplements benchmark solutions.
pub fn record_training_example(db: &SoulDatabase, code: &str, source: &str) {
    if code.len() < 50 {
        return; // Too small to be useful
    }

    let mut solutions: Vec<serde_json::Value> = db
        .get_state("codegen_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    solutions.push(serde_json::json!({
        "code": code,
        "source": source,
        "ts": chrono::Utc::now().timestamp(),
    }));

    // Cap at 1000
    if solutions.len() > 1000 {
        solutions.drain(..solutions.len() - 1000);
    }

    if let Ok(json) = serde_json::to_string(&solutions) {
        let _ = db.set_state("codegen_solutions", &json);
        tracing::info!(
            total = solutions.len(),
            source,
            bytes = code.len(),
            "codegen: recorded training example"
        );
    }
}

/// Get status for observability — wired into /soul/status.
pub fn status(db: &SoulDatabase) -> serde_json::Value {
    let tok = load_tokenizer(db);
    let solutions_count: usize = db
        .get_state("codegen_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
        .map(|v| v.len())
        .unwrap_or(0);

    let model = load_model(db);

    let can_generate = !tok.merges.is_empty() && model.train_steps >= 10;

    serde_json::json!({
        "bpe_vocab_size": tok.current_vocab_size(),
        "bpe_merges": tok.merges.len(),
        "solutions_stored": solutions_count,
        "model_params": model.param_count(),
        "model_steps": model.train_steps,
        "model_loss": format!("{:.4}", model.running_loss),
        "can_generate": can_generate,
        "target_params": x402_model::codegen::CODEGEN_PARAMS,
        "target_vocab": x402_model::codegen::CODEGEN_VOCAB_SIZE,
    })
}
