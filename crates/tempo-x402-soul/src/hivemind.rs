//! Hivemind: Stigmergic Swarm Intelligence.
//!
//! ## The Leap
//!
//! Brain thinks. Cortex predicts. Genesis evolves. But agents still work in isolation —
//! they share data but don't truly COORDINATE. The Hivemind changes that.
//!
//! Inspired by ant colonies: no central coordinator, no explicit messages, just
//! **chemical trails** (pheromones) that emerge into collective intelligence.
//!
//! ## Architecture
//!
//! ### 1. Stigmergic Pheromone Trails
//! Agents leave markers on resources (files, actions, error patterns):
//! - **Attractant** (positive valence): "this path leads to success"
//! - **Repellent** (negative valence): "this path is a dead end"
//! - Markers **decay over time** — stale knowledge fades naturally
//! - When planning, agents consult the pheromone map
//! - No explicit coordination needed — stigmergy IS the coordination
//!
//! ### 2. Swarm Goal Awareness
//! Track what each peer is currently working on → avoid duplication.
//! If peer A is working on "fix benchmark", peer B should do something else.
//!
//! ### 3. Collective Intelligence Summary
//! Aggregate cortex + genesis snapshots from all peers into a unified
//! swarm-level intelligence report. Agents see the COLLECTIVE knowledge,
//! not just their own.
//!
//! ### 4. Reputation-Weighted Influence
//! Better-performing agents' pheromones count more. An agent with 80%
//! prediction accuracy influences the swarm more than one with 50%.
//!
//! ## Why Stigmergy Works
//!
//! Ant colonies solve NP-hard optimization problems (traveling salesman,
//! shortest path, load balancing) through stigmergy alone. It works because:
//! - **Positive feedback**: good paths get reinforced by multiple agents
//! - **Negative feedback**: bad paths decay naturally through evaporation
//! - **Distributed**: no single point of failure, no coordinator bottleneck
//! - **Adaptive**: the pheromone field continuously adapts to changing conditions
//! - **Emergent**: complex collective behavior from simple individual rules

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cortex::CortexSnapshot;
use crate::db::SoulDatabase;
use crate::genesis::GenePoolSnapshot;

// ── Constants ────────────────────────────────────────────────────────

/// Maximum pheromone trails stored.
const MAX_TRAILS: usize = 500;
/// Pheromone decay rate per cycle (evaporation).
const PHEROMONE_DECAY: f32 = 0.95;
/// Minimum intensity before a trail is pruned.
const PRUNE_THRESHOLD: f32 = 0.01;
/// How much a single agent deposit affects intensity.
const DEPOSIT_STRENGTH: f32 = 0.3;
/// Reputation decay (toward 0.5 neutral) per cycle.
const _REPUTATION_DECAY: f32 = 0.99;

// ── Core Types ───────────────────────────────────────────────────────

/// A pheromone trail: a marker left by an agent on a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pheromone {
    /// What resource this marks (file path, action type, error pattern, goal keyword).
    pub resource: String,
    /// Category of resource for structured lookup.
    pub category: PheromoneCategory,
    /// Valence: -1.0 (repellent/danger) to +1.0 (attractant/opportunity).
    pub valence: f32,
    /// Intensity: 0.0 (faded) to 1.0+ (fresh/reinforced). Decays over time.
    pub intensity: f32,
    /// Which agent deposited this.
    pub source_agent: String,
    /// When it was deposited.
    pub timestamp: i64,
    /// How many agents have reinforced this trail.
    pub reinforcement_count: u32,
}

/// Categories of pheromone markers for structured queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PheromoneCategory {
    /// Marks on source files: "this file is worth editing" / "this file causes errors"
    File,
    /// Marks on action types: "edit_code works well here" / "run_shell is risky"
    Action,
    /// Marks on error patterns: "compile_error in brain.rs → dead end"
    ErrorPattern,
    /// Marks on goal keywords: "benchmark improvement is fertile ground"
    GoalKeyword,
    /// Marks on step sequences: "read→edit→check→commit works for this type of goal"
    StepSequence,
}

impl std::fmt::Display for PheromoneCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PheromoneCategory::File => write!(f, "file"),
            PheromoneCategory::Action => write!(f, "action"),
            PheromoneCategory::ErrorPattern => write!(f, "error"),
            PheromoneCategory::GoalKeyword => write!(f, "goal"),
            PheromoneCategory::StepSequence => write!(f, "sequence"),
        }
    }
}

