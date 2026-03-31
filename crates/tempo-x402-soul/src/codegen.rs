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

/// Train the BPE tokenizer on accumulated code (benchmark solutions + commit diffs).
pub fn train_tokenizer(db: &SoulDatabase) {
    let solutions: Vec<serde_json::Value> = db
        .get_state("codegen_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    if solutions.is_empty() {
        tracing::debug!("codegen: BPE skip — 0 training examples (need benchmark passes or commits)");
        return;
    }

    let mut corpus = String::new();
    for sol in &solutions {
        if let Some(code) = sol.get("code").and_then(|v| v.as_str()) {
            corpus.push_str(code);
            corpus.push('\n');
        }
    }

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
        corpus_bytes = corpus.len(),
        vocab_before = before_vocab,
        vocab_after = after_vocab,
        compression_ratio = format!("{ratio:.2}"),
        "codegen: BPE tokenizer trained"
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

/// Train the code generation model on accumulated solutions.
/// Minimum 2 solutions (was 5). Every training step counts when bootstrapping.
pub fn train_model(db: &SoulDatabase) {
    let tok = load_tokenizer(db);
    if tok.merges.is_empty() {
        tracing::debug!("codegen: model skip — BPE not trained yet (0 merges)");
        return;
    }

    let solutions: Vec<serde_json::Value> = db
        .get_state("codegen_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    if solutions.len() < 2 {
        tracing::debug!(
            solutions = solutions.len(),
            "codegen: model skip — need >=2 training examples"
        );
        return;
    }

    let mut model = load_model(db);
    let mut total_loss = 0.0f32;
    let mut trained = 0u32;

    // Train on up to 20 recent solutions per cycle
    for sol in solutions.iter().rev().take(20) {
        let Some(code) = sol.get("code").and_then(|v| v.as_str()) else {
            continue;
        };

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

        // Train on sliding windows
        let window_size = 64.min(tokens.len());
        for start in (0..tokens.len().saturating_sub(window_size)).step_by(32) {
            let end = (start + window_size).min(tokens.len());
            let window = &tokens[start..end];
            let loss = model.train_step(window, 0.001);
            total_loss += loss;
            trained += 1;
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
            "codegen: model training cycle complete"
        );
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

        // Argmax
        let next_token = logits
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx as u32)
            .unwrap_or(x402_model::bpe::EOS_TOKEN);

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
