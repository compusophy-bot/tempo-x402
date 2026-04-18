//! Unified model training — one model, all cognitive tasks.
//!
//! Trains the unified encoder-decoder on data from all sources:
//! - Brain: step outcomes → fast head (predict success/error)
//! - Quality: diff features → fast head (predict quality score)
//! - Codegen: (test code, solution) pairs → slow head (generate code)
//! - Plan: (goal, step sequence) pairs → slow head (generate plans)
//!
//! The shared encoder learns from ALL tasks simultaneously.

use crate::db::SoulDatabase;

const MAX_TRAIN_SECS: u64 = 30;

/// Load the unified model from file or sled.
fn load_model(db: &SoulDatabase) -> x402_model::unified::UnifiedModel {
    // Try file first
    let path = std::path::Path::new("/tmp/unified_model.json");
    if path.exists() {
        if let Ok(json) = std::fs::read_to_string(path) {
            if let Some(model) = x402_model::unified::UnifiedModel::from_json(&json) {
                return model;
            }
        }
    }
    // Fall back to sled
    match db.get_state("unified_model_meta").ok().flatten() {
        Some(meta) if meta.contains("steps") => {
            // Model exists but stored in file — if file missing, reinit
            x402_model::unified::UnifiedModel::new()
        }
        _ => x402_model::unified::UnifiedModel::new(),
    }
}

/// Save the unified model to file + lightweight marker in sled.
fn save_model(db: &SoulDatabase, model: &x402_model::unified::UnifiedModel) {
    let json = model.to_json();
    let path = std::path::Path::new("/tmp/unified_model.json");
    if let Err(e) = std::fs::write(path, &json) {
        tracing::warn!(error = %e, "Failed to save unified model");
    }
    let marker = format!(
        r#"{{"steps":{},"loss":{:.4},"params":{}}}"#,
        model.train_steps,
        model.running_loss,
        model.param_count(),
    );
    let _ = db.set_state("unified_model_meta", &marker);
}

