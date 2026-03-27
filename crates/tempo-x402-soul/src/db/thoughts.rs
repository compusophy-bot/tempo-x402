//! Thought CRUD and neuroplastic memory methods.
use super::*;

impl SoulDatabase {
    /// Store a thought.
    pub fn insert_thought(&self, thought: &Thought) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO thoughts (id, thought_type, content, context, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                thought.id,
                thought.thought_type.as_str(),
                thought.content,
                thought.context,
                thought.created_at,
            ],
        )?;
        Ok(())
    }

    /// Delete a single thought by ID. Returns 1 if deleted, 0 if not found.
    pub fn delete_thought(&self, id: &str) -> Result<usize, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let deleted = conn.execute("DELETE FROM thoughts WHERE id = ?1", params![id])?;
        Ok(deleted)
    }

    /// Get the most recent N thoughts, newest first.
    pub fn recent_thoughts(&self, limit: u32) -> Result<Vec<Thought>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, thought_type, content, context, created_at, salience, memory_tier, strength \
             FROM thoughts ORDER BY created_at DESC LIMIT ?1",
        )?;

        let thoughts = stmt
            .query_map(params![limit], |row| {
                let type_str: String = row.get(1)?;
                Ok(Thought {
                    id: row.get(0)?,
                    thought_type: ThoughtType::parse(&type_str).unwrap_or(ThoughtType::Observation),
                    content: row.get(2)?,
                    context: row.get(3)?,
                    created_at: row.get(4)?,
                    salience: row.get(5)?,
                    memory_tier: row.get(6)?,
                    strength: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(thoughts)
    }

    /// Get the most recent N thoughts of specific types, newest first.
    pub fn recent_thoughts_by_type(
        &self,
        types: &[ThoughtType],
        limit: u32,
    ) -> Result<Vec<Thought>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        if types.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<String> = types
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let query = format!(
            "SELECT id, thought_type, content, context, created_at, salience, memory_tier, strength FROM thoughts \
             WHERE thought_type IN ({}) ORDER BY created_at DESC LIMIT ?{}",
            placeholders.join(", "),
            types.len() + 1
        );

        let mut stmt = conn.prepare(&query)?;

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = types
            .iter()
            .map(|t| Box::new(t.as_str().to_string()) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        params_vec.push(Box::new(limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let thoughts = stmt
            .query_map(params_refs.as_slice(), |row| {
                let type_str: String = row.get(1)?;
                Ok(Thought {
                    id: row.get(0)?,
                    thought_type: ThoughtType::parse(&type_str).unwrap_or(ThoughtType::Observation),
                    content: row.get(2)?,
                    context: row.get(3)?,
                    created_at: row.get(4)?,
                    salience: row.get(5)?,
                    memory_tier: row.get(6)?,
                    strength: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(thoughts)
    }

    /// Insert a thought with salience metadata.
    pub fn insert_thought_with_salience(
        &self,
        thought: &Thought,
        salience: f64,
        salience_factors_json: &str,
        tier: &str,
        strength: f64,
    ) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO thoughts (id, thought_type, content, context, created_at, salience, salience_factors, memory_tier, strength, prediction_error) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
            params![
                thought.id,
                thought.thought_type.as_str(),
                thought.content,
                thought.context,
                thought.created_at,
                salience,
                salience_factors_json,
                tier,
                strength,
            ],
        )?;
        Ok(())
    }

    /// Run a decay cycle: reduce strength per tier, prune thoughts below threshold.
    /// Long-term thoughts are never pruned.
    pub fn run_decay_cycle(&self, prune_threshold: f64) -> Result<(u32, u32), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        // Decay each tier
        let sensory_decayed = conn.execute(
            "UPDATE thoughts SET strength = strength * 0.3 WHERE memory_tier = 'sensory' AND strength IS NOT NULL",
            [],
        )? as u32;
        conn.execute(
            "UPDATE thoughts SET strength = strength * 0.95 WHERE memory_tier = 'working' AND strength IS NOT NULL",
            [],
        )?;
        conn.execute(
            "UPDATE thoughts SET strength = strength * 0.995 WHERE memory_tier = 'long_term' AND strength IS NOT NULL",
            [],
        )?;

        // Prune below threshold (except long_term)
        let pruned = conn.execute(
            "DELETE FROM thoughts WHERE strength IS NOT NULL AND strength < ?1 AND (memory_tier != 'long_term' OR memory_tier IS NULL)",
            params![prune_threshold],
        )? as u32;

        Ok((sensory_decayed, pruned))
    }

    /// Auto-promote high-salience sensory thoughts to working tier.
    pub fn promote_salient_sensory(&self, salience_threshold: f64) -> Result<u32, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let promoted = conn.execute(
            "UPDATE thoughts SET memory_tier = 'working' WHERE memory_tier = 'sensory' AND salience IS NOT NULL AND salience > ?1",
            params![salience_threshold],
        )? as u32;

        Ok(promoted)
    }

    /// Increment a pattern's count. Returns the new count.
    pub fn increment_pattern(&self, fingerprint: &str) -> Result<u64, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO pattern_counts (fingerprint, count, last_seen_at) VALUES (?1, 1, ?2) \
             ON CONFLICT(fingerprint) DO UPDATE SET count = count + 1, last_seen_at = ?2",
            params![fingerprint, now],
        )?;

        let count: i64 = conn.query_row(
            "SELECT count FROM pattern_counts WHERE fingerprint = ?1",
            params![fingerprint],
            |row| row.get(0),
        )?;

        Ok(count as u64)
    }

    /// Get pattern counts for multiple fingerprints.
    pub fn get_pattern_counts(
        &self,
        fingerprints: &[String],
    ) -> Result<HashMap<String, u64>, SoulError> {
        if fingerprints.is_empty() {
            return Ok(HashMap::new());
        }
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let placeholders: Vec<String> = fingerprints
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let query = format!(
            "SELECT fingerprint, count FROM pattern_counts WHERE fingerprint IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&query)?;
        let params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = fingerprints
            .iter()
            .map(|f| Box::new(f.clone()) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut result = HashMap::new();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let fp: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((fp, count as u64))
        })?;
        for row in rows {
            let (fp, count) = row?;
            result.insert(fp, count);
        }

        Ok(result)
    }
}
