//! Learning Acceleration: the second derivative of intelligence.
//!
//! α = "Is learning itself speeding up?"
//!
//! The system already tracks first derivatives (dF/dt, fitness trend, Ψ velocity).
//! This module computes SECOND derivatives — whether the rate of improvement is
//! itself increasing or decreasing.
//!
//! ```text
//! α = w_F     × (-d²F/dt²)              // Free energy accelerating down = good
//!   + w_elo   × d(ELO)/dt               // ELO climbing = good
//!   + w_brain × (-d(brain_loss)/dt)      // Loss decreasing faster = good
//!   + w_cg    × (-d(codegen_loss)/dt)    // Codegen improving = good
//!   + w_fit   × d²(fitness)/dt²          // Fitness accelerating up = good
//! ```
//!
//! α > 0 → ACCELERATING — learning is speeding up
//! α ≈ 0 → CRUISING — steady learning
//! α < 0 → DECELERATING — learning is slowing down
//!
//! Injected into agent prompts so it can optimize for meta-learning.

use crate::db::SoulDatabase;
use serde::{Deserialize, Serialize};

// Component weights — sum to 1.0
const W_FREE_ENERGY: f64 = 0.30;
const W_ELO: f64 = 0.30;
const W_BRAIN_LOSS: f64 = 0.15;
const W_CODEGEN_LOSS: f64 = 0.10;
const W_FITNESS: f64 = 0.15;