/// Train the unified model on all available cognitive data.
/// Called from the thinking loop's background training block.
pub fn train_cycle(db: &SoulDatabase) {
    let start = std::time::Instant::now();
    let mut model = load_model(db);
    let tok = crate::codegen::load_tokenizer(db);
    if tok.merges.is_empty() {
        return; // BPE not ready
    }

    let mut total_loss = 0.0f32;
    let mut trained = 0u32;
    let mut fast_trained = 0u32;
    let mut slow_trained = 0u32;

    // ── FAST HEAD: Brain prediction data ──
    // Collect recent step outcomes and train the fast head to predict success
    let plan_outcomes = db.get_recent_plan_outcomes(20).unwrap_or_default();
    for outcome in plan_outcomes.iter().take(5) {
        if start.elapsed().as_secs() >= MAX_TRAIN_SECS {
            break;
        }

        // Encode the outcome as a text prompt for the shared encoder
        let input_text = format!(
            "[PREDICT] goal={} status={} steps={} replan={}",
            &outcome
                .goal_description
                .chars()
                .take(100)
                .collect::<String>(),
            outcome.status,
            outcome.steps_completed,
            outcome.replan_count,
        );

        let mut tokens = vec![x402_model::unified::TASK_PREDICT];
        tokens.extend(tok.encode(&input_text));
        if tokens.len() > 128 {
            tokens.truncate(128);
        }
        if tokens.len() < 3 {
            continue;
        }

        // Target: [success_prob, 11 error cats (one-hot), 11 capabilities, quality]
        let mut targets = vec![0.0f32; x402_model::unified::FAST_OUTPUT];
        targets[0] = if outcome.status == "completed" {
            1.0
        } else {
            0.0
        };
        // Error category (if failed)
        if let Some(ref cat) = outcome.error_category {
            let cat_idx = match cat.as_str() {
                "compilation" => 1,
                "test_failure" => 2,
                "runtime_error" => 3,
                "timeout" => 4,
                "protected_file" => 5,
                "empty_output" => 6,
                "api_error" => 7,
                _ => 8,
            };
            if cat_idx < 12 {
                targets[cat_idx] = 1.0;
            }
        }

        let lr = learning_rate(model.train_steps);
        let loss = model.train_fast(&tokens, &targets, lr);
        total_loss += loss;
        trained += 1;
        fast_trained += 1;
    }

    // ── SLOW HEAD: Codegen data (test → solution pairs) ──
    let solutions: Vec<serde_json::Value> = db
        .get_state("codegen_solutions")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let offset = (model.train_steps as usize) % solutions.len().max(1);
    for sol in solutions.iter().cycle().skip(offset).take(3) {
        if start.elapsed().as_secs() >= MAX_TRAIN_SECS {
            break;
        }

        let Some(code) = sol.get("code").and_then(|v| v.as_str()) else {
            continue;
        };
        if code.len() < 50 {
            continue;
        }

        // Context: test code (if available) with [CODE] prefix
        let context_text = if let Some(ctx) = sol.get("context").and_then(|v| v.as_str()) {
            format!("[CODE] {}", &ctx.chars().take(1000).collect::<String>())
        } else {
            format!("[CODE] {}", &code.chars().take(200).collect::<String>())
        };

        let mut context_tokens = vec![x402_model::unified::TASK_CODE];
        context_tokens.extend(tok.encode(&context_text));
        if context_tokens.len() > x402_model::unified::MAX_SEQ {
            context_tokens.truncate(x402_model::unified::MAX_SEQ);
        }

        let mut target_tokens = vec![x402_model::bpe::BOS_TOKEN];
        target_tokens.extend(tok.encode(code));
        target_tokens.push(x402_model::bpe::EOS_TOKEN);
        if target_tokens.len() > 64 {
            target_tokens.truncate(64);
        }
        if target_tokens.len() < 3 {
            continue;
        }

        let lr = learning_rate(model.train_steps);
        let loss = model.train_slow(&context_tokens, &target_tokens, lr);
        total_loss += loss;
        trained += 1;
        slow_trained += 1;
    }

    if trained > 0 {
        save_model(db, &model);
        tracing::info!(
            trained,
            fast = fast_trained,
            slow = slow_trained,
            loss = format!("{:.4}", total_loss / trained as f32),
            running_loss = format!("{:.4}", model.running_loss),
            steps = model.train_steps,
            params = model.param_count(),
            elapsed_secs = start.elapsed().as_secs(),
            "Unified model trained (all tasks)"
        );

        // Store loss for status API
        let _ = db.set_state("unified_model_loss", &format!("{:.4}", model.running_loss));
        let _ = db.set_state("unified_model_steps", &model.train_steps.to_string());
    }
}

/// Learning rate with warmup and cosine decay.
fn learning_rate(step: u64) -> f32 {
    let s = step as f32;
    if s < 200.0 {
        0.0005 + (0.002 - 0.0005) * (s / 200.0)
    } else {
        let decay = ((s - 200.0) * std::f32::consts::PI / 10000.0).cos();
        0.0005 + (0.002 - 0.0005) * 0.5 * (1.0 + decay)
    }
}

/// Status for /soul/status API.
pub fn status(db: &SoulDatabase) -> serde_json::Value {
    let loss = db
        .get_state("unified_model_loss")
        .ok()
        .flatten()
        .unwrap_or_else(|| "N/A".to_string());
    let steps = db
        .get_state("unified_model_steps")
        .ok()
        .flatten()
        .unwrap_or_else(|| "0".to_string());
    serde_json::json!({
        "loss": loss,
        "steps": steps,
        "params": x402_model::unified::UnifiedModel::new().param_count(),
        "architecture": "shared encoder (3 layers D=384) + fast head + slow decoder",
    })
}
