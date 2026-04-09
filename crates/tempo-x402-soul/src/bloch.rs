//! Bloch Sphere Cognitive State — continuous cognitive geometry.
//!
//! Replaces 13 discrete states (5 CognitiveState + 4 EnergyRegime + 4 Drive)
//! with one continuous point on the unit sphere S².
//!
//! ## Parameterization
//!
//! ```text
//! θ ∈ [0, π]:    exploit ↔ explore
//!   θ = 0:       pure exploitation (high confidence, repeat what works)
//!   θ = π/2:     balanced (learning)
//!   θ = π:       pure exploration (try everything new)
//!
//! φ ∈ [0, 2π]:   self-focus ↔ colony-focus
//!   φ = 0:       self-improvement
//!   φ = π:       peer collaboration
//! ```
//!
//! ## Evolution
//!
//! The state evolves via gradient descent on the sphere:
//! - Free Energy F(t) drives θ: decreasing F → exploit, increasing F → explore
//! - Colony consciousness Ψ(t) drives φ: increasing Ψ → colony, decreasing Ψ → self
//!
//! ## Cartesian Representation
//!
//! For interpolation and peer sync, the Bloch state can be expressed as a 3D vector:
//! ```text
//! x = sin(θ) cos(φ)   — exploration × self-focus
//! y = sin(θ) sin(φ)   — exploration × colony-focus
//! z = cos(θ)          — exploitation axis
//! ```

use crate::db::SoulDatabase;

/// The cognitive state of an agent, represented as a point on the Bloch sphere.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlochState {
    /// Polar angle: exploit (0) ↔ explore (π)
    pub theta: f64,
    /// Azimuthal angle: self-focus (0) ↔ colony-focus (π) ↔ self (2π)
    pub phi: f64,
    /// Angular velocity (momentum for smoother trajectories)
    pub d_theta: f64,
    pub d_phi: f64,
}

impl Default for BlochState {
    fn default() -> Self {
        Self {
            theta: std::f64::consts::FRAC_PI_2, // balanced (learning)
            phi: 0.0,                           // self-focused initially
            d_theta: 0.0,
            d_phi: 0.0,
        }
    }
}

impl BlochState {
    /// Create a new state at the balanced/self-focused position.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cartesian representation for interpolation and peer sync.
    pub fn to_cartesian(&self) -> (f64, f64, f64) {
        let x = self.theta.sin() * self.phi.cos();
        let y = self.theta.sin() * self.phi.sin();
        let z = self.theta.cos();
        (x, y, z)
    }

    /// Reconstruct from Cartesian (normalize to unit sphere).
    pub fn from_cartesian(x: f64, y: f64, z: f64) -> Self {
        let r = (x * x + y * y + z * z).sqrt().max(1e-10);
        let xn = x / r;
        let yn = y / r;
        let zn = z / r;
        Self {
            theta: zn.clamp(-1.0, 1.0).acos(),
            phi: yn.atan2(xn).rem_euclid(std::f64::consts::TAU),
            d_theta: 0.0,
            d_phi: 0.0,
        }
    }

    /// How much the agent should exploit vs explore (0.0 = exploit, 1.0 = explore).
    pub fn exploration_factor(&self) -> f64 {
        self.theta / std::f64::consts::PI
    }

    /// How much the agent should focus on colony vs self (0.0 = self, 1.0 = colony).
    pub fn colony_factor(&self) -> f64 {
        // Map phi to [0, 1] where phi=π is maximum colony focus
        (self.phi.sin().abs()).min(1.0)
    }

