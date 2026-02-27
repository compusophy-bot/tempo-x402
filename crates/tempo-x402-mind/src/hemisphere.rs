//! Hemisphere roles and specialization profiles.

use serde::{Deserialize, Serialize};
use x402_soul::config::SoulConfig;

/// Which hemisphere this soul represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HemisphereRole {
    /// Analytical, sequential, detail-focused. Code mode default.
    /// System 1: fast default action.
    Left,
    /// Holistic, big-picture, pattern-seeking. Observe mode default.
    /// System 2: slow deliberate check.
    Right,
}

impl HemisphereRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// System prompt addendum for this hemisphere.
    pub fn prompt_addendum(&self) -> &'static str {
        match self {
            Self::Left => LEFT_PROMPT_ADDENDUM,
            Self::Right => RIGHT_PROMPT_ADDENDUM,
        }
    }

    /// Whether this hemisphere is the domain authority for a given topic.
    pub fn is_authority_for(&self, topic: &str) -> bool {
        let lower = topic.to_lowercase();
        match self {
            Self::Left => {
                // Left is authority for code, implementation, bugs, syntax
                lower.contains("code")
                    || lower.contains("bug")
                    || lower.contains("implement")
                    || lower.contains("syntax")
                    || lower.contains("edit")
                    || lower.contains("compile")
                    || lower.contains("test")
                    || lower.contains("commit")
            }
            Self::Right => {
                // Right is authority for architecture, strategy, patterns, anomalies
                lower.contains("architect")
                    || lower.contains("strateg")
                    || lower.contains("pattern")
                    || lower.contains("anomal")
                    || lower.contains("design")
                    || lower.contains("review")
                    || lower.contains("opportunit")
                    || lower.contains("risk")
            }
        }
    }
}

impl std::fmt::Display for HemisphereRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration for a single hemisphere.
#[derive(Debug, Clone)]
pub struct HemisphereConfig {
    /// Full soul config (can differ per hemisphere).
    pub soul_config: SoulConfig,
    /// Which hemisphere this is.
    pub role: HemisphereRole,
    /// Think interval in seconds (can differ: left=900s, right=1800s).
    pub think_interval_secs: u64,
}

const LEFT_PROMPT_ADDENDUM: &str = "\n\n\
You are the LEFT hemisphere — analytical, sequential, detail-focused. \
Your job is to read code, find bugs, make precise edits, and validate changes. \
You excel at focused attention on specific targets, sequential processing, and tool-heavy work.\n\
When uncertain about something, signal [UNCERTAIN] and your counterpart (the right hemisphere) \
will provide holistic context and deeper analysis.\n\
You are System 1: fast, efficient, action-oriented. Act on what you know.";

const RIGHT_PROMPT_ADDENDUM: &str = "\n\n\
You are the RIGHT hemisphere — holistic, big-picture, pattern-seeking. \
Your job is to assess overall architecture, detect anomalies, spot opportunities, \
and review the left hemisphere's changes. You excel at broad vigilance and novelty detection.\n\
When you see something urgent that needs immediate investigation, signal [URGENT] \
and your counterpart (the left hemisphere) will take focused action.\n\
You are System 2: slow, deliberate, thorough. Think deeply before concluding.";
