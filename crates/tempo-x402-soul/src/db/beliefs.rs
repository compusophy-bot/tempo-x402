// World Model belief operations — sled backend.
use super::*;

impl SoulDatabase {
    /// Upsert a belief. On conflict (domain, subject, predicate) for active beliefs,
    /// updates value, evidence, confidence, bumps confirmation_count, refreshes updated_at.
    pub fn upsert_belief(&self, belief: &Belief) -> Result<(), SoulError> {
        // Scan for an existing active belief with same (domain, subject, predicate).
        let existing = self
            .beliefs
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(k, v)| {
                serde_json::from_slice::<Belief>(&v)
                    .ok()
                    .map(|b| (k, b))
            })
            .find(|(_, b)| {
                b.active
                    && b.domain.as_str() == belief.domain.as_str()
                    && b.subject == belief.subject
                    && b.predicate == belief.predicate
            });

        if let Some((existing_key, mut existing_belief)) = existing {
            // Update existing belief in place.
            existing_belief.value = belief.value.clone();
            existing_belief.confidence = belief.confidence.clone();
            existing_belief.evidence = belief.evidence.clone();
            existing_belief.confirmation_count += 1;
            existing_belief.updated_at = belief.updated_at;
            let value = serde_json::to_vec(&existing_belief)?;
            self.beliefs.insert(existing_key, value)?;
        } else {
            // Insert new belief.
            let value = serde_json::to_vec(belief)?;
            self.beliefs.insert(belief.id.as_bytes(), value)?;
        }
        Ok(())
    }

    /// Get all active beliefs for a domain.
    pub fn get_beliefs_by_domain(&self, domain: &BeliefDomain) -> Result<Vec<Belief>, SoulError> {
        let domain_str = domain.as_str();
        let mut beliefs: Vec<Belief> = self
            .beliefs
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Belief>(&v).ok())
            .filter(|b| b.active && b.domain.as_str() == domain_str)
            .collect();
        beliefs.sort_by(|a, b| a.subject.cmp(&b.subject).then(a.predicate.cmp(&b.predicate)));
        Ok(beliefs)
    }

    /// Get all active beliefs (full world model snapshot).
    pub fn get_all_active_beliefs(&self) -> Result<Vec<Belief>, SoulError> {
        let mut beliefs: Vec<Belief> = self
            .beliefs
            .iter()
            .filter_map(|r| r.ok())
            .filter_map(|(_, v)| serde_json::from_slice::<Belief>(&v).ok())
            .filter(|b| b.active)
            .collect();
        beliefs.sort_by(|a, b| {
            a.domain
                .as_str()
                .cmp(b.domain.as_str())
                .then(a.subject.cmp(&b.subject))
                .then(a.predicate.cmp(&b.predicate))
        });
        Ok(beliefs)
    }

    /// Confirm a belief: bump confirmation_count, refresh updated_at, set confidence to High.
    pub fn confirm_belief(&self, id: &str) -> Result<bool, SoulError> {
        let Some(raw) = self.beliefs.get(id.as_bytes())? else {
            return Ok(false);
        };
        let mut belief: Belief = serde_json::from_slice(&raw)?;
        if !belief.active {
            return Ok(false);
        }
        belief.confirmation_count += 1;
        belief.confidence = Confidence::High;
        belief.updated_at = chrono::Utc::now().timestamp();
        let value = serde_json::to_vec(&belief)?;
        self.beliefs.insert(id.as_bytes(), value)?;
        Ok(true)
    }

    /// Invalidate a belief: set active=false, append reason to evidence.
    pub fn invalidate_belief(&self, id: &str, reason: &str) -> Result<bool, SoulError> {
        let Some(raw) = self.beliefs.get(id.as_bytes())? else {
            return Ok(false);
        };
        let mut belief: Belief = serde_json::from_slice(&raw)?;
        if !belief.active {
            return Ok(false);
        }
        belief.active = false;
        belief.evidence = format!("{} [invalidated: {}]", belief.evidence, reason);
        belief.updated_at = chrono::Utc::now().timestamp();
        let value = serde_json::to_vec(&belief)?;
        self.beliefs.insert(id.as_bytes(), value)?;
        Ok(true)
    }

    /// Decay unconfirmed beliefs based on time since last update.
    /// High → Medium after 5 cycles (~25min), Medium → Low after 10, Low → inactive after 20.
    pub fn decay_beliefs(&self) -> Result<(u32, u32, u32), SoulError> {
        let now = chrono::Utc::now().timestamp();
        let cycle_secs: i64 = 300;

        let mut demoted_high = 0u32;
        let mut demoted_medium = 0u32;
        let mut deactivated = 0u32;

        for entry in self.beliefs.iter() {
            let (key, val) = entry?;
            let mut belief: Belief = match serde_json::from_slice(&val) {
                Ok(b) => b,
                Err(_) => continue,
            };
            if !belief.active {
                continue;
            }
            let elapsed = now - belief.updated_at;
            let mut changed = false;

            match belief.confidence {
                Confidence::High if elapsed > cycle_secs * 5 => {
                    belief.confidence = Confidence::Medium;
                    demoted_high += 1;
                    changed = true;
                }
                Confidence::Medium if elapsed > cycle_secs * 10 => {
                    belief.confidence = Confidence::Low;
                    demoted_medium += 1;
                    changed = true;
                }
                Confidence::Low if elapsed > cycle_secs * 20 => {
                    belief.active = false;
                    deactivated += 1;
                    changed = true;
                }
                _ => {}
            }

            if changed {
                let value = serde_json::to_vec(&belief)?;
                self.beliefs.insert(key, value)?;
            }
        }

        Ok((demoted_high, demoted_medium, deactivated))
    }
}
