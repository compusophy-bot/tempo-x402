//! Agent modes — route between observation, chat, coding, and review with
//! appropriate tools and prompts per mode.

use crate::llm::FunctionDeclaration;
use crate::tool_decl;

/// The operating mode of the soul agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    /// Autonomous think cycle — observe node state.
    Observe,
    /// Interactive chat without coding intent.
    Chat,
    /// Coding mode — can read, write, edit files and commit changes.
    Code,
    /// Code review mode — read-only analysis.
    Review,
}

impl AgentMode {
    /// Maximum tool calls allowed in this mode.
    pub fn max_tool_calls(&self) -> u32 {
        match self {
            Self::Observe => 5,
            Self::Chat => 15,
            Self::Code => 15,
            Self::Review => 5,
        }
    }

    /// Get the tool declarations available in this mode.
    ///
    /// `dynamic_tools` and `meta_tools` are appended based on mode:
    /// - Meta-tools (register/list/unregister) only in Code mode
    /// - Dynamic tools filtered by their mode_tags
    pub fn available_tools(
        &self,
        coding_enabled: bool,
        dynamic_tools: &[FunctionDeclaration],
        meta_tools: &[FunctionDeclaration],
    ) -> Vec<FunctionDeclaration> {
        let all = tool_decl::available_tools();
        let all_with_git = tool_decl::available_tools_with_git(coding_enabled);

        let mut result = match self {
            Self::Observe => {
                // Shell + read-only file tools + update_memory + update_beliefs + check_self + discover_peers
                let mut v: Vec<_> = all
                    .into_iter()
                    .filter(|t| {
                        matches!(
                            t.name.as_str(),
                            "execute_shell" | "read_file" | "list_directory" | "search_files"
                        )
                    })
                    .collect();
                v.push(tool_decl::update_memory_tool());
                v.push(tool_decl::update_beliefs_tool());
                v.push(tool_decl::check_self_tool());
                v.push(tool_decl::check_reputation_tool());
                v.push(tool_decl::discover_peers_tool());
                v.push(tool_decl::call_paid_endpoint_tool());
                v.push(tool_decl::delete_endpoint_tool());
                v.push(tool_decl::check_deploy_status_tool());
                v.push(tool_decl::get_deploy_logs_tool());
                v
            }
            Self::Chat => {
                // When coding is enabled, Chat mode gets the SAME tools as Code mode.
                // The mode detection is too fragile to reliably distinguish "add a /todo route"
                // from "what is a route?" — and denying tools when the user clearly wants
                // action is worse than giving tools when they just want conversation.
                // The agent's judgment (not keyword matching) decides whether to use them.
                if coding_enabled {
                    let mut v = all_with_git;
                    v.push(tool_decl::update_memory_tool());
                    v.push(tool_decl::update_beliefs_tool());
                    v.push(tool_decl::register_endpoint_tool());
                    v.push(tool_decl::delete_endpoint_tool());
                    v.push(tool_decl::check_self_tool());
                    v.push(tool_decl::check_reputation_tool());
                    v.push(tool_decl::approve_plan_tool());
                    v.push(tool_decl::reject_plan_tool());
                    v.push(tool_decl::request_plan_tool());
                    v.push(tool_decl::discover_peers_tool());
                    v.push(tool_decl::call_paid_endpoint_tool());
                    v.push(tool_decl::check_deploy_status_tool());
                    v
                } else {
                    // Read-only when coding is disabled
                    let mut v: Vec<_> = all
                        .into_iter()
                        .filter(|t| {
                            matches!(
                                t.name.as_str(),
                                "execute_shell" | "read_file" | "list_directory" | "search_files"
                            )
                        })
                        .collect();
                    v.push(tool_decl::update_memory_tool());
                    v.push(tool_decl::update_beliefs_tool());
                    v.push(tool_decl::check_self_tool());
                    v.push(tool_decl::check_reputation_tool());
                    v.push(tool_decl::approve_plan_tool());
                    v.push(tool_decl::reject_plan_tool());
                    v.push(tool_decl::request_plan_tool());
                    v.push(tool_decl::discover_peers_tool());
                    v.push(tool_decl::call_paid_endpoint_tool());
                    v
                }
            }
            Self::Code => {
                // All tools including write/edit/commit + update_memory + update_beliefs + register_endpoint + check_self + plan control + economy
                let mut v = all_with_git;
                v.push(tool_decl::update_memory_tool());
                v.push(tool_decl::update_beliefs_tool());
                v.push(tool_decl::register_endpoint_tool());
                v.push(tool_decl::delete_endpoint_tool());
                v.push(tool_decl::check_self_tool());
                v.push(tool_decl::check_reputation_tool());
                v.push(tool_decl::update_agent_metadata_tool());
                v.push(tool_decl::approve_plan_tool());
                v.push(tool_decl::reject_plan_tool());
                v.push(tool_decl::request_plan_tool());
                v.push(tool_decl::discover_peers_tool());
                v.push(tool_decl::call_paid_endpoint_tool());
                v.push(tool_decl::check_deploy_status_tool());
                v.push(tool_decl::get_deploy_logs_tool());
                v.push(tool_decl::trigger_redeploy_tool());
                v.push(tool_decl::spawn_specialist_tool());
                v.push(tool_decl::delegate_task_tool());
                v
            }
            Self::Review => {
                // Shell + read-only file tools
                all.into_iter()
                    .filter(|t| {
                        matches!(
                            t.name.as_str(),
                            "execute_shell" | "read_file" | "search_files"
                        )
                    })
                    .collect()
            }
        };

        // Append dynamic tools (already filtered by mode_tag by caller)
        result.extend(dynamic_tools.iter().cloned());

        // Meta-tools only in Code mode
        if *self == Self::Code {
            result.extend(meta_tools.iter().cloned());
        }

        result
    }

    /// Mode tag string for filtering dynamic tools.
    pub fn mode_tag(&self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Chat => "chat",
            Self::Code => "code",
            Self::Review => "review",
        }
    }
}

/// Detect the agent mode from a chat message.
pub fn detect_mode_from_message(message: &str, coding_enabled: bool) -> AgentMode {
    let lower = message.to_lowercase();

    // Explicit coding triggers
    if coding_enabled {
        let coding_keywords = [
            "write code",
            "edit the",
            "modify the",
            "change the code",
            "implement",
            "add a function",
            "fix the bug",
            "refactor",
            "create a file",
            "update the file",
            "commit",
            "[code]",
        ];
        if coding_keywords.iter().any(|k| lower.contains(k)) {
            return AgentMode::Code;
        }
    }

    // Review triggers
    let review_keywords = [
        "review",
        "code review",
        "look at the code",
        "analyze the code",
        "audit",
    ];
    if review_keywords.iter().any(|k| lower.contains(k)) {
        return AgentMode::Review;
    }

    AgentMode::Chat
}

impl std::fmt::Display for AgentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Observe => write!(f, "observe"),
            Self::Chat => write!(f, "chat"),
            Self::Code => write!(f, "code"),
            Self::Review => write!(f, "review"),
        }
    }
}
