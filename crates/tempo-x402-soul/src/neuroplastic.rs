//! Neuroplastic memory: salience scoring and tiered memory with decay.
//!
//! Two neuroscience-inspired systems:
//! 1. **Salience** — not all thoughts matter equally. Novelty, reward, and reinforcement
//!    determine how important a thought is.
//! 2. **Tiered memory** — sensory (fast decay), working (moderate), long-term (near-permanent).
//!    High-salience sensory memories get promoted to working memory.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::memory::ThoughtType;
use crate::observer::NodeSnapshot;

/// Memory tier with characteristic decay rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// Fast-decaying: ~2 cycles effective lifespan. Raw observations.
    Sensory,
    /// Moderate decay: ~90 cycles. Active reasoning and decisions.
    Working,
    /// Near-permanent: ~900 cycles. Consolidated insights, high-salience decisions.
    LongTerm,
}

impl MemoryTier {
    /// Decay multiplier applied to strength each cycle.
    pub fn decay_rate(&self) -> f64 {
        match self {
            Self::Sensory => 0.3,
            Self::Working => 0.95,
            Self::LongTerm => 0.995,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sensory => "sensory",
            Self::Working => "working",
            Self::LongTerm => "long_term",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sensory" => Some(Self::Sensory),
            "working" => Some(Self::Working),
            "long_term" => Some(Self::LongTerm),
            _ => None,
        }
    }
}

/// Per-endpoint reward attribution: what changed and where.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardBreakdown {
    /// Total reward signal (0.0..=0.8).
    pub total_reward: f64,
    /// Endpoints that appeared since last snapshot.
    pub new_endpoints: Vec<String>,
    /// Endpoints that gained new payments since last snapshot.
    pub growing_endpoints: Vec<String>,
    /// Endpoints with zero payments (have never earned).
    pub stagnant_endpoints: Vec<String>,
}

/// Compute per-endpoint reward signal by diffing two snapshots.
///
/// - New endpoint → +0.3 reward
/// - Endpoint with new payments → +0.5 reward (split across all growing)
/// - Endpoint with zero payments → stagnant (no reward, but tracked)
/// - Total capped at 0.8
pub fn compute_reward_signal(
    snapshot: &NodeSnapshot,
    prev_snapshot: Option<&NodeSnapshot>,
) -> RewardBreakdown {
    let mut breakdown = RewardBreakdown {
        total_reward: 0.0,
        new_endpoints: vec![],
        growing_endpoints: vec![],
        stagnant_endpoints: vec![],
    };

    let Some(prev) = prev_snapshot else {
        // No previous snapshot — classify current endpoints but no reward
        for ep in &snapshot.endpoints {
            if ep.payment_count == 0 {
                breakdown.stagnant_endpoints.push(ep.slug.clone());
            }
        }
        return breakdown;
    };

    // Build lookup of previous endpoints by slug
    let prev_map: std::collections::HashMap<&str, &crate::observer::EndpointSummary> = prev
        .endpoints
        .iter()
        .map(|ep| (ep.slug.as_str(), ep))
        .collect();

    for ep in &snapshot.endpoints {
        match prev_map.get(ep.slug.as_str()) {
            None => {
                // New endpoint
                breakdown.new_endpoints.push(ep.slug.clone());
            }
            Some(prev_ep) => {
                if ep.payment_count > prev_ep.payment_count {
                    breakdown.growing_endpoints.push(ep.slug.clone());
                } else if ep.payment_count == 0 {
                    breakdown.stagnant_endpoints.push(ep.slug.clone());
                }
            }
        }
    }

    // Score: new endpoints contribute 0.3 each (capped), growing contribute 0.5 total
    let new_reward = (breakdown.new_endpoints.len() as f64 * 0.3).min(0.6);
    let growing_reward = if breakdown.growing_endpoints.is_empty() {
        0.0
    } else {
        0.5
    };

    breakdown.total_reward = (new_reward + growing_reward).min(0.8);
    breakdown
}

/// Breakdown of salience factors for a thought.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceFactors {
    /// How novel is this content? (0.0 = seen many times, 1.0 = never seen)
    pub novelty: f64,
    /// Was there a positive change in payments/revenue? (0.0 = no, up to 0.8)
    pub reward_signal: f64,
    /// Constant small boost for being recent (0.1).
    pub recency_boost: f64,
    /// How often has this pattern been seen before? Logarithmic reinforcement.
    pub reinforcement: f64,
}