const HISTORY_SIZE: usize = 60;
const MIN_POINTS: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationComponent {
    pub signal: String,
    pub velocity: f64,     // first derivative
    pub acceleration: f64, // second derivative
    pub weight: f64,
    pub contribution: f64, // weight × normalized_acceleration
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccelerationRegime {
    Accelerating,
    Cruising,
    Decelerating,
    Stalled,
}

impl std::fmt::Display for AccelerationRegime {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AccelerationRegime::Accelerating => write!(f, "ACCELERATING"),
            AccelerationRegime::Cruising => write!(f, "CRUISING"),
            AccelerationRegime::Decelerating => write!(f, "DECELERATING"),
            AccelerationRegime::Stalled => write!(f, "STALLED"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningAcceleration {
    pub alpha: f64,
    pub regime: AccelerationRegime,
    pub components: Vec<AccelerationComponent>,
    pub timestamp: i64,
}

/// Measure learning acceleration from all available signals.
/// Called once per cycle after free_energy::measure().
pub fn measure(db: &SoulDatabase) -> LearningAcceleration {
    let now = chrono::Utc::now().timestamp();
    let mut components = Vec::new();

    // 1. Free Energy: want d²F/dt² to be negative (accelerating improvement)
    let fe_comp = compute_from_state(
        db,
        "free_energy_history",
        "free_energy",
        |json| {
            let arr: Vec<serde_json::Value> =
                serde_json::from_str(json).ok()?;
            Some(
                arr.iter()
                    .filter_map(|v| v.get("total").and_then(|t| t.as_f64()))
                    .collect(),
            )
        },
        W_FREE_ENERGY,
        true, // invert: decreasing F is good
    );
    components.push(fe_comp);

    // 2. ELO: want dELO/dt to be positive (climbing)
    let elo_comp = compute_from_state(
        db,
        "elo_history",
        "elo",
        |json| {
            let arr: Vec<serde_json::Value> =
                serde_json::from_str(json).ok()?;
            Some(
                arr.iter()
                    .filter_map(|v| v.get("rating").and_then(|r| r.as_f64()))
                    .collect(),
            )
        },
        W_ELO,
        false,
    );
    components.push(elo_comp);

    // 3. Brain loss: want d(loss)/dt to be negative (decreasing)
    let brain_comp = compute_from_state(
        db,
        "brain_loss_history",
        "brain_loss",
        |json| serde_json::from_str::<Vec<(i64, f64)>>(json)
            .ok()
            .map(|v| v.into_iter().map(|(_, loss)| loss).collect()),
        W_BRAIN_LOSS,
        true, // invert: decreasing loss is good
    );
    components.push(brain_comp);

    // 4. Codegen loss: want d(loss)/dt to be negative
    let cg_comp = compute_from_state(
        db,
        "codegen_loss_history",
        "codegen_loss",
        |json| serde_json::from_str::<Vec<(i64, f64)>>(json)
            .ok()
            .map(|v| v.into_iter().map(|(_, loss)| loss).collect()),
        W_CODEGEN_LOSS,
        true,
    );
    components.push(cg_comp);

    // 5. Fitness: want d²fitness/dt² to be positive (accelerating improvement)
    let fit_comp = compute_from_state(
        db,
        "fitness_history",
        "fitness",
        |json| {
            let arr: Vec<serde_json::Value> =
                serde_json::from_str(json).ok()?;
            Some(
                arr.iter()
                    .filter_map(|v| v.get("total").and_then(|t| t.as_f64()))
                    .collect(),
            )
        },
        W_FITNESS,
        false,
    );
    components.push(fit_comp);

    // Compute alpha
    let alpha: f64 = components.iter().map(|c| c.contribution).sum();

    let regime = if alpha > 0.005 {
        AccelerationRegime::Accelerating
    } else if alpha < -0.005 {
        AccelerationRegime::Decelerating
    } else {
        // Check if velocity is also ~0
        let total_velocity: f64 = components.iter().map(|c| c.velocity.abs()).sum();
        if total_velocity < 0.01 {
            AccelerationRegime::Stalled
        } else {
            AccelerationRegime::Cruising
        }
    };

    let accel = LearningAcceleration {
        alpha,
        regime,
        components,
        timestamp: now,
    };

    // Persist
    if let Ok(json) = serde_json::to_string(&accel) {
        let _ = db.set_state("acceleration_current", &json);
    }

    // Append to history
    let mut history: Vec<(i64, f64)> = db
        .get_state("acceleration_history")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    history.push((now, alpha));
    if history.len() > HISTORY_SIZE {
        history.drain(..history.len() - HISTORY_SIZE);
    }
    if let Ok(json) = serde_json::to_string(&history) {
        let _ = db.set_state("acceleration_history", &json);
    }

    accel
}

/// Load current acceleration without recomputation.
pub fn load_current(db: &SoulDatabase) -> Option<LearningAcceleration> {
    db.get_state("acceleration_current")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Status for /soul/status endpoint.
pub fn status(db: &SoulDatabase) -> serde_json::Value {
    match load_current(db) {
        Some(a) => serde_json::json!({
            "alpha": format!("{:.6}", a.alpha),
            "regime": a.regime.to_string(),
            "components": a.components.iter().map(|c| serde_json::json!({
                "signal": c.signal,
                "velocity": format!("{:.6}", c.velocity),
                "acceleration": format!("{:.6}", c.acceleration),
                "contribution": format!("{:.6}", c.contribution),
            })).collect::<Vec<_>>(),
            "timestamp": a.timestamp,
        }),
        None => serde_json::json!({
            "alpha": "0.000000",
            "regime": "STALLED",
            "components": [],
            "timestamp": 0,
        }),
    }
}

/// Prompt section for injection into goal creation and planning prompts.
pub fn prompt_section(db: &SoulDatabase) -> String {
    let accel = match load_current(db) {
        Some(a) => a,
        None => return String::new(),
    };

    let direction = match accel.regime {
        AccelerationRegime::Accelerating =>
            "Your learning is SPEEDING UP. Whatever you're doing, keep doing it. Maximize α.",
        AccelerationRegime::Cruising =>
            "Your learning is STEADY. Try something new to break through the plateau.",
        AccelerationRegime::Decelerating =>
            "WARNING: Your learning is SLOWING DOWN. Change strategy. Try harder problems, different approaches, or consolidate what you know.",
        AccelerationRegime::Stalled =>
            "CRITICAL: Learning has STALLED. Velocity and acceleration are both near zero. You need a fundamentally different approach.",
    };

    let mut lines = vec![
        format!(
            "# Learning Acceleration: \u{03B1} = {:+.4} [{}]",
            accel.alpha, accel.regime
        ),
        direction.to_string(),
    ];

    for c in &accel.components {
        let arrow = if c.contribution > 0.001 {
            "\u{2191}"
        } else if c.contribution < -0.001 {
            "\u{2193}"
        } else {
            "\u{2192}"
        };
        lines.push(format!(
            "  {}: velocity={:+.4} accel={:+.4} {}",
            c.signal, c.velocity, c.acceleration, arrow
        ));
    }

    lines.push(
        "\u{03B1} > 0 \u{2192} Optimize for the RATE of learning, not just learning itself."
            .to_string(),
    );

    lines.join("\n")
}

// ── Internal ────────────────────────────────────────────────────────

/// Compute velocity and acceleration from a soul_state history key.
fn compute_from_state(
    db: &SoulDatabase,
    state_key: &str,
    signal_name: &str,
    parser: impl Fn(&str) -> Option<Vec<f64>>,
    weight: f64,
    invert: bool,
) -> AccelerationComponent {
    let values = db
        .get_state(state_key)
        .ok()
        .flatten()
        .and_then(|s| parser(&s))
        .unwrap_or_default();

    let (velocity, acceleration) = if values.len() >= MIN_POINTS {
        let vel = linear_regression_slope(&values);
        // For acceleration: compute slopes over sliding windows, then slope of slopes
        let accel = compute_acceleration(&values);
        if invert {
            (-vel, -accel)
        } else {
            (vel, accel)
        }
    } else {
        (0.0, 0.0)
    };

    let contribution = weight * acceleration.clamp(-1.0, 1.0);

    AccelerationComponent {
        signal: signal_name.to_string(),
        velocity,
        acceleration,
        weight,
        contribution,
    }
}

/// Linear regression slope over a sequence of values (evenly spaced).
fn linear_regression_slope(values: &[f64]) -> f64 {
    let n = values.len();
    if n < 3 {
        return 0.0;
    }
    let len = n as f64;
    let x_mean = (len - 1.0) / 2.0;
    let y_mean: f64 = values.iter().sum::<f64>() / len;

    let mut num = 0.0;
    let mut den = 0.0;
    for (i, y) in values.iter().enumerate() {
        let x = i as f64;
        num += (x - x_mean) * (y - y_mean);
        den += (x - x_mean) * (x - x_mean);
    }

    if den.abs() < 1e-10 {
        return 0.0;
    }
    let slope = num / den;
    if slope.is_finite() { slope } else { 0.0 }
}

/// Compute acceleration as the slope of windowed velocities.
/// Split the sequence into overlapping windows, compute velocity per window,
/// then compute the slope of those velocities.
fn compute_acceleration(values: &[f64]) -> f64 {
    let n = values.len();
    if n < 8 {
        return 0.0;
    }

    let window = n / 3; // ~3 windows
    if window < 3 {
        return 0.0;
    }

    let mut velocities = Vec::new();
    for start in (0..n - window).step_by(window / 2) {
        let end = (start + window).min(n);
        let slice = &values[start..end];
        velocities.push(linear_regression_slope(slice));
    }

    if velocities.len() < 2 {
        return 0.0;
    }

    linear_regression_slope(&velocities)
}
