//! Thought types and structures for the soul's memory.

use serde::{Deserialize, Serialize};

/// The type of thought recorded by the soul.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThoughtType {
    /// Raw observation of node state.
    Observation,
    /// LLM reasoning about the current state.
    Reasoning,
    /// A suggested action (logged only, not executed in v1).
    Decision,
    /// Self-reflection on past thoughts or patterns.
    Reflection,
}

impl ThoughtType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Reasoning => "reasoning",
            Self::Decision => "decision",
            Self::Reflection => "reflection",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "observation" => Some(Self::Observation),
            "reasoning" => Some(Self::Reasoning),
            "decision" => Some(Self::Decision),
            "reflection" => Some(Self::Reflection),
            _ => None,
        }
    }
}

/// A single thought stored in the soul's memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thought {
    /// Unique identifier.
    pub id: String,
    /// The type of this thought.
    pub thought_type: ThoughtType,
    /// The content of the thought.
    pub content: String,
    /// Optional JSON context (e.g., the snapshot that triggered this thought).
    pub context: Option<String>,
    /// Unix timestamp when this thought was created.
    pub created_at: i64,
}