/// Compute salience score and factor breakdown for a thought.
///
/// Weights: novelty 40%, reward_signal 35%, recency 12%, reinforcement 13%.
pub fn compute_salience(
    _thought_type: &ThoughtType,
    content: &str,
    snapshot: &NodeSnapshot,
    prev_snapshot: Option<&NodeSnapshot>,
    pattern_counts: &HashMap<String, u64>,
) -> (f64, SalienceFactors) {
    let fp = content_fingerprint(content);
    let count = pattern_counts.get(&fp).copied().unwrap_or(0);

    // Novelty: never seen = 1.0, otherwise diminishing
    let novelty = if count == 0 {
        1.0
    } else {
        (1.0 / (count as f64 + 1.0)).min(0.5)
    };

    // Reward signal: per-endpoint attribution
    let reward_breakdown = compute_reward_signal(snapshot, prev_snapshot);
    let reward = reward_breakdown.total_reward;

    // Recency: constant small boost
    let recency = 0.1;

    // Reinforcement: logarithmic growth for repeated patterns
    let reinforcement = if count > 1 {
        (0.1 * (count as f64).ln()).min(0.5)
    } else {
        0.0
    };

    let salience =
        (novelty * 0.4 + reward * 0.35 + recency * 0.12 + reinforcement * 0.13).clamp(0.0, 1.0);

    let factors = SalienceFactors {
        novelty,
        reward_signal: reward,
        recency_boost: recency,
        reinforcement,
    };

    (salience, factors)
}

/// Determine the initial memory tier for a thought based on type and salience.
pub fn initial_tier(thought_type: &ThoughtType, salience: f64) -> MemoryTier {
    match thought_type {
        ThoughtType::Observation => MemoryTier::Sensory,
        ThoughtType::Reasoning | ThoughtType::Decision | ThoughtType::Reflection => {
            if salience > 0.7 {
                MemoryTier::LongTerm
            } else {
                MemoryTier::Working
            }
        }
        ThoughtType::MemoryConsolidation => MemoryTier::LongTerm,
        // Everything else (tool executions, chat, mutations) → working
        _ => MemoryTier::Working,
    }
}

