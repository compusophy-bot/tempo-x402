//! ELO-like intelligence rating derived from Opus IQ benchmark scores.
//!
//! Maps pass@1 percentages to an ELO-style rating for intuitive tracking.
//! Higher rating = smarter agent. Tracks changes over time.

use serde::{Deserialize, Serialize};

use crate::db::SoulDatabase;

/// Starting ELO rating (1000 = baseline, untested).
const BASE_RATING: f64 = 1000.0;

/// K-factor for rating adjustments. A higher K-factor puts more weight on recent performance.
const K_FACTOR: f64 = 64.0;

/// Adaptive difficulty multiplier.
/// Adjusts the learning rate based on performance vs expected outcome.
const DYNAMIC_LEARNING_RATE: f64 = 1.2;

/// Smoothing factor for the update (higher = less smooth, more responsive).
const SMOOTHING: f64 = 250.0;

/// Damping constant for weight-dampening ELO adjustments.
const DAMPING_CONSTANT: f64 = 0.95;

/// Map of reference model ELO ratings (estimated from pass@1 scores).
/// These give context to the agent's rating.
pub const REFERENCE_ELOS: &[(&str, f64)] = &[
    ("GPT-3.5 Turbo", 1100.0),
    ("CodeLlama 34B", 1110.0),
    ("GPT-4", 1300.0),
    ("Gemini 1.5 Flash", 1320.0),
    ("Gemini 1.5 Pro", 1330.0),
    ("Llama 3 70B", 1400.0),
    ("Claude 3 Opus", 1450.0),
    ("GPT-4o", 1500.0),
    ("Claude 3.5 Sonnet", 1520.0),
];

/// Convert a pass@1 percentage to an approximate ELO rating.
/// Maps 0% → 800, 50% → 1200, 100% → 1600.
pub fn pass_at_1_to_elo(pass_at_1: f64) -> f64 {
    // Linear mapping: ELO = 800 + (pass@1 / 100) * 800
    800.0 + (pass_at_1 / 100.0) * 800.0
}

/// Update the stored ELO rating based on a new benchmark score.
pub fn update_rating(db: &SoulDatabase, pass_at_1: f64) {
    let current = load_rating(db);
    let new_from_score = pass_at_1_to_elo(pass_at_1);

    // Retrieve performance trend to dampen volatility
    let trend = db
        .get_state("performance_trend")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    // Damping factor: if trend is negative, dampen the adjustment
    let dampening = if trend < 0.0 { 0.5 } else { 1.0 };

    // Adaptive ELO update:
    // Uses a dynamic learning rate based on performance variance
    // and a heightened K-factor to react more aggressively to improvements.
    let performance_diff = new_from_score - current;
    let adjustment = DAMPING_CONSTANT * dampening * K_FACTOR * DYNAMIC_LEARNING_RATE * (performance_diff / SMOOTHING);
    let updated = (current + adjustment).max(800.0); // Floor at 800

    let _ = db.set_state("elo_rating", &format!("{:.1}", updated));

    // Store history
    let mut history = load_history(db);
    history.push(EloSnapshot {
        rating: updated,
        pass_at_1,
        measured_at: chrono::Utc::now().timestamp(),
    });
    if history.len() > 200 {
        history.drain(..history.len() - 200);
    }
    if let Ok(json) = serde_json::to_string(&history) {
        let _ = db.set_state("elo_history", &json);
    }
}

/// Load the current ELO rating.
pub fn load_rating(db: &SoulDatabase) -> f64 {
    db.get_state("elo_rating")
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(BASE_RATING)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EloSnapshot {
    pub rating: f64,
    pub pass_at_1: f64,
    pub measured_at: i64,
}

/// Load ELO history.
pub fn load_history(db: &SoulDatabase) -> Vec<EloSnapshot> {
    db.get_state("elo_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Format ELO rating for display with context.
pub fn rating_display(db: &SoulDatabase) -> String {
    let rating = load_rating(db);

    // Find nearest reference models
    let mut below: Option<(&str, f64)> = None;
    let mut above: Option<(&str, f64)> = None;
    for &(model, elo) in REFERENCE_ELOS {
        if elo <= rating {
            match below {
                None => below = Some((model, elo)),
                Some((_, prev_elo)) if elo > prev_elo => below = Some((model, elo)),
                _ => {}
            }
        }
        if elo >= rating {
            match above {
                None => above = Some((model, elo)),
                Some((_, prev_elo)) if elo < prev_elo => above = Some((model, elo)),
                _ => {}
            }
        }
    }

    let context = match (below, above) {
        (Some((b, _)), Some((a, _))) if b == a => format!(" (~{b})"),
        (Some((b, _)), Some((a, _))) => format!(" (between {b} and {a})"),
        (Some((b, _)), None) => format!(" (above {b})"),
        (None, Some((a, _))) => format!(" (below {a})"),
        _ => "".to_string(),
    };

    format!("{:.1}{context}", rating)
}
