//! Integration layer between tempo-x402-model (transformer) and the soul.
//!
//! Handles: loading/saving model to soul_state, training from plan outcomes,
//! generating plans, and exposing model status for observability.

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;

/// Model status for the API/dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub param_count: usize,
    pub train_steps: u64,
    pub running_loss: f32,
    pub vocab_size: usize,
    pub templates_trained_on: u64,
    pub plans_generated: u64,
    pub last_train_loss: f32,
}

/// Load the plan transformer from soul_state.
pub fn load_model(db: &SoulDatabase) -> x402_model::PlanTransformer {
    match db.get_state("plan_transformer").ok().flatten() {
        Some(json) if !json.is_empty() => {
            x402_model::PlanTransformer::from_json(&json).unwrap_or_default()
        }
        _ => x402_model::PlanTransformer::new(),
    }
}

/// Save the plan transformer to soul_state.
pub fn save_model(db: &SoulDatabase, model: &x402_model::PlanTransformer) {
    let json = model.to_json();
    if let Err(e) = db.set_state("plan_transformer", &json) {
        tracing::warn!(error = %e, "Failed to save plan transformer");
    }
}

/// Load the vocabulary from soul_state.
pub fn load_vocab(db: &SoulDatabase) -> x402_model::Vocab {
    match db.get_state("plan_transformer_vocab").ok().flatten() {
        Some(json) if !json.is_empty() => serde_json::from_str(&json).unwrap_or_default(),
        _ => x402_model::Vocab::new(),
    }
}

/// Save the vocabulary to soul_state.
pub fn save_vocab(db: &SoulDatabase, vocab: &x402_model::Vocab) {
    if let Ok(json) = serde_json::to_string(vocab) {
        let _ = db.set_state("plan_transformer_vocab", &json);
    }
}

/// Train the model on recent successful plan outcomes.
/// Called every N cycles from the thinking loop.
/// Returns (examples_trained, loss).
pub fn train_from_outcomes(db: &SoulDatabase) -> (usize, f32) {
    let outcomes = match db.get_recent_plan_outcomes(20) {
        Ok(o) => o,
        Err(_) => return (0, 0.0),
    };

    let mut model = load_model(db);
    let mut vocab = load_vocab(db);

    let mut examples: Vec<x402_model::TrainingExample> = Vec::new();

    for outcome in &outcomes {
        // Only train on completed (not trivial) plans
        if outcome.status != "completed" {
            continue;
        }

        // Convert step summaries to tokens
        let step_tokens: Vec<u32> = outcome
            .steps_succeeded
            .iter()
            .map(|s| {
                // Extract step type from summary (e.g., "read foo.rs" → "read_file")
                let step_type = summary_to_step_type(s);
                x402_model::Vocab::step_to_token(&step_type)
            })
            .filter(|&t| t != x402_model::vocab::UNK)
            .collect();

        if step_tokens.is_empty() {
            continue;
        }

        // Extract goal keywords as context tokens
        let keywords = crate::genesis::extract_keywords_pub(&outcome.goal_description);
        let context_tokens = vocab.tokenize_context(&keywords);

        examples.push(x402_model::TrainingExample {
            context: context_tokens,
            steps: step_tokens,
            weight: 1.0, // Could weight by plan fitness
            source: "local".to_string(),
        });
    }

    if examples.is_empty() {
        return (0, 0.0);
    }

    let (trained, loss) = x402_model::train_batch(&mut model, &examples);

    if trained > 0 {
        save_model(db, &model);
        save_vocab(db, &vocab);

        // Track training stats
        let total_trained: u64 = db
            .get_state("model_templates_trained")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let _ = db.set_state(
            "model_templates_trained",
            &(total_trained + trained as u64).to_string(),
        );
        let _ = db.set_state("model_last_train_loss", &format!("{:.4}", loss));

        tracing::info!(
            trained,
            loss = format!("{:.4}", loss),
            total_steps = model.train_steps,
            running_loss = format!("{:.4}", model.running_loss),
            "Plan transformer training cycle"
        );
    }

    (trained, loss)
}

/// Generate a plan using the transformer model.
/// Returns step type names if the model has been trained, None if untrained.
pub fn generate_plan(db: &SoulDatabase, goal_description: &str) -> Option<Vec<String>> {
    let model = load_model(db);

    // Don't use untrained model
    if model.train_steps < 50 {
        return None;
    }

    let mut vocab = load_vocab(db);
    let keywords = crate::genesis::extract_keywords_pub(goal_description);
    let context = vocab.tokenize_context(&keywords);
    save_vocab(db, &vocab);

    let plan = x402_model::inference::generate_best_plan(&model, &context, 5, 10)?;

    if plan.steps.is_empty() {
        return None;
    }

    // Track generation count
    let generated: u64 = db
        .get_state("model_plans_generated")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let _ = db.set_state("model_plans_generated", &(generated + 1).to_string());

    tracing::info!(
        steps = ?plan.steps,
        confidence = format!("{:.3}", plan.confidence),
        "Plan transformer generated plan"
    );

    Some(plan.steps)
}

/// Get model status for observability.
pub fn status(db: &SoulDatabase) -> ModelStatus {
    let model = load_model(db);

    let templates_trained_on: u64 = db
        .get_state("model_templates_trained")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let plans_generated: u64 = db
        .get_state("model_plans_generated")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let last_train_loss: f32 = db
        .get_state("model_last_train_loss")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    ModelStatus {
        param_count: model.param_count(),
        train_steps: model.train_steps,
        running_loss: model.running_loss,
        vocab_size: x402_model::vocab::VOCAB_SIZE,
        templates_trained_on,
        plans_generated,
        last_train_loss,
    }
}

/// Convert a step summary (e.g., "read foo.rs", "edit crates/...") to a step type name.
fn summary_to_step_type(summary: &str) -> String {
    let lower = summary.to_lowercase();
    if lower.starts_with("read ") {
        "read_file".to_string()
    } else if lower.starts_with("search ") {
        "search_code".to_string()
    } else if lower.starts_with("ls ") {
        "list_dir".to_string()
    } else if lower.starts_with("shell:") || lower.starts_with("shell ") {
        "run_shell".to_string()
    } else if lower.starts_with("commit") {
        "commit".to_string()
    } else if lower.starts_with("cargo check") {
        "cargo_check".to_string()
    } else if lower.starts_with("generate ") {
        "generate_code".to_string()
    } else if lower.starts_with("edit ") {
        "edit_code".to_string()
    } else if lower.starts_with("think") {
        "think".to_string()
    } else if lower.starts_with("discover") {
        "discover_peers".to_string()
    } else if lower.starts_with("create /x/") {
        "create_script_endpoint".to_string()
    } else if lower.starts_with("test /x/") {
        "test_script_endpoint".to_string()
    } else if lower.starts_with("check /") || lower.starts_with("check self") {
        "check_self".to_string()
    } else if lower.contains("peer/") || lower.contains("(paid)") {
        "call_peer".to_string()
    } else if lower.starts_with("clone") {
        "clone_self".to_string()
    } else if lower.starts_with("review pr") {
        "review_peer_pr".to_string()
    } else {
        "run_shell".to_string()
    }
}