/// Content fingerprint: first 60 chars lowercased and trimmed, for pattern matching.
pub fn content_fingerprint(content: &str) -> String {
    content.trim().to_lowercase().chars().take(60).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot(payments: u64, revenue: &str, endpoints: u32, children: u32) -> NodeSnapshot {
        NodeSnapshot {
            uptime_secs: 3600,
            endpoint_count: endpoints,
            total_revenue: revenue.to_string(),
            total_payments: payments,
            children_count: children,
            wallet_address: None,
            instance_id: None,
            generation: 0,
            endpoints: vec![],
            peers: vec![],
        }
    }

    #[test]
    fn test_content_fingerprint() {
        let fp = content_fingerprint(
            "  Hello World, this is a test of the fingerprinting system that should truncate  ",
        );
        assert!(fp.len() <= 60);
        assert!(fp.starts_with("hello world"));
    }

    #[test]
    fn test_compute_salience_novel() {
        let snap = test_snapshot(10, "100.0", 3, 0);
        let pattern_counts = HashMap::new();
        let (salience, factors) = compute_salience(
            &ThoughtType::Observation,
            "brand new observation",
            &snap,
            None,
            &pattern_counts,
        );
        assert!(salience > 0.0);
        assert!((factors.novelty - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_salience_repeated() {
        let snap = test_snapshot(10, "100.0", 3, 0);
        let mut pattern_counts = HashMap::new();
        let fp = content_fingerprint("same observation again");
        pattern_counts.insert(fp, 5);
        let (_, factors) = compute_salience(
            &ThoughtType::Observation,
            "same observation again",
            &snap,
            None,
            &pattern_counts,
        );
        assert!(factors.novelty < 0.5);
        assert!(factors.reinforcement > 0.0);
    }

    #[test]
    fn test_compute_salience_with_reward() {
        let prev = NodeSnapshot {
            endpoints: vec![test_endpoint("weather", 10, "100.0")],
            ..test_snapshot(10, "100.0", 1, 0)
        };
        let snap = NodeSnapshot {
            endpoints: vec![test_endpoint("weather", 15, "150.0")],
            ..test_snapshot(15, "150.0", 1, 0)
        };
        let (salience, factors) = compute_salience(
            &ThoughtType::Observation,
            "new observation",
            &snap,
            Some(&prev),
            &HashMap::new(),
        );
        assert!(factors.reward_signal > 0.0);
        assert!(salience > 0.3); // novelty + reward
    }

    #[test]
    fn test_initial_tier() {
        assert_eq!(
            initial_tier(&ThoughtType::Observation, 0.5),
            MemoryTier::Sensory
        );
        assert_eq!(
            initial_tier(&ThoughtType::Reasoning, 0.3),
            MemoryTier::Working
        );
        assert_eq!(
            initial_tier(&ThoughtType::Decision, 0.8),
            MemoryTier::LongTerm
        );
        assert_eq!(
            initial_tier(&ThoughtType::MemoryConsolidation, 0.1),
            MemoryTier::LongTerm
        );
    }

    #[test]
    fn test_memory_tier_decay_rates() {
        assert!((MemoryTier::Sensory.decay_rate() - 0.3).abs() < f64::EPSILON);
        assert!((MemoryTier::Working.decay_rate() - 0.95).abs() < f64::EPSILON);
        assert!((MemoryTier::LongTerm.decay_rate() - 0.995).abs() < f64::EPSILON);
    }

    fn test_endpoint(slug: &str, payments: i64, revenue: &str) -> crate::observer::EndpointSummary {
        crate::observer::EndpointSummary {
            slug: slug.to_string(),
            price: "$0.01".to_string(),
            description: None,
            request_count: payments * 2,
            payment_count: payments,
            revenue: revenue.to_string(),
        }
    }

    #[test]
    fn test_compute_reward_signal_no_prev() {
        let snap = NodeSnapshot {
            endpoints: vec![
                test_endpoint("weather", 0, "0"),
                test_endpoint("translate", 5, "0.05"),
            ],
            ..test_snapshot(5, "0.05", 2, 0)
        };
        let breakdown = compute_reward_signal(&snap, None);
        assert!((breakdown.total_reward - 0.0).abs() < f64::EPSILON);
        assert_eq!(breakdown.stagnant_endpoints, vec!["weather"]);
    }

    #[test]
    fn test_compute_reward_signal_new_endpoint() {
        let prev = NodeSnapshot {
            endpoints: vec![test_endpoint("weather", 5, "0.05")],
            ..test_snapshot(5, "0.05", 1, 0)
        };
        let snap = NodeSnapshot {
            endpoints: vec![
                test_endpoint("weather", 5, "0.05"),
                test_endpoint("translate", 0, "0"),
            ],
            ..test_snapshot(5, "0.05", 2, 0)
        };
        let breakdown = compute_reward_signal(&snap, Some(&prev));
        assert_eq!(breakdown.new_endpoints, vec!["translate"]);
        assert!((breakdown.total_reward - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_reward_signal_growing_endpoint() {
        let prev = NodeSnapshot {
            endpoints: vec![test_endpoint("weather", 5, "0.05")],
            ..test_snapshot(5, "0.05", 1, 0)
        };
        let snap = NodeSnapshot {
            endpoints: vec![test_endpoint("weather", 10, "0.10")],
            ..test_snapshot(10, "0.10", 1, 0)
        };
        let breakdown = compute_reward_signal(&snap, Some(&prev));
        assert_eq!(breakdown.growing_endpoints, vec!["weather"]);
        assert!((breakdown.total_reward - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_reward_signal_capped() {
        let prev = NodeSnapshot {
            endpoints: vec![],
            ..test_snapshot(0, "0", 0, 0)
        };
        // 3 new endpoints (3 * 0.3 = 0.9 but capped at 0.6) + no growing = 0.6
        let snap = NodeSnapshot {
            endpoints: vec![
                test_endpoint("a", 0, "0"),
                test_endpoint("b", 0, "0"),
                test_endpoint("c", 0, "0"),
            ],
            ..test_snapshot(0, "0", 3, 0)
        };
        let breakdown = compute_reward_signal(&snap, Some(&prev));
        assert_eq!(breakdown.new_endpoints.len(), 3);
        assert!(breakdown.total_reward <= 0.8);
    }
}
