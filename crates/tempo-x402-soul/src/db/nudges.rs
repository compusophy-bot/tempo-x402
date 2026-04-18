// Nudge queue operations (insert, fetch unprocessed, mark processed).
use super::*;

impl SoulDatabase {
    pub fn insert_nudge(
        &self,
        source: &str,
        content: &str,
        priority: u32,
    ) -> Result<String, SoulError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let nudge = Nudge {
            id: id.clone(),
            source: source.to_string(),
            content: content.to_string(),
            priority,
            created_at: now,
            processed_at: None,
            active: true,
        };
        let value = serde_json::to_vec(&nudge)?;
        self.nudges.insert(id.as_bytes(), value)?;
        Ok(id)
    }

    /// Get unprocessed nudges, ordered by priority DESC then created_at ASC.
    pub fn get_unprocessed_nudges(&self, limit: u32) -> Result<Vec<Nudge>, SoulError> {
        let mut nudges: Vec<Nudge> = self
            .nudges
            .iter()
            .filter_map(|res| {
                let (_k, v) = res.ok()?;
                let n: Nudge = serde_json::from_slice(&v).ok()?;
                if n.active && n.processed_at.is_none() {
                    Some(n)
                } else {
                    None
                }
            })
            .collect();
        nudges.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        nudges.truncate(limit as usize);
        Ok(nudges)
    }

    /// Mark a nudge as processed.
    pub fn mark_nudge_processed(&self, id: &str) -> Result<(), SoulError> {
        if let Some(v) = self.nudges.get(id.as_bytes())? {
            let mut nudge: Nudge = serde_json::from_slice(&v)?;
            nudge.processed_at = Some(chrono::Utc::now().timestamp());
            let value = serde_json::to_vec(&nudge)?;
            self.nudges.insert(id.as_bytes(), value)?;
        }
        Ok(())
    }
}
