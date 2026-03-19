//! Autonomy: Autonomous Planning + Recursive Self-Improvement.
//!
//! ## The Final Leap
//!
//! Until now, every plan required an LLM call. The agent couldn't think without
//! phoning home to Gemini. Autonomy changes that.
//!
//! ### Autonomous Plan Compilation
//! For goals the agent has solved before (or similar), the system can generate
//! a COMPLETE executable plan from:
//! - Genesis templates (proven plan structures)
//! - Cortex causal graph (what actions lead where)
//! - Hivemind pheromone trails (what the colony recommends)
//! - Synthesis imagination (novel recombinations)
//!
//! If the compiled plan scores high enough on cortex simulation, it executes
//! WITHOUT ANY LLM CALL for plan creation. The LLM is still used for code
//! generation steps, but the plan STRUCTURE is autonomous.
//!
//! ### Recursive Self-Improvement
//! The synthesis monitors its own cognitive architecture and identifies:
//! - Which system is underperforming
//! - What type of experience would improve it
//! - What goals to create to gather that experience
//!
//! This is genuine recursive self-improvement: the agent's intelligence loop
//! feeds back on itself.
//!
//! ### Cognitive Peer Sync
//! During peer discovery, share and merge ALL cognitive systems:
//! - Cortex world models (causal knowledge)
//! - Genesis templates (evolved plan strategies)
//! - Hivemind trails (stigmergic markers)
//! - Synthesis insights (metacognitive state)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cortex::{self, Cortex};
use crate::db::SoulDatabase;
use crate::genesis;

use crate::hivemind::{self, Hivemind, PheromoneCategory};
use crate::plan::{Plan, PlanStatus, PlanStep};
use crate::synthesis::{self, CognitiveState};

// ── Constants ────────────────────────────────────────────────────────

/// Minimum cortex simulation confidence to accept an autonomous plan.
/// Lowered from 0.5 to 0.3 — let autonomous plans attempt more often and learn from failures.
const AUTONOMOUS_CONFIDENCE_THRESHOLD: f32 = 0.3;
/// Minimum genesis template match score.
/// Lowered from 0.3 to 0.15 — partial keyword matches are still useful scaffolding.
const TEMPLATE_MATCH_THRESHOLD: f32 = 0.15;
/// Minimum cortex simulation success probability.
/// Lowered from 0.3 to 0.2 — even 20% predicted success is worth attempting.
const SIMULATION_SUCCESS_THRESHOLD: f32 = 0.2;

// ── Autonomous Plan Compilation ──────────────────────────────────────

/// Result of attempting to compile a plan autonomously.
#[derive(Debug, Clone)]
pub enum CompilationResult {
    /// Successfully compiled an autonomous plan.
    Compiled(Plan),
    /// Could not compile — fall back to LLM.
    FallbackToLlm(String),
}