    /// Legacy compatibility: map to the closest discrete regime.
    pub fn regime(&self) -> &'static str {
        let explore = self.exploration_factor();
        if explore > 0.7 {
            "EXPLORE"
        } else if explore < 0.3 {
            "EXPLOIT"
        } else {
            "LEARN"
        }
    }

    /// Legacy compatibility: map to closest drive.
    pub fn drive(&self) -> &'static str {
        let explore = self.exploration_factor();
        let colony = self.colony_factor();
        if explore > 0.6 {
            "explore"
        } else if explore < 0.3 {
            "exploit"
        } else if colony > 0.5 {
            "collaborate"
        } else {
            "neutral"
        }
    }

    /// Evolve the state based on Free Energy and Colony Consciousness.
    ///
    /// - `f_trend`: dF/dt — positive = surprise increasing, negative = learning
    /// - `psi_trend`: dΨ/dt — positive = colony getting smarter, negative = diverging
    /// - `dt`: time step (typically 1.0 per cycle)
    pub fn evolve(&mut self, f_trend: f64, psi_trend: f64, dt: f64) {
        // Damping coefficient (prevents oscillation)
        let damping = 0.8;
        // Learning rates for each axis
        let alpha = 0.3; // sensitivity to F trend
        let beta = 0.2; // sensitivity to Ψ trend

        // Free Energy drives θ:
        // Increasing F (more surprise) → rotate toward explore (increase θ)
        // Decreasing F (less surprise) → rotate toward exploit (decrease θ)
        let f_force = alpha * f_trend;

        // Ψ drives φ:
        // Increasing Ψ → rotate toward colony (increase φ toward π)
        // Decreasing Ψ → rotate toward self (decrease φ toward 0)
        let target_phi = if psi_trend > 0.0 {
            std::f64::consts::PI // colony focus
        } else {
            0.0 // self focus
        };
        let phi_error = target_phi - self.phi;
        let psi_force = beta * psi_trend.abs() * phi_error.signum();

        // Update velocities with damping (momentum-based smooth transitions)
        self.d_theta = damping * self.d_theta + f_force * dt;
        self.d_phi = damping * self.d_phi + psi_force * dt;

        // Update positions
        self.theta += self.d_theta * dt;
        self.phi += self.d_phi * dt;

        // Clamp θ to [0, π]
        self.theta = self.theta.clamp(0.01, std::f64::consts::PI - 0.01);
        // Wrap φ to [0, 2π)
        self.phi = self.phi.rem_euclid(std::f64::consts::TAU);
    }

    /// Temporal oscillator modulation factor.
    /// Returns a multiplier for oscillator effective period based on
    /// proximity of the operation to the current cognitive state.
    ///
    /// Operations aligned with the current state fire more frequently.
    pub fn oscillator_modulation(&self, operation: &str) -> f64 {
        let explore = self.exploration_factor();
        let colony = self.colony_factor();

        match operation {
            // Exploration-aligned operations fire more when exploring
            "cortex_dreaming" | "genesis_evolution" => {
                1.0 + explore * 1.5 // up to 2.5x faster when exploring
            }
            // Exploitation-aligned operations fire more when exploiting
            "benchmark" | "brain_training" => {
                1.0 + (1.0 - explore) * 1.5 // up to 2.5x faster when exploiting
            }
            // Colony-aligned operations fire more when colony-focused
            "peer_sync" => {
                1.0 + colony * 2.0 // up to 3.0x faster when colony-focused
            }
            // Self-repair accelerates when stuck (high explore + low progress)
            "self_repair" => {
                if explore > 0.7 {
                    2.0
                } else {
                    1.0
                }
            }
            _ => 1.0,
        }
    }

    /// Merge with a peer's Bloch state (entanglement analog).
    /// The merge weight is based on the peer's fitness relative to self.
    pub fn merge_peer(&mut self, peer: &BlochState, peer_fitness: f64, self_fitness: f64) {
        let weight = (peer_fitness / self_fitness.max(0.01)).clamp(0.1, 2.0);
        let merge_rate = 0.1 * weight; // conservative merge

        // Merge in Cartesian space (avoids angle wraparound issues)
        let (sx, sy, sz) = self.to_cartesian();
        let (px, py, pz) = peer.to_cartesian();

        let mx = sx * (1.0 - merge_rate) + px * merge_rate;
        let my = sy * (1.0 - merge_rate) + py * merge_rate;
        let mz = sz * (1.0 - merge_rate) + pz * merge_rate;

        let merged = BlochState::from_cartesian(mx, my, mz);
        self.theta = merged.theta;
        self.phi = merged.phi;
        // Damp velocities after merge (reduce oscillation)
        self.d_theta *= 0.5;
        self.d_phi *= 0.5;
    }

    /// Format for prompt injection — compact TOON-style.
    pub fn prompt_section(&self) -> String {
        format!(
            "cognitive_state = {regime} (θ={theta:.2}, φ={phi:.2})\n\
             exploration = {explore:.0}%\n\
             colony_focus = {colony:.0}%\n\
             drive = {drive}",
            regime = self.regime(),
            theta = self.theta,
            phi = self.phi,
            explore = self.exploration_factor() * 100.0,
            colony = self.colony_factor() * 100.0,
            drive = self.drive(),
        )
    }
}

// ── Persistence ──────────────────────────────────────────────────────

