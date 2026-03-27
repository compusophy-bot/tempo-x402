//! Mixture of Experts (MoE) router for colony-wide brain predictions.
//!
//! Instead of averaging all peer brain weights into one 1.2M brain (lossy),
//! MoE maintains each peer's brain separately and learns WHICH peer to trust
//! for which type of problem. N peers = N×1.2M effective parameters, accessed
//! through a small routing network.
//!
//! ## How it works
//!
//! 1. Each peer's capability profile (per-skill success rates) is synced during peer discovery
//! 2. The router maps problem features → expert weights (which peer to trust)
//! 3. For predictions: run local brain + consult peer expertise → weighted combination
//! 4. For task delegation: route hard sub-problems to the best expert
//!
//! ## Training
//!
//! The router trains on outcome feedback: when a peer was consulted and the prediction
//! was right/wrong, the router updates its weights toward/away from that peer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::brain::BrainPrediction;
use crate::db::SoulDatabase;

// ── Expert Profile ──────────────────────────────────────────────────

/// A peer's expertise profile — what they're good at and how reliable they are.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertProfile {
    /// Peer instance ID.
    pub peer_id: String,
    /// Peer URL for delegation.
    pub url: String,
    /// Per-capability success rate (0.0 - 1.0).
    pub capability_scores: HashMap<String, f64>,
    /// Overall success rate.
    pub overall_success: f64,
    /// How many predictions we've verified from this peer.
    pub verified_predictions: u64,
    /// How many were correct.
    pub correct_predictions: u64,
    /// Brain training steps (proxy for experience).
    pub brain_steps: u64,
    /// Last updated timestamp.
    pub updated_at: i64,
}

impl ExpertProfile {
    /// Accuracy of this expert's predictions as verified locally.
    pub fn accuracy(&self) -> f64 {
        if self.verified_predictions == 0 {
            0.5 // uninformative prior
        } else {
            self.correct_predictions as f64 / self.verified_predictions as f64
        }
    }

    /// Trust weight for this expert — combination of accuracy and experience.
    pub fn trust_weight(&self) -> f64 {
        let acc = self.accuracy();
        // More experience = more trust in the accuracy estimate
        let confidence = 1.0 - (1.0 / (1.0 + self.verified_predictions as f64 / 10.0));
        // Blend accuracy with prior (0.5) based on confidence
        let blended_accuracy = confidence * acc + (1.0 - confidence) * 0.5;
        // Scale by brain experience (more trained = more trustworthy)
        let experience_factor = (self.brain_steps as f64 / 10000.0).min(1.0);
        blended_accuracy * (0.5 + 0.5 * experience_factor)
    }
}

// ── Router ──────────────────────────────────────────────────────────

/// The MoE router — maintains expert profiles and routes problems to the best expert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoeRouter {
    /// Known expert profiles indexed by peer_id.
    pub experts: HashMap<String, ExpertProfile>,
    /// Per-capability routing: capability → ranked list of (peer_id, weight).
    pub routing_table: HashMap<String, Vec<(String, f64)>>,
    /// Total routing decisions made.
    pub total_routes: u64,
    /// How many delegated tasks succeeded.
    pub delegation_successes: u64,
}

impl MoeRouter {
    pub fn new() -> Self {
        Self {
            experts: HashMap::new(),
            routing_table: HashMap::new(),
            total_routes: 0,
            delegation_successes: 0,
        }
    }

    /// Update an expert's profile from peer sync data.
    pub fn update_expert(
        &mut self,
        peer_id: &str,
        url: &str,
        capability_scores: HashMap<String, f64>,
        overall_success: f64,
        brain_steps: u64,
    ) {
        let expert = self
            .experts
            .entry(peer_id.to_string())
            .or_insert(ExpertProfile {
                peer_id: peer_id.to_string(),
                url: url.to_string(),
                capability_scores: HashMap::new(),
                overall_success: 0.0,
                verified_predictions: 0,
                correct_predictions: 0,
                brain_steps: 0,
                updated_at: 0,
            });
        expert.url = url.to_string();
        expert.capability_scores = capability_scores;
        expert.overall_success = overall_success;
        expert.brain_steps = brain_steps;
        expert.updated_at = chrono::Utc::now().timestamp();

        // Rebuild routing table
        self.rebuild_routing_table();
    }

    /// Record a prediction outcome — update expert accuracy tracking.
    pub fn record_outcome(&mut self, peer_id: &str, was_correct: bool) {
        if let Some(expert) = self.experts.get_mut(peer_id) {
            expert.verified_predictions += 1;
            if was_correct {
                expert.correct_predictions += 1;
            }
        }
    }

    /// Record a delegation outcome.
    pub fn record_delegation(&mut self, success: bool) {
        self.total_routes += 1;
        if success {
            self.delegation_successes += 1;
        }
    }

    /// Get the best expert for a given capability.
    /// Returns (peer_id, url, trust_weight) or None if no experts known.
    pub fn best_expert_for(&self, capability: &str) -> Option<(&str, &str, f64)> {
        self.routing_table
            .get(capability)
            .and_then(|ranked| ranked.first())
            .and_then(|(peer_id, weight)| {
                self.experts
                    .get(peer_id)
                    .map(|e| (e.peer_id.as_str(), e.url.as_str(), *weight))
            })
    }