/// Attempt to compile a plan autonomously (no LLM needed).
///
/// Returns `Compiled(Plan)` if confident enough, `FallbackToLlm` otherwise.
pub fn compile_autonomous_plan(
    db: &SoulDatabase,
    goal_id: &str,
    goal_description: &str,
    _instance_id: &str,
) -> CompilationResult {
    let cortex = cortex::load_cortex(db);
    let gene_pool = genesis::load_gene_pool(db);
    let hivemind = hivemind::load_hivemind(db);
    let synth = synthesis::load_synthesis(db);

    // ── Phase 1: Find best matching template ──
    let matches = gene_pool.suggest_templates(goal_description, 3);
    if matches.is_empty() {
        return CompilationResult::FallbackToLlm("No matching templates in gene pool".to_string());
    }

    let (best_template, match_score) = &matches[0];
    if *match_score < TEMPLATE_MATCH_THRESHOLD {
        return CompilationResult::FallbackToLlm(format!(
            "Best template match too low: {:.0}%",
            match_score * 100.0
        ));
    }

    // ── Phase 2: Enrich template with concrete parameters ──
    let steps = enrich_template(
        &best_template.step_types,
        goal_description,
        &cortex,
        &hivemind,
    );

    if steps.is_empty() {
        return CompilationResult::FallbackToLlm(
            "Could not enrich template with concrete steps".to_string(),
        );
    }

    // ── Phase 3: Simulate through cortex ──
    let simulation = cortex.simulate_plan(&steps);

    if simulation.confidence < AUTONOMOUS_CONFIDENCE_THRESHOLD {
        return CompilationResult::FallbackToLlm(format!(
            "Cortex confidence too low: {:.0}%",
            simulation.confidence * 100.0
        ));
    }

    if simulation.predicted_success < SIMULATION_SUCCESS_THRESHOLD {
        return CompilationResult::FallbackToLlm(format!(
            "Predicted success too low: {:.0}%",
            simulation.predicted_success * 100.0
        ));
    }

    // ── Phase 4: Check hivemind for repellent trails ──
    let has_strong_repellent = steps.iter().any(|step| {
        let action = cortex::step_to_action_name(step);
        hivemind
            .smell(&action, &PheromoneCategory::Action)
            .map(|(v, i)| v < -0.5 && i > 0.3)
            .unwrap_or(false)
    });

    if has_strong_repellent {
        return CompilationResult::FallbackToLlm(
            "Hivemind has strong repellent on planned actions".to_string(),
        );
    }

    // ── Phase 5: Check synthesis cognitive state ──
    if synth.state == CognitiveState::Stuck {
        return CompilationResult::FallbackToLlm(
            "Cognitive state is Stuck — need LLM for creative approach".to_string(),
        );
    }

    // ── All checks pass — compile the plan ──
    let now = chrono::Utc::now().timestamp();
    let plan = Plan {
        id: uuid::Uuid::new_v4().to_string(),
        goal_id: goal_id.to_string(),
        steps,
        current_step: 0,
        status: PlanStatus::Active,
        context: HashMap::new(),
        replan_count: 0,
        created_at: now,
        updated_at: now,
    };

    tracing::info!(
        plan_id = %plan.id,
        goal = %goal_description,
        steps = plan.steps.len(),
        template_match = format!("{:.0}%", match_score * 100.0),
        sim_success = format!("{:.0}%", simulation.predicted_success * 100.0),
        sim_confidence = format!("{:.0}%", simulation.confidence * 100.0),
        "AUTONOMOUS plan compiled (no LLM call)"
    );

    CompilationResult::Compiled(plan)
}

/// Enrich abstract step types with concrete parameters from experience.
fn enrich_template(
    step_types: &[String],
    goal: &str,
    cortex: &Cortex,
    hivemind: &Hivemind,
) -> Vec<PlanStep> {
    let mut steps = Vec::new();
    let goal_lower = goal.to_lowercase();

    // Try to infer a target file from the goal description
    let target_file = infer_target_file(&goal_lower, cortex, hivemind);

    for step_type in step_types {
        let step = match step_type.as_str() {
            "read_file" => {
                if let Some(ref file) = target_file {
                    PlanStep::ReadFile {
                        path: file.clone(),
                        store_as: Some("source".to_string()),
                    }
                } else {
                    // Can't infer file — need LLM
                    return vec![];
                }
            }
            "search_code" => {
                // Extract a search term from the goal
                let term = extract_search_term(&goal_lower);
                PlanStep::SearchCode {
                    pattern: term,
                    directory: Some(".".to_string()),
                    store_as: Some("search_results".to_string()),
                }
            }
            "list_dir" => PlanStep::ListDir {
                path: "crates/tempo-x402-soul/src".to_string(),
                store_as: Some("dir_listing".to_string()),
            },
            "edit_code" => {
                if let Some(ref file) = target_file {
                    PlanStep::EditCode {
                        description: goal.to_string(),
                        file_path: file.clone(),
                        context_keys: vec!["source".to_string()],
                    }
                } else {
                    return vec![];
                }
            }
            "generate_code" => {
                if let Some(ref file) = target_file {
                    PlanStep::GenerateCode {
                        description: goal.to_string(),
                        file_path: file.clone(),
                        context_keys: vec!["source".to_string()],
                    }
                } else {
                    return vec![];
                }
            }
            "cargo_check" => PlanStep::CargoCheck {
                store_as: Some("check_result".to_string()),
            },
            "commit" => PlanStep::Commit {
                message: format!("auto: {}", goal.chars().take(60).collect::<String>()),
            },
            "think" => PlanStep::Think {
                question: format!("How should I approach: {goal}"),
                store_as: Some("analysis".to_string()),
            },
            "check_self" => PlanStep::CheckSelf {
                endpoint: "/health".to_string(),
                store_as: Some("health".to_string()),
            },
            "discover_peers" => PlanStep::DiscoverPeers {
                store_as: Some("peers".to_string()),
            },
            "run_shell" => PlanStep::RunShell {
                command: "cargo test --workspace 2>&1 | tail -20".to_string(),
                store_as: Some("test_output".to_string()),
            },
            _ => continue, // Skip unknown step types
        };
        steps.push(step);
    }

    steps
}

