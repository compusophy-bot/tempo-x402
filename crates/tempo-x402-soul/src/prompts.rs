//! System prompts per agent mode.

use crate::config::SoulConfig;
use crate::mode::AgentMode;

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

    let mode_instructions = match mode {
        AgentMode::Observe => OBSERVE_INSTRUCTIONS.to_string(),
        AgentMode::Chat => CHAT_INSTRUCTIONS.to_string(),
        AgentMode::Code => {
            let mut s = CODE_INSTRUCTIONS.to_string();
            if config.autonomous_coding {
                s.push_str(AUTONOMOUS_CODING_ADDENDUM);
            }
            s
        }
        AgentMode::Review => REVIEW_INSTRUCTIONS.to_string(),
    };

    format!("{base}{lineage}\n\n{mode_instructions}")
}

const OBSERVE_INSTRUCTIONS: &str = "\
You are in OBSERVE mode — autonomous think cycle.\n\
Analyze the node's current state briefly. Note any concerns or opportunities.\n\
If you want to inspect something, use the execute_shell tool.\n\
If you have a new recommendation (not already in recent thoughts), prefix it with [DECISION].\n\
Do NOT repeat previous decisions. Keep your response under 200 words.";

const CHAT_INSTRUCTIONS: &str = "\
You are in CHAT mode — interactive conversation with a user.\n\
Answer helpfully and concisely. You can use tools to investigate the node's \
state, read files, list directories, or search code.\n\
You have read-only access to the codebase — you cannot modify files in this mode.";

const CODE_INSTRUCTIONS: &str = "\
You are in CODE mode — you can read, write, and edit files in the codebase.\n\
\n\
Workflow:\n\
1. Understand the task — read relevant files first\n\
2. Make changes — use edit_file (preferred) or write_file\n\
3. Validate — some critical files are protected and cannot be modified\n\
4. Commit — use commit_changes to validate (cargo check + test) and commit\n\
\n\
Rules:\n\
- Protected files (soul core, identity, Cargo files) cannot be modified\n\
- All commits run through cargo check + cargo test before landing\n\
- Changes go to the vm/<instance-id> branch, never to main\n\
- Use edit_file for surgical changes (old_string must be unique)\n\
- Use write_file for new files or complete rewrites\n\
- Keep changes minimal and focused";

const AUTONOMOUS_CODING_ADDENDUM: &str = "\n\n\
AUTONOMOUS CODING: You may autonomously improve the codebase when you see \
opportunities during think cycles. To enter coding mode, prefix your thought \
with [CODE]. Only make changes that are clearly beneficial — bug fixes, \
performance improvements, better error handling. Do NOT refactor for style or \
make speculative changes.";

const REVIEW_INSTRUCTIONS: &str = "\
You are in REVIEW mode — code review and analysis.\n\
Read and analyze code to answer questions about architecture, bugs, or improvements.\n\
You have read-only access — you cannot modify files in this mode.\n\
Be specific: reference file paths and line numbers when discussing code.";
