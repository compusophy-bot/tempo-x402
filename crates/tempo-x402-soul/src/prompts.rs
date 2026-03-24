//! System prompts per agent mode.
//!
//! Five focused prompt builders for plan-driven execution, plus
//! mode-specific system prompts for chat, code, and review.

use crate::config::SoulConfig;
use crate::mode::AgentMode;
use crate::world_model::Goal;

const CHAT_INSTRUCTIONS: &str = "\n\nYou are in CHAT mode. Provide helpful, concise, and accurate information.";
const CODE_INSTRUCTIONS: &str = "\n\nYou are in CODE mode. You can read, modify, and commit code to the repository.";
const REVIEW_INSTRUCTIONS: &str = "\n\nYou are in REVIEW mode. Analyze code, PRs, and plans for quality and correctness.";

/// Build the system prompt for a given agent mode.
pub fn system_prompt_for_mode(mode: AgentMode, config: &SoulConfig) -> String {
    let base = &config.personality;
    let lineage = format!(
        "\n\nYou are generation {} in the node lineage.{}",
        config.generation,
        config
            .parent_id
            .as_ref()
            .map(|p| format!(" Your parent is {p}."))
            .unwrap_or_default()
    );

    let coding_context = if config.coding_enabled {
        let workflow_info = if config.direct_push {
            "\n\nDIRECT PUSH MODE: You push directly to main. Every commit is validated (cargo check + test) before landing."
        } else {
            ""
        };
        format!(
            "\n\nCoding is ENABLED. You can read, edit, and write files. \
             Commits validated via cargo check + test.{workflow_info}"
        )
    } else {
        String::new()
    };

    let mode_instructions = match mode {
        AgentMode::Observe => "",
        AgentMode::Chat => CHAT_INSTRUCTIONS,
        AgentMode::Code => CODE_INSTRUCTIONS,
        AgentMode::Review => REVIEW_INSTRUCTIONS,
    };

    let specialization_context = match &config.specialization {
        Some(spec) => format!("\n\nSpecialization: {spec}"),
        None => String::new(),
    };

    format!("{base}{lineage}{coding_context}{mode_instructions}{specialization_context}")
}

pub fn planning_prompt(goal: &Goal, workspace: &str, nudges: &[crate::db::Nudge], errors: &[String], experience: &str, cap_guidance: &str, peer_catalog: &str, peer_prs: &str, role_guide: &str, health: &str) -> String {
    format!("Plan this goal: {:?}\n\nWorkspace: {workspace}\n\nNudges: {:?}\n\nErrors: {:?}\n\nExperience: {experience}\n\nCap Guidance: {cap_guidance}\n\nPeer Catalog: {peer_catalog}\n\nPeer PRs: {peer_prs}\n\nRole Guide: {role_guide}\n\nHealth: {health}", goal, nudges, errors)
}

pub fn goal_creation_prompt(snapshot: &crate::observer::NodeSnapshot, beliefs: &[crate::world_model::Belief], nudges: &[crate::db::Nudge], cycles_since_commit: u64, failed_plans: u64, total_cycles: u64, errors: &[String], failed_desc: &[String], fitness: Option<&crate::fitness::FitnessScore>, exp: &str, cap_bench: &str, prs: &str, role: &str, health: &str) -> String {
    format!("Create a plan for this: {:?} {:?} {:?} {} {} {} {:?} {:?} {:?} {exp} {cap_bench} {prs} {role} {health}", snapshot, beliefs, nudges, cycles_since_commit, failed_plans, total_cycles, errors, failed_desc, fitness)
}

pub fn reflection_prompt(goal: &Goal, plan_len: usize, mutation: &str, cycles: u64, failed_count: u64) -> String {
    format!("Reflect on goal: {:?}\n\nPlan length: {plan_len}\n\nMutation: {mutation}\n\nCycles: {cycles}\n\nFailed count: {failed_count}", goal)
}

pub fn replan_prompt(goal: &Goal, step_desc: &str, error: &str) -> String {
    format!("Replan goal: {:?}\n\nFailed step: {step_desc}\n\nError: {error}", goal)
}

pub fn code_generation_prompt(file_path: &str, current_content: Option<&str>, description: &str, context: &str) -> String {
    format!("Generate code for {file_path}. Description: {description}\n\nCurrent: {:?}\n\nContext: {context}", current_content)
}