/// What a peer is currently doing (for goal deconfliction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerActivity {
    /// Peer instance ID.
    pub instance_id: String,
    /// Current active goal description (if any).
    pub active_goal: Option<String>,
    /// Goal keywords for deduplication matching.
    pub goal_keywords: Vec<String>,
    /// Peer's fitness score.
    pub fitness: f32,
    /// Peer's dominant emotional drive.
    pub drive: String,
    /// When this activity was last updated.
    pub updated_at: i64,
}

/// Collective intelligence summary aggregated from all peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmIntelligence {
    /// Number of peers contributing to this summary.
    pub peer_count: u32,
    /// Collective prediction accuracy (weighted average).
    pub collective_accuracy: f32,
    /// Collective emotional valence (swarm mood).
    pub swarm_valence: f32,
    /// Dominant swarm drive.
    pub swarm_drive: String,
    /// Top curiosity frontiers across all peers.
    pub collective_curiosity: Vec<(String, f32)>,
    /// Top gene pool templates across all peers.
    pub top_templates: Vec<String>,
    /// What goals the swarm is collectively pursuing.
    pub active_goals: Vec<String>,
    /// Discovered patterns (from collective dream insights).
    pub collective_insights: Vec<String>,
}

/// Peer reputation: how trustworthy is this agent's knowledge?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerReputation {
    pub instance_id: String,
    /// Reputation score: 0.0 (unreliable) to 1.0 (highly reliable).
    pub score: f32,
    /// How many interactions this reputation is based on.
    pub interactions: u32,
    /// Prediction accuracy of this peer (if known).
    pub prediction_accuracy: Option<f32>,
    /// This peer's fitness score (if known).
    pub fitness: Option<f32>,
}

// ── The Hivemind ─────────────────────────────────────────────────────

/// The Hivemind: stigmergic swarm intelligence layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hivemind {
    /// The pheromone field: markers left by agents on resources.
    pub trails: Vec<Pheromone>,
    /// What each peer is currently working on.
    pub peer_activities: Vec<PeerActivity>,
    /// Peer reputation scores (reputation-weighted influence).
    pub reputations: HashMap<String, PeerReputation>,
    /// Aggregate swarm intelligence.
    pub swarm_intel: Option<SwarmIntelligence>,
    /// Total pheromone deposits ever made.
    pub total_deposits: u64,
    /// Total evaporation cycles.
    pub evaporation_cycles: u64,
}

impl Default for Hivemind {
    fn default() -> Self {
        Self::new()
    }
}

impl Hivemind {
    /// Create a new empty hivemind.
    pub fn new() -> Self {
        Self {
            trails: Vec::new(),
            peer_activities: Vec::new(),
            reputations: HashMap::new(),
            swarm_intel: None,
            total_deposits: 0,
            evaporation_cycles: 0,
        }
    }

    // ── Pheromone Deposits ───────────────────────────────────────────

    /// Deposit a pheromone trail on a resource.
    /// Positive valence = attractant (good path), negative = repellent (dead end).
    pub fn deposit(
        &mut self,
        resource: &str,
        category: PheromoneCategory,
        valence: f32,
        source_agent: &str,
    ) {
        let now = chrono::Utc::now().timestamp();
        let valence = valence.clamp(-1.0, 1.0);

        // Check if trail already exists for this resource + category
        if let Some(existing) = self
            .trails
            .iter_mut()
            .find(|t| t.resource == resource && t.category == category)
        {
            // Reinforce existing trail
            existing.intensity = (existing.intensity + DEPOSIT_STRENGTH).min(3.0);
            // Blend valence toward new deposit
            existing.valence = existing.valence * 0.7 + valence * 0.3;
            existing.reinforcement_count += 1;
            existing.timestamp = now;
            self.total_deposits += 1;
            return;
        }

        // Create new trail
        self.trails.push(Pheromone {
            resource: resource.to_string(),
            category,
            valence,
            intensity: DEPOSIT_STRENGTH,
            source_agent: source_agent.to_string(),
            timestamp: now,
            reinforcement_count: 1,
        });
        self.total_deposits += 1;

        // Evict if over capacity
        if self.trails.len() > MAX_TRAILS {
            // Remove weakest trail
            if let Some(weakest_idx) = self
                .trails
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.intensity
                        .partial_cmp(&b.intensity)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
            {
                self.trails.remove(weakest_idx);
            }
        }
    }