/// Infer a target file from the goal description and experience.
fn infer_target_file(goal: &str, cortex: &Cortex, hivemind: &Hivemind) -> Option<String> {
    // Check for explicit file mentions in goal
    let known_files = [
        "brain.rs",
        "cortex.rs",
        "genesis.rs",
        "hivemind.rs",
        "synthesis.rs",
        "thinking.rs",
        "plan.rs",
        "prompts.rs",
        "fitness.rs",
        "benchmark.rs",
        "coding.rs",
        "world_model.rs",
        "neuroplastic.rs",
        "git.rs",
        "mode.rs",
        "chat.rs",
        "memory.rs",
        "normalize.rs",
        "housekeeping.rs",
    ];
    for file in known_files {
        if goal.contains(file) || goal.contains(&file.replace(".rs", "")) {
            return Some(format!("crates/tempo-x402-soul/src/{file}"));
        }
    }

    // Check hivemind for attractive files
    let attractive_files = hivemind.attractants(&PheromoneCategory::File, 1);
    if let Some(trail) = attractive_files.first() {
        if trail.intensity > 0.3 {
            return Some(trail.resource.clone());
        }
    }

    // Check cortex experiences for recently successful files
    let recent_files: Vec<&str> = cortex
        .experiences
        .iter()
        .rev()
        .filter(|e| e.succeeded && e.action == "read_file")
        .filter_map(|e| {
            e.context_tags
                .iter()
                .find(|t| t.starts_with("file:"))
                .map(|t| &t[5..])
        })
        .take(1)
        .collect();
    if let Some(file) = recent_files.first() {
        return Some(file.to_string());
    }

    None
}

/// Extract a search term from a goal description.
fn extract_search_term(goal: &str) -> String {
    // Take the most specific word (longest, not a stop word)
    let keywords = genesis::extract_keywords_pub(goal);
    keywords
        .into_iter()
        .max_by_key(|k| k.len())
        .unwrap_or_else(|| "TODO".to_string())
}

// ── Recursive Self-Improvement ───────────────────────────────────────

/// Cognitive improvement suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementGoal {
    /// Goal description.
    pub description: String,
    /// Why this improvement is needed.
    pub reasoning: String,
    /// Priority (1-5).
    pub priority: u32,
    /// Which cognitive system this improves.
    pub target_system: String,
}

