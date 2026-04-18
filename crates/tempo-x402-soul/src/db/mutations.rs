//! Mutation CRUD methods.
use super::*;

impl SoulDatabase {
    /// Record a mutation (code change attempt).
    pub fn insert_mutation(&self, mutation: &Mutation) -> Result<(), SoulError> {
        let value = serde_json::to_vec(mutation)?;
        self.mutations.insert(mutation.id.as_bytes(), value)?;
        Ok(())
    }

    /// Get recent mutations, newest first.
    pub fn recent_mutations(&self, limit: u32) -> Result<Vec<Mutation>, SoulError> {
        let mut mutations: Vec<Mutation> = self
            .mutations
            .iter()
            .filter_map(|res| {
                let (_k, v) = res.ok()?;
                serde_json::from_slice(&v).ok()
            })
            .collect();
        mutations.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        mutations.truncate(limit as usize);
        Ok(mutations)
    }
}