    /// Deposit pheromones based on a step execution result.
    /// This is the primary way pheromones are created during normal operation.
    pub fn deposit_from_step(
        &mut self,
        action: &str,
        file_path: Option<&str>,
        goal_keywords: &[String],
        succeeded: bool,
        source_agent: &str,
    ) {
        let valence = if succeeded { 0.5 } else { -0.5 };

        // Mark the action type
        self.deposit(action, PheromoneCategory::Action, valence, source_agent);

        // Mark the file (if applicable)
        if let Some(path) = file_path {
            self.deposit(path, PheromoneCategory::File, valence, source_agent);
        }

        // Mark goal keywords
        for kw in goal_keywords.iter().take(3) {
            self.deposit(kw, PheromoneCategory::GoalKeyword, valence, source_agent);
        }
    }

    // ── Pheromone Queries ────────────────────────────────────────────

    /// Get the pheromone intensity for a resource.
    /// Returns (valence, intensity) or None if no trail exists.
    pub fn smell(&self, resource: &str, category: &PheromoneCategory) -> Option<(f32, f32)> {
        self.trails
            .iter()
            .find(|t| t.resource == resource && t.category == *category)
            .map(|t| (t.valence, t.intensity))
    }

    /// Get all trails for a category, sorted by intensity (strongest first).
    pub fn trails_by_category(&self, category: &PheromoneCategory) -> Vec<&Pheromone> {
        let mut trails: Vec<&Pheromone> = self
            .trails
            .iter()
            .filter(|t| &t.category == category)
            .collect();
        trails.sort_by(|a, b| {
            b.intensity
                .partial_cmp(&a.intensity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        trails
    }

    /// Get the strongest attractants (positive trails) for a category.
    pub fn attractants(&self, category: &PheromoneCategory, top_n: usize) -> Vec<&Pheromone> {
        let mut trails: Vec<&Pheromone> = self
            .trails
            .iter()
            .filter(|t| &t.category == category && t.valence > 0.0)
            .collect();
        trails.sort_by(|a, b| {
            let score_a = a.valence * a.intensity;
            let score_b = b.valence * b.intensity;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        trails.truncate(top_n);
        trails
    }

    /// Get the strongest repellents (negative trails) for a category.
    pub fn repellents(&self, category: &PheromoneCategory, top_n: usize) -> Vec<&Pheromone> {
        let mut trails: Vec<&Pheromone> = self
            .trails
            .iter()
            .filter(|t| &t.category == category && t.valence < 0.0)
            .collect();
        trails.sort_by(|a, b| {
            let score_a = a.valence.abs() * a.intensity;
            let score_b = b.valence.abs() * b.intensity;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        trails.truncate(top_n);
        trails
    }

    // ── Evaporation ──────────────────────────────────────────────────

    /// Evaporate all pheromone trails (decay intensity) and prune faded ones.
    /// Called every cycle to simulate natural evaporation.
    pub fn evaporate(&mut self) -> usize {
        self.evaporation_cycles += 1;

        for trail in &mut self.trails {
            trail.intensity *= PHEROMONE_DECAY;
        }

        let before = self.trails.len();
        self.trails.retain(|t| t.intensity >= PRUNE_THRESHOLD);
        before - self.trails.len()
    }

    // ── Swarm Goal Coordination ──────────────────────────────────────

    /// Update what a peer is currently working on.
    pub fn update_peer_activity(
        &mut self,
        instance_id: &str,
        active_goal: Option<String>,
        fitness: f32,
        drive: &str,
    ) {
        let now = chrono::Utc::now().timestamp();
        let goal_keywords = active_goal
            .as_ref()
            .map(|g| crate::genesis::extract_keywords_pub(g))
            .unwrap_or_default();

        if let Some(activity) = self
            .peer_activities
            .iter_mut()
            .find(|a| a.instance_id == instance_id)
        {
            activity.active_goal = active_goal;
            activity.goal_keywords = goal_keywords;
            activity.fitness = fitness;
            activity.drive = drive.to_string();
            activity.updated_at = now;
        } else {
            self.peer_activities.push(PeerActivity {
                instance_id: instance_id.to_string(),
                active_goal,
                goal_keywords,
                fitness,
                drive: drive.to_string(),
                updated_at: now,
            });
        }

        // Prune stale activities (older than 1 hour)
        self.peer_activities.retain(|a| now - a.updated_at < 3600);
    }

    /// Check if a proposed goal overlaps with what peers are already doing.
    /// Returns (is_duplicate, overlapping_peer_id).
    pub fn check_goal_overlap(&self, goal: &str) -> Option<String> {
        let keywords = crate::genesis::extract_keywords_pub(goal);
        if keywords.is_empty() {
            return None;
        }

        for activity in &self.peer_activities {
            if activity.active_goal.is_none() || activity.goal_keywords.is_empty() {
                continue;
            }
            let overlap = keyword_jaccard(&keywords, &activity.goal_keywords);
            if overlap > 0.5 {
                return Some(activity.instance_id.clone());
            }
        }
        None
    }

    // ── Reputation ───────────────────────────────────────────────────

    /// Update a peer's reputation based on observed quality.
    pub fn update_reputation(
        &mut self,
        instance_id: &str,
        prediction_accuracy: Option<f32>,
        fitness: Option<f32>,
    ) {
        let rep = self
            .reputations
            .entry(instance_id.to_string())
            .or_insert(PeerReputation {
                instance_id: instance_id.to_string(),
                score: 0.5,
                interactions: 0,
                prediction_accuracy: None,
                fitness: None,
            });

        rep.interactions += 1;
        rep.prediction_accuracy = prediction_accuracy.or(rep.prediction_accuracy);
        rep.fitness = fitness.or(rep.fitness);

        // Compute reputation from available signals
        let mut signals = Vec::new();
        if let Some(acc) = rep.prediction_accuracy {
            signals.push(acc);
        }
        if let Some(fit) = rep.fitness {
            signals.push(fit);
        }

        if !signals.is_empty() {
            let avg = signals.iter().sum::<f32>() / signals.len() as f32;
            // Blend toward computed reputation
            let alpha = (rep.interactions as f32 / 10.0).min(1.0);
            rep.score = rep.score * (1.0 - alpha * 0.1) + avg * (alpha * 0.1);
        }
    }

    /// Get reputation weight for a peer (used to weight their pheromone influence).
    pub fn reputation_weight(&self, instance_id: &str) -> f32 {
        self.reputations
            .get(instance_id)
            .map(|r| r.score)
            .unwrap_or(0.5) // Unknown peers get neutral weight
    }

    // ── Collective Intelligence ──────────────────────────────────────

    /// Aggregate intelligence from peer cortex + genesis snapshots.
    pub fn aggregate_swarm_intel(
        &mut self,
        peer_cortexes: &[(String, CortexSnapshot)],
        peer_gene_pools: &[(String, GenePoolSnapshot)],
    ) {
        if peer_cortexes.is_empty() && peer_gene_pools.is_empty() {
            return;
        }

        let mut total_accuracy = 0.0f32;
        let mut total_valence = 0.0f32;
        let mut weight_sum = 0.0f32;
        let mut curiosity_map: HashMap<String, f32> = HashMap::new();
        let mut all_insights = Vec::new();
        let mut all_goals = Vec::new();
        let mut drive_counts: HashMap<String, u32> = HashMap::new();

        // Aggregate cortex data
        for (peer_id, cortex) in peer_cortexes {
            let weight = self.reputation_weight(peer_id);

            if cortex.total_experiences > 0 {
                let accuracy = cortex.total_experiences as f32; // proxy
                total_accuracy += accuracy * weight;
                weight_sum += weight;
            }

            // Merge curiosity frontiers
            for (action, score) in &cortex.curiosity_frontier {
                let entry = curiosity_map.entry(action.clone()).or_insert(0.0);
                *entry = (*entry + score * weight).min(1.0);
            }

            // Collect insights
            for insight in &cortex.insights {
                all_insights.push(format!(
                    "[{}] {}",
                    peer_id.chars().take(8).collect::<String>(),
                    insight.pattern
                ));
            }
        }

        // Aggregate genesis data
        let mut template_summaries = Vec::new();
        for (_peer_id, pool) in peer_gene_pools {
            for template in pool.templates.iter().take(3) {
                template_summaries.push(format!(
                    "{} ({:.0}%)",
                    template.step_types.join("→"),
                    template.fitness * 100.0,
                ));
            }
        }

        // Aggregate peer activities
        for activity in &self.peer_activities {
            if let Some(goal) = &activity.active_goal {
                all_goals.push(goal.clone());
            }
            total_valence += activity.fitness;
            *drive_counts.entry(activity.drive.clone()).or_insert(0) += 1;
        }

        let peer_count = (peer_cortexes.len() + self.peer_activities.len()) as u32;
        let swarm_drive = drive_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(drive, _)| drive)
            .unwrap_or_else(|| "neutral".to_string());

        // Build sorted curiosity list
        let mut curiosity_list: Vec<(String, f32)> = curiosity_map.into_iter().collect();
        curiosity_list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        curiosity_list.truncate(10);

        all_insights.truncate(10);
        template_summaries.truncate(5);
        all_goals.truncate(10);

        self.swarm_intel = Some(SwarmIntelligence {
            peer_count,
            collective_accuracy: if weight_sum > 0.0 {
                total_accuracy / weight_sum
            } else {
                0.5
            },
            swarm_valence: if peer_count > 0 {
                total_valence / peer_count as f32
            } else {
                0.0
            },
            swarm_drive,
            collective_curiosity: curiosity_list,
            top_templates: template_summaries,
            active_goals: all_goals,
            collective_insights: all_insights,
        });
    }

    // ── Prompt Generation ────────────────────────────────────────────

    /// Generate a prompt section with swarm intelligence for goal creation.
    pub fn prompt_section(&self) -> String {
        let mut lines = Vec::new();

        // Only generate if we have meaningful data
        let has_trails = !self.trails.is_empty();
        let has_swarm = self.swarm_intel.is_some();
        let has_activities = !self.peer_activities.is_empty();

        if !has_trails && !has_swarm && !has_activities {
            return String::new();
        }

        lines.push("# Hivemind: Swarm Intelligence".to_string());

        // Pheromone trails
        if has_trails {
            // Show top attractants
            let attract_files = self.attractants(&PheromoneCategory::File, 3);
            let attract_actions = self.attractants(&PheromoneCategory::Action, 3);
            let repel_files = self.repellents(&PheromoneCategory::File, 3);
            let repel_actions = self.repellents(&PheromoneCategory::Action, 3);

            if !attract_files.is_empty() || !attract_actions.is_empty() {
                lines.push("## Fertile Ground (strong positive trails)".to_string());
                for trail in attract_files.iter().chain(attract_actions.iter()) {
                    lines.push(format!(
                        "- [{}] {} (intensity: {:.0}%, reinforced {}x)",
                        trail.category,
                        trail.resource,
                        trail.intensity * 100.0,
                        trail.reinforcement_count,
                    ));
                }
            }

            if !repel_files.is_empty() || !repel_actions.is_empty() {
                lines.push("## Dead Ends (strong negative trails — AVOID)".to_string());
                for trail in repel_files.iter().chain(repel_actions.iter()) {
                    lines.push(format!(
                        "- [{}] {} (repellent: {:.0}%, reinforced {}x)",
                        trail.category,
                        trail.resource,
                        trail.intensity * 100.0,
                        trail.reinforcement_count,
                    ));
                }
            }
        }

        // Swarm coordination: what are peers doing?
        if has_activities {
            let active: Vec<_> = self
                .peer_activities
                .iter()
                .filter(|a| a.active_goal.is_some())
                .collect();
            if !active.is_empty() {
                lines.push("## Peer Activity (avoid duplicating their work)".to_string());
                for a in active.iter().take(5) {
                    lines.push(format!(
                        "- {} is working on: {} (fitness: {:.0}%, drive: {})",
                        a.instance_id.chars().take(8).collect::<String>(),
                        a.active_goal.as_deref().unwrap_or("unknown"),
                        a.fitness * 100.0,
                        a.drive,
                    ));
                }
            }
        }

        // Collective intelligence
        if let Some(intel) = &self.swarm_intel {
            if intel.peer_count > 0 {
                lines.push(format!(
                    "## Collective ({} peers, swarm mood: {}, valence: {:+.2})",
                    intel.peer_count, intel.swarm_drive, intel.swarm_valence,
                ));

                if !intel.collective_curiosity.is_empty() {
                    lines.push("Swarm curiosity frontier:".to_string());
                    for (action, score) in intel.collective_curiosity.iter().take(5) {
                        lines.push(format!(
                            "- {action}: {:.0}% collective curiosity",
                            score * 100.0
                        ));
                    }
                }

                if !intel.collective_insights.is_empty() {
                    lines.push("Swarm insights:".to_string());
                    for insight in intel.collective_insights.iter().take(3) {
                        lines.push(format!("- {insight}"));
                    }
                }
            }
        }

        lines.join("\n")
    }

    // ── Merge from Peer ──────────────────────────────────────────────

    /// Import pheromone trails from a peer, weighted by their reputation.
    pub fn import_peer_trails(&mut self, peer_id: &str, peer_trails: &[Pheromone]) {
        let weight = self.reputation_weight(peer_id);

        for trail in peer_trails {
            let weighted_intensity = trail.intensity * weight;
            if weighted_intensity < PRUNE_THRESHOLD {
                continue;
            }

            if let Some(existing) = self
                .trails
                .iter_mut()
                .find(|t| t.resource == trail.resource && t.category == trail.category)
            {
                // Blend with existing
                existing.intensity = (existing.intensity + weighted_intensity * 0.3).min(3.0);
                existing.valence = existing.valence * 0.8 + trail.valence * 0.2;
                existing.reinforcement_count += 1;
            } else if self.trails.len() < MAX_TRAILS {
                // Import new trail
                let mut imported = trail.clone();
                imported.intensity = weighted_intensity;
                imported.source_agent = peer_id.to_string();
                self.trails.push(imported);
            }
        }
    }

    /// Export trails for peer sharing (top trails by intensity).
    pub fn export_trails(&self, top_n: usize) -> Vec<Pheromone> {
        let mut trails = self.trails.clone();
        trails.sort_by(|a, b| {
            b.intensity
                .partial_cmp(&a.intensity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        trails.truncate(top_n);
        trails
    }

    // ── Persistence ──────────────────────────────────────────────────

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

// ── Persistence helpers ──────────────────────────────────────────────

/// Load hivemind from database.
pub fn load_hivemind(db: &SoulDatabase) -> Hivemind {
    match db.get_state("hivemind_state").ok().flatten() {
        Some(json) => Hivemind::from_json(&json).unwrap_or_default(),
        None => Hivemind::new(),
    }
}

/// Save hivemind to database.
pub fn save_hivemind(db: &SoulDatabase, hive: &Hivemind) {
    let json = hive.to_json();
    if let Err(e) = db.set_state("hivemind_state", &json) {
        tracing::warn!(error = %e, "Failed to save hivemind state");
    }
}

// ── Utility ──────────────────────────────────────────────────────────

/// Jaccard similarity between two keyword sets.
fn keyword_jaccard(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let set_a: std::collections::HashSet<&str> = a.iter().map(|s| s.as_str()).collect();
    let set_b: std::collections::HashSet<&str> = b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count() as f32;
    let union = set_a.union(&set_b).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_hivemind() {
        let hive = Hivemind::new();
        assert_eq!(hive.trails.len(), 0);
        assert_eq!(hive.total_deposits, 0);
    }

    #[test]
    fn test_deposit_and_smell() {
        let mut hive = Hivemind::new();
        hive.deposit("brain.rs", PheromoneCategory::File, 0.8, "agent-1");

        let smell = hive.smell("brain.rs", &PheromoneCategory::File);
        assert!(smell.is_some());
        let (valence, intensity) = smell.unwrap();
        assert!(valence > 0.0);
        assert!(intensity > 0.0);
    }

    #[test]
    fn test_reinforcement() {
        let mut hive = Hivemind::new();
        hive.deposit("edit_code", PheromoneCategory::Action, 0.5, "agent-1");
        let initial = hive.trails[0].intensity;

        hive.deposit("edit_code", PheromoneCategory::Action, 0.7, "agent-2");
        assert!(
            hive.trails[0].intensity > initial,
            "Reinforcement should increase intensity"
        );
        assert_eq!(hive.trails[0].reinforcement_count, 2);
    }

    #[test]
    fn test_evaporation() {
        let mut hive = Hivemind::new();
        hive.deposit("test", PheromoneCategory::Action, 0.5, "agent-1");

        // Evaporate many times
        for _ in 0..100 {
            hive.evaporate();
        }

        // Weak trails should be pruned
        assert!(
            hive.trails.is_empty(),
            "Heavily evaporated trails should be pruned"
        );
    }

    #[test]
    fn test_attractants_and_repellents() {
        let mut hive = Hivemind::new();
        hive.deposit("good_file.rs", PheromoneCategory::File, 0.9, "agent-1");
        hive.deposit("bad_file.rs", PheromoneCategory::File, -0.8, "agent-1");
        hive.deposit("neutral.rs", PheromoneCategory::File, 0.0, "agent-1");

        let attract = hive.attractants(&PheromoneCategory::File, 5);
        assert_eq!(attract.len(), 1);
        assert_eq!(attract[0].resource, "good_file.rs");

        let repel = hive.repellents(&PheromoneCategory::File, 5);
        assert_eq!(repel.len(), 1);
        assert_eq!(repel[0].resource, "bad_file.rs");
    }

    #[test]
    fn test_goal_overlap_detection() {
        let mut hive = Hivemind::new();
        hive.update_peer_activity(
            "agent-1",
            Some("Fix compile errors in benchmark module".to_string()),
            0.6,
            "exploit",
        );

        // Similar goal should be detected
        let overlap = hive.check_goal_overlap("Fix compile errors in benchmark scoring");
        assert!(
            overlap.is_some(),
            "Similar goals should be detected as overlapping"
        );

        // Different goal should not overlap
        let no_overlap = hive.check_goal_overlap("Create new REST endpoint for payments");
        assert!(no_overlap.is_none(), "Different goals should not overlap");
    }

    #[test]
    fn test_reputation_weighting() {
        let mut hive = Hivemind::new();

        // Agent with good reputation
        hive.update_reputation("good-agent", Some(0.9), Some(0.8));
        hive.update_reputation("good-agent", Some(0.9), Some(0.8));
        hive.update_reputation("good-agent", Some(0.9), Some(0.8));

        // Agent with poor reputation
        hive.update_reputation("bad-agent", Some(0.2), Some(0.1));

        let good_weight = hive.reputation_weight("good-agent");
        let bad_weight = hive.reputation_weight("bad-agent");
        let unknown_weight = hive.reputation_weight("unknown-agent");

        assert!(
            good_weight > bad_weight,
            "Good agent should have higher weight: {} vs {}",
            good_weight,
            bad_weight
        );
        assert!(
            (unknown_weight - 0.5).abs() < 0.01,
            "Unknown agent should have neutral weight"
        );
    }

    #[test]
    fn test_peer_trail_import() {
        let mut hive_a = Hivemind::new();
        let mut hive_b = Hivemind::new();

        // Agent A discovers something
        hive_a.deposit("cortex.rs", PheromoneCategory::File, 0.9, "agent-a");
        hive_a.deposit("cortex.rs", PheromoneCategory::File, 0.9, "agent-a");

        // Agent B imports A's trails
        let exported = hive_a.export_trails(50);
        hive_b.import_peer_trails("agent-a", &exported);

        assert!(
            !hive_b.trails.is_empty(),
            "B should have imported trails from A"
        );
    }

    #[test]
    fn test_prompt_section() {
        let mut hive = Hivemind::new();
        hive.deposit("brain.rs", PheromoneCategory::File, 0.8, "agent-1");
        hive.deposit("broken.rs", PheromoneCategory::File, -0.7, "agent-1");
        hive.update_peer_activity(
            "agent-2",
            Some("Improving benchmark scores".to_string()),
            0.7,
            "explore",
        );

        let prompt = hive.prompt_section();
        assert!(prompt.contains("Hivemind"));
        assert!(prompt.contains("brain.rs"));
        assert!(prompt.contains("broken.rs"));
    }

    #[test]
    fn test_serialization() {
        let mut hive = Hivemind::new();
        hive.deposit("test", PheromoneCategory::Action, 0.5, "agent-1");

        let json = hive.to_json();
        let restored = Hivemind::from_json(&json).unwrap();
        assert_eq!(restored.trails.len(), 1);
        assert_eq!(restored.total_deposits, 1);
    }

    #[test]
    fn test_deposit_from_step() {
        let mut hive = Hivemind::new();
        hive.deposit_from_step(
            "edit_code",
            Some("brain.rs"),
            &["benchmark".to_string(), "fix".to_string()],
            true,
            "agent-1",
        );

        // Should have trails for action, file, and goal keywords
        assert!(hive.trails.len() >= 3);
    }
}
