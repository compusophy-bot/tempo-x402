// Structured event operations: insert, query, resolve, prune
use super::*;

impl SoulDatabase {
    /// Insert a structured event.
    pub fn insert_event(&self, event: &SoulEvent) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        conn.execute(
            "INSERT INTO events (id, level, code, category, message, context, \
             plan_id, goal_id, step_index, tool_name, peer_url, \
             resolved, resolved_at, resolution, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                event.id,
                event.level,
                event.code,
                event.category,
                event.message,
                event.context,
                event.plan_id,
                event.goal_id,
                event.step_index,
                event.tool_name,
                event.peer_url,
                event.resolved as i32,
                event.resolved_at,
                event.resolution,
                event.created_at,
            ],
        )?;
        Ok(())
    }

    /// Query events with filtering.
    pub fn query_events(&self, filter: &EventFilter) -> Result<Vec<SoulEvent>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut sql = String::from(
            "SELECT id, level, code, category, message, context, \
             plan_id, goal_id, step_index, tool_name, peer_url, \
             resolved, resolved_at, resolution, created_at \
             FROM events WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1u32;

        if let Some(ref level) = filter.level {
            sql.push_str(&format!(" AND level = ?{param_idx}"));
            param_values.push(Box::new(level.clone()));
            param_idx += 1;
        }
        if let Some(ref code_prefix) = filter.code_prefix {
            sql.push_str(&format!(" AND code LIKE ?{param_idx}"));
            param_values.push(Box::new(format!("{code_prefix}%")));
            param_idx += 1;
        }
        if let Some(ref category) = filter.category {
            sql.push_str(&format!(" AND category = ?{param_idx}"));
            param_values.push(Box::new(category.clone()));
            param_idx += 1;
        }
        if let Some(ref plan_id) = filter.plan_id {
            sql.push_str(&format!(" AND plan_id = ?{param_idx}"));
            param_values.push(Box::new(plan_id.clone()));
            param_idx += 1;
        }
        if let Some(resolved) = filter.resolved {
            sql.push_str(&format!(" AND resolved = ?{param_idx}"));
            param_values.push(Box::new(resolved as i32));
            param_idx += 1;
        }
        if let Some(since) = filter.since {
            sql.push_str(&format!(" AND created_at >= ?{param_idx}"));
            param_values.push(Box::new(since));
            param_idx += 1;
        }
        if let Some(until) = filter.until {
            sql.push_str(&format!(" AND created_at <= ?{param_idx}"));
            param_values.push(Box::new(until));
            param_idx += 1;
        }

        let limit = filter.limit.min(200);
        sql.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ?{} OFFSET ?{}",
            param_idx,
            param_idx + 1
        ));
        param_values.push(Box::new(limit));
        param_values.push(Box::new(filter.offset));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|b| b.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let events = stmt
            .query_map(param_refs.as_slice(), |row| {
                let resolved_int: i32 = row.get(11)?;
                Ok(SoulEvent {
                    id: row.get(0)?,
                    level: row.get(1)?,
                    code: row.get(2)?,
                    category: row.get(3)?,
                    message: row.get(4)?,
                    context: row.get(5)?,
                    plan_id: row.get(6)?,
                    goal_id: row.get(7)?,
                    step_index: row.get(8)?,
                    tool_name: row.get(9)?,
                    peer_url: row.get(10)?,
                    resolved: resolved_int != 0,
                    resolved_at: row.get(12)?,
                    resolution: row.get(13)?,
                    created_at: row.get(14)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Resolve a single event by ID.
    pub fn resolve_event(&self, id: &str, resolution: &str) -> Result<(), SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE events SET resolved = 1, resolved_at = ?1, resolution = ?2 WHERE id = ?3",
            params![now, resolution, id],
        )?;
        Ok(())
    }

    /// Resolve all unresolved events matching a code prefix.
    pub fn resolve_events_by_code(
        &self,
        code_prefix: &str,
        resolution: &str,
    ) -> Result<u32, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let count = conn.execute(
            "UPDATE events SET resolved = 1, resolved_at = ?1, resolution = ?2 \
             WHERE resolved = 0 AND code LIKE ?3",
            params![now, resolution, format!("{code_prefix}%")],
        )? as u32;
        Ok(count)
    }

    /// Resolve all unresolved events matching a code prefix for a specific plan.
    pub fn resolve_events_by_plan(
        &self,
        code_prefix: &str,
        plan_id: &str,
        resolution: &str,
    ) -> Result<u32, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;
        let now = chrono::Utc::now().timestamp();
        let count = conn.execute(
            "UPDATE events SET resolved = 1, resolved_at = ?1, resolution = ?2 \
             WHERE resolved = 0 AND code LIKE ?3 AND plan_id = ?4",
            params![now, resolution, format!("{code_prefix}%"), plan_id],
        )? as u32;
        Ok(count)
    }

    /// Get unresolved events at a given level.
    pub fn get_unresolved_events(
        &self,
        level: &str,
        limit: u32,
    ) -> Result<Vec<SoulEvent>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, level, code, category, message, context, \
             plan_id, goal_id, step_index, tool_name, peer_url, \
             resolved, resolved_at, resolution, created_at \
             FROM events WHERE resolved = 0 AND level = ?1 \
             ORDER BY created_at DESC LIMIT ?2",
        )?;

        let events = stmt
            .query_map(params![level, limit], |row| {
                let resolved_int: i32 = row.get(11)?;
                Ok(SoulEvent {
                    id: row.get(0)?,
                    level: row.get(1)?,
                    code: row.get(2)?,
                    category: row.get(3)?,
                    message: row.get(4)?,
                    context: row.get(5)?,
                    plan_id: row.get(6)?,
                    goal_id: row.get(7)?,
                    step_index: row.get(8)?,
                    tool_name: row.get(9)?,
                    peer_url: row.get(10)?,
                    resolved: resolved_int != 0,
                    resolved_at: row.get(12)?,
                    resolution: row.get(13)?,
                    created_at: row.get(14)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Count events at a given level since a timestamp.
    pub fn count_events_since(&self, level: &str, since: i64) -> Result<u64, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM events WHERE level = ?1 AND created_at >= ?2",
            params![level, since],
            |row| row.get(0),
        )?;

        Ok(count as u64)
    }

    /// Get the most recent event at a given level.
    pub fn get_latest_event_by_level(&self, level: &str) -> Result<Option<SoulEvent>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let result = conn
            .query_row(
                "SELECT id, level, code, category, message, context, \
                 plan_id, goal_id, step_index, tool_name, peer_url, \
                 resolved, resolved_at, resolution, created_at \
                 FROM events WHERE level = ?1 \
                 ORDER BY created_at DESC LIMIT 1",
                params![level],
                |row| {
                    let resolved_int: i32 = row.get(11)?;
                    Ok(SoulEvent {
                        id: row.get(0)?,
                        level: row.get(1)?,
                        code: row.get(2)?,
                        category: row.get(3)?,
                        message: row.get(4)?,
                        context: row.get(5)?,
                        plan_id: row.get(6)?,
                        goal_id: row.get(7)?,
                        step_index: row.get(8)?,
                        tool_name: row.get(9)?,
                        peer_url: row.get(10)?,
                        resolved: resolved_int != 0,
                        resolved_at: row.get(12)?,
                        resolution: row.get(13)?,
                        created_at: row.get(14)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Get top error codes by count in a time window.
    pub fn top_event_codes_since(
        &self,
        level: &str,
        since: i64,
        limit: u32,
    ) -> Result<Vec<ErrorCodeCount>, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let mut stmt = conn.prepare(
            "SELECT code, COUNT(*) as cnt FROM events \
             WHERE level = ?1 AND created_at >= ?2 \
             GROUP BY code ORDER BY cnt DESC LIMIT ?3",
        )?;

        let codes = stmt
            .query_map(params![level, since, limit], |row| {
                Ok(ErrorCodeCount {
                    code: row.get(0)?,
                    count: row.get::<_, i64>(1)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(codes)
    }

    /// Prune events with tiered retention.
    pub fn prune_events(&self) -> Result<u32, SoulError> {
        let conn = self.conn.lock().map_err(|_| {
            SoulError::Database(rusqlite::Error::InvalidParameterName(
                "lock poisoned".into(),
            ))
        })?;

        let now = chrono::Utc::now().timestamp();
        let one_day = 86400i64;
        let mut total = 0u32;

        // Debug: 1 day
        total += conn
            .execute(
                "DELETE FROM events WHERE level = 'debug' AND created_at < ?1",
                params![now - one_day],
            )
            .unwrap_or(0) as u32;

        // Info: 3 days
        total += conn
            .execute(
                "DELETE FROM events WHERE level = 'info' AND created_at < ?1",
                params![now - one_day * 3],
            )
            .unwrap_or(0) as u32;

        // Warn: 7 days (resolved: 3 days)
        total += conn
            .execute(
                "DELETE FROM events WHERE level = 'warn' AND created_at < ?1",
                params![now - one_day * 7],
            )
            .unwrap_or(0) as u32;
        total += conn
            .execute(
                "DELETE FROM events WHERE level = 'warn' AND resolved = 1 AND created_at < ?1",
                params![now - one_day * 3],
            )
            .unwrap_or(0) as u32;

        // Error: 30 days (resolved: 7 days)
        total += conn
            .execute(
                "DELETE FROM events WHERE level = 'error' AND created_at < ?1",
                params![now - one_day * 30],
            )
            .unwrap_or(0) as u32;
        total += conn
            .execute(
                "DELETE FROM events WHERE level = 'error' AND resolved = 1 AND created_at < ?1",
                params![now - one_day * 7],
            )
            .unwrap_or(0) as u32;

        // Hard cap: 5000 events
        total += conn
            .execute(
                "DELETE FROM events WHERE id IN (
                    SELECT id FROM events ORDER BY created_at DESC LIMIT -1 OFFSET 5000
                )",
                [],
            )
            .unwrap_or(0) as u32;

        Ok(total)
    }
}
