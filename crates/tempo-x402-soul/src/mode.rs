//! Agent modes — route between observation, chat, coding, and review with
//! appropriate tools and prompts per mode.

use crate::llm::FunctionDeclaration;
use crate::tools;

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
            Self::Code => 50,
            Self::Review => 10,
        }
    }

    /// Get the tool declarations available in this mode.
    pub fn available_tools(&self, coding_enabled: bool) -> Vec<FunctionDeclaration> {
        let all = tools::available_tools();
        let all_with_git = tools::available_tools_with_git(coding_enabled);

        match self {
            Self::Observe => {
                // Only execute_shell
                all.into_iter()
                    .filter(|t| t.name == "execute_shell")
                    .collect()
            }
            Self::Chat => {
                // Shell + read-only file tools
                all.into_iter()
                    .filter(|t| {
                        matches!(
                            t.name.as_str(),
                            "execute_shell" | "read_file" | "list_directory" | "search_files"
                        )
                    })
                    .collect()
            }
            Self::Code => {
                // All tools including write/edit/commit
                all_with_git
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