    /// Get ranked experts for a capability (all of them, best first).
    pub fn ranked_experts_for(&self, capability: &str) -> Vec<(&ExpertProfile, f64)> {
        self.routing_table
            .get(capability)
            .map(|ranked| {
                ranked
                    .iter()
                    .filter_map(|(peer_id, weight)| self.experts.get(peer_id).map(|e| (e, *weight)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Combine a local prediction with expert knowledge.
    /// Returns a weighted prediction that blends local brain with colony expertise.
    pub fn combine_prediction(
        &self,
        local_pred: &BrainPrediction,
        capability: &str,
    ) -> BrainPrediction {
        let experts = self.ranked_experts_for(capability);
        if experts.is_empty() {
            return local_pred.clone(); // no experts, use local
        }

        // Local brain gets weight 1.0 (baseline)
        let local_weight = 1.0f64;
        let mut total_weight = local_weight;
        let mut weighted_success = local_pred.success_prob as f64 * local_weight;

        // Each expert contributes based on their trust weight × capability score
        for (expert, trust) in &experts {
            let cap_score = expert
                .capability_scores
                .get(capability)
                .copied()
                .unwrap_or(expert.overall_success);
            // Expert's "prediction" for this capability is their success rate
            let expert_pred = cap_score;
            weighted_success += expert_pred * trust;
            total_weight += trust;
        }

        let combined_success = (weighted_success / total_weight) as f32;

        BrainPrediction {
            success_prob: combined_success,
            // Keep local brain's error classification (experts don't provide this remotely)
            likely_error: local_pred.likely_error.clone(),
            error_confidence: local_pred.error_confidence
                * (local_weight as f32 / total_weight as f32),
            capability_confidence: local_pred.capability_confidence.clone(),
        }
    }

    /// Generate a routing summary for prompt injection.
    /// Tells the agent which peers are experts at what.
    pub fn prompt_section(&self) -> String {
        if self.experts.is_empty() {
            return String::new();
        }

        let mut lines = vec!["# Colony Expertise Map (MoE Router)".to_string()];
        lines.push("Peers ranked by capability:".to_string());

        for (capability, ranked) in &self.routing_table {
            if ranked.is_empty() {
                continue;
            }
            let top: Vec<String> = ranked
                .iter()
                .take(3)
                .filter_map(|(peer_id, weight)| {
                    self.experts.get(peer_id).map(|e| {
                        format!(
                            "{} ({:.0}% accuracy, {:.0}% trust)",
                            &peer_id[..8.min(peer_id.len())],
                            e.accuracy() * 100.0,
                            weight * 100.0,
                        )
                    })
                })
                .collect();
            if !top.is_empty() {
                lines.push(format!("- {}: {}", capability, top.join(", ")));
            }
        }

        if self.total_routes > 0 {
            let success_rate = self.delegation_successes as f64 / self.total_routes as f64 * 100.0;
            lines.push(format!(
                "Delegation track record: {}/{} ({:.0}%)",
                self.delegation_successes, self.total_routes, success_rate
            ));
        }

        lines.join("\n")
    }

    /// Rebuild the routing table from expert profiles.
    fn rebuild_routing_table(&mut self) {
        self.routing_table.clear();

        // Collect all known capabilities
        let mut all_capabilities: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for expert in self.experts.values() {
            all_capabilities.extend(expert.capability_scores.keys().cloned());
        }

        // For each capability, rank experts by trust weight × capability score
        for cap in &all_capabilities {
            let mut ranked: Vec<(String, f64)> = self
                .experts
                .values()
                .map(|e| {
                    let cap_score = e.capability_scores.get(cap).copied().unwrap_or(0.0);
                    let trust = e.trust_weight();
                    (e.peer_id.clone(), trust * cap_score)
                })
                .filter(|(_, score)| *score > 0.0)
                .collect();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            self.routing_table.insert(cap.clone(), ranked);
        }
    }
}

// ── Persistence ─────────────────────────────────────────────────────

/// Load MoE router from soul_state.
pub fn load_router(db: &SoulDatabase) -> MoeRouter {
    db.get_state("moe_router")
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_else(MoeRouter::new)
}

/// Save MoE router to soul_state.
pub fn save_router(db: &SoulDatabase, router: &MoeRouter) {
    if let Ok(json) = serde_json::to_string(router) {
        let _ = db.set_state("moe_router", &json);
    }
}

// ── Integration helpers ─────────────────────────────────────────────

/// Update the MoE router from peer sync data.
/// Called during discover_peers when capability profiles are exchanged.
pub fn update_from_peer_sync(
    db: &SoulDatabase,
    peer_id: &str,
    peer_url: &str,
    capability_profile: &HashMap<String, f64>,
    overall_success: f64,
    brain_steps: u64,
) {
    let mut router = load_router(db);
    router.update_expert(
        peer_id,
        peer_url,
        capability_profile.clone(),
        overall_success,
        brain_steps,
    );
    save_router(db, &router);
}

/// Map a PlanStep to a capability name for routing.
pub fn step_to_capability(step: &crate::plan::PlanStep) -> &'static str {
    use crate::plan::PlanStep;
    match step {
        PlanStep::ReadFile { .. } | PlanStep::ListDir { .. } => "FileRead",
        PlanStep::GenerateCode { .. } => "CodeGen",
        PlanStep::EditCode { .. } => "CodeGen",
        PlanStep::SearchCode { .. } => "CodeSearch",
        PlanStep::RunShell { .. } => "ShellExec",
        PlanStep::Commit { .. } => "GitOps",
        PlanStep::Think { .. } => "CodeGen",
        PlanStep::CheckSelf { .. } => "PeerCall",
        PlanStep::CreateScriptEndpoint { .. } => "EndpointCreate",
        PlanStep::TestScriptEndpoint { .. } => "TestPass",
        PlanStep::CargoCheck { .. } => "CodeCompile",
        _ => "General",
    }
}