/// Load Bloch state from soul database.
pub fn load_bloch(db: &SoulDatabase) -> BlochState {
    db.get_state("bloch_state")
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Save Bloch state to soul database.
pub fn save_bloch(db: &SoulDatabase, state: &BlochState) {
    if let Ok(json) = serde_json::to_string(state) {
        let _ = db.set_state("bloch_state", &json);
    }
}

/// Evolve the Bloch state based on current Free Energy and Psi trends.
/// Called once per thinking cycle from the main loop.
pub fn tick(db: &SoulDatabase, f_trend: f64, psi_trend: f64) -> BlochState {
    let mut state = load_bloch(db);
    state.evolve(f_trend, psi_trend, 1.0);
    save_bloch(db, &state);
    state
}

// ── Status for API ───────────────────────────────────────────────────

/// Bloch state for inclusion in /soul/status response.
pub fn status(db: &SoulDatabase) -> serde_json::Value {
    let state = load_bloch(db);
    let (x, y, z) = state.to_cartesian();
    serde_json::json!({
        "theta": state.theta,
        "phi": state.phi,
        "exploration": state.exploration_factor(),
        "colony_focus": state.colony_factor(),
        "regime": state.regime(),
        "drive": state.drive(),
        "cartesian": {"x": x, "y": y, "z": z},
        "d_theta": state.d_theta,
        "d_phi": state.d_phi,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_balanced() {
        let s = BlochState::new();
        assert!((s.exploration_factor() - 0.5).abs() < 0.01);
        assert_eq!(s.regime(), "LEARN");
    }

    #[test]
    fn test_evolve_toward_exploit_when_f_decreasing() {
        let mut s = BlochState::new();
        // Simulate 20 cycles of decreasing F
        for _ in 0..20 {
            s.evolve(-0.1, 0.0, 1.0); // F trending down
        }
        assert!(
            s.theta < std::f64::consts::FRAC_PI_2,
            "should move toward exploit (lower θ)"
        );
        assert_eq!(s.regime(), "EXPLOIT");
    }

    #[test]
    fn test_evolve_toward_explore_when_f_increasing() {
        let mut s = BlochState::new();
        for _ in 0..20 {
            s.evolve(0.1, 0.0, 1.0); // F trending up (more surprise)
        }
        assert!(
            s.theta > std::f64::consts::FRAC_PI_2,
            "should move toward explore (higher θ)"
        );
        assert_eq!(s.regime(), "EXPLORE");
    }

    #[test]
    fn test_evolve_toward_colony_when_psi_increasing() {
        let mut s = BlochState::new();
        for _ in 0..30 {
            s.evolve(0.0, 0.05, 1.0); // Ψ trending up
        }
        assert!(s.colony_factor() > 0.3, "should increase colony focus");
    }

    #[test]
    fn test_cartesian_roundtrip() {
        let s = BlochState {
            theta: 1.2,
            phi: 2.5,
            d_theta: 0.0,
            d_phi: 0.0,
        };
        let (x, y, z) = s.to_cartesian();
        let s2 = BlochState::from_cartesian(x, y, z);
        assert!((s.theta - s2.theta).abs() < 1e-10);
        assert!((s.phi - s2.phi).abs() < 1e-10);
    }

    #[test]
    fn test_peer_merge() {
        let mut self_state = BlochState::new(); // balanced
        let peer_state = BlochState {
            theta: 0.5,
            phi: 0.0,
            d_theta: 0.0,
            d_phi: 0.0,
        }; // exploiting
        self_state.merge_peer(&peer_state, 0.8, 0.5); // peer is fitter
        assert!(
            self_state.theta < std::f64::consts::FRAC_PI_2,
            "should shift toward peer's exploit"
        );
    }

    #[test]
    fn test_oscillator_modulation() {
        let exploring = BlochState {
            theta: 2.5,
            phi: 0.0,
            d_theta: 0.0,
            d_phi: 0.0,
        };
        let exploiting = BlochState {
            theta: 0.3,
            phi: 0.0,
            d_theta: 0.0,
            d_phi: 0.0,
        };

        // Cortex dreaming should be faster when exploring
        assert!(
            exploring.oscillator_modulation("cortex_dreaming")
                > exploiting.oscillator_modulation("cortex_dreaming")
        );
        // Benchmark should be faster when exploiting
        assert!(
            exploiting.oscillator_modulation("benchmark")
                > exploring.oscillator_modulation("benchmark")
        );
    }
}
