//! Mutation CRUD methods.
use super::*;

impl SoulDatabase {
    /// Record a mutation (code change attempt).
    pub fn insert_mutation(&self, mutation: &Mutation) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO mutations (id, commit_sha, branch, description, files_changed, cargo_check_passed, cargo_test_passed, created_at, goal_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                mutation.id,
                mutation.commit_sha,
                mutation.branch,
                mutation.description,
                mutation.files_changed,
                mutation.cargo_check_passed as i32,
                mutation.cargo_test_passed as i32,
                mutation.created_at,
                mutation.goal_id,
            ],
        )?;
        Ok(())
    }

    /// Get recent mutations, newest first.
    pub fn recent_mutations(&self, limit: u32) -> Result<Vec<Mutation>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, commit_sha, branch, description, files_changed, cargo_check_passed, cargo_test_passed, created_at, goal_id \
             FROM mutations ORDER BY created_at DESC LIMIT ?1",
        )?;

        let mutations = stmt
            .query_map(params![limit], |row| {
                let check: i32 = row.get(5)?;
                let test: i32 = row.get(6)?;
                Ok(Mutation {
                    id: row.get(0)?,
                    commit_sha: row.get(1)?,
                    branch: row.get(2)?,
                    description: row.get(3)?,
                    files_changed: row.get(4)?,
                    cargo_check_passed: check != 0,
                    cargo_test_passed: test != 0,
                    created_at: row.get(7)?,
                    goal_id: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mutations)
    }
}