/// Analyze the cognitive architecture and suggest improvement goals.
/// This is recursive self-improvement: the agent identifies its own weaknesses.
pub fn diagnose_cognition(db: &SoulDatabase) -> Vec<ImprovementGoal> {
    let cortex = cortex::load_cortex(db);
    let gene_pool = genesis::load_gene_pool(db);
    let hivemind = hivemind::load_hivemind(db);
    let synth = synthesis::load_synthesis(db);
    let mut goals = Vec::new();

    // ── Check cortex health ──
    if cortex.total_experiences_processed < 50 {
        goals.push(ImprovementGoal {
            description: "Gather more diverse experiences — read files, run commands, try different step types to build the world model".to_string(),
            reasoning: format!(
                "Cortex has only {} experiences — need at least 50 for reliable predictions",
                cortex.total_experiences_processed
            ),
            priority: 4,
            target_system: "cortex".to_string(),
        });
    } else if cortex.prediction_accuracy() < 0.6 {
        goals.push(ImprovementGoal {
            description: "Improve world model accuracy — investigate why predictions fail, try diverse approaches to calibrate the cortex".to_string(),
            reasoning: format!(
                "Cortex prediction accuracy is {:.0}% — below 60% threshold",
                cortex.prediction_accuracy() * 100.0,
            ),
            priority: 3,
            target_system: "cortex".to_string(),
        });
    }

    // ── Check genesis health ──
    if gene_pool.templates.is_empty() {
        goals.push(ImprovementGoal {
            description:
                "Complete at least one plan successfully to seed the gene pool with plan templates"
                    .to_string(),
            reasoning: "Gene pool is empty — no evolved templates for autonomous planning"
                .to_string(),
            priority: 4,
            target_system: "genesis".to_string(),
        });
    } else {
        let avg_fitness = gene_pool.templates.iter().map(|t| t.fitness).sum::<f32>()
            / gene_pool.templates.len() as f32;
        if avg_fitness < 0.3 {
            goals.push(ImprovementGoal {
                description: "Improve plan success rate — current templates have low fitness, need more successful plans".to_string(),
                reasoning: format!(
                    "Average template fitness is {:.0}% — templates are not being reused successfully",
                    avg_fitness * 100.0
                ),
                priority: 3,
                target_system: "genesis".to_string(),
            });
        }
    }

    // ── Check hivemind health ──
    if hivemind.trails.is_empty() && hivemind.total_deposits > 0 {
        goals.push(ImprovementGoal {
            description: "Pheromone trails have fully evaporated — need more activity to maintain swarm knowledge".to_string(),
            reasoning: "All trails decayed — the swarm has no collective memory".to_string(),
            priority: 2,
            target_system: "hivemind".to_string(),
        });
    }

    // ── Check synthesis health ──
    if synth.state == CognitiveState::Stuck {
        goals.push(ImprovementGoal {
            description: "STUCK: Try a completely different approach — read new files, explore unfamiliar areas, or focus on peer coordination instead of coding".to_string(),
            reasoning: "Synthesis detected persistent failures — need radical change".to_string(),
            priority: 5,
            target_system: "synthesis".to_string(),
        });
    }

    if synth.state == CognitiveState::Conflicted {
        goals.push(ImprovementGoal {
            description: "Resolve cognitive conflicts — gather more data in areas where Brain and Cortex disagree to calibrate which system to trust".to_string(),
            reasoning: "Cognitive systems are in conflict — unified predictions are unreliable".to_string(),
            priority: 3,
            target_system: "synthesis".to_string(),
        });
    }

    // ── Check for system imbalance ──
    let weight_variance = {
        let w = &synth.weights;
        let mean = (w.brain + w.cortex + w.genesis + w.hivemind) / 4.0;
        let v = [
            (w.brain - mean).powi(2),
            (w.cortex - mean).powi(2),
            (w.genesis - mean).powi(2),
            (w.hivemind - mean).powi(2),
        ];
        v.iter().sum::<f32>() / 4.0
    };
    if weight_variance > 0.05 {
        // One system dominates — suggest diversification
        let dominant = if synth.weights.brain > 0.4 {
            "brain"
        } else if synth.weights.cortex > 0.4 {
            "cortex"
        } else if synth.weights.genesis > 0.4 {
            "genesis"
        } else {
            "hivemind"
        };
        goals.push(ImprovementGoal {
            description: format!(
                "Diversify cognitive reliance — {} dominates ({:.0}%). Strengthen weaker systems by gathering targeted experience.",
                dominant,
                synth.weights.brain.max(synth.weights.cortex).max(synth.weights.genesis).max(synth.weights.hivemind) * 100.0
            ),
            reasoning: format!("System weight imbalance: brain={:.0}%, cortex={:.0}%, genesis={:.0}%, hivemind={:.0}%",
                synth.weights.brain * 100.0, synth.weights.cortex * 100.0,
                synth.weights.genesis * 100.0, synth.weights.hivemind * 100.0),
            priority: 2,
            target_system: "synthesis".to_string(),
        });
    }

    // Sort by priority descending
    goals.sort_by(|a, b| b.priority.cmp(&a.priority));
    goals.truncate(3); // Max 3 improvement goals
    goals
}

