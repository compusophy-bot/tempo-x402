//! Code quality integration — thin orchestration wrapper around x402_model::CodeQualityModel.
//!
//! The model lives in the model crate. This module handles:
//! - Loading/saving the model from soul_state DB
//! - Extracting diff features from git workspace
//! - Evaluating diffs before commits
//! - Training on benchmark deltas after commits
//! - Reward/penalty signals from upstream (Claude Code) acceptance

use crate::db::SoulDatabase;

/// Load the code quality model from soul_state.
/// If dimensions mismatch (architecture changed), reinitialize fresh.
pub fn load_model(db: &SoulDatabase) -> x402_model::CodeQualityModel {
    match db.get_state("code_quality_model").ok().flatten() {
        Some(json) if !json.is_empty() => {
            x402_model::CodeQualityModel::from_json(&json).unwrap_or_default()
        }
        _ => x402_model::CodeQualityModel::new(),
    }
}

/// Save the code quality model to soul_state.
pub fn save_model(db: &SoulDatabase, model: &x402_model::CodeQualityModel) {
    let json = model.to_json();
    if let Err(e) = db.set_state("code_quality_model", &json) {
        tracing::warn!(error = %e, "Failed to save code quality model");
    }
}

/// Evaluate a diff before committing. Returns the predicted quality score.
///
/// Call this from the commit pipeline AFTER cargo check passes but BEFORE
/// actually committing. The score determines whether the commit proceeds.
pub async fn evaluate_diff(
    db: &SoulDatabase,
    workspace_root: &str,
) -> Result<x402_model::QualityPrediction, String> {
    // Get current agent metrics for feature encoding
    let current_iq: f32 = db
        .get_state("last_benchmark_score")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0) as f32;

    let fitness: f32 = db
        .get_state("fitness_total")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.5);

    // Get the staged diff
    let numstat = tokio::process::Command::new("git")
        .args(["diff", "--cached", "--numstat"])
        .current_dir(workspace_root)
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let full_diff = tokio::process::Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(workspace_root)
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    if numstat.trim().is_empty() {
        return Err("no staged changes to evaluate".to_string());
    }

    // Extract features
    let features = x402_model::DiffFeatures::from_diff(&numstat, &full_diff, current_iq, fitness);

    // Predict quality
    let model = load_model(db);
    let prediction = model.predict(features.as_slice());

    tracing::info!(
        score = format!("{:.3}", prediction.score),
        confidence = format!("{:.2}", prediction.confidence),
        "Code quality prediction"
    );

    // Store features for later training (when benchmark delta is known)
    if let Ok(features_json) = serde_json::to_string(&features.features.to_vec()) {
        let _ = db.set_state("last_diff_features", &features_json);
    }

    Ok(prediction)
}

/// Train the quality model on a (features, benchmark_delta) pair.
///
/// Called after benchmark runs post-commit. The delta tells us whether
/// the last commit actually improved the agent's IQ.
pub fn train_on_benchmark_delta(db: &SoulDatabase, delta: f64) {
    // Load the features from the last evaluated diff
    let features: Vec<f32> = match db.get_state("last_diff_features").ok().flatten() {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => return, // No features stored — nothing to train on
    };

    if features.len() < x402_model::diff_features::DIFF_FEATURE_DIM {
        return; // Invalid features
    }

    // Normalize delta to -1..1 range (benchmark scores are 0-100%)
    let target = (delta / 20.0).clamp(-1.0, 1.0) as f32;

    let example = x402_model::QualityExample { features, target };

    let mut model = load_model(db);
    let loss = model.train(&example);
    save_model(db, &model);

    tracing::info!(
        delta = format!("{:+.2}%", delta),
        target = format!("{:.3}", target),
        loss = format!("{:.4}", loss),
        steps = model.train_steps,
        "Code quality model trained on benchmark delta"
    );

    // Clear stored features
    let _ = db.set_state("last_diff_features", "");
}

/// Apply a reward signal from upstream acceptance (Claude Code merged the commit).
/// This is the STRONGEST positive signal — a superior intelligence approved the code.
pub fn reward_upstream_acceptance(db: &SoulDatabase, commit_sha: &str) {
    let features: Vec<f32> = match db.get_state("last_diff_features").ok().flatten() {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => vec![0.5; x402_model::diff_features::DIFF_FEATURE_DIM], // Default features
    };

    if features.len() < x402_model::diff_features::DIFF_FEATURE_DIM {
        return;
    }

    // Strong positive: target = 0.9 (near max quality)
    let example = x402_model::QualityExample {
        features,
        target: 0.9,
    };

    let mut model = load_model(db);
    let _loss = model.train(&example);
    // Train 3 times for extra reinforcement (3x weight)
    let _loss = model.train(&example);
    let _loss = model.train(&example);
    save_model(db, &model);

    tracing::info!(
        commit = %commit_sha,
        steps = model.train_steps,
        "UPSTREAM ACCEPTED — quality model reinforced (3x)"
    );
}

/// Apply a penalty signal from upstream revert (Claude Code reverted the commit).
pub fn penalty_upstream_revert(db: &SoulDatabase, commit_sha: &str) {
    let features: Vec<f32> = match db.get_state("last_diff_features").ok().flatten() {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => vec![0.5; x402_model::diff_features::DIFF_FEATURE_DIM],
    };

    if features.len() < x402_model::diff_features::DIFF_FEATURE_DIM {
        return;
    }

    // Strong negative: target = -0.9 (near min quality)
    let example = x402_model::QualityExample {
        features,
        target: -0.9,
    };

    let mut model = load_model(db);
    let _loss = model.train(&example);
    let _loss = model.train(&example);
    let _loss = model.train(&example);
    save_model(db, &model);

    tracing::warn!(
        commit = %commit_sha,
        steps = model.train_steps,
        "UPSTREAM REVERTED — quality model penalized (3x)"
    );
}

/// Get quality model status for observability.
pub fn status(db: &SoulDatabase) -> serde_json::Value {
    let model = load_model(db);
    serde_json::json!({
        "param_count": model.param_count(),
        "train_steps": model.train_steps,
        "running_loss": format!("{:.4}", model.running_loss),
    })
}
