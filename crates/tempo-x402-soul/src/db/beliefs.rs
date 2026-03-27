// World Model belief operations.
use super::*;

impl SoulDatabase {
    /// Upsert a belief. On conflict (domain, subject, predicate) for active beliefs,
    /// updates value, evidence, confidence, bumps confirmation_count, refreshes updated_at.
    pub fn upsert_belief(&self, belief: &Belief) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO beliefs (id, domain, subject, predicate, value, confidence, evidence, confirmation_count, created_at, updated_at, active) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT(domain, subject, predicate) WHERE active = 1 DO UPDATE SET \
               id = excluded.id, value = excluded.value, confidence = excluded.confidence, evidence = excluded.evidence, \
               confirmation_count = confirmation_count + 1, \
               updated_at = excluded.updated_at",
            params![
                belief.id,
                belief.domain.as_str(),
                belief.subject,
                belief.predicate,
                belief.value,
                belief.confidence.as_str(),
                belief.evidence,
                belief.confirmation_count,
                belief.created_at,
                belief.updated_at,
                belief.active as i32,
            ],
        )?;
        Ok(())
    }

    /// Get all active beliefs for a domain.
    pub fn get_beliefs_by_domain(&self, domain: &BeliefDomain) -> Result<Vec<Belief>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, domain, subject, predicate, value, confidence, evidence, \
             confirmation_count, created_at, updated_at, active \
             FROM beliefs WHERE domain = ?1 AND active = 1 ORDER BY subject, predicate",
        )?;

        let beliefs = stmt
            .query_map(params![domain.as_str()], Self::row_to_belief)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(beliefs)
    }

    /// Get all active beliefs (full world model snapshot).
    pub fn get_all_active_beliefs(&self) -> Result<Vec<Belief>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, domain, subject, predicate, value, confidence, evidence, \
             confirmation_count, created_at, updated_at, active \
             FROM beliefs WHERE active = 1 ORDER BY domain, subject, predicate",
        )?;

        let beliefs = stmt
            .query_map([], Self::row_to_belief)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(beliefs)
    }

    /// Confirm a belief: bump confirmation_count, refresh updated_at, set confidence to High.
    pub fn confirm_belief(&self, id: &str) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE beliefs SET confirmation_count = confirmation_count + 1, \
             updated_at = ?1, confidence = 'high' WHERE id = ?2 AND active = 1",
            params![now, id],
        )?;
        Ok(rows > 0)
    }

    /// Invalidate a belief: set active=false, append reason to evidence.
    pub fn invalidate_belief(&self, id: &str, reason: &str) -> Result<bool, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE beliefs SET active = 0, updated_at = ?1, \
             evidence = evidence || ' [invalidated: ' || ?2 || ']' \
             WHERE id = ?3 AND active = 1",
            params![now, reason, id],
        )?;
        Ok(rows > 0)
    }

    /// Decay unconfirmed beliefs based on cycles since last update.
    /// Uses the cycle count stored in soul_state to determine staleness.
    /// High → Medium after 5 cycles unconfirmed, Medium → Low after 10, Low → inactive after 20.
    pub fn decay_beliefs(&self) -> Result<(u32, u32, u32), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();

        // Thresholds in seconds (approximation: 1 cycle ~ 300s = 5min)
        let cycle_secs: i64 = 300;

        // High → Medium: unconfirmed for 5 cycles (~25min)
        let demoted_high = conn.execute(
            "UPDATE beliefs SET confidence = 'medium' \
             WHERE active = 1 AND confidence = 'high' AND (?1 - updated_at) > ?2",
            params![now, cycle_secs * 5],
        )? as u32;

        // Medium → Low: unconfirmed for 10 cycles (~50min)
        let demoted_medium = conn.execute(
            "UPDATE beliefs SET confidence = 'low' \
             WHERE active = 1 AND confidence = 'medium' AND (?1 - updated_at) > ?2",
            params![now, cycle_secs * 10],
        )? as u32;

        // Low → inactive: unconfirmed for 20 cycles (~100min)
        let deactivated = conn.execute(
            "UPDATE beliefs SET active = 0 \
             WHERE active = 1 AND confidence = 'low' AND (?1 - updated_at) > ?2",
            params![now, cycle_secs * 20],
        )? as u32;

        Ok((demoted_high, demoted_medium, deactivated))
    }

    /// Helper: map a row to a Belief.
    fn row_to_belief(row: &rusqlite::Row) -> Result<Belief, rusqlite::Error> {
        let domain_str: String = row.get(1)?;
        let confidence_str: String = row.get(5)?;
        let active_int: i32 = row.get(10)?;
        Ok(Belief {
            id: row.get(0)?,
            domain: BeliefDomain::parse(&domain_str).unwrap_or(BeliefDomain::Node),
            subject: row.get(2)?,
            predicate: row.get(3)?,
            value: row.get(4)?,
            confidence: Confidence::parse(&confidence_str),
            evidence: row.get(6)?,
            confirmation_count: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            active: active_int != 0,
        })
    }
}
