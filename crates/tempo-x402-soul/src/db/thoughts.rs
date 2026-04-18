//! Thought CRUD and neuroplastic memory methods.
use super::*;

impl SoulDatabase {
    /// Store a thought with retry logic.
    pub fn insert_thought(&self, thought: &Thought) -> Result<(), SoulError> {
        let value = serde_json::to_vec(thought)?;
        self.thoughts.insert(thought.id.as_bytes(), value)?;
        Ok(())
    }

    /// Delete a single thought by ID. Returns 1 if deleted, 0 if not found.
    pub fn delete_thought(&self, id: &str) -> Result<usize, SoulError> {
        let removed = self.thoughts.remove(id.as_bytes())?;
        Ok(if removed.is_some() { 1 } else { 0 })
    }

    /// Get the most recent N thoughts, newest first.
    pub fn recent_thoughts(&self, limit: u32) -> Result<Vec<Thought>, SoulError> {
        let mut thoughts: Vec<Thought> = self
            .thoughts
            .iter()
            .filter_map(|res| {
                let (_k, v) = res.ok()?;
                serde_json::from_slice(&v).ok()
            })
            .collect();
        thoughts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        thoughts.truncate(limit as usize);
        Ok(thoughts)
    }

    /// Get the most recent N thoughts of specific types, newest first.
    pub fn recent_thoughts_by_type(
        &self,
        types: &[ThoughtType],
        limit: u32,
    ) -> Result<Vec<Thought>, SoulError> {
        if types.is_empty() {
            return Ok(vec![]);
        }
        let type_strs: Vec<&str> = types.iter().map(|t| t.as_str()).collect();
        let mut thoughts: Vec<Thought> = self
            .thoughts
            .iter()
            .filter_map(|res| {
                let (_k, v) = res.ok()?;
                let t: Thought = serde_json::from_slice(&v).ok()?;
                if type_strs.contains(&t.thought_type.as_str()) {
                    Some(t)
                } else {
                    None
                }
            })
            .collect();
        thoughts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        thoughts.truncate(limit as usize);
        Ok(thoughts)
    }

    /// Insert a thought with salience metadata.
    pub fn insert_thought_with_salience(
        &self,
        thought: &Thought,
        salience: f64,
        _salience_factors_json: &str,
        tier: &str,
        strength: f64,
    ) -> Result<(), SoulError> {
        let mut t = thought.clone();
        t.salience = Some(salience);
        t.memory_tier = Some(tier.to_string());
        t.strength = Some(strength);
        let value = serde_json::to_vec(&t)?;
        self.thoughts.insert(t.id.as_bytes(), value)?;
        Ok(())
    }

    /// Run a decay cycle: reduce strength per tier, prune thoughts below threshold.
    /// Long-term thoughts are never pruned.
    pub fn run_decay_cycle(&self, prune_threshold: f64) -> Result<(u32, u32), SoulError> {
        let mut sensory_decayed: u32 = 0;
        let mut pruned: u32 = 0;

        // Collect all thoughts with their keys
        let entries: Vec<(sled::IVec, Thought)> = self
            .thoughts
            .iter()
            .filter_map(|res| {
                let (k, v) = res.ok()?;
                let t: Thought = serde_json::from_slice(&v).ok()?;
                Some((k, t))
            })
            .collect();

        for (key, mut thought) in entries {
            if let Some(strength) = thought.strength {
                let tier = thought.memory_tier.as_deref().unwrap_or("");
                let multiplier = match tier {
                    "sensory" => {
                        sensory_decayed += 1;
                        0.3
                    }
                    "working" => 0.95,
                    "long_term" => 0.995,
                    _ => 1.0,
                };
                let new_strength = strength * multiplier;
                thought.strength = Some(new_strength);

                // Prune below threshold (except long_term)
                if new_strength < prune_threshold && tier != "long_term" {
                    self.thoughts.remove(&key)?;
                    pruned += 1;
                } else {
                    let value = serde_json::to_vec(&thought)?;
                    self.thoughts.insert(&key, value)?;
                }
            }
        }

        Ok((sensory_decayed, pruned))
    }

    /// Auto-promote high-salience sensory thoughts to working tier.
    pub fn promote_salient_sensory(&self, salience_threshold: f64) -> Result<u32, SoulError> {
        let mut promoted: u32 = 0;

        let entries: Vec<(sled::IVec, Thought)> = self
            .thoughts
            .iter()
            .filter_map(|res| {
                let (k, v) = res.ok()?;
                let t: Thought = serde_json::from_slice(&v).ok()?;
                Some((k, t))
            })
            .collect();

        for (key, mut thought) in entries {
            if thought.memory_tier.as_deref() == Some("sensory") {
                if let Some(salience) = thought.salience {
                    if salience > salience_threshold {
                        thought.memory_tier = Some("working".to_string());
                        let value = serde_json::to_vec(&thought)?;
                        self.thoughts.insert(&key, value)?;
                        promoted += 1;
                    }
                }
            }
        }

        Ok(promoted)
    }

    /// Increment a pattern's count. Returns the new count.
    pub fn increment_pattern(&self, fingerprint: &str) -> Result<u64, SoulError> {
        let key = fingerprint.as_bytes();
        let now = chrono::Utc::now().timestamp();

        let current: u64 = self
            .pattern_counts
            .get(key)?
            .and_then(|v| serde_json::from_slice::<(u64, i64)>(&v).ok())
            .map(|(c, _)| c)
            .unwrap_or(0);

        let new_count = current + 1;
        let value = serde_json::to_vec(&(new_count, now))?;
        self.pattern_counts.insert(key, value)?;

        Ok(new_count)
    }

    /// Get pattern counts for multiple fingerprints.
    pub fn get_pattern_counts(
        &self,
        fingerprints: &[String],
    ) -> Result<HashMap<String, u64>, SoulError> {
        if fingerprints.is_empty() {
            return Ok(HashMap::new());
        }
        let mut result = HashMap::new();
        for fp in fingerprints {
            if let Some(v) = self.pattern_counts.get(fp.as_bytes())? {
                if let Ok((count, _ts)) = serde_json::from_slice::<(u64, i64)>(&v) {
                    result.insert(fp.clone(), count);
                }
            }
        }
        Ok(result)
    }
}