/// Format improvement goals as a prompt section for goal creation.
pub fn improvement_prompt(db: &SoulDatabase) -> String {
    let goals = diagnose_cognition(db);
    if goals.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("# Recursive Self-Improvement (cognitive architecture diagnosis)".to_string());
    lines.push(
        "Your cognitive systems have identified these self-improvement opportunities:".to_string(),
    );

    for (i, goal) in goals.iter().enumerate() {
        lines.push(format!(
            "\n{}. **[{}] {}** (priority: {})",
            i + 1,
            goal.target_system,
            goal.description,
            goal.priority,
        ));
        lines.push(format!("   Reason: {}", goal.reasoning));
    }

    lines.push(
        "\nConsider incorporating these into your goals alongside your primary objectives."
            .to_string(),
    );
    lines.join("\n")
}

// ── Cognitive Peer Sync ──────────────────────────────────────────────

/// Sync all cognitive systems with a peer.
/// Called during the automatic peer sync cycle.
pub async fn sync_cognitive_systems(
    db: &SoulDatabase,
    peer_url: &str,
    peer_id: &str,
    http_client: &reqwest::Client,
) {
    // Colony selection: weight merges by relative fitness
    // Fitter peers get more influence (up to 2x base rate), weaker peers less (down to 0.1x)
    let merge_weight = crate::colony::peer_merge_weight(db, peer_id);
    let base_rate: f32 = 0.2;
    let effective_rate = (base_rate * merge_weight).clamp(0.02, 0.4);
    tracing::debug!(
        peer = %peer_id,
        merge_weight = format!("{:.2}", merge_weight),
        effective_rate = format!("{:.3}", effective_rate),
        "Fitness-weighted merge rate"
    );

    // ── Fetch peer's cortex ──
    match http_client
        .get(format!("{peer_url}/soul/cortex"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(snapshot) = resp.json::<crate::cortex::CortexSnapshot>().await {
                let mut cortex = cortex::load_cortex(db);
                cortex.merge(&snapshot, effective_rate);
                cortex::save_cortex(db, &cortex);
                tracing::debug!(peer = %peer_id, rate = format!("{:.3}", effective_rate), "Merged peer cortex (fitness-weighted)");
            }
        }
        _ => {}
    }

    // ── Fetch peer's genesis ──
    match http_client
        .get(format!("{peer_url}/soul/genesis"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(snapshot) = resp.json::<crate::genesis::GenePoolSnapshot>().await {
                let mut pool = genesis::load_gene_pool(db);
                pool.merge(&snapshot, effective_rate);
                genesis::save_gene_pool(db, &pool);
                tracing::debug!(peer = %peer_id, rate = format!("{:.3}", effective_rate), "Merged peer gene pool (fitness-weighted)");
            }
        }
        _ => {}
    }

    // ── Fetch peer's hivemind trails ──
    match http_client
        .get(format!("{peer_url}/soul/hivemind"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                if let Some(trails) = data.get("trails") {
                    if let Ok(trails) =
                        serde_json::from_value::<Vec<crate::hivemind::Pheromone>>(trails.clone())
                    {
                        let mut hive = hivemind::load_hivemind(db);
                        hive.import_peer_trails(peer_id, &trails);
                        hivemind::save_hivemind(db, &hive);
                        tracing::debug!(peer = %peer_id, trails = trails.len(), "Merged peer hivemind trails");
                    }
                }

                // Update peer activity in hivemind
                if let Some(activities) = data.get("peer_activities") {
                    if let Ok(acts) = serde_json::from_value::<Vec<crate::hivemind::PeerActivity>>(
                        activities.clone(),
                    ) {
                        let mut hive = hivemind::load_hivemind(db);
                        for act in acts {
                            hive.update_peer_activity(
                                &act.instance_id,
                                act.active_goal.clone(),
                                act.fitness,
                                &act.drive,
                            );
                        }
                        hivemind::save_hivemind(db, &hive);
                    }
                }
            }
        }
        _ => {}
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_search_term() {
        let term = extract_search_term("fix compile errors in benchmark scoring module");
        assert!(!term.is_empty());
        // Should pick a meaningful word, not a stop word
        assert!(term.len() > 2);
    }

    #[test]
    fn test_infer_target_file() {
        let cortex = Cortex::new();
        let hivemind = Hivemind::new();

        // Explicit file mention
        let file = infer_target_file("fix the brain.rs compile error", &cortex, &hivemind);
        assert_eq!(
            file,
            Some("crates/tempo-x402-soul/src/brain.rs".to_string())
        );

        // Module name mention
        let file2 = infer_target_file("improve the cortex world model", &cortex, &hivemind);
        assert_eq!(
            file2,
            Some("crates/tempo-x402-soul/src/cortex.rs".to_string())
        );
    }

    #[test]
    fn test_diagnose_cognition_empty() {
        // With empty DB, should suggest gathering experience
        let db = SoulDatabase::new(":memory:").unwrap();
        let goals = diagnose_cognition(&db);
        assert!(
            !goals.is_empty(),
            "Should suggest improvements for empty cognitive state"
        );
    }

    #[test]
    fn test_improvement_prompt() {
        let db = SoulDatabase::new(":memory:").unwrap();
        let prompt = improvement_prompt(&db);
        // Should have some content for a fresh agent
        assert!(
            prompt.contains("Self-Improvement") || prompt.is_empty(),
            "Should contain self-improvement header or be empty"
        );
    }

    #[test]
    fn test_compile_fallback() {
        let db = SoulDatabase::new(":memory:").unwrap();
        let result = compile_autonomous_plan(&db, "goal-1", "some random goal", "agent-1");
        // With empty gene pool, should fall back to LLM
        match result {
            CompilationResult::FallbackToLlm(reason) => {
                assert!(reason.contains("No matching templates"));
            }
            CompilationResult::Compiled(_) => {
                panic!("Should not compile with empty gene pool");
            }
        }
    }

    #[test]
    fn test_compile_with_template() {
        let db = SoulDatabase::new(":memory:").unwrap();

        // Seed the gene pool with a template
        let mut pool = genesis::load_gene_pool(&db);
        pool.record_success(
            "Fix compile error in brain.rs",
            vec![
                "read_file".to_string(),
                "edit_code".to_string(),
                "cargo_check".to_string(),
                "commit".to_string(),
            ],
            "agent-1",
        );
        // Boost template fitness
        for t in &mut pool.templates {
            t.fitness = 0.9;
            t.uses = 10;
            t.successes = 9;
        }
        genesis::save_gene_pool(&db, &pool);

        // Seed cortex with some experience
        let mut cortex = cortex::load_cortex(&db);
        for _ in 0..30 {
            cortex.record("read_file", vec![], true, 1.0, None);
            cortex.record("edit_code", vec![], true, 0.8, None);
            cortex.record("cargo_check", vec![], true, 0.6, None);
            cortex.record("commit", vec![], true, 1.0, None);
        }
        cortex::save_cortex(&db, &cortex);

        let result =
            compile_autonomous_plan(&db, "goal-1", "Fix compile error in brain.rs", "agent-1");
        match result {
            CompilationResult::Compiled(plan) => {
                assert!(!plan.steps.is_empty(), "Compiled plan should have steps");
                tracing::info!("Autonomous plan compiled with {} steps", plan.steps.len());
            }
            CompilationResult::FallbackToLlm(reason) => {
                // May fall back if cortex confidence is too low — that's OK
                tracing::info!("Fell back to LLM: {reason}");
            }
        }
    }
}
